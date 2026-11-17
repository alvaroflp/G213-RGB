//! Logitech G213 Prodigy lighting driver.
//!
//! The G213 exposes a vendor-defined HID++ interface (usage page `0xFF43`,
//! report IDs `0x11`/`0x12`). Lighting is controlled with 20-byte output
//! reports on report ID `0x11`:
//!
//! * direct colour: `11 FF 0C 3A <zone> 01 <r> <g> <b> 02 …`
//! * effect:        `11 FF 0C 3C 00 <mode> <r> <g> <b> …`
//!
//! The protocol was reverse-engineered by the OpenRGB project
//! (`LogitechG213Controller`), which is the reference used here.
//!
//! We talk to `/dev/hidraw*` directly with libc instead of pulling in hidapi:
//! the only operations needed are `open`, `write`, `poll` and `read`.

use crate::config::{Effect, Layout, Rgb, WaveDirection, protocol_speed};
use std::fs;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const VENDOR_ID: u16 = 0x046d;
pub const PRODUCT_ID: u16 = 0xc336;
pub const CONTROL_USAGE_PAGE: u16 = 0xff43;
pub const REPORT_ID: u8 = 0x11;
pub const PACKET_LEN: usize = 20;

/// Maximum wait for a matching HID++ acknowledgement on the worker thread.
const ACK_TIMEOUT: Duration = Duration::from_millis(100);
/// How often an unavailable keyboard is probed again.
const RECONNECT_INTERVAL: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Searching,
    Connected,
    PermissionDenied,
    NotFound,
    Error,
}

#[derive(Clone, Debug)]
pub struct DeviceStatus {
    pub state: ConnectionState,
    pub path: Option<PathBuf>,
    pub name: Option<String>,
    pub detail: Option<String>,
}

impl DeviceStatus {
    pub fn searching() -> Self {
        Self {
            state: ConnectionState::Searching,
            path: None,
            name: None,
            detail: None,
        }
    }

