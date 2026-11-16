//! Local bindings and the evdev/uinput macro runner.

use evdev::{
    AttributeSet, EventType, InputEvent, KeyCode, raw_stream::RawDevice, uinput::VirtualDevice,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs, io,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

pub const MAX_STEPS: usize = 24;
pub const MAX_NAME: usize = 40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Peripheral {
    #[default]
    Keyboard,
    Mouse,
}

impl Peripheral {
    pub fn label(self) -> &'static str {
        match self {
            Self::Keyboard => "G213 Prodigy",
            Self::Mouse => "G203",
        }
    }

    pub fn triggers(self) -> &'static [(&'static str, KeyCode)] {
        use KeyCode as K;
        match self {
            Self::Keyboard => &[
                ("F1", K::KEY_F1),
                ("F2", K::KEY_F2),
                ("F3", K::KEY_F3),
                ("F4", K::KEY_F4),
                ("F5", K::KEY_F5),
                ("F6", K::KEY_F6),
                ("F7", K::KEY_F7),
                ("F8", K::KEY_F8),
                ("F9", K::KEY_F9),
                ("F10", K::KEY_F10),
                ("F11", K::KEY_F11),
                ("F12", K::KEY_F12),
            ],
            Self::Mouse => &[
                ("Clique da roda", K::BTN_MIDDLE),
                ("Avançar", K::BTN_EXTRA),
                ("Voltar", K::BTN_SIDE),
            ],
        }
    }

    fn from_ids(bus: u16, vendor: u16, product: u16) -> Option<Self> {
        if (bus, vendor) != (3, 0x046d) {
            return None;
        }
        match product {
            0xc336 => Some(Self::Keyboard),
            0xc084 | 0xc092 | 0xc09d => Some(Self::Mouse),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Shortcut(String),
    Delay(u16),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub device: Peripheral,
    pub trigger: u16,
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub bindings: Vec<Binding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Shortcut(Vec<KeyCode>),
    Delay(Duration),
}

impl Binding {
    pub fn validate(&self) -> Result<(), String> {
        self.compile().map(|_| ())
    }

    fn compile(&self) -> Result<Vec<Action>, String> {
        if self.name.trim().is_empty() || self.name.chars().count() > MAX_NAME {
            return Err("Use um nome de 1 a 40 caracteres.".into());
        }
        if !self
            .device
            .triggers()
            .iter()
            .any(|(_, code)| code.code() == self.trigger)
        {
            return Err("Gatilho incompatível com o dispositivo.".into());
        }
        if self.steps.is_empty()
            || self.steps.len() > MAX_STEPS
            || !self
                .steps
                .iter()
                .any(|step| matches!(step, Step::Shortcut(_)))
        {
            return Err("Inclua pelo menos um atalho e até 24 etapas.".into());
        }
        self.steps
            .iter()
            .enumerate()
            .map(|(index, step)| match step {
                Step::Delay(ms @ 10..=5000) => {
                    Ok(Action::Delay(Duration::from_millis((*ms).into())))
                }
                Step::Delay(_) => Err(format!(
                    "Etapa {}: use uma pausa de 10 a 5000 ms.",
                    index + 1
                )),
                Step::Shortcut(text) => {
                    parse_shortcut(text).map(Action::Shortcut).ok_or_else(|| {
                        format!(
                            "Etapa {}: atalho inválido. Exemplo: Ctrl+Shift+T.",
                            index + 1
                        )
                    })
                }
            })
            .collect()
    }
}

fn parse_shortcut(text: &str) -> Option<Vec<KeyCode>> {
    use KeyCode as K;
    const LETTERS: [K; 26] = [
        K::KEY_A,
        K::KEY_B,
        K::KEY_C,
        K::KEY_D,
        K::KEY_E,
        K::KEY_F,
        K::KEY_G,
        K::KEY_H,
        K::KEY_I,
        K::KEY_J,
        K::KEY_K,
        K::KEY_L,
        K::KEY_M,
        K::KEY_N,
        K::KEY_O,
        K::KEY_P,
        K::KEY_Q,
        K::KEY_R,
        K::KEY_S,
        K::KEY_T,
        K::KEY_U,
        K::KEY_V,
        K::KEY_W,
        K::KEY_X,
        K::KEY_Y,
        K::KEY_Z,
    ];
    const DIGITS: [K; 10] = [
        K::KEY_0,
        K::KEY_1,
        K::KEY_2,
        K::KEY_3,
        K::KEY_4,
        K::KEY_5,
        K::KEY_6,
        K::KEY_7,
        K::KEY_8,
        K::KEY_9,
    ];
    let mut parts = text.split('+').peekable();
    let mut chord = Vec::new();
    while let Some(part) = parts.next() {
        let part = part.trim().to_ascii_lowercase();
        let key = if parts.peek().is_some() {
            match part.as_str() {
                "ctrl" => K::KEY_LEFTCTRL,
                "shift" => K::KEY_LEFTSHIFT,
                "alt" => K::KEY_LEFTALT,
                "super" => K::KEY_LEFTMETA,
                _ => return None,
            }
        } else {
            match part.as_bytes() {
                [letter @ b'a'..=b'z'] => LETTERS[usize::from(letter - b'a')],
                [digit @ b'0'..=b'9'] => DIGITS[usize::from(digit - b'0')],
                _ => match part.as_str() {
                    "enter" => K::KEY_ENTER,
                    "tab" => K::KEY_TAB,
                    "esc" => K::KEY_ESC,
                    "espaço" | "space" => K::KEY_SPACE,
                    "backspace" => K::KEY_BACKSPACE,
                    "delete" => K::KEY_DELETE,
                    "up" => K::KEY_UP,
                    "down" => K::KEY_DOWN,
                    "left" => K::KEY_LEFT,
                    "right" => K::KEY_RIGHT,
                    _ => {
                        Peripheral::Keyboard
                            .triggers()
                            .iter()
                            .find(|(label, _)| label.eq_ignore_ascii_case(&part))?
                            .1
                    }
                },
            }
        };
        if chord.contains(&key) || chord.len() == 5 {
            return None;
        }
        chord.push(key);
    }
    Some(chord)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Paused,
    Waiting,
    Ready { keyboard: bool, mouse: bool },
    Error(String),
}

pub struct Worker {
    tx: mpsc::Sender<Settings>,
    status: Arc<Mutex<Status>>,
}

impl Worker {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::Paused));
        let reporting = status.clone();
        if let Err(error) = thread::Builder::new()
            .name("g213-macros".into())
            .spawn(move || run(rx, reporting))
        {
            *status.lock().unwrap() = Status::Error(error.to_string());
        }
        Self { tx, status }
    }

    pub fn update(&self, settings: &Settings) {
        let _ = self.tx.send(settings.clone());
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}

