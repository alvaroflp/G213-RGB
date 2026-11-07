//! The application shell and screens.
//!
//! The layout mirrors the OpenLogi desktop app: a gallery screen that lists
//! devices, and a device page with a compact identity header, a stable section
//! rail and a single centred content column. All controls are painted by
//! [`crate::widgets`] so the visual language matches that source.

use crate::config::{
    self, Config, Effect, Layout, Rgb, ThemeMode, WaveDirection, ZONE_COUNT, ZONE_NAMES,
};
use crate::device::{ApplyState, ConnectionState, DeviceStatus, DeviceWorker};
use crate::keyboard::{self, KeyboardState};
use crate::theme::{self, Palette, Weight};
use crate::widgets::{self, ButtonKind};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Layout as ELayout, Margin, Pos2, Rect,
    Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, pos2,
    vec2,
};
use std::time::{Duration, Instant};

const DEVICE_NAME: &str = "Logitech G213 Prodigy";
const CARD_W: f32 = 300.0;
const CARD_H: f32 = 312.0;

/// The nine preset colours, shared by the lighting and effect tabs.
const SWATCHES: [Rgb; 9] = [
    Rgb::new(0xff, 0x3b, 0x30),
    Rgb::new(0xff, 0x95, 0x00),
    Rgb::new(0xff, 0xcc, 0x00),
    Rgb::new(0x34, 0xc7, 0x59),
    Rgb::new(0x00, 0xc7, 0xbe),
    Rgb::new(0x00, 0x7a, 0xff),
    Rgb::new(0x58, 0x56, 0xd6),
    Rgb::new(0xaf, 0x52, 0xde),
    Rgb::new(0xff, 0x2d, 0x55),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Home,
    Detail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Lighting,
    Effects,
    Device,
}

impl Tab {
    const ALL: [Self; 3] = [Self::Lighting, Self::Effects, Self::Device];

    fn label(self) -> &'static str {
        match self {
            Self::Lighting => "Iluminação",
            Self::Effects => "Efeitos",
            Self::Device => "Dispositivo",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Lighting => egui_phosphor::regular::LIGHTBULB,
            Self::Effects => egui_phosphor::regular::SPARKLE,
            Self::Device => egui_phosphor::regular::KEYBOARD,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorTarget {
    Uniform,
    Effect,
    Zone(usize),
}

struct ColorEditor {
    target: ColorTarget,
    color: Rgb,
    hex: String,
}

struct Toast {
    text: String,
    until: Instant,
}

enum SwatchPick {
    None,
    Color(Rgb),
    Custom,
}

pub struct G213App {
    config: Config,
    worker: DeviceWorker,
    status: DeviceStatus,
    last_status_check: Instant,
    preferred: Option<ThemeMode>,
    view: View,
    tab: Tab,
    selected_zone: usize,
    dirty: bool,
    last_edit: Option<Instant>,
    save_error: Option<String>,
    settings_open: bool,
    color_editor: Option<ColorEditor>,
    toast: Option<Toast>,
}

impl G213App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx);

        let config = Config::load();
        theme::set_preference(&cc.egui_ctx, config.theme);

        let worker = DeviceWorker::start();
        worker.apply(ApplyState::from(&config));
        let preferred = Some(config.theme);

