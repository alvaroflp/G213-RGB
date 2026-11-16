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
pub const ZONE_NAMES: [&str; ZONE_COUNT] =
    ["Esquerda", "Centro", "Direita", "Setas / Home", "Numpad"];

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
        let text = text.trim();
        let text = text.strip_prefix('#').unwrap_or(text);
        if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
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

    /// Shift hue in sRGB while preserving saturation and value.
    pub fn with_hue(self, degrees: u16) -> Self {
        if degrees.is_multiple_of(360) {
            return self;
        }
        let mut hsv =
            egui::ecolor::Hsva::from_rgb([self.r, self.g, self.b].map(|c| f32::from(c) / 255.0));
        hsv.h = (hsv.h + f32::from(degrees) / 360.0).fract();
        let [r, g, b] = hsv.to_rgb().map(|c| (c * 255.0).round() as u8);
        Self::new(r, g, b)
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    pub enabled: bool,
    pub layout: Layout,
    pub color: Rgb,
    pub zones: [Rgb; ZONE_COUNT],
    /// 1..=100. Applied in software by scaling every colour we send.
    pub brightness: u8,
    pub hue: u16,
    pub effect: Effect,
    pub effect_color: Rgb,
    /// 1..=100, where 100 is the fastest. Mapped onto the device's 10..=200.
    pub speed: u8,
    pub direction: WaveDirection,
}

impl Default for Lighting {
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
            hue: 0,
            effect: Effect::Static,
            effect_color: Rgb::ACCENT,
            speed: 55,
            direction: WaveDirection::LeftToRight,
        }
    }
}

