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

/// How long we wait for the keyboard's acknowledgement before moving on.
const ACK_TIMEOUT_MS: i32 = 20;
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

    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| "Logitech G213 Prodigy".to_string())
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
    pub path: PathBuf,
    pub name: String,
}

/// Everything the device needs in order to render the current lighting.
#[derive(Clone, Debug)]
pub struct ApplyState {
    pub enabled: bool,
    pub layout: Layout,
    pub color: Rgb,
    pub zones: [Rgb; 5],
    pub brightness: u8,
    pub effect: Effect,
    pub effect_color: Rgb,
    pub speed: u8,
    pub direction: WaveDirection,
}

impl From<&crate::config::Config> for ApplyState {
    fn from(config: &crate::config::Config) -> Self {
        Self {
            enabled: config.enabled,
            layout: config.layout,
            color: config.color,
            zones: config.zones,
            brightness: config.brightness,
            effect: config.effect,
            effect_color: config.effect_color,
            speed: config.speed,
            direction: config.direction,
        }
    }
}

/// Locate the G213 control interface.
///
/// Both the boot keyboard interface and the vendor interface share the same
/// USB IDs, so the report descriptor is what tells them apart: the control
/// interface declares usage page `0xFF43` and report ID `0x11`.
pub fn discover() -> Option<FoundDevice> {
    if let Ok(override_path) = std::env::var("G213_HIDRAW")
        && !override_path.is_empty()
    {
        let path = PathBuf::from(override_path);
        let name = read_hid_name(&path).unwrap_or_else(|| "Logitech G213 Prodigy".to_string());
        return Some(FoundDevice { path, name });
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
        if !is_g213(&uevent) {
            continue;
        }
        let descriptor = fs::read(entry.join("device/report_descriptor")).unwrap_or_default();
        if !is_control_interface(&descriptor) {
            continue;
        }
        let Some(hidraw) = entry.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let name = uevent_value(&uevent, "HID_NAME")
            .unwrap_or_else(|| "Logitech G213 Prodigy".to_string());
        return Some(FoundDevice {
            path: PathBuf::from("/dev").join(hidraw),
            name,
        });
    }
    None
}

fn is_g213(uevent: &str) -> bool {
    let Some(id) = uevent_value(uevent, "HID_ID") else {
        return false;
    };
    let mut parts = id.split(':');
    let (_bus, vendor, product) = match (parts.next(), parts.next(), parts.next()) {
        (Some(bus), Some(vendor), Some(product)) => (bus, vendor, product),
        _ => return false,
    };
    let parse = |text: &str| u32::from_str_radix(text, 16).ok();
    parse(vendor) == Some(u32::from(VENDOR_ID)) && parse(product) == Some(u32::from(PRODUCT_ID))
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
    let has_usage_page = descriptor
        .windows(3)
        .any(|window| window == [0x06, 0x43, 0xff]);
    let has_report_id = descriptor.windows(2).any(|window| window == [0x85, REPORT_ID]);
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
                    let color = state.color.scaled(state.brightness);
                    for zone in 1..=5u8 {
                        self.set_direct(zone, color)?;
                    }
                    Ok(())
                }
                Layout::Zones => {
                    for (index, color) in state.zones.iter().enumerate() {
                        self.set_direct(index as u8 + 1, color.scaled(state.brightness))?;
                    }
                    Ok(())
                }
            },
            effect => {
                let color = state.effect_color.scaled(state.brightness);
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

    pub fn set_mode(
        &self,
        mode: u8,
        color: Rgb,
        speed: u8,
        direction: u8,
    ) -> io::Result<()> {
        self.send(&build_mode_packet(mode, color, speed, direction))
    }

    fn send(&self, packet: &[u8; PACKET_LEN]) -> io::Result<()> {
        let written = unsafe {
            libc::write(
                self.fd,
                packet.as_ptr().cast::<libc::c_void>(),
                PACKET_LEN,
            )
        };
        if written < 0 {
            return Err(io::Error::last_os_error());
        }
        if written as usize != PACKET_LEN {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "gravação parcial no dispositivo HID",
            ));
        }

        // OpenRGB blocks on hid_read() here. Poll first so a silent device can
        // never freeze the UI thread; the acknowledgement itself carries no
        // useful data.
        let mut poll_fd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll_fd, 1, ACK_TIMEOUT_MS) };
        if ready > 0 && poll_fd.revents & libc::POLLIN != 0 {
            let mut ack = [0u8; PACKET_LEN];
            unsafe {
                libc::read(
                    self.fd,
                    ack.as_mut_ptr().cast::<libc::c_void>(),
                    PACKET_LEN,
                );
            }
        }
        Ok(())
    }
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

enum Command {
    Apply(Box<ApplyState>),
    Refresh,
}