    pub fn connected(path: PathBuf, name: String) -> Self {
        Self {
            state: ConnectionState::Connected,
            path: Some(path),
            name: Some(name),
            detail: None,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    pub fn path_label(&self) -> String {
        self.path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "—".to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundDevice {
    pub model: Model,
    pub path: PathBuf,
    pub name: String,
}

impl FoundDevice {
    pub fn open(&self) -> io::Result<G213> {
        let handle = G213::open(&self.path)?;
        self.model.initialize(&handle)?;
        Ok(handle)
    }
}

/// Everything the device needs in order to render the current lighting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyState {
    pub enabled: bool,
    pub layout: Layout,
    pub color: Rgb,
    pub zones: [Rgb; 5],
    pub brightness: u8,
    pub hue: u16,
    pub effect: Effect,
    pub effect_color: Rgb,
    pub speed: u8,
    pub direction: WaveDirection,
}

impl From<&crate::config::Config> for ApplyState {
    fn from(config: &crate::config::Config) -> Self {
        Self::from(&config.keyboard_lighting())
    }
}

impl From<&crate::config::Lighting> for ApplyState {
    fn from(settings: &crate::config::Lighting) -> Self {
        Self {
            enabled: settings.enabled,
            layout: settings.layout,
            color: settings.color,
            zones: settings.zones,
            brightness: settings.brightness,
            hue: settings.hue,
            effect: settings.effect,
            effect_color: settings.effect_color,
            speed: settings.speed,
            direction: settings.direction,
        }
    }
}

impl ApplyState {
    fn render_color(&self, color: Rgb) -> Rgb {
        color.with_hue(self.hue).scaled(self.brightness)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    G213,
    G203Prodigy,
    G203Lightsync,
}

impl Model {
    pub fn name(self) -> &'static str {
        match self {
            Self::G213 => "Logitech G213 Prodigy",
            Self::G203Prodigy => "Logitech G203 Prodigy",
            Self::G203Lightsync => "Logitech G203 LIGHTSYNC",
        }
    }

    fn from_uevent(uevent: &str) -> Option<Self> {
        let id = uevent_value(uevent, "HID_ID")?;
        let mut parts = id.split(':').map(|part| u16::from_str_radix(part, 16).ok());
        if parts.next()? != Some(3) || parts.next()? != Some(VENDOR_ID) {
            return None;
        }
        match parts.next()?? {
            PRODUCT_ID => Some(Self::G213),
            0xc084 => Some(Self::G203Prodigy),
            0xc092 | 0xc09d => Some(Self::G203Lightsync),
            _ => None,
        }
    }

    fn matches_descriptor(self, descriptor: &[u8]) -> bool {
        if self == Self::G213 {
            return is_control_interface(descriptor);
        }
        descriptor.windows(7).any(|item| {
            item == [0x06, 0x00, 0xff, 0x09, 0x02, 0xa1, 0x01]
                || (self == Self::G203Prodigy && item == [0x06, 0x00, 0xff, 0x09, 0x01, 0xa1, 0x01])
        }) && descriptor.windows(2).any(|item| item == [0x85, REPORT_ID])
    }

    fn initialize(self, device: &G213) -> io::Result<()> {
        match self {
            Self::G213 => Ok(()),
            // OpenRGB's direct mode disables repeated flash writes.
            Self::G203Prodigy => device.send(&[0x10, 0xff, 0x0e, 0x8a, 1, 1, 0]),
            Self::G203Lightsync => device.send(&packet(&[0x11, 0xff, 0x0e, 0x50, 1, 3, 7])),
        }
    }

    pub fn apply(self, device: &G213, state: &ApplyState) -> io::Result<()> {
        if self == Self::G213 {
            return device.apply(state);
        }
        let enabled = state.enabled && state.effect != Effect::Off;
        let color = if enabled {
            state.render_color(state.color)
        } else {
            Rgb::BLACK
        };
        device.send(&self.mouse_packet(color, enabled))
    }

    fn mouse_packet(self, color: Rgb, enabled: bool) -> [u8; PACKET_LEN] {
        let Rgb { r, g, b } = color;
        match self {
            Self::G203Prodigy => packet(&[0x11, 0xff, 0x0e, 0x3a, 0, 1, r, g, b, 2]),
            // Use the native whole-device effect. Direct writes to the three zones
            // have a documented one-update lag in OpenRGB's LIGHTSYNC controller.
            Self::G203Lightsync => packet(&[
                0x11,
                0xff,
                0x0e,
                0x10,
                0,
                u8::from(enabled),
                r,
                g,
                b,
                if enabled { 2 } else { 0 },
                0,
                0,
                0,
                0,
                0,
                0,
                1,
            ]),
            Self::G213 => unreachable!("mouse packet requested for G213"),
        }
    }
}

fn packet(bytes: &[u8]) -> [u8; PACKET_LEN] {
    let mut packet = [0; PACKET_LEN];
    packet[..bytes.len()].copy_from_slice(bytes);
    packet
}

/// Locate the G213 control interface.
///
/// Both the boot keyboard interface and the vendor interface share the same
/// USB IDs, so the report descriptor is what tells them apart: the control
/// interface declares usage page `0xFF43` and report ID `0x11`.
pub fn discover() -> Option<FoundDevice> {
    discover_matching(false)
}

pub fn discover_matching(mouse: bool) -> Option<FoundDevice> {
    if !mouse
        && let Ok(override_path) = std::env::var("G213_HIDRAW")
        && !override_path.is_empty()
    {
        let path = PathBuf::from(override_path);
        let name = read_hid_name(&path).unwrap_or_else(|| "Logitech G213 Prodigy".to_string());
        return Some(FoundDevice {
            model: Model::G213,
            path,
            name,
        });
    }

    let mut entries: Vec<PathBuf> = fs::read_dir("/sys/class/hidraw")
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("hidraw"))
        })
        .collect();
    entries.sort();

    for entry in entries {
        let uevent = match fs::read_to_string(entry.join("device/uevent")) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let Some(model) = Model::from_uevent(&uevent) else {
            continue;
        };
        if mouse == (model == Model::G213) {
            continue;
        }
        if mouse
            && entry
                .join("device")
                .canonicalize()
                .ok()
                .and_then(|path| fs::read_to_string(path.parent()?.join("bInterfaceNumber")).ok())
                .is_none_or(|value| value.trim() != "01")
        {
            continue;
        }
        let descriptor = fs::read(entry.join("device/report_descriptor")).unwrap_or_default();
        if !model.matches_descriptor(&descriptor) {
            continue;
        }
        let Some(hidraw) = entry.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let name = uevent_value(&uevent, "HID_NAME").unwrap_or_else(|| model.name().to_string());
        return Some(FoundDevice {
            model,
            path: PathBuf::from("/dev").join(hidraw),
            name,
        });
    }
    None
}