        Self {
            config,
            worker,
            status: DeviceStatus::searching(),
            last_status_check: Instant::now(),
            preferred,
            view: View::Home,
            tab: Tab::Lighting,
            selected_zone: 0,
            dirty: false,
            last_edit: None,
            save_error: None,
            settings_open: false,
            color_editor: None,
            toast: None,
        }
    }

    /// Record an edit: push it to the keyboard immediately and schedule a
    /// debounced save.
    fn touch(&mut self) {
        self.config.sanitize();
        self.dirty = true;
        self.last_edit = Some(Instant::now());
        self.worker.apply(ApplyState::from(&self.config));
    }

    fn save_if_due(&mut self) {
        if !self.dirty {
            return;
        }
        let due = self
            .last_edit
            .map_or(true, |edit| edit.elapsed() >= Duration::from_millis(400));
        if !due {
            return;
        }
        match self.config.save() {
            Ok(()) => {
                self.dirty = false;
                self.save_error = None;
            }
            Err(err) => {
                self.save_error = Some(err.to_string());
                self.last_edit = Some(Instant::now());
            }
        }
    }

    fn notify(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast {
            text: text.into(),
            until: Instant::now() + Duration::from_millis(2400),
        });
    }

    fn status_label(&self) -> &'static str {
        match self.status.state {
            ConnectionState::Connected => "Conectado",
            ConnectionState::PermissionDenied => "Sem permissão",
            ConnectionState::NotFound => "Desconectado",
            ConnectionState::Searching => "Procurando",
            ConnectionState::Error => "Erro de conexão",
        }
    }

    fn status_color(&self) -> Color32 {
        match self.status.state {
            ConnectionState::Connected => theme::STATUS_CONNECTED,
            ConnectionState::PermissionDenied => theme::STATUS_DANGER,
            ConnectionState::NotFound => theme::STATUS_OFFLINE,
            ConnectionState::Searching => theme::STATUS_CONNECTING,
            ConnectionState::Error => theme::STATUS_DANGER,
        }
    }

    fn zone_colors(&self) -> [Color32; ZONE_COUNT] {
        let mut colors = [theme::color32(Rgb::BLACK); ZONE_COUNT];
        if !self.config.enabled {
            return colors;
        }
        match self.config.effect {
            Effect::Static => match self.config.layout {
                Layout::Uniform => {
                    let color = theme::color32(self.config.color);
                    colors = [color; ZONE_COUNT];
                }
                Layout::Zones => {
                    for (slot, value) in colors.iter_mut().zip(self.config.zones) {
                        *slot = theme::color32(value);
                    }
                }
            },
            Effect::Off => {}
            _ => {
                let color = theme::color32(self.config.effect_color);
                colors = [color; ZONE_COUNT];
            }
        }
        colors
    }

    fn keyboard_state(&self, selected: Option<usize>) -> KeyboardState {
        KeyboardState {
            zone_colors: self.zone_colors(),
            enabled: self.config.enabled && self.config.effect != Effect::Off,
            selected_zone: selected,
        }
    }

    fn target_color(&self, target: ColorTarget) -> Rgb {
        match target {
            ColorTarget::Uniform => self.config.color,
            ColorTarget::Effect => self.config.effect_color,
            ColorTarget::Zone(zone) => self.config.zone_color(zone),
        }
    }

    fn set_target_color(&mut self, target: ColorTarget, color: Rgb) {
        match target {
            ColorTarget::Uniform => self.config.color = color,
            ColorTarget::Effect => self.config.effect_color = color,
            ColorTarget::Zone(zone) => self.config.set_zone_color(zone, color),
        }
        self.touch();
    }

    fn open_color_editor(&mut self, target: ColorTarget) {
        let color = self.target_color(target);
        self.color_editor = Some(ColorEditor {
            target,
            color,
            hex: color.hex(),
        });
    }

    // -----------------------------------------------------------------------------------------
    // Shell
    // -----------------------------------------------------------------------------------------

    fn header_ui(&mut self, ui: &mut Ui, pal: Palette) {
        let full = ui.max_rect();
        let header = Rect::from_min_size(full.min, vec2(full.width(), theme::HEADER_H));
        let mut hud = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_size(
                    header.min + vec2(20.0, 0.0),
                    vec2((header.width() - 40.0).max(0.0), header.height()),
                ))
                .layout(ELayout::left_to_right(Align::Center)),
        );

        match self.view {
            View::Home => {
                hud.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(
                        RichText::new("Dispositivos")
                            .font(theme::font(20.0, Weight::SemiBold))
                            .color(pal.text),
                    );
                    let count = if self.status.state == ConnectionState::NotFound {
                        "Nenhum dispositivo"
                    } else {
                        "1 dispositivo"
                    };
                    ui.label(
                        RichText::new(count)
                            .font(theme::font(12.0, Weight::Regular))
                            .color(pal.text_muted),
                    );
                });
                hud.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, &pal, egui_phosphor::regular::GEAR, "Ajustes")
                        .clicked()
                    {
                        self.settings_open = true;
                    }
                    if widgets::button(
                        ui,
                        &pal,
                        "Atualizar",
                        Some(egui_phosphor::regular::ARROW_CLOCKWISE),
                        ButtonKind::Secondary,
                    )
                    .clicked()
                    {
                        self.worker.refresh();
                        self.status = self.worker.status();
                        self.notify("Procurando o teclado…");
                    }
                });
            }
            View::Detail => {
                if widgets::icon_button(
                    ui,
                    &pal,
                    egui_phosphor::regular::ARROW_LEFT,
                    "Voltar para dispositivos",
                )
                .clicked()
                {
                    self.view = View::Home;
                }
                hud.add_space(4.0);
                hud.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(
                        RichText::new(DEVICE_NAME)
                            .font(theme::font(18.0, Weight::SemiBold))
                            .color(pal.text),
                    );
                    ui.label(
                        RichText::new("Teclado · 5 zonas RGB")
                            .font(theme::font(12.0, Weight::Regular))
                            .color(pal.text_muted),
                    );
                });
                hud.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, &pal, egui_phosphor::regular::GEAR, "Ajustes")
                        .clicked()
                    {
                        self.settings_open = true;
                    }
                    widgets::status_pill(ui, &pal, self.status_label(), self.status_color());
                });
            }
        }

        ui.painter().rect_filled(
            Rect::from_min_size(
                pos2(full.left(), header.bottom() - 1.0),
                vec2(full.width(), 1.0),
            ),
            0.0,
            pal.border,
        );
    }

    fn rail_ui(&mut self, ui: &mut Ui, pal: Palette) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, pal.panel);
        ui.painter().rect_filled(
            Rect::from_min_size(
                pos2(full.right() - 1.0, full.top()),
                vec2(1.0, full.height()),
            ),
            0.0,
            pal.border,
        );

        let mut rail = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_size(
                    full.min + vec2(12.0, 16.0),
                    vec2((full.width() - 24.0).max(0.0), full.height() - 32.0),
                ))
                .layout(ELayout::top_down(Align::Min)),
        );
        rail.spacing_mut().item_spacing.y = 4.0;

        for tab in Tab::ALL {
            let selected = tab == self.tab;
            let (rect, response) =
                rail.allocate_exact_size(vec2(rail.available_width(), 36.0), Sense::click());
            if selected {
                rail.painter()
                    .rect(rect, widgets::CONTROL, pal.accent_tint(), Stroke::NONE, StrokeKind::Inside);
                rail.painter().rect_filled(
                    Rect::from_min_size(pos2(rect.left(), rect.center().y - 8.0), vec2(3.0, 16.0)),
                    CornerRadius::same(2),
                    theme::ACCENT,
                );
            } else if response.hovered() {
                rail.painter()
                    .rect_filled(rect, widgets::CONTROL, pal.control);
            }
            rail.painter().text(
                pos2(rect.left() + 12.0, rect.center().y),
                Align2::LEFT_CENTER,
                tab.icon(),
                FontId::proportional(16.0),
                if selected { theme::ACCENT } else { pal.text_muted },
            );
            rail.painter().text(
                pos2(rect.left() + 36.0, rect.center().y),
                Align2::LEFT_CENTER,
                tab.label(),
                theme::font(13.5, Weight::Medium),
                if selected { pal.text } else { pal.text_muted },
            );
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.tab = tab;
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Home
    // -----------------------------------------------------------------------------------------

    fn home_ui(&mut self, ui: &mut Ui, pal: Palette) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(24.0);
                let avail = ui.available_width();
                let content_w = (avail - 48.0).clamp(320.0, 760.0);
                let pad = ((avail - content_w) * 0.5).max(0.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.vertical(|ui| {
                        ui.set_width(content_w);
                        if self.connection_callout(ui, &pal) {
                            // The callout handled its own action.
                        }
                        ui.add_space(8.0);
                        let response = self.device_card(ui, &pal);
                        if response.clicked() {
                            self.view = View::Detail;
                        }
                    });
                });
                ui.add_space(24.0);
            });
    }

    fn device_card(&mut self, ui: &mut Ui, pal: &Palette) -> Response {
        let (rect, response) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::click());
        let hovered = response.hovered();
        let accent = theme::color32(self.config.preview_color());

        // The glow is the card's only decoration; it uses the current colour so
        // the gallery previews the lighting state at a glance.
        if self.config.enabled {
            for (index, spread) in [22.0f32, 16.0, 11.0, 7.0].into_iter().enumerate() {
                let alpha = [8u8, 12, 16, 22][index];
                ui.painter().rect_filled(
                    rect.shrink(10.0).expand(spread),
                    CornerRadius::same((22.0 + spread) as u8),
                    Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), alpha),
                );
            }
        }

        ui.painter().rect(
            rect,
            widgets::CARD,
            pal.panel,
            Stroke::new(
                1.0,
                if hovered { theme::ACCENT } else { pal.border },
            ),
            StrokeKind::Inside,
        );

        let preview = Rect::from_min_size(
            rect.min + vec2(18.0, 18.0),
            vec2(rect.width() - 36.0, 150.0),
        );
        let mut artwork = ui.new_child(
            UiBuilder::new()
                .max_rect(preview)
                .layout(ELayout::top_down(Align::Center)),
        );
        artwork.add_space(34.0);
        let mut state = self.keyboard_state(None);
        state.selected_zone = None;
        keyboard::keyboard(&mut artwork, pal, preview.width().min(242.0), &state, false);

        // Status pill, top-right over the artwork.
        let color = self.status_color();
        let label = self.status_label();
        let width = pill_width(ui, label);
        paint_pill(
            ui,
            pal,
            pos2(rect.right() - 16.0 - width, rect.top() + 14.0),
            label,
            color,
        );

        paint_text(
            ui,
            pos2(rect.left() + 18.0, preview.bottom() + 8.0),
            DEVICE_NAME,
            16.0,
            Weight::SemiBold,
            pal.text,
        );
        paint_text(
            ui,
            pos2(rect.left() + 18.0, preview.bottom() + 32.0),
            "Teclado",
            12.5,
            Weight::Regular,
            pal.text_muted,
        );

        let mut badge_x = rect.left() + 18.0;
        badge_x += paint_badge(ui, pal, pos2(badge_x, preview.bottom() + 58.0), "5 zonas RGB")
            + 6.0;
        paint_badge(ui, pal, pos2(badge_x, preview.bottom() + 58.0), "USB");

        ui.painter().rect_filled(
            Rect::from_min_size(
                pos2(rect.left() + 18.0, rect.bottom() - 52.0),
                vec2(rect.width() - 36.0, 1.0),
            ),
            0.0,
            pal.border,
        );
        paint_text(
            ui,
            pos2(rect.left() + 18.0, rect.bottom() - 34.0),
            if self.config.enabled && self.config.effect != Effect::Off {
                "Iluminação ativa"
            } else {
                "Iluminação desligada"
            },
            12.0,
            Weight::Regular,
            pal.text_muted,
        );
        let arrow = egui_phosphor::regular::ARROW_RIGHT;
        ui.painter().text(
            pos2(rect.right() - 18.0, rect.bottom() - 34.0),
            Align2::RIGHT_TOP,
            arrow,
            FontId::proportional(14.0),
            if hovered { theme::ACCENT } else { pal.text_muted },
        );

        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Renders a status notice. Returns true when the user clicked its action.
    fn connection_callout(&mut self, ui: &mut Ui, pal: &Palette) -> bool {
        let (color, icon, text, action) = match self.status.state {
            ConnectionState::Connected => return false,
            ConnectionState::PermissionDenied => (
                theme::STATUS_DANGER,
                egui_phosphor::regular::WARNING_CIRCLE,
                "Sem permissão para acessar o teclado. Execute ./install.sh para liberar o acesso.",
                Some("Copiar comando"),
            ),
            ConnectionState::NotFound => (
                theme::STATUS_OFFLINE,
                egui_phosphor::regular::PLUG,
                "Teclado não encontrado. Verifique o cabo USB e clique em atualizar.",
                Some("Atualizar"),
            ),
            ConnectionState::Searching => (
                theme::STATUS_CONNECTING,
                egui_phosphor::regular::SPINNER,
                "Procurando o Logitech G213…",
                None,
            ),
            ConnectionState::Error => (
                theme::STATUS_DANGER,
                egui_phosphor::regular::WARNING,
                "Falha ao conversar com o teclado.",
                Some("Atualizar"),
            ),
        };
        let clicked = widgets::callout(ui, pal, color, icon, text, action);
        if clicked {
            match action {
                Some("Copiar comando") => {
                    ui.ctx().copy_text("./install.sh".to_string());
                    self.notify("Comando copiado para a área de transferência");
                }
                _ => {
                    self.worker.refresh();
                    self.status = self.worker.status();
                    self.notify("Procurando o teclado…");
                }
            }
            ui.add_space(8.0);
        }
        clicked
    }

    // -----------------------------------------------------------------------------------------
    // Detail
    // -----------------------------------------------------------------------------------------

    fn detail_ui(&mut self, ui: &mut Ui, pal: Palette) {
        let full = ui.max_rect();
        let rail_rect = Rect::from_min_size(full.min, vec2(theme::RAIL_W, full.height()));
        let content_rect = Rect::from_min_max(
            pos2(full.min.x + theme::RAIL_W, full.min.y),
            full.max,
        );

        let mut rail = ui.new_child(UiBuilder::new().max_rect(rail_rect));
        self.rail_ui(&mut rail, pal);

        let mut content = ui.new_child(
            UiBuilder::new()
                .max_rect(content_rect)
                .layout(ELayout::top_down(Align::Min)),
        );
        match self.tab {
            Tab::Lighting => self.lighting_tab(&mut content, pal),
            Tab::Effects => self.effects_tab(&mut content, pal),
            Tab::Device => self.device_tab(&mut content, pal),
        }
    }

    fn lighting_tab(&mut self, ui: &mut Ui, pal: Palette) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(theme::SCREEN_PAD);
                centered(ui, theme::CONTENT_SMALL, |ui| {
                    let mut changed = false;

                    widgets::panel_card(
                        ui,
                        &pal,
                        Some((egui_phosphor::regular::LIGHTBULB, "Iluminação")),
                        |ui| {
                            let interactive =
                                self.config.layout == Layout::Zones && self.config.enabled;
                            let selected = if interactive {
                                Some(self.selected_zone)
                            } else {
                                None
                            };
                            let state = self.keyboard_state(selected);
                            let width = ui.available_width();
                            let (_, clicked) =
                                keyboard::keyboard(ui, &pal, width, &state, interactive);
                            if let Some(zone) = clicked {
                                self.selected_zone = zone;
                            }
                            ui.add_space(10.0);
                            let enabled = widgets::toggle_row(
                                ui,
                                &pal,
                                "Iluminação ativa",
                                "Liga ou desliga todo o RGB do teclado",
                                self.config.enabled,
                            );
                            if enabled != self.config.enabled {
                                self.config.enabled = enabled;
                                if enabled && self.config.effect == Effect::Off {
                                    // Turning the master switch back on should
                                    // produce light even if the last effect was Off.
                                    self.config.effect = Effect::Static;
                                }
                                changed = true;
                            }
                            ui.add_space(8.0);
                            if let Some(layout) = widgets::segmented(
                                ui,
                                &pal,
                                "layout",
                                &[
                                    (Layout::Uniform, "Uniforme"),
                                    (Layout::Zones, "Por zona"),
                                ],
                                self.config.layout,
                            ) && layout != self.config.layout
                            {
                                self.config.layout = layout;
                                changed = true;
                            }
                        },
                    );

                    ui.add_space(12.0);

                    widgets::panel_card(
                        ui,
                        &pal,
                        Some((egui_phosphor::regular::PALETTE, "Cor")),
                        |ui| {
                            match self.config.layout {
                                Layout::Uniform => {
                                    widgets::section_label(ui, &pal, "Cor do teclado");
                                    ui.add_space(8.0);
                                    match swatch_row(ui, &pal, self.config.color) {
                                        SwatchPick::Color(color) => {
                                            self.config.color = color;
                                            changed = true;
                                        }
                                        SwatchPick::Custom => {
                                            self.open_color_editor(ColorTarget::Uniform);
                                        }
                                        SwatchPick::None => {}
                                    }
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(self.config.color.hex())
                                            .font(theme::font(12.0, Weight::Medium))
                                            .color(pal.text_muted),
                                    );
                                }
                                Layout::Zones => {
                                    widgets::section_label(
                                        ui,
                                        &pal,
                                        "Zonas de iluminação",
                                    );
                                    ui.add_space(8.0);
                                    for (index, name) in ZONE_NAMES.iter().enumerate() {
                                        let selected = index == self.selected_zone;
                                        let color = theme::color32(self.config.zone_color(index));
                                        let (rect, response) = ui.allocate_exact_size(
                                            vec2(ui.available_width(), 38.0),
                                            Sense::click(),
                                        );
                                        if selected {
                                            ui.painter().rect(
                                                rect,
                                                widgets::CONTROL,
                                                pal.accent_tint(),
                                                Stroke::NONE,
                                                StrokeKind::Inside,
                                            );
                                        } else if response.hovered() {
                                            ui.painter().rect_filled(
                                                rect,
                                                widgets::CONTROL,
                                                pal.control,
                                            );
                                        }
                                        ui.painter().circle_filled(
                                            pos2(rect.left() + 14.0, rect.center().y),
                                            6.0,
                                            color,
                                        );
                                        ui.painter().circle_stroke(
                                            pos2(rect.left() + 14.0, rect.center().y),
                                            6.0,
                                            Stroke::new(1.0, pal.border),
                                        );
                                        ui.painter().text(
                                            pos2(rect.left() + 30.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            *name,
                                            theme::font(13.0, Weight::Medium),
                                            pal.text,
                                        );
                                        ui.painter().text(
                                            pos2(rect.right() - 12.0, rect.center().y),
                                            Align2::RIGHT_CENTER,
                                            self.config.zone_color(index).hex(),
                                            theme::font(12.0, Weight::Regular),
                                            pal.text_muted,
                                        );
                                        if response
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            self.selected_zone = index;
                                        }
                                    }
                                    ui.add_space(10.0);
                                    widgets::divider(ui, &pal);
                                    ui.add_space(10.0);
                                    widgets::section_label(
                                        ui,
                                        &pal,
                                        &format!("Cor de {}", ZONE_NAMES[self.selected_zone]),
                                    );
                                    ui.add_space(8.0);
                                    let current = self.config.zone_color(self.selected_zone);
                                    match swatch_row(ui, &pal, current) {
                                        SwatchPick::Color(color) => {
                                            self.config
                                                .set_zone_color(self.selected_zone, color);
                                            changed = true;
                                        }
                                        SwatchPick::Custom => {
                                            self.open_color_editor(ColorTarget::Zone(
                                                self.selected_zone,
                                            ));
                                        }
                                        SwatchPick::None => {}
                                    }
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(current.hex())
                                            .font(theme::font(12.0, Weight::Medium))
                                            .color(pal.text_muted),
                                    );
                                }
                            }
                        },
                    );

                    ui.add_space(12.0);

                    widgets::panel_card(
                        ui,
                        &pal,
                        Some((egui_phosphor::regular::SUN, "Brilho")),
                        |ui| {
                            let mut brightness = self.config.brightness;
                            if widgets::slider_row(
                                ui,
                                &pal,
                                "Brilho",
                                &mut brightness,
                                1..=100,
                            ) {
                                self.config.brightness = brightness;
                                changed = true;
                            }
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(
                                    "O brilho é aplicado por software, escalando as cores enviadas.",
                                )
                                .font(theme::font(11.5, Weight::Regular))
                                .color(pal.text_muted),
                            );
                        },
                    );

                    if changed {
                        self.touch();
                    }
                });
                ui.add_space(theme::SCREEN_PAD);
            });
    }

    fn effects_tab(&mut self, ui: &mut Ui, pal: Palette) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(theme::SCREEN_PAD);
                centered(ui, theme::CONTENT_SMALL, |ui| {
                    let mut changed = false;
                    let mut picked = None;

                    widgets::panel_card(
                        ui,
                        &pal,
                        Some((egui_phosphor::regular::SPARKLE, "Efeitos")),
                        |ui| {
                            let avail = ui.available_width();
                            let per_row = if avail >= 470.0 { 2 } else { 1 };
                            let card_w = if per_row == 2 {
                                (avail - 12.0) * 0.5
                            } else {
                                avail
                            };
                            let mut index = 0;
                            while index < Effect::ALL.len() {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 12.0;
                                    for offset in 0..per_row {
                                        let Some(effect) =
                                            Effect::ALL.get(index + offset).copied()
                                        else {
                                            break;
                                        };
                                        let response = widgets::choice_card(
                                            ui,
                                            &pal,
                                            vec2(card_w, 102.0),
                                            effect_icon(effect),
                                            effect.label(),
                                            effect.description(),
                                            self.config.effect == effect,
                                        );
                                        if response.clicked() {
                                            picked = Some(effect);
                                        }
                                    }
                                });
                                index += per_row;
                                ui.add_space(12.0);
                            }
                        },
                    );

                    if let Some(effect) = picked
                        && effect != self.config.effect
                    {
                        self.config.effect = effect;
                        self.config.enabled = effect != Effect::Off;
                        changed = true;
                    }

                    ui.add_space(12.0);

                    widgets::panel_card(
                        ui,
                        &pal,
                        Some((egui_phosphor::regular::SLIDERS, "Ajustes do efeito")),
                        |ui| {
                            match self.config.effect {
                                Effect::Off => {
                                    ui.label(
                                        RichText::new(
                                            "A iluminação está desligada. Escolha um efeito acima para acender o teclado.",
                                        )
                                        .font(theme::font(12.5, Weight::Regular))
                                        .color(pal.text_muted),
                                    );
                                }
                                Effect::Static => {
                                    ui.label(
                                        RichText::new(
                                            "O efeito estático usa as cores configuradas na aba Iluminação.",
                                        )
                                        .font(theme::font(12.5, Weight::Regular))
                                        .color(pal.text_muted),
                                    );
                                }
                                Effect::Cycle => {
                                    ui.label(
                                        RichText::new(
                                            "O ciclo percorre todo o espectro e ignora a cor escolhida.",
                                        )
                                        .font(theme::font(12.5, Weight::Regular))
                                        .color(pal.text_muted),
                                    );
                                }
                                _ => {
                                    widgets::section_label(ui, &pal, "Cor do efeito");
                                    ui.add_space(8.0);
                                    match swatch_row(ui, &pal, self.config.effect_color) {
                                        SwatchPick::Color(color) => {
                                            self.config.effect_color = color;
                                            changed = true;
                                        }
                                        SwatchPick::Custom => {
                                            self.open_color_editor(ColorTarget::Effect);
                                        }
                                        SwatchPick::None => {}
                                    }
                                    ui.add_space(10.0);
                                }
                            }

                            if matches!(
                                self.config.effect,
                                Effect::Breathing | Effect::Cycle | Effect::Wave
                            ) {
                                widgets::divider(ui, &pal);
                                ui.add_space(10.0);
                                let mut speed = self.config.speed;
                                if widgets::slider_row(
                                    ui,
                                    &pal,
                                    "Velocidade",
                                    &mut speed,
                                    1..=100,
                                ) {
                                    self.config.speed = speed;
                                    changed = true;
                                }
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Mais à direita, mais rápido.")
                                        .font(theme::font(11.5, Weight::Regular))
                                        .color(pal.text_muted),
                                );
                            }

                            if self.config.effect == Effect::Wave {
                                ui.add_space(10.0);
                                widgets::divider(ui, &pal);
                                ui.add_space(10.0);
                                widgets::section_label(ui, &pal, "Direção da onda");
                                ui.add_space(8.0);
                                if let Some(direction) = widgets::segmented(
                                    ui,
                                    &pal,
                                    "wave-direction",
                                    &[
                                        (WaveDirection::LeftToRight, "Esquerda → Direita"),
                                        (WaveDirection::RightToLeft, "Direita → Esquerda"),
                                    ],
                                    self.config.direction,
                                ) && direction != self.config.direction
                                {
                                    self.config.direction = direction;
                                    changed = true;
                                }
                                ui.add_space(8.0);
                                if let Some(direction) = widgets::segmented(
                                    ui,
                                    &pal,
                                    "wave-direction-radial",
                                    &[
                                        (WaveDirection::CenterToEdge, "Centro → Bordas"),
                                        (WaveDirection::EdgeToCenter, "Bordas → Centro"),
                                    ],
                                    self.config.direction,
                                ) && direction != self.config.direction
                                {
                                    self.config.direction = direction;
                                    changed = true;
                                }
                            }
                        },
                    );

                    if changed {
                        self.touch();
                    }
                });
                ui.add_space(theme::SCREEN_PAD);
            });
    }

    fn device_tab(&mut self, ui: &mut Ui, pal: Palette) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(theme::SCREEN_PAD);
                centered(ui, theme::CONTENT_SMALL, |ui| {
                    widgets::panel_card(
                        ui,
                        &pal,
                        Some((egui_phosphor::regular::KEYBOARD, "Detalhes do dispositivo")),
                        |ui| {
                            widgets::kv_row(ui, &pal, "Nome", DEVICE_NAME);
                            widgets::hairline(ui, &pal, 0.0);
                            widgets::kv_row(
                                ui,
                                &pal,
                                "Modelo",
                                self.status
                                    .name
                                    .as_deref()
                                    .unwrap_or("Logitech Gaming Keyboard G213"),
                            );
                            widgets::hairline(ui, &pal, 0.0);
                            widgets::kv_row(ui, &pal, "Conexão", "USB com fio");
                            widgets::hairline(ui, &pal, 0.0);
                            widgets::kv_row(ui, &pal, "ID USB", "046d:c336");
                            widgets::hairline(ui, &pal, 0.0);
                            widgets::kv_row(ui, &pal, "Zonas RGB", "5 áreas");
                            widgets::hairline(ui, &pal, 0.0);
                            widgets::kv_row(
                                ui,
                                &pal,
                                "Interface",
                                &self.status.path_label(),
                            );
                        },
                    );

                    ui.add_space(12.0);

                    widgets::panel_card(
                        ui,
                        &pal,
                        Some((egui_phosphor::regular::PLUG, "Acesso ao dispositivo")),
                        |ui| {
                            match self.status.state {
                                ConnectionState::Connected => {
                                    widgets::callout(
                                        ui,
                                        &pal,
                                        theme::STATUS_CONNECTED,
                                        egui_phosphor::regular::CHECK_CIRCLE,
                                        "Acesso liberado. As alterações são aplicadas na hora.",
                                        None,
                                    );
                                }
                                ConnectionState::PermissionDenied => {
                                    widgets::callout(
                                        ui,
                                        &pal,
                                        theme::STATUS_DANGER,
                                        egui_phosphor::regular::WARNING_CIRCLE,
                                        "O sistema não permite que este aplicativo abra o teclado.",
                                        None,
                                    );
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(
                                            "Execute o instalador uma vez para criar a regra udev e reconecte o teclado:",
                                        )
                                        .font(theme::font(12.5, Weight::Regular))
                                        .color(pal.text),
                                    );
                                    ui.add_space(8.0);
                                    code_block(ui, &pal, "./install.sh");
                                    ui.add_space(8.0);
                                    if widgets::button(
                                        ui,
                                        &pal,
                                        "Copiar comando",
                                        Some(egui_phosphor::regular::COPY),
                                        ButtonKind::Secondary,
                                    )
                                    .clicked()
                                    {
                                        ui.ctx().copy_text("./install.sh".to_string());
                                        self.notify("Comando copiado");
                                    }
                                }
                                ConnectionState::NotFound => {
                                    widgets::callout(
                                        ui,
                                        &pal,
                                        theme::STATUS_OFFLINE,
                                        egui_phosphor::regular::PLUG,
                                        "Teclado não encontrado. Conecte o G213 por USB.",
                                        Some("Atualizar"),
                                    );
                                }
                                ConnectionState::Searching => {
                                    widgets::callout(
                                        ui,
                                        &pal,
                                        theme::STATUS_CONNECTING,
                                        egui_phosphor::regular::SPINNER,
                                        "Procurando o teclado…",
                                        None,
                                    );
                                }
                                ConnectionState::Error => {
                                    widgets::callout(
                                        ui,
                                        &pal,
                                        theme::STATUS_DANGER,
                                        egui_phosphor::regular::WARNING,
                                        self.status
                                            .detail
                                            .as_deref()
                                            .unwrap_or("Falha ao acessar o teclado."),
                                        Some("Atualizar"),
                                    );
                                }
                            }
                        },
                    );

                    ui.add_space(12.0);

                    widgets::panel_card(
                        ui,
                        &pal,
                        Some((egui_phosphor::regular::INFO, "Sobre")),
                        |ui| {
                            widgets::kv_row(ui, &pal, "Aplicativo", "G213 RGB");
                            widgets::hairline(ui, &pal, 0.0);
                            widgets::kv_row(
                                ui,
                                &pal,
                                "Versão",
                                env!("CARGO_PKG_VERSION"),
                            );
                            widgets::hairline(ui, &pal, 0.0);
                            widgets::kv_row(ui, &pal, "Protocolo", "Logitech G213 / OpenRGB");
                            widgets::hairline(ui, &pal, 0.0);
                            widgets::kv_row(ui, &pal, "Interface", "OpenLogi");
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(format!(
                                    "Configuração salva em {}",
                                    config::config_path().display()
                                ))
                                .font(theme::font(11.5, Weight::Regular))
                                .color(pal.text_muted),
                            );
                            if let Some(error) = &self.save_error {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(format!("Falha ao salvar: {error}"))
                                        .font(theme::font(11.5, Weight::Regular))
                                        .color(theme::STATUS_DANGER),
                                );
                            }
                        },
                    );
                });
                ui.add_space(theme::SCREEN_PAD);
            });
    }

    // -----------------------------------------------------------------------------------------
    // Modals and overlays
    // -----------------------------------------------------------------------------------------

    fn settings_modal(&mut self, ctx: &egui::Context, pal: Palette) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let response = egui::Modal::new(Id::new("settings-modal"))
            .backdrop_color(Color32::from_black_alpha(110))
            .frame(modal_frame(&pal))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Ajustes")
                            .font(theme::font(18.0, Weight::SemiBold))
                            .color(pal.text),
                    );
                    ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(
                            ui,
                            &pal,
                            egui_phosphor::regular::X,
                            "Fechar",
                        )
                        .clicked()
                        {
                            open = false;
                        }
                    });
                });
                ui.add_space(14.0);
                widgets::section_label(ui, &pal, "Tema");
                ui.add_space(8.0);
                if let Some(theme) = widgets::segmented(
                    ui,
                    &pal,
                    "theme",
                    &[
                        (ThemeMode::System, "Sistema"),
                        (ThemeMode::Light, "Claro"),
                        (ThemeMode::Dark, "Escuro"),
                    ],
                    self.config.theme,
                ) && theme != self.config.theme
                {
                    self.config.theme = theme;
                    self.touch();
                }
                ui.add_space(16.0);
                widgets::divider(ui, &pal);
                ui.add_space(14.0);
                widgets::kv_row(ui, &pal, "Aplicativo", "G213 RGB");
                widgets::hairline(ui, &pal, 0.0);
                widgets::kv_row(ui, &pal, "Versão", env!("CARGO_PKG_VERSION"));
                widgets::hairline(ui, &pal, 0.0);
                widgets::kv_row(ui, &pal, "Tema da interface", "OpenLogi");
                ui.add_space(12.0);
                ui.label(
                    RichText::new(
                        "As alterações são aplicadas ao teclado e salvas automaticamente.",
                    )
                    .font(theme::font(11.5, Weight::Regular))
                    .color(pal.text_muted),
                );
                ui.add_space(16.0);
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, &pal, "Fechar", None, ButtonKind::Primary).clicked() {
                        open = false;
                    }
                });
            });
        if response.should_close() {
            open = false;
        }
        self.settings_open = open;
    }

    fn color_modal(&mut self, ctx: &egui::Context, pal: Palette) {
        let Some(mut editor) = self.color_editor.take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;

        let response = egui::Modal::new(Id::new("color-modal"))
            .backdrop_color(Color32::from_black_alpha(110))
            .frame(modal_frame(&pal))
            .show(ctx, |ui| {
                ui.set_width(400.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Cor personalizada")
                            .font(theme::font(18.0, Weight::SemiBold))
                            .color(pal.text),
                    );
                    ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(
                            ui,
                            &pal,
                            egui_phosphor::regular::X,
                            "Fechar",
                        )
                        .clicked()
                        {
                            open = false;
                        }
                    });
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                    ui.painter().rect(
                        rect,
                        widgets::CONTROL,
                        theme::color32(editor.color),
                        Stroke::new(1.0, pal.border),
                        StrokeKind::Inside,
                    );
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        widgets::section_label(ui, &pal, "Hexadecimal");
                        ui.add_space(4.0);
                        let response = ui.add(
                            TextEdit::singleline(&mut editor.hex)
                                .desired_width(120.0)
                                .char_limit(7)
                                .font(theme::font(14.0, Weight::Medium))
                                .text_color(pal.text),
                        );
                        if response.changed()
                            && let Some(color) = Rgb::from_hex(&editor.hex)
                        {
                            editor.color = color;
                        }
                    });
                });
                ui.add_space(14.0);

                let mut channels = [editor.color.r, editor.color.g, editor.color.b];
                let labels = ["Vermelho", "Verde", "Azul"];
                let mut changed = false;
                for (channel, label) in channels.iter_mut().zip(labels) {
                    if widgets::slider_row(ui, &pal, label, channel, 0..=255) {
                        changed = true;
                    }
                    ui.add_space(8.0);
                }
                if changed {
                    editor.color = Rgb::new(channels[0], channels[1], channels[2]);
                    editor.hex = editor.color.hex();
                }

                ui.add_space(10.0);
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, &pal, "Aplicar", None, ButtonKind::Primary).clicked() {
                        apply = true;
                        open = false;
                    }
                    if widgets::button(ui, &pal, "Cancelar", None, ButtonKind::Secondary)
                        .clicked()
                    {
                        open = false;
                    }
                });
            });
        if response.should_close() {
            open = false;
        }

        if apply {
            let target = editor.target;
            self.set_target_color(target, editor.color);
            self.notify("Cor aplicada");
        }
        if open {
            self.color_editor = Some(editor);
        }
    }

    fn toast_ui(&mut self, ctx: &egui::Context, pal: Palette) {
        let Some(toast) = &self.toast else {
            return;
        };
        if Instant::now() >= toast.until {
            self.toast = None;
            return;
        }
        let text = toast.text.clone();
        egui::Area::new(Id::new("toast"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -24.0))
            .show(ctx, |ui| {
                Frame::new()
                    .fill(pal.panel)
                    .stroke(Stroke::new(1.0, pal.border))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(text)
                                .font(theme::font(12.5, Weight::Medium))
                                .color(pal.text),
                        );
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(120));
    }
}

