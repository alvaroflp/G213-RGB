//! Visual language, derived from the OpenLogi design system.
//!
//! The palette follows `openlogi-desktop/themes/openlogi.json`: a near-neutral
//! page, slightly raised cards, one hairline border colour and a single blue
//! accent. Radii are 12 px for cards and 8 px for controls, so a control's
//! corner sits concentrically inside its card.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals};
use std::sync::Arc;

pub const ACCENT: Color32 = Color32::from_rgb(0x3b, 0x82, 0xf6);
pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(0x1d, 0x4e, 0xd8);
pub const STATUS_CONNECTED: Color32 = Color32::from_rgb(0x22, 0xc5, 0x5e);
pub const STATUS_CONNECTING: Color32 = Color32::from_rgb(0xea, 0xb3, 0x08);
pub const STATUS_OFFLINE: Color32 = Color32::from_rgb(0x6b, 0x72, 0x80);
pub const STATUS_DANGER: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);

/// Header height, matching OpenLogi's `HEADER_H`.
pub const HEADER_H: f32 = 64.0;
/// Left navigation rail width, matching OpenLogi's `DETAIL_RAIL_W`.
pub const RAIL_W: f32 = 168.0;
/// Inset around a tab body.
pub const SCREEN_PAD: f32 = 20.0;
/// Inner padding of a card.
pub const CARD_PAD: f32 = 18.0;
pub const CARD_RADIUS: u8 = 12;
pub const CONTROL_RADIUS: u8 = 8;
pub const CONTROL_H: f32 = 32.0;
/// Compact settings card width (`ContentWidth::Small`).
pub const CONTENT_SMALL: f32 = 560.0;
/// Wider single panel (`ContentWidth::Medium`).
pub const CONTENT_MEDIUM: f32 = 680.0;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub page: Color32,
    pub panel: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub input: Color32,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            dark: true,
            page: Color32::from_rgb(0x1a, 0x1a, 0x1d),
            panel: Color32::from_rgb(0x22, 0x22, 0x27),
            control: Color32::from_rgb(0x2b, 0x2b, 0x32),
            control_hover: Color32::from_rgb(0x33, 0x33, 0x3c),
            muted: Color32::from_rgb(0x2f, 0x2f, 0x36),
            border: Color32::from_rgb(0x2f, 0x2f, 0x36),
            text: Color32::from_rgb(0xf0, 0xf0, 0xf3),
            text_muted: Color32::from_rgb(0x8a, 0x8a, 0x93),
            input: Color32::from_rgb(0x1c, 0x1c, 0x21),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            page: Color32::from_rgb(0xf4, 0xf4, 0xf6),
            panel: Color32::from_rgb(0xff, 0xff, 0xff),
            control: Color32::from_rgb(0xec, 0xec, 0xf1),
            control_hover: Color32::from_rgb(0xe1, 0xe1, 0xe8),
            muted: Color32::from_rgb(0xe8, 0xe8, 0xed),
            border: Color32::from_rgb(0xd9, 0xd9, 0xe0),
            text: Color32::from_rgb(0x1a, 0x1a, 0x1d),
            text_muted: Color32::from_rgb(0x6b, 0x6b, 0x73),
            input: Color32::from_rgb(0xf7, 0xf7, 0xf9),
        }
    }

    pub fn palette(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    /// The accent at OpenLogi's ~12 % tint, used for selected rows and cards.
    pub fn accent_tint(self) -> Color32 {
        blend(ACCENT, self.panel, 0.12)
    }

    pub fn accent_tint_hover(self) -> Color32 {
        blend(ACCENT, self.panel, 0.18)
    }

    /// Text placed on top of an accent-filled control.
    pub fn on_accent(self) -> Color32 {
        Color32::WHITE
    }

    pub fn danger_tint(self) -> Color32 {
        blend(STATUS_DANGER, self.panel, 0.12)
    }

    pub fn success_tint(self) -> Color32 {
        blend(STATUS_CONNECTED, self.panel, 0.12)
    }

    pub fn warning_tint(self) -> Color32 {
        blend(STATUS_CONNECTING, self.panel, 0.14)
    }

    pub fn is_dark(self) -> bool {
        self.dark
    }
}

pub fn blend(foreground: Color32, background: Color32, alpha: f32) -> Color32 {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |fg: u8, bg: u8| (f32::from(fg) * a + f32::from(bg) * (1.0 - a)).round() as u8;
    Color32::from_rgb(
        mix(foreground.r(), background.r()),
        mix(foreground.g(), background.g()),
        mix(foreground.b(), background.b()),
    )
}

pub fn color32(color: crate::config::Rgb) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    SemiBold,
}