fn uevent_value(uevent: &str, key: &str) -> Option<String> {
    uevent.lines().find_map(|line| {
        line.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('='))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn is_control_interface(descriptor: &[u8]) -> bool {
    let has_usage_page = descriptor.windows(3).any(|window| {
        window
            == [
                0x06,
                CONTROL_USAGE_PAGE as u8,
                (CONTROL_USAGE_PAGE >> 8) as u8,
            ]
    });
    let has_report_id = descriptor
        .windows(2)
        .any(|window| window == [0x85, REPORT_ID]);
    has_usage_page && has_report_id
}

fn read_hid_name(device: &Path) -> Option<String> {
    let name = device.file_name()?.to_str()?;
    let uevent = fs::read_to_string(
        Path::new("/sys/class/hidraw")
            .join(name)
            .join("device/uevent"),
    )
    .ok()?;
    uevent_value(&uevent, "HID_NAME")
}

/// An open G213 control interface.
pub struct G213 {
    file: fs::File,
    fd: libc::c_int,
}

impl G213 {
    /// Detect an unplug even while no lighting changes are being sent.
    fn disconnected(&self) -> bool {
        let mut descriptor = libc::pollfd {
            fd: self.fd,
            events: 0,
            revents: 0,
        };
        // SAFETY: one live pollfd, no blocking, and the file owns fd for this call.
        let ready = unsafe { libc::poll(&mut descriptor, 1, 0) };
        ready > 0 && descriptor.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)?;
        let fd = file.as_raw_fd();
        Ok(Self { file, fd })
    }

    /// Push a complete lighting state to the keyboard.
    pub fn apply(&self, state: &ApplyState) -> io::Result<()> {
        // An explicit "off" effect and the master toggle both mean mode 0.
        if !state.enabled || state.effect == Effect::Off {
            return self.set_mode(0x00, Rgb::BLACK, state.speed, 0x01);
        }

        match state.effect {
            Effect::Off => unreachable!("handled above"),
            Effect::Static => match state.layout {
                Layout::Uniform => {
                    let color = state.render_color(state.color);
                    self.set_mode(Effect::Static.protocol_mode(), color, state.speed, 0x01)
                }
                Layout::Zones => {
                    for (index, color) in state.zones.iter().enumerate() {
                        self.set_direct(index as u8 + 1, state.render_color(*color))?;
                    }
                    Ok(())
                }
            },
            effect => {
                let color = state.render_color(state.effect_color);
                self.set_mode(
                    effect.protocol_mode(),
                    color,
                    protocol_speed(state.speed),
                    state.direction.protocol_value(),
                )
            }
        }
    }

    pub fn set_direct(&self, zone: u8, color: Rgb) -> io::Result<()> {
        self.send(&build_direct_packet(zone, color))
    }

    pub fn set_mode(&self, mode: u8, color: Rgb, speed: u8, direction: u8) -> io::Result<()> {
        self.send(&build_mode_packet(mode, color, speed, direction))
    }

    fn send(&self, packet: &[u8]) -> io::Result<()> {
        let written = unsafe {
            libc::write(
                self.fd,
                packet.as_ptr().cast::<libc::c_void>(),
                packet.len(),
            )
        };
        if written < 0 {
            return Err(io::Error::last_os_error());
        }
        if written as usize != packet.len() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "gravação parcial no dispositivo HID",
            ));
        }

        // The control interface can also produce input reports. Wait for
        // this command's response so keyboard input or a HID++ error
        // cannot be mistaken for successful lighting.
        let deadline = Instant::now() + ACK_TIMEOUT;
        let mut poll_fd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            // SAFETY: poll_fd describes one live descriptor owned by self.
            let ready = unsafe { libc::poll(&mut poll_fd, 1, remaining.as_millis() as i32 + 1) };
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if ready == 0 {
                break;
            }
            if poll_fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "dispositivo HID desconectado",
                ));
            }
            let mut reply = [0u8; 64];
            // SAFETY: reply is writable for all 64 bytes.
            let count = unsafe { libc::read(self.fd, reply.as_mut_ptr().cast(), reply.len()) };
            if count < 0 {
                let error = io::Error::last_os_error();
                if matches!(
                    error.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) {
                    continue;
                }
                return Err(error);
            }
            if let Some(result) = hidpp_response(packet, &reply[..count as usize]) {
                return result;
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "sem confirmação HID++ da iluminação",
        ))
    }
}

