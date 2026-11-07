//! Persistent settings for the application.
//!
//! The config is deliberately small and human-readable. It lives in
//! `$XDG_CONFIG_HOME/g213-rgb/config.json` (or `~/.config/...`) and is written
//! atomically so a crash mid-save cannot leave a truncated file behind.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

/// The G213 exposes five lighting areas, not per-key LEDs.
pub const ZONE_COUNT: usize = 5;

/// Portuguese labels for the five areas, in protocol order (`0x01..=0x05`).
pub const ZONE_NAMES: [&str; ZONE_COUNT] = [
    "Esquerda",
    "Centro",
    "Direita",
    "Setas / Home",
    "Numpad",
];

/// A plain sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const WHITE: Self = Self::new(0xff, 0xff, 0xff);
    pub const BLACK: Self = Self::new(0x00, 0x00, 0x00);
    pub const ACCENT: Self = Self::new(0x3b, 0x82, 0xf6);

    /// `#RRGGBB`, as shown next to custom colours.
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Parses `#RRGGBB` or `RRGGBB`.
    pub fn from_hex(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('#');
        if text.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&text[0..2], 16).ok()?;
        let g = u8::from_str_radix(&text[2..4], 16).ok()?;
        let b = u8::from_str_radix(&text[4..6], 16).ok()?;
        Some(Self::new(r, g, b))
    }

    /// Software brightness (the G213 protocol has no brightness command).
    pub fn scaled(self, percent: u8) -> Self {
        let p = u32::from(percent.min(100));
        let scale = |c: u8| ((u32::from(c) * p + 50) / 100) as u8;
        Self::new(scale(self.r), scale(self.g), scale(self.b))
    }

    /// Linear blend, `t = 0` returns `self`.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        Self::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
        )
    }

    /// Relative luminance, used to pick readable text over a swatch.
    pub fn luminance(self) -> f32 {
        let channel = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// One colour for the whole keyboard.
    #[default]
    Uniform,
    /// A colour per lighting area.
    Zones,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Off,
    #[default]
    Static,
    Breathing,
    Cycle,
    Wave,
}