impl eframe::App for G213App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.last_status_check.elapsed() >= Duration::from_millis(400) {
            self.last_status_check = Instant::now();
            self.status = self.worker.status();
        }
        if self.preferred != Some(self.config.theme) {
            theme::set_preference(&ctx, self.config.theme);
            self.preferred = Some(self.config.theme);
        }
        self.save_if_due();

        let dark = theme::resolve_dark(&ctx, self.config.theme);
        let pal = Palette::palette(dark);

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, pal.page);

        let mut shell = ui.new_child(
            UiBuilder::new()
                .max_rect(full)
                .layout(ELayout::top_down(Align::Min)),
        );
        self.header_ui(&mut shell, pal);

        let body = Rect::from_min_max(
            pos2(full.min.x, full.min.y + theme::HEADER_H),
            full.max,
        );
        let mut body_ui = shell.new_child(
            UiBuilder::new()
                .max_rect(body)
                .layout(ELayout::top_down(Align::Min)),
        );
        match self.view {
            View::Home => self.home_ui(&mut body_ui, pal),
            View::Detail => self.detail_ui(&mut body_ui, pal),
        }

        self.settings_modal(&ctx, pal);
        self.color_modal(&ctx, pal);
        self.toast_ui(&ctx, pal);

        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let dark = match self.config.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => true,
        };
        Palette::palette(dark).page.to_normalized_gamma_f32()
    }
}