/// HID++ 1.0 (short/0x8f) and 2.0 (long/0xff) errors carry the
/// original feature and function one byte later than a normal reply.
fn hidpp_response(request: &[u8], reply: &[u8]) -> Option<io::Result<()>> {
    let [report, address, feature, function, rest @ ..] = reply else {
        return None;
    };
    if !matches!(report, 0x10..=0x12) || *address != request[1] {
        return None;
    }
    if matches!((*report, *feature), (0x10, 0x8f) | (0x11 | 0x12, 0xff))
        && *function == request[2]
        && let [original_function, code, ..] = rest
        && *original_function == request[3]
    {
        return Some(Err(io::Error::other(format!(
            "comando de iluminação recusado (HID++ 0x{code:02X})"
        ))));
    }
    (*feature == request[2] && *function == request[3]).then_some(Ok(()))
}

impl Drop for G213 {
    fn drop(&mut self) {
        // `file` owns the descriptor and closes it.
        let _ = &self.file;
    }
}

pub fn build_direct_packet(zone: u8, color: Rgb) -> [u8; PACKET_LEN] {
    let mut packet = [0u8; PACKET_LEN];
    packet[0] = REPORT_ID;
    packet[1] = 0xFF;
    packet[2] = 0x0C;
    packet[3] = 0x3A;
    packet[4] = zone;
    packet[5] = 0x01;
    packet[6] = color.r;
    packet[7] = color.g;
    packet[8] = color.b;
    packet[9] = 0x02;
    packet
}

pub fn build_mode_packet(mode: u8, color: Rgb, speed: u8, direction: u8) -> [u8; PACKET_LEN] {
    let mut packet = [0u8; PACKET_LEN];
    packet[0] = REPORT_ID;
    packet[1] = 0xFF;
    packet[2] = 0x0C;
    packet[3] = 0x3C;
    packet[4] = 0x00;
    packet[5] = mode;
    packet[6] = color.r;
    packet[7] = color.g;
    packet[8] = color.b;

    let wide = u16::from(speed) * 100;
    let hi = (wide >> 8) as u8;
    let lo = (wide & 0xFF) as u8;

    match mode {
        0x02 => {
            // Breathing: speed at 9..10, trailing 0x64 at 12.
            packet[9] = hi;
            packet[10] = lo;
            packet[12] = 0x64;
        }
        0x03 => {
            // Cycle: speed at 11..12, trailing 0x64 at 13.
            packet[11] = hi;
            packet[12] = lo;
            packet[13] = 0x64;
        }
        0x04 => {
            // Wave: speed at 11..12, direction at 13, high speed byte at 15.
            packet[11] = hi;
            packet[12] = lo;
            packet[13] = direction;
            packet[15] = hi;
        }
        _ => {}
    }
    packet
}

// ---------------------------------------------------------------------------------------------
// Background worker
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DeliveryState {
    #[default]
    Idle,
    Pending,
    Sending,
    Confirmed,
    Failed(String),
}

#[derive(Default)]
struct Delivery {
    revision: u64,
    state: DeliveryState,
}

impl Delivery {
    fn queue(&mut self) -> u64 {
        self.revision += 1;
        self.state = DeliveryState::Pending;
        self.revision
    }

    fn report(&mut self, revision: u64, state: DeliveryState) {
        // Acknowledgements for a superseded color must not replace the status
        // displayed for the newest edit.
        if self.revision == revision {
            self.state = state;
        }
    }
}

struct LightingRequest {
    state: ApplyState,
    revision: u64,
    retry_now: bool,
}