impl Lighting {
    /// Clamp values that came from a hand-edited file.
    pub fn sanitize(&mut self) {
        self.brightness = self.brightness.clamp(1, 100);
        self.speed = self.speed.clamp(1, 100);
        self.hue %= 360;
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

    pub fn render_color(&self, color: Rgb) -> Rgb {
        color.with_hue(self.hue).scaled(self.brightness)
    }

    /// Approximate the native animation without sending animation frames to HID.
    pub fn preview_colors(&self, seconds: f64) -> [Rgb; ZONE_COUNT] {
        if !self.enabled || self.effect == Effect::Off {
            return [Rgb::BLACK; ZONE_COUNT];
        }
        let phase = (seconds / (f64::from(protocol_speed(self.speed)) / 10.0)) as f32;
        let rainbow = |phase: f32| {
            let colors = [
                Rgb::new(255, 60, 80),
                Rgb::new(255, 190, 40),
                Rgb::new(40, 220, 120),
                Rgb::new(30, 180, 255),
                Rgb::new(140, 80, 255),
                Rgb::new(255, 60, 80),
            ];
            let position = phase.rem_euclid(1.0) * 5.0;
            let index = (position.floor() as usize).min(4);
            colors[index].lerp(colors[index + 1], position.fract())
        };
        match self.effect {
            Effect::Static => match self.layout {
                Layout::Uniform => [self.render_color(self.color); ZONE_COUNT],
                Layout::Zones => self.zones.map(|c| self.render_color(c)),
            },
            Effect::Breathing => {
                let pulse = (0.5 + 0.5 * (phase * std::f32::consts::TAU).cos())
                    * f32::from(self.brightness);
                [self.effect_color
                    .with_hue(self.hue)
                    .scaled(pulse.round() as u8); ZONE_COUNT]
            }
            Effect::Cycle => [rainbow(phase); ZONE_COUNT],
            Effect::Wave => std::array::from_fn(|zone| {
                let x = zone as f32 / (ZONE_COUNT - 1) as f32;
                let offset = match self.direction {
                    WaveDirection::LeftToRight => -x,
                    WaveDirection::RightToLeft => x,
                    WaveDirection::CenterToEdge => -(x - 0.5).abs() * 2.0,
                    WaveDirection::EdgeToCenter => (x - 0.5).abs() * 2.0,
                };
                rainbow(phase + offset * 0.7)
            }),
            Effect::Off => [Rgb::BLACK; ZONE_COUNT],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub lighting: Lighting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SolidLight {
    pub enabled: bool,
    pub color: Rgb,
    pub brightness: u8,
    pub hue: u16,
}

impl Default for SolidLight {
    fn default() -> Self {
        Self {
            enabled: true,
            color: Rgb::ACCENT,
            brightness: 100,
            hue: 0,
        }
    }
}

impl SolidLight {
    pub fn as_lighting(self) -> Lighting {
        Lighting {
            enabled: self.enabled,
            color: self.color,
            brightness: self.brightness,
            hue: self.hue,
            ..Lighting::default()
        }
    }
}

/// Keep the original flat JSON compatible with versions before profiles.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub lighting: Lighting,
    pub theme: ThemeMode,
    pub profiles: Vec<Profile>,
    pub mouse: SolidLight,
    pub shared: SolidLight,
    pub sync_colors: bool,
}

impl Config {
    pub fn keyboard_lighting(&self) -> Lighting {
        if self.sync_colors {
            self.shared.as_lighting()
        } else {
            self.lighting.clone()
        }
    }

    pub fn mouse_lighting(&self) -> Lighting {
        if self.sync_colors {
            self.shared
        } else {
            self.mouse
        }
        .as_lighting()
    }

    pub fn sanitize(&mut self) {
        self.lighting.sanitize();
        for light in [&mut self.mouse, &mut self.shared] {
            light.brightness = light.brightness.clamp(1, 100);
            light.hue %= 360;
        }
        for profile in &mut self.profiles {
            profile.lighting.sanitize();
        }
    }

    pub fn save_profile(&mut self, name: &str) -> Result<(), &'static str> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err("Use um nome de 1 a 40 caracteres.");
        }
        if self
            .profiles
            .iter()
            .chain(builtin_profiles().iter())
            .any(|p| p.name.to_lowercase() == name.to_lowercase())
        {
            return Err("Já existe um perfil com esse nome.");
        }
        if self.profiles.len() >= 32 {
            return Err("Você já tem 32 perfis. Remova um para continuar.");
        }
        self.profiles.push(Profile {
            name: name.to_owned(),
            lighting: self.lighting.clone(),
        });
        Ok(())
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

pub fn builtin_profiles() -> Vec<Profile> {
    let make = |name: &str, lighting| Profile {
        name: name.into(),
        lighting,
    };
    vec![
        make(
            "Aurora",
            Lighting {
                layout: Layout::Zones,
                zones: [
                    Rgb::new(93, 72, 238),
                    Rgb::new(145, 70, 255),
                    Rgb::new(215, 71, 190),
                    Rgb::new(47, 185, 210),
                    Rgb::new(46, 218, 164),
                ],
                ..Lighting::default()
            },
        ),
        make(
            "Foco",
            Lighting {
                color: Rgb::WHITE,
                brightness: 40,
                ..Lighting::default()
            },
        ),
        make(
            "Oceano",
            Lighting {
                effect: Effect::Breathing,
                effect_color: Rgb::new(0, 190, 220),
                speed: 30,
                brightness: 75,
                ..Lighting::default()
            },
        ),
        make(
            "Espectro",
            Lighting {
                effect: Effect::Wave,
                speed: 55,
                ..Lighting::default()
            },
        ),
    ]
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
    fn shared_light_survives_reload_and_keeps_individual_settings() {
        let mut config = Config::default();
        config.lighting.layout = Layout::Zones;
        config.mouse.color = Rgb::WHITE;
        config.shared.color = Rgb::new(255, 0, 0);
        config.shared.hue = 120;
        config.shared.brightness = 50;
        config.sync_colors = true;
        let mut saved: Config =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        let green = Rgb::new(0, 128, 0);
        assert_eq!(
            saved.keyboard_lighting().preview_colors(0.0),
            [green; ZONE_COUNT]
        );
        assert_eq!(saved.mouse_lighting().preview_colors(0.0)[0], green);
        saved.sync_colors = false;
        assert_eq!(saved.keyboard_lighting().layout, Layout::Zones);
        assert_eq!(saved.mouse_lighting().color, Rgb::WHITE);
    }

    #[test]
    fn hex_round_trip() {
        let color = Rgb::new(0x3b, 0x82, 0xf6);
        assert_eq!(color.hex(), "#3B82F6");
        assert_eq!(Rgb::from_hex("#3b82f6"), Some(color));
        assert_eq!(Rgb::from_hex("3B82F6"), Some(color));
        assert_eq!(Rgb::from_hex("#3b82f"), None);
        assert_eq!(Rgb::from_hex("#gg82f6"), None);
        for invalid in ["aé123", "💡ab", "##123456", "１２", "12345g"] {
            assert_eq!(Rgb::from_hex(invalid), None);
        }
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
        assert!(config.lighting.enabled);
        assert_eq!(config.lighting.brightness, 100);
        assert_eq!(config.lighting.zones.len(), ZONE_COUNT);
    }

    #[test]
    fn old_flat_config_retains_lighting_and_theme() {
        let mut config: Config = serde_json::from_str(r##"{"color":{"r":19,"g":85,"b":170},"layout":"zones","brightness":42,"effect":"breathing","speed":0,"theme":"dark"}"##).unwrap();
        config.sanitize();
        assert_eq!(config.lighting.color, Rgb::new(19, 85, 170));
        assert_eq!(config.lighting.layout, Layout::Zones);
        assert_eq!(config.lighting.brightness, 42);
        assert_eq!(config.lighting.speed, 1);
        assert_eq!(config.lighting.effect, Effect::Breathing);
        assert_eq!(config.theme, ThemeMode::Dark);
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn profiles_round_trip_independently_of_current_lighting_and_theme() {
        let mut config = Config {
            theme: ThemeMode::Dark,
            ..Config::default()
        };
        config.lighting = builtin_profiles().remove(0).lighting;
        config.save_profile("  Trabalho  ").unwrap();
        let saved = config.lighting.clone();
        config.lighting = Lighting::default();
        let json = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.profiles[0].name, "Trabalho");
        assert_eq!(restored.profiles[0].lighting, saved);
        assert_eq!(restored.lighting, Lighting::default());
        assert_eq!(restored.theme, ThemeMode::Dark);
        assert!(!json.contains("\"lighting\":{\"lighting\""));
    }

    #[test]
    fn profile_names_are_validated_without_overwriting_existing_profiles() {
        let mut config = Config::default();
        assert!(config.save_profile("  ").is_err());
        assert!(config.save_profile("aurora").is_err());
        assert!(config.save_profile(&"x".repeat(41)).is_err());
        config.save_profile("Concentração").unwrap();
        assert!(config.save_profile("CONCENTRAÇÃO").is_err());
        assert_eq!(config.profiles.len(), 1);
    }

    #[test]
    fn preview_respects_power_brightness_and_native_effect_capabilities() {
        let mut lighting = Lighting {
            brightness: 50,
            ..Lighting::default()
        };
        assert_eq!(
            lighting.preview_colors(0.0),
            [lighting.color.scaled(50); ZONE_COUNT]
        );
        lighting.effect = Effect::Cycle;
        let cycle = lighting.preview_colors(0.3);
        lighting.brightness = 1;
        lighting.effect_color = Rgb::BLACK;
        assert_eq!(lighting.preview_colors(0.3), cycle);
        assert_ne!(lighting.preview_colors(3.0), cycle);
        lighting.effect = Effect::Wave;
        let wave = lighting.preview_colors(0.0);
        assert_ne!(wave[0], wave[1]);
        lighting.enabled = false;
        assert_eq!(lighting.preview_colors(0.0), [Rgb::BLACK; ZONE_COUNT]);
    }
}