impl Drop for G213App {
    fn drop(&mut self) {
        // The debounce may not have fired when the window is closed.
        let _ = self.config.save();
    }
}

// ---------------------------------------------------------------------------------------------
// Small painting helpers
// ---------------------------------------------------------------------------------------------

/// A centred, fixed-width content column, matching OpenLogi's tab bodies.
fn centered(ui: &mut Ui, width: f32, add_contents: impl FnOnce(&mut Ui)) {
    let avail = ui.available_width();
    let width = width.min((avail - theme::SCREEN_PAD * 2.0).max(280.0));
    let pad = ((avail - width) * 0.5).max(theme::SCREEN_PAD);
    ui.horizontal(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.set_width(width);
            add_contents(ui);
        });
    });
}

fn effect_icon(effect: Effect) -> &'static str {
    match effect {
        Effect::Off => egui_phosphor::regular::POWER,
        Effect::Static => egui_phosphor::regular::LIGHTBULB,
        Effect::Breathing => egui_phosphor::regular::DROP,
        Effect::Cycle => egui_phosphor::regular::ARROW_CLOCKWISE,
        Effect::Wave => egui_phosphor::regular::WAVES,
    }
}

fn modal_frame(pal: &Palette) -> Frame {
    Frame::new()
        .fill(pal.panel)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(widgets::CARD)
        .inner_margin(Margin::same(22))
}