fn family(weight: Weight) -> FontFamily {
    match weight {
        Weight::Regular => FontFamily::Proportional,
        Weight::Medium => FontFamily::Name("inter-medium".into()),
        Weight::SemiBold => FontFamily::Name("inter-semibold".into()),
    }
}

pub fn font(size: f32, weight: Weight) -> FontId {
    FontId::new(size, family(weight))
}

/// Bundle Inter and Phosphor. Inter is licensed under the SIL Open Font
/// License; Phosphor icons under MIT. Both live in `assets/`.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "inter-regular".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter-medium".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter-semibold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-SemiBold.ttf"
        ))),
    );

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter-regular".to_owned());
    fonts.families.insert(
        FontFamily::Name("inter-medium".into()),
        vec!["inter-medium".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("inter-semibold".into()),
        vec!["inter-semibold".to_owned()],
    );

    ctx.set_fonts(fonts);
}

/// Apply the palette to egui's own widgets (sliders, inputs, modals, tooltips)
/// so built-in controls match the hand-painted surfaces.
///
/// Both the dark and the light [`Style`](egui::Style) are configured, so
/// following the system theme can flip between two fully themed styles.
pub fn apply_style(ctx: &egui::Context) {
    apply_theme_style(ctx, egui::Theme::Dark, Palette::dark());
    apply_theme_style(ctx, egui::Theme::Light, Palette::light());
}

fn apply_theme_style(ctx: &egui::Context, theme: egui::Theme, pal: Palette) {
    let mut style = (*ctx.style_of(theme)).clone();
    style.visuals = if pal.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    let v = &mut style.visuals;
    v.panel_fill = pal.page;
    v.window_fill = pal.panel;
    v.extreme_bg_color = pal.input;
    v.faint_bg_color = pal.control;
    v.override_text_color = Some(pal.text);
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = STATUS_CONNECTING;
    v.error_fg_color = STATUS_DANGER;
    v.selection.bg_fill = blend(ACCENT, pal.panel, 0.35);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_stroke = Stroke::new(1.0, pal.border);
    v.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    v.menu_corner_radius = CornerRadius::same(10);
    v.widgets.noninteractive.bg_fill = pal.panel;
    v.widgets.noninteractive.weak_bg_fill = pal.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, pal.text);
    v.widgets.noninteractive.corner_radius = CornerRadius::same(CONTROL_RADIUS);
    v.widgets.inactive.bg_fill = pal.control;
    v.widgets.inactive.weak_bg_fill = pal.control;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, pal.border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, pal.text);
    v.widgets.inactive.corner_radius = CornerRadius::same(CONTROL_RADIUS);
    v.widgets.hovered.bg_fill = pal.control_hover;
    v.widgets.hovered.weak_bg_fill = pal.control_hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, pal.border);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, pal.text);
    v.widgets.hovered.corner_radius = CornerRadius::same(CONTROL_RADIUS);
    v.widgets.active.bg_fill = pal.control_hover;
    v.widgets.active.weak_bg_fill = pal.control_hover;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, pal.text);
    v.widgets.active.corner_radius = CornerRadius::same(CONTROL_RADIUS);
    v.widgets.open.bg_fill = pal.control_hover;
    v.widgets.open.bg_stroke = Stroke::new(1.0, pal.border);
    v.widgets.open.corner_radius = CornerRadius::same(CONTROL_RADIUS);

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.slider_width = 220.0;
    style.spacing.interact_size.y = CONTROL_H;
    style.text_styles = [
        (
            egui::TextStyle::Heading,
            font(20.0, Weight::SemiBold),
        ),
        (egui::TextStyle::Body, font(15.0, Weight::Regular)),
        (egui::TextStyle::Button, font(14.0, Weight::Medium)),
        (egui::TextStyle::Small, font(12.0, Weight::Regular)),
        (
            egui::TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_style_of(theme, style);
}

/// Map the saved preference onto egui's own theme selection.
pub fn set_preference(ctx: &egui::Context, mode: crate::config::ThemeMode) {
    ctx.set_theme(match mode {
        crate::config::ThemeMode::System => egui::ThemePreference::System,
        crate::config::ThemeMode::Light => egui::ThemePreference::Light,
        crate::config::ThemeMode::Dark => egui::ThemePreference::Dark,
    });
}

/// Resolve `Sistema` against the desktop's preference. egui reports `None`
/// on some platforms; dark is a reasonable fallback.
pub fn resolve_dark(ctx: &egui::Context, mode: crate::config::ThemeMode) -> bool {
    match mode {
        crate::config::ThemeMode::Light => false,
        crate::config::ThemeMode::Dark => true,
        crate::config::ThemeMode::System => ctx
            .system_theme()
            .map(|theme| theme == egui::Theme::Dark)
            .unwrap_or(true),
    }
}