/// Owns the device on a background thread so a slow acknowledgement can never
/// stall a frame. New states replace older queued ones (latest wins), and the
/// last state is re-applied automatically when the keyboard comes back.
pub struct DeviceWorker {
    tx: Sender<Command>,
    status: Arc<Mutex<DeviceStatus>>,
}

impl DeviceWorker {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(DeviceStatus::searching()));
        let worker_status = Arc::clone(&status);
        let _ = thread::Builder::new()
            .name("g213-device".to_string())
            .spawn(move || worker_loop(rx, worker_status));
        Self { tx, status }
    }

    pub fn status(&self) -> DeviceStatus {
        match self.status.lock() {
            Ok(status) => status.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn apply(&self, state: ApplyState) {
        let _ = self.tx.send(Command::Apply(Box::new(state)));
    }

    pub fn refresh(&self) {
        let _ = self.tx.send(Command::Refresh);
    }
}

fn worker_loop(rx: Receiver<Command>, status: Arc<Mutex<DeviceStatus>>) {
    let mut found: Option<FoundDevice> = None;
    let mut device: Option<G213> = None;
    let mut wanted: Option<ApplyState> = None;
    let mut need_apply = true;
    let mut next_scan = Instant::now();

    while Instant::now() >= next_scan && device.is_none() {
        try_connect(&mut found, &mut device, &status);
        next_scan = Instant::now() + RECONNECT_INTERVAL;
    }

    loop {
        let timeout = next_scan.saturating_duration_since(Instant::now());
        match rx.recv_timeout(timeout.max(Duration::from_millis(50))) {
            Ok(Command::Apply(state)) => {
                wanted = Some(*state);
                need_apply = true;
            }
            Ok(Command::Refresh) => {
                found = None;
                device = None;
                next_scan = Instant::now();
                need_apply = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        // Collapse a burst of slider changes into the newest state.
        while let Ok(command) = rx.try_recv() {
            match command {
                Command::Apply(state) => {
                    wanted = Some(*state);
                    need_apply = true;
                }
                Command::Refresh => {
                    found = None;
                    device = None;
                    next_scan = Instant::now();
                    need_apply = true;
                }
            }
        }

        if device.is_none() && Instant::now() >= next_scan {
            try_connect(&mut found, &mut device, &status);
            next_scan = Instant::now() + RECONNECT_INTERVAL;
            if device.is_some() {
                need_apply = true;
            }
        }

        if need_apply
            && let (Some(dev), Some(state)) = (device.as_ref(), wanted.as_ref())
        {
            match dev.apply(state) {
                Ok(()) => {
                    need_apply = false;
                    if let Some(found) = found.as_ref() {
                        set_status(&status, DeviceStatus::connected(found.path.clone(), found.name.clone()));
                    }
                }
                Err(err) => {
                    let permission_denied = err.kind() == io::ErrorKind::PermissionDenied;
                    device = None;
                    let mut next = DeviceStatus {
                        state: if permission_denied {
                            ConnectionState::PermissionDenied
                        } else {
                            ConnectionState::Error
                        },
                        path: found.as_ref().map(|found| found.path.clone()),
                        name: found.as_ref().map(|found| found.name.clone()),
                        detail: Some(err.to_string()),
                    };
                    next.detail = Some(err.to_string());
                    set_status(&status, next);
                    next_scan = Instant::now();
                }
            }
        }
    }
}

fn try_connect(
    found: &mut Option<FoundDevice>,
    device: &mut Option<G213>,
    status: &Arc<Mutex<DeviceStatus>>,
) {
    if found.is_none() {
        *found = discover();
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

    match G213::open(&info.path) {
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

/// `--diagnose`: print what was found and whether it can be opened.
pub fn run_diagnose() -> i32 {
    println!("G213 RGB — diagnóstico");
    println!();

    let Some(found) = discover() else {
        println!("Nenhum Logitech G213 (046d:c336) encontrado.");
        println!("Verifique se o teclado está conectado por USB.");
        return 1;
    };

    println!("Dispositivo : {}", found.name);
    println!("Interface   : {}", found.path.display());

    match G213::open(&found.path) {
        Ok(_) => {
            println!("Acesso      : OK");
            0
        }
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            println!("Acesso      : NEGADO ({err})");
            println!();
            println!("Execute o instalador para liberar o acesso sem root:");
            println!("    ./install.sh");
            println!("Depois desconecte e reconecte o teclado.");
            2
        }
        Err(err) => {
            println!("Acesso      : ERRO ({err})");
            3
        }
    }
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
        assert!(is_g213(uevent));
        assert_eq!(
            uevent_value(uevent, "HID_NAME").as_deref(),
            Some("Logitech Gaming Keyboard G213")
        );
        assert!(!is_g213("HID_ID=0003:0000046D:0000C335\n"));
    }
}