type Bindings = HashMap<(Peripheral, u16), Vec<Action>>;

struct Source {
    device: RawDevice,
    peripheral: Peripheral,
    held: HashSet<u16>,
    dropped: bool,
}

impl Source {
    fn released_trigger(&mut self, bindings: &Bindings) -> io::Result<Option<(Peripheral, u16)>> {
        let mut trigger = None;
        for event in self.device.fetch_events()? {
            if event.event_type() == EventType::SYNCHRONIZATION {
                if event.code() == evdev::SynchronizationCode::SYN_DROPPED.0 {
                    self.held.clear();
                    trigger = None;
                    self.dropped = true;
                } else if event.code() == evdev::SynchronizationCode::SYN_REPORT.0 {
                    self.dropped = false;
                }
            } else if !self.dropped && event.event_type() == EventType::KEY {
                let key = event.code();
                if bindings.contains_key(&(self.peripheral, key)) {
                    match event.value() {
                        1 => {
                            self.held.insert(key);
                        }
                        0 if self.held.remove(&key) => {
                            trigger = Some((self.peripheral, key));
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(trigger)
    }
}

fn sysfs_peripheral(event: &Path) -> Option<Peripheral> {
    let id = event.join("device/id");
    let read =
        |field| u16::from_str_radix(fs::read_to_string(id.join(field)).ok()?.trim(), 16).ok();
    Peripheral::from_ids(read("bustype")?, read("vendor")?, read("product")?)
}

fn discover(bindings: &Bindings, sources: &mut HashMap<PathBuf, Source>) -> Option<String> {
    let entries = match fs::read_dir("/sys/class/input") {
        Ok(entries) => entries,
        Err(err) => return Some(format!("Não foi possível buscar entradas: {err}")),
    };
    let mut error = None;
    for entry in entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("event"))
    {
        let Some(peripheral) = sysfs_peripheral(&entry.path()) else {
            continue;
        };
        if !bindings.keys().any(|(device, _)| *device == peripheral) {
            continue;
        }
        let path = Path::new("/dev/input").join(entry.file_name());
        if sources.contains_key(&path) {
            continue;
        }
        match RawDevice::open(&path) {
            Ok(device) => {
                let id = device.input_id();
                if Peripheral::from_ids(id.bus_type().0, id.vendor(), id.product())
                    != Some(peripheral)
                    || !device.supported_keys().is_some_and(|keys| {
                        bindings
                            .keys()
                            .any(|(kind, key)| *kind == peripheral && keys.contains(KeyCode(*key)))
                    })
                {
                    continue;
                }
                let fd = device.as_raw_fd();
                // SAFETY: the open device owns this descriptor.
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                if flags >= 0
                    && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } >= 0
                {
                    sources.insert(
                        path,
                        Source {
                            device,
                            peripheral,
                            held: HashSet::new(),
                            dropped: false,
                        },
                    );
                }
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                error = Some(format!(
                    "Sem acesso às entradas de {}. Instale com --with-macros.",
                    peripheral.label()
                ));
            }
            Err(_) => {}
        }
    }
    error
}

fn make_output() -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::new();
    // The full standard key range lets the desktop classify this as a keyboard
    // even if the only configured action is a small chord such as Ctrl+T.
    for code in 1..=255 {
        keys.insert(KeyCode(code));
    }
    VirtualDevice::builder()?
        .name("G213 RGB Macros")
        .with_keys(&keys)?
        .build()
}

fn emit_shortcut(output: &mut VirtualDevice, keys: &[KeyCode]) -> io::Result<()> {
    let event = |key: &KeyCode, value| InputEvent::new(EventType::KEY.0, key.code(), value);
    output.emit(&keys.iter().map(|key| event(key, 1)).collect::<Vec<_>>())?;
    output.emit(
        &keys
            .iter()
            .rev()
            .map(|key| event(key, 0))
            .collect::<Vec<_>>(),
    )
}

fn run(rx: mpsc::Receiver<Settings>, report: Arc<Mutex<Status>>) {
    let publish = |status| *report.lock().unwrap() = status;
    let mut bindings = Bindings::new();
    let mut sources = HashMap::new();
    let mut output = None;
    let mut queue = VecDeque::new();
    let mut next_scan = Instant::now();
    let mut ready_at = Instant::now();

    loop {
        let interval = Duration::from_millis(if sources.is_empty() { 250 } else { 10 });
        match rx.recv_timeout(interval) {
            Ok(mut settings) => {
                while let Ok(latest) = rx.try_recv() {
                    settings = latest;
                }
                bindings.clear();
                queue.clear();
                sources.clear();
                output = None;
                if settings.enabled {
                    for binding in settings.bindings {
                        if let Ok(actions) = binding.compile() {
                            bindings
                                .entry((binding.device, binding.trigger))
                                .or_insert(actions);
                        }
                    }
                }
                next_scan = Instant::now();
                publish(if bindings.is_empty() {
                    Status::Paused
                } else {
                    Status::Waiting
                });
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if bindings.is_empty() {
            continue;
        }

        let now = Instant::now();
        if now >= next_scan {
            next_scan = now + Duration::from_secs(1);
            let error = discover(&bindings, &mut sources);
            if !sources.is_empty() && output.is_none() {
                match make_output() {
                    Ok(device) => {
                        output = Some(device);
                        // Let the desktop register the new virtual keyboard.
                        ready_at = now + Duration::from_millis(300);
                    }
                    Err(err) => {
                        sources.clear();
                        publish(Status::Error(format!(
                            "/dev/uinput: {err}. Instale com --with-macros."
                        )));
                        continue;
                    }
                }
            }
            publish(match error {
                Some(error) => Status::Error(error),
                None if sources.is_empty() => Status::Waiting,
                None => Status::Ready {
                    keyboard: sources
                        .values()
                        .any(|source| source.peripheral == Peripheral::Keyboard),
                    mouse: sources
                        .values()
                        .any(|source| source.peripheral == Peripheral::Mouse),
                },
            });
        }

        let mut trigger = None;
        sources.retain(|_, source| match source.released_trigger(&bindings) {
            Ok(found) => {
                trigger = trigger.or(found);
                true
            }
            Err(err) => err.kind() == io::ErrorKind::WouldBlock,
        });
        if queue.is_empty()
            && now >= ready_at
            && let Some(trigger) = trigger
        {
            queue.extend(bindings[&trigger].iter().cloned());
        }
        if now >= ready_at
            && let Some(action) = queue.pop_front()
        {
            match action {
                Action::Delay(duration) => ready_at = now + duration,
                Action::Shortcut(chord) => {
                    if let Some(device) = &mut output {
                        if let Err(err) = emit_shortcut(device, &chord) {
                            output = None;
                            queue.clear();
                            publish(Status::Error(format!("Falha ao executar o atalho: {err}")));
                        } else {
                            ready_at = now + Duration::from_millis(25);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_shortcuts_and_device_triggers() {
        assert_eq!(
            parse_shortcut("Ctrl + Shift + T"),
            Some(vec![
                KeyCode::KEY_LEFTCTRL,
                KeyCode::KEY_LEFTSHIFT,
                KeyCode::KEY_T
            ])
        );
        for invalid in ["", "Ctrl+Ctrl+A", "A+B", "Ctrl+", "Ctrl+Super+"] {
            assert!(parse_shortcut(invalid).is_none(), "{invalid}");
        }
        let mut binding = Binding {
            device: Peripheral::Mouse,
            trigger: KeyCode::BTN_SIDE.code(),
            name: "Atalho".into(),
            steps: vec![Step::Delay(75), Step::Shortcut("Ctrl+T".into())],
        };
        assert!(binding.validate().is_ok());
        binding.trigger = KeyCode::KEY_F4.code();
        assert!(binding.validate().is_err());
    }
}