enum Command {
    Apply(Box<LightingRequest>),
    Refresh,
}

/// Owns the device on a background thread so a slow acknowledgement can never
/// stall a frame. The latest requested state is reapplied on reconnect.
pub struct DeviceWorker {
    tx: Option<Sender<Command>>,
    thread: Option<thread::JoinHandle<()>>,
    status: Arc<Mutex<DeviceStatus>>,
    delivery: Arc<Mutex<Delivery>>,
}

impl DeviceWorker {
    pub fn start() -> Self {
        Self::for_mouse(false)
    }

    pub fn for_mouse(mouse: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(DeviceStatus::searching()));
        let delivery = Arc::new(Mutex::new(Delivery::default()));
        let worker_status = Arc::clone(&status);
        let worker_delivery = Arc::clone(&delivery);
        let thread = thread::Builder::new()
            .name(if mouse { "g203-device" } else { "g213-device" }.into())
            .spawn(move || worker_loop(rx, worker_status, worker_delivery, mouse))
            .ok();
        Self {
            tx: Some(tx),
            thread,
            status,
            delivery,
        }
    }

    pub fn status(&self) -> DeviceStatus {
        self.status
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn delivery(&self) -> DeliveryState {
        self.delivery
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .state
            .clone()
    }

    pub fn apply(&self, state: ApplyState) {
        self.submit(state, false);
    }

    pub fn retry(&self, state: ApplyState) {
        self.submit(state, true);
    }

    fn submit(&self, state: ApplyState, retry_now: bool) {
        let mut delivery = self.delivery.lock().unwrap_or_else(|err| err.into_inner());
        let revision = delivery.queue();
        if let Some(tx) = &self.tx
            && tx
                .send(Command::Apply(Box::new(LightingRequest {
                    state,
                    revision,
                    retry_now,
                })))
                .is_err()
        {
            delivery.report(
                revision,
                DeliveryState::Failed("O serviço do dispositivo não está em execução.".into()),
            );
        }
    }

    pub fn refresh(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Refresh);
        }
    }
}