fn swatch_row(ui: &mut Ui, pal: &Palette, current: Rgb) -> SwatchPick {
    let mut pick = SwatchPick::None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for color in SWATCHES {
            if widgets::swatch(ui, pal, color, current == color).clicked() {
                pick = SwatchPick::Color(color);
            }
        }
        ui.add_space(4.0);
        if widgets::custom_swatch(ui, pal).clicked() {
            pick = SwatchPick::Custom;
        }
    });
    pick
}

fn paint_text(
    ui: &Ui,
    pos: Pos2,
    text: &str,
    size: f32,
    weight: Weight,
    color: Color32,
) -> Rect {
    ui.painter().text(
        pos,
        Align2::LEFT_TOP,
        text,
        theme::font(size, weight),
        color,
    )
}

fn pill_width(ui: &Ui, label: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        theme::font(11.5, Weight::Medium),
        Color32::WHITE,
    );
    22.0 + galley.size().x + 12.0
}

fn paint_pill(ui: &Ui, pal: &Palette, top_left: Pos2, label: &str, color: Color32) -> Rect {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::font(11.5, Weight::Medium), color);
    let rect = Rect::from_min_size(
        top_left,
        vec2(22.0 + galley.size().x + 12.0, 24.0),
    );
    ui.painter().rect(
        rect,
        CornerRadius::same(12),
        theme::blend(color, pal.panel, 0.14),
        Stroke::new(1.0, theme::blend(color, pal.panel, 0.32)),
        StrokeKind::Inside,
    );
    ui.painter()
        .circle_filled(pos2(rect.left() + 11.0, rect.center().y), 3.5, color);
    ui.painter().galley(
        pos2(rect.left() + 20.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    rect
}

fn paint_badge(ui: &Ui, pal: &Palette, top_left: Pos2, label: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        theme::font(11.0, Weight::Medium),
        pal.text_muted,
    );
    let width = galley.size().x + 16.0;
    let rect = Rect::from_min_size(top_left, vec2(width, 21.0));
    ui.painter().rect(
        rect,
        CornerRadius::same(11),
        pal.muted,
        Stroke::new(1.0, pal.border),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        pos2(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        pal.text_muted,
    );
    width
}

fn code_block(ui: &mut Ui, pal: &Palette, command: &str) {
    Frame::new()
        .fill(pal.input)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(command)
                    .font(FontId::monospace(13.0))
                    .color(pal.text),
            );
        });
}