impl Effect {
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Static,
        Self::Breathing,
        Self::Cycle,
        Self::Wave,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Desligado",
            Self::Static => "Estático",
            Self::Breathing => "Respiração",
            Self::Cycle => "Ciclo",
            Self::Wave => "Onda",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Off => "Apaga a iluminação do teclado.",
            Self::Static => "Uma cor fixa, uniforme ou por zona.",
            Self::Breathing => "A cor pulsa suavemente.",
            Self::Cycle => "O teclado percorre todo o espectro.",
            Self::Wave => "Uma onda de cor atravessa as zonas.",
        }
    }

    /// Native G213 effect code (`0x00..=0x04`).
    pub fn protocol_mode(self) -> u8 {
        match self {
            Self::Off => 0x00,
            Self::Static => 0x01,
            Self::Breathing => 0x02,
            Self::Cycle => 0x03,
            Self::Wave => 0x04,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveDirection {
    #[default]
    LeftToRight,
    RightToLeft,
    CenterToEdge,
    EdgeToCenter,
}

impl WaveDirection {
    pub const ALL: [Self; 4] = [
        Self::LeftToRight,
        Self::RightToLeft,
        Self::CenterToEdge,
        Self::EdgeToCenter,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::LeftToRight => "Esquerda → Direita",
            Self::RightToLeft => "Direita → Esquerda",
            Self::CenterToEdge => "Centro → Bordas",
            Self::EdgeToCenter => "Bordas → Centro",
        }
    }

    /// Direction byte expected by the G213 wave packet.
    pub fn protocol_value(self) -> u8 {
        match self {
            Self::LeftToRight => 0x01,
            Self::RightToLeft => 0x06,
            Self::CenterToEdge => 0x03,
            Self::EdgeToCenter => 0x08,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::Light => "Claro",
            Self::Dark => "Escuro",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub layout: Layout,
    pub color: Rgb,
    pub zones: [Rgb; ZONE_COUNT],
    /// 1..=100. Applied in software by scaling every colour we send.
    pub brightness: u8,
    pub effect: Effect,
    pub effect_color: Rgb,
    /// 1..=100, where 100 is the fastest. Mapped onto the device's 10..=200.
    pub speed: u8,
    pub direction: WaveDirection,
    pub theme: ThemeMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            layout: Layout::Uniform,
            color: Rgb::ACCENT,
            zones: [
                Rgb::new(0xff, 0x3b, 0x30),
                Rgb::new(0xff, 0x95, 0x00),
                Rgb::new(0xff, 0xcc, 0x00),
                Rgb::new(0x34, 0xc7, 0x59),
                Rgb::new(0x00, 0x7a, 0xff),
            ],
            brightness: 100,
            effect: Effect::Static,
            effect_color: Rgb::ACCENT,
            speed: 55,
            direction: WaveDirection::LeftToRight,
            theme: ThemeMode::System,
        }
    }
}

impl Config {
    /// Clamp values that came from a hand-edited file.
    pub fn sanitize(&mut self) {
        self.brightness = self.brightness.clamp(1, 100);
        self.speed = self.speed.clamp(1, 100);
    }

    /// The colour a lighting action targets.
    pub fn zone_color(&self, zone: usize) -> Rgb {
        self.zones[zone.min(ZONE_COUNT - 1)]
    }

    pub fn set_zone_color(&mut self, zone: usize, color: Rgb) {
        if zone < ZONE_COUNT {
            self.zones[zone] = color;
        }
    }

    /// Colour used for the device illustration.
    pub fn preview_color(&self) -> Rgb {
        match self.layout {
            Layout::Uniform => self.color,
            Layout::Zones => {
                let sum = self.zones.iter().fold([0u32; 3], |mut acc, c| {
                    acc[0] += u32::from(c.r);
                    acc[1] += u32::from(c.g);
                    acc[2] += u32::from(c.b);
                    acc
                });
                let n = ZONE_COUNT as u32;
                Rgb::new(
                    (sum[0] / n) as u8,
                    (sum[1] / n) as u8,
                    (sum[2] / n) as u8,
                )
            }
        }
    }

    pub fn load() -> Self {
        let path = config_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(mut config) => {
                config.sanitize();
                config
            }
            Err(err) => {
                eprintln!(
                    "aviso: não foi possível ler {}: {err}. Usando os padrões.",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}

/// The device protocol's internal speed: 10 is fastest, 200 is slowest.
pub fn protocol_speed(ui_speed: u8) -> u8 {
    let speed = i32::from(ui_speed.clamp(1, 100));
    (210 - 2 * speed).clamp(10, 200) as u8
}

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME")
        && !dir.is_empty()
    {
        return PathBuf::from(dir).join("g213-rgb");
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return PathBuf::from(home).join(".config/g213-rgb");
    }
    PathBuf::from(".g213-rgb")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let color = Rgb::new(0x3b, 0x82, 0xf6);
        assert_eq!(color.hex(), "#3B82F6");
        assert_eq!(Rgb::from_hex("#3b82f6"), Some(color));
        assert_eq!(Rgb::from_hex("3B82F6"), Some(color));
        assert_eq!(Rgb::from_hex("#3b82f"), None);
        assert_eq!(Rgb::from_hex("#gg82f6"), None);
    }

    #[test]
    fn brightness_scales_round_trip() {
        let color = Rgb::new(200, 100, 50);
        assert_eq!(color.scaled(100), color);
        assert_eq!(color.scaled(0), Rgb::BLACK);
        assert_eq!(color.scaled(50), Rgb::new(100, 50, 25));
    }

    #[test]
    fn ui_speed_maps_to_protocol_range() {
        assert_eq!(protocol_speed(100), 10);
        assert_eq!(protocol_speed(1), 200);
        assert_eq!(protocol_speed(0), 200);
        assert_eq!(protocol_speed(255), 10);
    }

    #[test]
    fn defaults_are_sane() {
        let config = Config::default();
        assert!(config.enabled);
        assert_eq!(config.brightness, 100);
        assert_eq!(config.zones.len(), ZONE_COUNT);
    }
}