impl Drop for DeviceWorker {
    fn drop(&mut self) {
        drop(self.tx.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn worker_loop(
    rx: Receiver<Command>,
    status: Arc<Mutex<DeviceStatus>>,
    delivery: Arc<Mutex<Delivery>>,
    mouse: bool,
) {
    let mut found: Option<FoundDevice> = None;
    let mut device: Option<G213> = None;
    let mut wanted: Option<Box<LightingRequest>> = None;
    let mut need_apply = true;
    let mut next_scan = Instant::now();
    let report = |request: &LightingRequest, state| {
        delivery
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .report(request.revision, state);
    };

    loop {
        let timeout = if device.is_some() {
            RECONNECT_INTERVAL
        } else {
            next_scan.saturating_duration_since(Instant::now())
        };
        let first = match rx.recv_timeout(timeout.max(Duration::from_millis(50))) {
            Ok(command) => Some(command),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        // Collapse a burst of slider changes into the newest state, while
        // retaining explicit requests to check the USB connection immediately.
        let mut refresh = false;
        for command in first.into_iter().chain(rx.try_iter()) {
            match command {
                Command::Apply(request) => {
                    refresh |= request.retry_now && device.is_none();
                    wanted = Some(request);
                    need_apply = true;
                }
                Command::Refresh => refresh = true,
            }
        }

        if refresh || device.as_ref().is_some_and(G213::disconnected) {
            device = None;
            found = None;
            need_apply = true;
            next_scan = Instant::now();
            if let Some(request) = &wanted {
                report(request, DeliveryState::Pending);
            }
        }

        if device.is_none() && Instant::now() >= next_scan {
            try_connect(&mut found, &mut device, &status, mouse);
            next_scan = Instant::now() + RECONNECT_INTERVAL;
            if device.is_some() {
                need_apply = true;
            } else if let Some(request) = &wanted {
                let connection = status.lock().unwrap_or_else(|err| err.into_inner());
                let state = if matches!(
                    connection.state,
                    ConnectionState::PermissionDenied | ConnectionState::Error
                ) {
                    DeliveryState::Failed(
                        connection
                            .detail
                            .clone()
                            .unwrap_or_else(|| "Falha ao abrir o dispositivo.".into()),
                    )
                } else {
                    DeliveryState::Pending
                };
                report(request, state);
            }
        }

        if need_apply
            && let (Some(dev), Some(request), Some(info)) =
                (device.as_ref(), wanted.as_ref(), found.as_ref())
        {
            report(request, DeliveryState::Sending);
            match info.model.apply(dev, &request.state) {
                Ok(()) => {
                    need_apply = false;
                    report(request, DeliveryState::Confirmed);
                }
                Err(err) => {
                    let error = err.to_string();
                    let state = match err.kind() {
                        io::ErrorKind::PermissionDenied => ConnectionState::PermissionDenied,
                        io::ErrorKind::NotConnected | io::ErrorKind::NotFound => {
                            ConnectionState::NotFound
                        }
                        _ => ConnectionState::Error,
                    };
                    report(
                        request,
                        if state == ConnectionState::NotFound {
                            DeliveryState::Pending
                        } else {
                            DeliveryState::Failed(error.clone())
                        },
                    );
                    set_status(
                        &status,
                        DeviceStatus {
                            state,
                            path: Some(info.path.clone()),
                            name: Some(info.name.clone()),
                            detail: Some(error),
                        },
                    );
                    device = None;
                    found = None;
                    next_scan = Instant::now() + RECONNECT_INTERVAL;
                }
            }
        }
    }
}

fn try_connect(
    found: &mut Option<FoundDevice>,
    device: &mut Option<G213>,
    status: &Arc<Mutex<DeviceStatus>>,
    mouse: bool,
) {
    if found.is_none() {
        *found = discover_matching(mouse);
    }
    let Some(info) = found.as_ref().cloned() else {
        set_status(
            status,
            DeviceStatus {
                state: ConnectionState::NotFound,
                path: None,
                name: None,
                detail: None,
            },
        );
        return;
    };

    match info.open() {
        Ok(handle) => {
            set_status(
                status,
                DeviceStatus::connected(info.path.clone(), info.name.clone()),
            );
            *device = Some(handle);
        }
        Err(err) => {
            let state = match err.kind() {
                io::ErrorKind::PermissionDenied => ConnectionState::PermissionDenied,
                io::ErrorKind::NotFound => ConnectionState::NotFound,
                _ => ConnectionState::Error,
            };
            if state == ConnectionState::NotFound {
                *found = None;
            }
            set_status(
                status,
                DeviceStatus {
                    state,
                    path: Some(info.path.clone()),
                    name: Some(info.name.clone()),
                    detail: Some(err.to_string()),
                },
            );
        }
    }
}

fn set_status(status: &Arc<Mutex<DeviceStatus>>, value: DeviceStatus) {
    if let Ok(mut slot) = status.lock() {
        *slot = value;
    }
}

// ---------------------------------------------------------------------------------------------
// CLI helpers
// ---------------------------------------------------------------------------------------------

/// `--diagnose`: check access to each USB device without writing a HID report.
pub fn run_diagnose() -> i32 {
    println!("G213 RGB — diagnóstico");
    let mut available = false;
    let mut result = 0;
    for (mouse, label) in [(false, "G213 · teclado"), (true, "G203 · mouse")] {
        println!("\n{label}");
        if let Some(found) = discover_matching(mouse) {
            available = true;
            println!("  {} · {}", found.name, found.path.display());
            // Model::initialize may write commands. Open only the transport here.
            match G213::open(&found.path) {
                Ok(_) => println!("  Acesso USB: OK"),
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    println!("  Acesso USB negado: {error}");
                    println!("  Execute ./install.sh e reconecte o dispositivo.");
                    result = result.max(2);
                }
                Err(error) => {
                    println!("  Falha no acesso USB: {error}");
                    result = result.max(3);
                }
            }
        } else {
            println!("  Interface de controle não encontrada.");
        }
    }
    if available { result } else { 1 }
}

/// `--test`: paint the five areas in distinct colours.
pub fn run_test() -> i32 {
    let Some(found) = discover() else {
        eprintln!("Nenhum Logitech G213 encontrado.");
        return 1;
    };
    let device = match G213::open(&found.path) {
        Ok(device) => device,
        Err(err) => {
            eprintln!("Não foi possível abrir {}: {err}", found.path.display());
            if err.kind() == io::ErrorKind::PermissionDenied {
                eprintln!("Execute ./install.sh e reconecte o teclado.");
            }
            return 2;
        }
    };

    let colors = [
        Rgb::new(0xff, 0x3b, 0x30),
        Rgb::new(0xff, 0x95, 0x00),
        Rgb::new(0xff, 0xcc, 0x00),
        Rgb::new(0x34, 0xc7, 0x59),
        Rgb::new(0x00, 0x7a, 0xff),
    ];
    for (index, color) in colors.iter().enumerate() {
        if let Err(err) = device.set_direct(index as u8 + 1, *color) {
            eprintln!("Falha ao gravar na zona {}: {err}", index + 1);
            return 3;
        }
    }
    println!("Cores de teste aplicadas nas 5 zonas.");
    0
}

/// `--off`: switch the keyboard lighting off.
pub fn run_off() -> i32 {
    let Some(found) = discover() else {
        eprintln!("Nenhum Logitech G213 encontrado.");
        return 1;
    };
    let device = match G213::open(&found.path) {
        Ok(device) => device,
        Err(err) => {
            eprintln!("Não foi possível abrir {}: {err}", found.path.display());
            return 2;
        }
    };
    match device.set_mode(0x00, Rgb::BLACK, 50, 0x01) {
        Ok(()) => {
            println!("Iluminação desligada.");
            0
        }
        Err(err) => {
            eprintln!("Falha ao enviar o comando: {err}");
            3
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_acknowledgement_cannot_confirm_a_new_color() {
        let mut delivery = Delivery::default();
        let old = delivery.queue();
        delivery.report(old, DeliveryState::Sending);
        let current = delivery.queue();
        delivery.report(old, DeliveryState::Confirmed);
        assert_eq!(delivery.state, DeliveryState::Pending);
        delivery.report(current, DeliveryState::Failed("HID timeout".into()));
        assert!(matches!(delivery.state, DeliveryState::Failed(_)));
        let retry = delivery.queue();
        delivery.report(retry, DeliveryState::Confirmed);
        assert_eq!(delivery.state, DeliveryState::Confirmed);
    }

    #[test]
    fn supported_mouse_variants_use_their_own_lighting_protocol() {
        let prodigy = Model::from_uevent("HID_ID=0003:0000046D:0000C084").unwrap();
        let lightsync = Model::from_uevent("HID_ID=0003:0000046D:0000C09D").unwrap();
        let color = Rgb::new(17, 34, 51);
        assert_eq!(
            &prodigy.mouse_packet(color, true)[..10],
            &[0x11, 0xff, 0x0e, 0x3a, 0, 1, 17, 34, 51, 2]
        );
        assert_eq!(
            lightsync.mouse_packet(color, true),
            [
                0x11, 0xff, 0x0e, 0x10, 0, 1, 17, 34, 51, 2, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0
            ]
        );
        assert_eq!(lightsync.mouse_packet(Rgb::BLACK, false)[5], 0);
        assert!(lightsync.matches_descriptor(&[0x06, 0, 0xff, 9, 2, 0xa1, 1, 0x85, 0x11]));
        assert!(!lightsync.matches_descriptor(&[0x06, 0, 0xff, 9, 1, 0xa1, 1, 0x85, 0x11]));
        assert_eq!(Model::from_uevent("HID_ID=0005:0000046D:0000C092"), None);
    }

    #[test]
    fn shared_rgb_reaches_both_device_protocols() {
        use std::os::{fd::OwnedFd, unix::net::UnixDatagram};
        let mut config = crate::config::Config {
            sync_colors: true,
            ..Default::default()
        };
        config.shared.color = Rgb::new(255, 0, 0);
        config.shared.hue = 120;
        config.shared.brightness = 50;
        for (model, state, expected) in [
            (
                Model::G213,
                ApplyState::from(&config),
                [0x11, 0xff, 0x0c, 0x3c, 0, 1, 0, 128, 0],
            ),
            (
                Model::G203Lightsync,
                ApplyState::from(&config.mouse_lighting()),
                [0x11, 0xff, 0x0e, 0x10, 0, 1, 0, 128, 0],
            ),
        ] {
            let (socket, peer) = UnixDatagram::pair().unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            let file = fs::File::from(OwnedFd::from(socket));
            let device = G213 {
                fd: file.as_raw_fd(),
                file,
            };
            let responder = thread::spawn(move || {
                let mut request = [0; PACKET_LEN];
                assert_eq!(peer.recv(&mut request).unwrap(), PACKET_LEN);
                assert_eq!(request[..9], expected);
                // A keyboard input report may arrive before the command reply.
                peer.send(&[0x01, 0x00, 0x00, 0x00]).unwrap();
                let reply = packet(&request[..4]);
                peer.send(&reply).unwrap();
                // Keep the peer alive until the reader has consumed the datagrams.
                peer
            });
            model.apply(&device, &state).unwrap();
            responder.join().unwrap();
        }
    }

    #[test]
    fn hidpp_errors_are_not_acknowledgements() {
        let request = [0x11, 0xff, 0x0e, 0x10];
        let reply = [0x11, 0xff, 0xff, 0x0e, 0x10, 0x05];
        assert!(
            hidpp_response(&request, &reply)
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("0x05")
        );
        assert!(hidpp_response(&request, &[0x11, 0xff, 0x0e, 0x50]).is_none());
    }

    #[test]
    fn idle_disconnect_is_detected_without_writing() {
        use std::os::fd::OwnedFd;
        let (socket, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        let file = fs::File::from(OwnedFd::from(socket));
        let device = G213 {
            fd: file.as_raw_fd(),
            file,
        };
        assert!(!device.disconnected());
        drop(peer);
        assert!(device.disconnected());
    }

    #[test]
    fn direct_packet_matches_openrgb_layout() {
        let packet = build_direct_packet(3, Rgb::new(0x12, 0x34, 0x56));
        assert_eq!(
            &packet[..10],
            &[0x11, 0xFF, 0x0C, 0x3A, 0x03, 0x01, 0x12, 0x34, 0x56, 0x02]
        );
        assert!(packet[10..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn breathing_packet_speed_layout() {
        let packet = build_mode_packet(0x02, Rgb::new(1, 2, 3), 50, 0x01);
        // 50 * 100 = 5000 = 0x1388
        assert_eq!(packet[5], 0x02);
        assert_eq!(packet[9], 0x13);
        assert_eq!(packet[10], 0x88);
        assert_eq!(packet[12], 0x64);
        assert_eq!(packet[13], 0x00);
    }

    #[test]
    fn cycle_packet_speed_layout() {
        let packet = build_mode_packet(0x03, Rgb::BLACK, 200, 0x01);
        // 200 * 100 = 20000 = 0x4E20
        assert_eq!(packet[11], 0x4E);
        assert_eq!(packet[12], 0x20);
        assert_eq!(packet[13], 0x64);
    }

    #[test]
    fn wave_packet_direction_and_speed() {
        let packet = build_mode_packet(0x04, Rgb::BLACK, 10, 0x06);
        // 10 * 100 = 1000 = 0x03E8
        assert_eq!(packet[11], 0x03);
        assert_eq!(packet[12], 0xE8);
        assert_eq!(packet[13], 0x06);
        assert_eq!(packet[15], 0x03);
    }

    #[test]
    fn descriptor_detection_accepts_control_interface() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x06, 0x43, 0xFF, 0x85, 0x11, 0x91, 0x00, 0xC0,
        ];
        assert!(is_control_interface(&descriptor));
        assert!(!is_control_interface(&[0x05, 0x01, 0x09, 0x06, 0x85, 0x01]));
    }

    #[test]
    fn uevent_parsing_is_case_insensitive_for_ids() {
        let uevent = "DRIVER=hid-generic\nHID_ID=0003:0000046D:0000C336\nHID_NAME=Logitech Gaming Keyboard G213\n";
        assert_eq!(Model::from_uevent(uevent), Some(Model::G213));
        assert_eq!(
            uevent_value(uevent, "HID_NAME").as_deref(),
            Some("Logitech Gaming Keyboard G213")
        );
        assert_eq!(Model::from_uevent("HID_ID=0003:0000046D:0000C335\n"), None);
    }
}
