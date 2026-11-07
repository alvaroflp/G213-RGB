//! The application shell and screens.
//!
//! OpenLogi-inspired gallery, top-level device tabs and local lighting profiles.
//! Product photography identifies the keyboard; a separate diagram previews RGB.

use crate::config::{
    self, Config, Effect, Layout, Lighting, Rgb, ThemeMode, WaveDirection, ZONE_COUNT, ZONE_NAMES,
};
use crate::device::{ApplyState, ConnectionState, DeviceStatus, DeviceWorker};
use crate::keyboard::{self, KeyboardState};
use crate::theme::{self, Palette, Weight};
use crate::widgets::{self, ButtonKind};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Layout as ELayout, Margin, Rect,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, pos2, vec2,
};
use std::time::{Duration, Instant};

mod profiles;

const DEVICE_NAME: &str = "Logitech G213 Prodigy";

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
    Profiles,
    Device,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Lighting, Self::Effects, Self::Profiles, Self::Device];

    fn label(self) -> &'static str {
        match self {
            Self::Lighting => "Iluminação",
            Self::Effects => "Efeitos",
            Self::Profiles => "Perfis",
            Self::Device => "Dispositivo",
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
    worker: Option<DeviceWorker>,
    preview: bool,
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
    product: crate::product::ProductImage,
    last_lighting: Lighting,
    undo: Option<Lighting>,
    profile_name: String,
    profile_error: Option<String>,
    deleted_profile: Option<config::Profile>,
}

impl G213App {
    pub fn new(cc: &eframe::CreationContext<'_>, preview: bool) -> Self {
        Self::with_context(&cc.egui_ctx, preview)
    }

    fn with_context(ctx: &egui::Context, preview: bool) -> Self {
        theme::install_fonts(ctx);
        theme::apply_style(ctx);

        let config = if preview {
            Config {
                theme: ThemeMode::Light,
                ..Config::default()
            }
        } else {
            Config::load()
        };
        theme::set_preference(ctx, config.theme);

        let worker = (!preview).then(DeviceWorker::start);
        if let Some(worker) = &worker {
            worker.apply(ApplyState::from(&config));
        }
        let preferred = Some(config.theme);

        let product = crate::product::ProductImage::load(ctx);
        let last_lighting = config.lighting.clone();
        Self {
            preview,
            product,
            last_lighting,
            undo: None,
            profile_name: String::new(),
            profile_error: None,
            deleted_profile: None,
            config,
            worker,
            status: if preview {
                DeviceStatus {
                    state: ConnectionState::NotFound,
                    path: None,
                    name: None,
                    detail: None,
                }
            } else {
                DeviceStatus::searching()
            },
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
        if self.last_lighting != self.config.lighting {
            self.undo = Some(std::mem::replace(
                &mut self.last_lighting,
                self.config.lighting.clone(),
            ));
            if let Some(worker) = &self.worker {
                worker.apply(ApplyState::from(&self.config));
            }
        }
    }

    fn save_if_due(&mut self) {
        if !self.dirty || self.preview {
            return;
        }
        let due = self
            .last_edit
            .is_none_or(|edit| edit.elapsed() >= Duration::from_millis(400));
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

    fn keyboard_state(&self, selected: Option<usize>, seconds: f64) -> KeyboardState {
        KeyboardState {
            zone_colors: self
                .config
                .lighting
                .preview_colors(seconds)
                .map(theme::color32),
            enabled: self.config.lighting.enabled && self.config.lighting.effect != Effect::Off,
            selected_zone: selected,
        }
    }

    fn target_color(&self, target: ColorTarget) -> Rgb {
        match target {
            ColorTarget::Uniform => self.config.lighting.color,
            ColorTarget::Effect => self.config.lighting.effect_color,
            ColorTarget::Zone(zone) => self.config.lighting.zone_color(zone),
        }
    }

    fn set_target_color(&mut self, target: ColorTarget, color: Rgb) {
        match target {
            ColorTarget::Uniform => self.config.lighting.color = color,
            ColorTarget::Effect => self.config.lighting.effect_color = color,
            ColorTarget::Zone(zone) => self.config.lighting.set_zone_color(zone, color),
        }
        if target != ColorTarget::Effect {
            self.config.lighting.effect = Effect::Static;
            self.config.lighting.enabled = true;
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
                .max_rect(header.shrink2(vec2(24.0, 0.0)))
                .layout(ELayout::left_to_right(Align::Center)),
        );
        if self.view == View::Home {
            hud.label(
                RichText::new(egui_phosphor::regular::KEYBOARD)
                    .font(theme::icon(25.0))
                    .color(theme::ACCENT),
            );
            hud.add_space(4.0);
            hud.label(RichText::new("G213 RGB").font(theme::font(19.0, Weight::SemiBold)));
        } else {
            if widgets::icon_button(
                &mut hud,
                &pal,
                egui_phosphor::regular::ARROW_LEFT,
                "Dispositivos (Esc)",
            )
            .clicked()
            {
                self.view = View::Home;
            }
            hud.label(RichText::new("G213 Prodigy").font(theme::font(18.0, Weight::SemiBold)));
            hud.add_space(if full.width() > 1050.0 { 40.0 } else { 12.0 });
            for tab in Tab::ALL {
                let kind = if self.tab == tab {
                    ButtonKind::Secondary
                } else {
                    ButtonKind::Ghost
                };
                if widgets::button(&mut hud, &pal, tab.label(), None, kind).clicked() {
                    self.tab = tab;
                }
            }
        }
        hud.with_layout(ELayout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(
                ui,
                &pal,
                egui_phosphor::regular::GEAR,
                "Ajustes de aparência",
            )
            .clicked()
            {
                self.settings_open = true;
            }
            if self.view == View::Home {
                if widgets::icon_button(
                    ui,
                    &pal,
                    egui_phosphor::regular::ARROW_CLOCKWISE,
                    "Buscar dispositivos",
                )
                .clicked()
                {
                    self.refresh();
                }
            } else {
                widgets::status_pill(ui, &pal, self.status_label(), self.status_color());
            }
        });
        ui.painter().hline(
            header.x_range(),
            header.bottom() - 1.0,
            Stroke::new(1.0, pal.border),
        );
    }

    fn refresh(&mut self) {
        if let Some(worker) = &self.worker {
            worker.refresh();
            self.status = DeviceStatus::searching();
        } else {
            self.notify("Prévia: nenhum dispositivo será acessado");
            return;
        }
        self.notify("Procurando o G213…");
    }

    fn home_ui(&mut self, ui: &mut Ui, pal: Palette) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(22.0);
                centered(ui, 820.0, |ui| {
                    ui.label(
                        RichText::new("Seus dispositivos")
                            .font(theme::font(28.0, Weight::SemiBold)),
                    );
                    ui.label(
                        RichText::new("Seu teclado. Suas cores. Tudo neste computador.")
                            .size(14.0)
                            .color(pal.text_muted),
                    );
                    ui.add_space(12.0);
                    self.connection_callout(ui, &pal);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - 430.0) * 0.5).max(0.0));
                        ui.vertical(|ui| {
                            ui.set_width(430.0);
                            let card = widgets::panel_card(ui, &pal, None, |ui| {
                                ui.horizontal(|ui| {
                                    widgets::badge(ui, &pal, "TECLADO");
                                    ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                                        widgets::status_pill(
                                            ui,
                                            &pal,
                                            self.status_label(),
                                            self.status_color(),
                                        );
                                    });
                                });
                                ui.add_space(16.0);
                                self.product.show(ui, ui.available_width());
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new("G213 Prodigy")
                                        .font(theme::font(24.0, Weight::SemiBold)),
                                );
                                ui.label(
                                    RichText::new("Logitech · USB · 5 zonas RGB")
                                        .size(13.0)
                                        .color(pal.text_muted),
                                );
                                ui.add_space(14.0);
                                widgets::divider(ui, &pal);
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    let summary = if self.config.lighting.enabled {
                                        self.config.lighting.effect.label()
                                    } else {
                                        "Desligado"
                                    };
                                    ui.label(
                                        RichText::new(format!("Iluminação · {summary}"))
                                            .size(12.0)
                                            .color(pal.text_muted),
                                    );
                                    ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                                        if widgets::button(
                                            ui,
                                            &pal,
                                            "Personalizar",
                                            Some(egui_phosphor::regular::ARROW_RIGHT),
                                            ButtonKind::Primary,
                                        )
                                        .clicked()
                                        {
                                            self.view = View::Detail;
                                        }
                                    });
                                });
                            });
                            if ui
                                .interact(
                                    card.response.rect,
                                    ui.id().with("device-card"),
                                    Sense::click(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.view = View::Detail;
                            }
                        });
                    });
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Compatível com Logitech G213 Prodigy")
                                .size(12.0)
                                .color(pal.text_muted),
                        );
                        ui.label(
                            RichText::new("Configuração local · Sem conta · Sem telemetria")
                                .size(11.0)
                                .color(pal.text_muted),
                        );
                    });
                });
                ui.add_space(16.0);
            });
    }

    /// Renders a status notice. Returns true when the user clicked its action.
    fn connection_callout(&mut self, ui: &mut Ui, pal: &Palette) -> bool {
        if self.preview {
            widgets::callout(
                ui,
                pal,
                theme::ACCENT,
                egui_phosphor::regular::EYE,
                "Modo de prévia. Experimente sem alterar o teclado ou suas configurações.",
                None,
            );
            return false;
        }
        let (color, icon, text, action) = match self.status.state {
            ConnectionState::Connected => return false,
            ConnectionState::PermissionDenied => (
                theme::STATUS_DANGER,
                egui_phosphor::regular::WARNING_CIRCLE,
                "Libere o acesso ao G213 com o instalador e reconecte o cabo USB.",
                Some("Copiar comando"),
            ),
            ConnectionState::NotFound => (
                theme::STATUS_OFFLINE,
                egui_phosphor::regular::PLUG,
                "Conecte seu G213 por USB. Você pode preparar a iluminação enquanto isso.",
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
                "Não foi possível acessar o G213. Verifique a conexão USB.",
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
                    self.refresh();
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
        ScrollArea::vertical()
            .id_salt(("detail", self.tab as u8))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(24.0);
                centered(ui, 1120.0, |ui| {
                    if self.status.state != ConnectionState::Connected {
                        self.connection_callout(ui, &pal);
                        ui.add_space(16.0);
                    }
                    if self.tab == Tab::Device {
                        self.device_tab(ui, pal);
                        return;
                    }
                    if self.tab == Tab::Profiles {
                        self.profiles_tab(ui, pal);
                        return;
                    }
                    let width = ui.available_width();
                    if width >= 820.0 {
                        let right = if width >= 1000.0 { 416.0 } else { 360.0 };
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(width - right - 28.0);
                                self.product_panel(ui, pal);
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                ui.set_width(right);
                                if self.tab == Tab::Lighting {
                                    self.lighting_tab(ui, pal);
                                } else {
                                    self.effects_tab(ui, pal);
                                }
                            });
                        });
                    } else {
                        self.product_panel(ui, pal);
                        ui.add_space(20.0);
                        if self.tab == Tab::Lighting {
                            self.lighting_tab(ui, pal);
                        } else {
                            self.effects_tab(ui, pal);
                        }
                    }
                });
                ui.add_space(24.0);
            });
    }

    fn product_panel(&mut self, ui: &mut Ui, pal: Palette) {
        ui.label(
            RichText::new("FEITO PARA O SEU G213")
                .font(theme::font(11.0, Weight::Medium))
                .color(pal.text_muted),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(if self.tab == Tab::Lighting {
                "Dê cor ao seu espaço."
            } else {
                "Luz em movimento."
            })
            .font(theme::font(26.0, Weight::SemiBold)),
        );
        ui.add_space(22.0);
        let width = ui.available_width().min(440.0);
        ui.vertical_centered(|ui| {
            self.product.show(ui, width);
        });
        ui.add_space(18.0);
        widgets::panel_card(ui, &pal, None, |ui| {
            ui.horizontal(|ui| {
                widgets::section_label(ui, &pal, "Prévia da iluminação");
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    widgets::badge(ui, &pal, "SIMULAÇÃO");
                });
            });
            ui.add_space(12.0);
            let interactive =
                self.tab == Tab::Lighting && self.config.lighting.layout == Layout::Zones;
            let state = self.keyboard_state(
                if interactive {
                    Some(self.selected_zone)
                } else {
                    None
                },
                ui.input(|i| i.time),
            );
            let clicked = ui
                .vertical_centered(|ui| {
                    keyboard::keyboard(
                        ui,
                        &pal,
                        ui.available_width().min(380.0),
                        &state,
                        interactive,
                    )
                    .1
                })
                .inner;
            if let Some(zone) = clicked {
                self.selected_zone = zone;
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(if interactive {
                    "Clique em uma das cinco áreas para escolher a cor."
                } else {
                    "A prévia é aproximada; a foto acima identifica o produto."
                })
                .size(11.5)
                .color(pal.text_muted),
            );
        });
    }

    fn lighting_tab(&mut self, ui: &mut Ui, pal: Palette) {
        widgets::panel_card(ui, &pal, None, |ui| {
            let on = self.config.lighting.enabled && self.config.lighting.effect != Effect::Off;
            let enabled =
                widgets::toggle_row(ui, &pal, "Iluminação", "Acenda o teclado do seu jeito.", on);
            if enabled != on {
                self.config.lighting.enabled = enabled;
                if enabled && self.config.lighting.effect == Effect::Off {
                    self.config.lighting.effect = Effect::Static;
                }
                self.touch();
            }
            ui.add_space(12.0);
            widgets::divider(ui, &pal);
            ui.add_space(12.0);
            if self.config.lighting.effect != Effect::Static {
                ui.label(
                    RichText::new("Ao editar uma cor, o efeito muda para Estático.")
                        .size(12.0)
                        .color(pal.text_muted),
                );
                ui.add_space(6.0);
            }
            if let Some(layout) = widgets::segmented(
                ui,
                &pal,
                "layout",
                &[(Layout::Uniform, "Uniforme"), (Layout::Zones, "Por zona")],
                self.config.lighting.layout,
            ) && layout != self.config.lighting.layout
            {
                self.config.lighting.layout = layout;
                self.config.lighting.effect = Effect::Static;
                self.config.lighting.enabled = true;
                self.touch();
            }
            ui.add_space(12.0);
            let target = if self.config.lighting.layout == Layout::Zones {
                ui.horizontal(|ui| {
                    for (index, zone_name) in ZONE_NAMES.iter().enumerate() {
                        let color = self.config.lighting.zone_color(index);
                        let label = format!("{}", index + 1);
                        let response = ui.add(
                            egui::Button::new(RichText::new(label).color(
                                if color.luminance() > 0.4 {
                                    Color32::BLACK
                                } else {
                                    Color32::WHITE
                                },
                            ))
                            .fill(theme::color32(color))
                            .min_size(vec2(52.0, 32.0))
                            .stroke(Stroke::new(
                                if index == self.selected_zone {
                                    2.0
                                } else {
                                    0.0
                                },
                                theme::ACCENT,
                            )),
                        );
                        if response.on_hover_text(*zone_name).clicked() {
                            self.selected_zone = index;
                        }
                    }
                });
                ui.add_space(8.0);
                widgets::section_label(
                    ui,
                    &pal,
                    &format!(
                        "Zona {} · {}",
                        self.selected_zone + 1,
                        ZONE_NAMES[self.selected_zone]
                    ),
                );
                ColorTarget::Zone(self.selected_zone)
            } else {
                widgets::section_label(ui, &pal, "Cor do teclado");
                ColorTarget::Uniform
            };
            ui.add_space(8.0);
            self.color_controls(ui, pal, target);
            if self.config.lighting.layout == Layout::Zones {
                ui.add_space(4.0);
                if widgets::button(
                    ui,
                    &pal,
                    "Copiar cor para todas as zonas",
                    Some(egui_phosphor::regular::COPY),
                    ButtonKind::Ghost,
                )
                .clicked()
                {
                    self.config.lighting.zones = [self.target_color(target); ZONE_COUNT];
                    self.config.lighting.effect = Effect::Static;
                    self.config.lighting.enabled = true;
                    self.touch();
                }
            }
            ui.add_space(12.0);
            widgets::divider(ui, &pal);
            ui.add_space(12.0);
            self.brightness_ui(ui, pal);
        });
        ui.add_space(12.0);
        self.quick_profiles(ui, pal);
        self.history_actions(ui, pal);
    }

    fn color_controls(&mut self, ui: &mut Ui, pal: Palette, target: ColorTarget) {
        let current = self.target_color(target);
        match swatch_row(ui, &pal, current) {
            SwatchPick::Color(color) => self.set_target_color(target, color),
            SwatchPick::Custom => self.open_color_editor(target),
            SwatchPick::None => {}
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(current.hex())
                    .font(theme::font(12.0, Weight::Medium))
                    .color(pal.text_muted),
            );
            ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, &pal, "Personalizar cor", None, ButtonKind::Ghost).clicked()
                {
                    self.open_color_editor(target);
                }
            });
        });
    }

    fn brightness_ui(&mut self, ui: &mut Ui, pal: Palette) {
        if matches!(self.config.lighting.effect, Effect::Cycle | Effect::Wave) {
            ui.label(
                RichText::new("O brilho e as cores deste efeito são definidos pelo teclado.")
                    .size(12.0)
                    .color(pal.text_muted),
            );
        } else if self.config.lighting.effect != Effect::Off
            && widgets::slider_row(
                ui,
                &pal,
                "Brilho",
                &mut self.config.lighting.brightness,
                1..=100,
            )
        {
            self.touch();
        }
    }

    fn effects_tab(&mut self, ui: &mut Ui, pal: Palette) {
        widgets::panel_card(
            ui,
            &pal,
            Some((egui_phosphor::regular::SPARKLE, "Efeitos do teclado")),
            |ui| {
                for effect in Effect::ALL {
                    let selected = self.config.lighting.effect == effect;
                    let response = ui.add(
                        egui::Button::new(RichText::new(effect.label()).size(14.0))
                            .fill(if selected {
                                pal.accent_tint()
                            } else {
                                pal.input
                            })
                            .stroke(Stroke::new(
                                1.0,
                                if selected { theme::ACCENT } else { pal.border },
                            ))
                            .min_size(vec2(ui.available_width(), 38.0)),
                    );
                    if response.on_hover_text(effect.description()).clicked() {
                        self.config.lighting.effect = effect;
                        self.config.lighting.enabled = effect != Effect::Off;
                        self.touch();
                    }
                }
                ui.add_space(10.0);
                let effect = self.config.lighting.effect;
                ui.label(
                    RichText::new(effect.description())
                        .size(12.0)
                        .color(pal.text_muted),
                );
                if effect == Effect::Breathing {
                    ui.add_space(10.0);
                    self.color_controls(ui, pal, ColorTarget::Effect);
                }
                if matches!(effect, Effect::Breathing | Effect::Cycle | Effect::Wave) {
                    ui.add_space(10.0);
                    if widgets::slider_row(
                        ui,
                        &pal,
                        "Velocidade",
                        &mut self.config.lighting.speed,
                        1..=100,
                    ) {
                        self.touch();
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Suave").size(11.0).color(pal.text_muted));
                        ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new("Rápida").size(11.0).color(pal.text_muted));
                        });
                    });
                }
                if effect == Effect::Wave {
                    ui.add_space(8.0);
                    egui::ComboBox::from_id_salt("wave-direction")
                        .selected_text(self.config.lighting.direction.label())
                        .width(ui.available_width() - 8.0)
                        .show_ui(ui, |ui| {
                            for direction in WaveDirection::ALL {
                                if ui
                                    .selectable_value(
                                        &mut self.config.lighting.direction,
                                        direction,
                                        direction.label(),
                                    )
                                    .changed()
                                {
                                    self.touch();
                                }
                            }
                        });
                }
                ui.add_space(12.0);
                self.brightness_ui(ui, pal);
                if effect == Effect::Static
                    && widgets::button(
                        ui,
                        &pal,
                        "Editar cores e zonas",
                        None,
                        ButtonKind::Secondary,
                    )
                    .clicked()
                {
                    self.tab = Tab::Lighting;
                }
            },
        );
        self.history_actions(ui, pal);
    }

    fn quick_profiles(&mut self, ui: &mut Ui, pal: Palette) {
        widgets::panel_card(
            ui,
            &pal,
            Some((egui_phosphor::regular::STACK, "Uma ideia para começar")),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    for profile in config::builtin_profiles() {
                        if widgets::button(ui, &pal, &profile.name, None, ButtonKind::Secondary)
                            .clicked()
                        {
                            self.config.lighting = profile.lighting;
                            self.touch();
                            self.notify(format!("Perfil {} selecionado", profile.name));
                        }
                    }
                });
                ui.add_space(4.0);
                if widgets::button(
                    ui,
                    &pal,
                    "Salvar ou gerenciar perfis",
                    Some(egui_phosphor::regular::ARROW_RIGHT),
                    ButtonKind::Ghost,
                )
                .clicked()
                {
                    self.tab = Tab::Profiles;
                }
            },
        );
    }

    fn history_actions(&mut self, ui: &mut Ui, pal: Palette) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.undo.is_some(), |ui| {
                if widgets::button(
                    ui,
                    &pal,
                    "Desfazer",
                    Some(egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE),
                    ButtonKind::Ghost,
                )
                .clicked()
                {
                    self.undo_lighting();
                }
            });
            ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, &pal, "Restaurar padrão", None, ButtonKind::Ghost).clicked()
                {
                    self.config.lighting = Lighting::default();
                    self.touch();
                    self.notify("Iluminação padrão restaurada. Você pode desfazer.");
                }
            });
        });
    }

    fn undo_lighting(&mut self) {
        if let Some(previous) = self.undo.take() {
            self.config.lighting = previous;
            self.touch();
            self.notify("Alteração desfeita");
        }
    }

    fn device_tab(&mut self, ui: &mut Ui, pal: Palette) {
        ui.scope(|ui| {
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
                                    if widgets::callout(
                                        ui,
                                        &pal,
                                        theme::STATUS_OFFLINE,
                                        egui_phosphor::regular::PLUG,
                                        "Teclado não encontrado. Conecte o G213 por USB.",
                                        Some("Atualizar"),
                                    ) { self.refresh(); }
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
                                    if widgets::callout(
                                        ui,
                                        &pal,
                                        theme::STATUS_DANGER,
                                        egui_phosphor::regular::WARNING,
                                        self.status
                                            .detail
                                            .as_deref()
                                            .unwrap_or("Falha ao acessar o teclado."),
                                        Some("Atualizar"),
                                    ) { self.refresh(); }
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
                            widgets::kv_row(ui, &pal, "Referência visual", "OpenLogi");
                            ui.hyperlink_to("Conhecer o OpenLogi", "https://github.com/AprilNEA/OpenLogi");
                            ui.label(RichText::new("Projeto independente, sem afiliação com Logitech ou OpenLogi.").size(11.5).color(pal.text_muted));
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
                        if widgets::icon_button(ui, &pal, egui_phosphor::regular::X, "Fechar")
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
                widgets::kv_row(ui, &pal, "Aparência", self.config.theme.label());
                ui.add_space(12.0);
                ui.label(
                    RichText::new(if self.preview {
                        "Neste modo, as alterações são temporárias e não acessam o teclado."
                    } else {
                        "As alterações são salvas neste computador e enviadas ao G213 conectado."
                    })
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
                        if widgets::icon_button(ui, &pal, egui_phosphor::regular::X, "Fechar")
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
                let valid = Rgb::from_hex(&editor.hex).is_some();
                if !valid {
                    ui.colored_label(
                        theme::STATUS_DANGER,
                        "Use seis dígitos hexadecimais: #RRGGBB.",
                    );
                }
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(valid, |ui| {
                        if widgets::button(ui, &pal, "Aplicar", None, ButtonKind::Primary).clicked()
                        {
                            apply = true;
                            open = false;
                        }
                    });
                    if widgets::button(ui, &pal, "Cancelar", None, ButtonKind::Secondary).clicked()
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
            self.notify("Cor selecionada");
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

impl G213App {
    fn render(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();

        if self.last_status_check.elapsed() >= Duration::from_millis(400) {
            self.last_status_check = Instant::now();
            if let Some(worker) = &self.worker {
                self.status = worker.status();
            }
        }
        if self.preferred != Some(self.config.theme) {
            theme::set_preference(&ctx, self.config.theme);
            self.preferred = Some(self.config.theme);
        }
        self.save_if_due();
        if !self.settings_open && self.color_editor.is_none() && !ctx.text_edit_focused() {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.view = View::Home;
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.undo_lighting();
            }
        }

        let dark = theme::resolve_dark(&ctx, self.config.theme);
        let pal = Palette::for_mode(dark);

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
            pos2(full.max.x, full.max.y - 36.0),
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

        let footer = Rect::from_min_max(pos2(full.left(), full.bottom() - 36.0), full.max);
        ui.painter().rect_filled(footer, 0.0, pal.page);
        ui.painter()
            .hline(footer.x_range(), footer.top(), Stroke::new(1.0, pal.border));
        let mut footer_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(footer.shrink2(vec2(24.0, 0.0)))
                .layout(ELayout::left_to_right(Align::Center)),
        );
        let (message, color) = if self.preview {
            ("Prévia · alterações temporárias", theme::ACCENT)
        } else if self.save_error.is_some() {
            ("Falha ao salvar · veja Dispositivo", theme::STATUS_DANGER)
        } else if self.dirty {
            ("Salvando alterações…", pal.text_muted)
        } else {
            ("Configuração local · salvamento automático", pal.text_muted)
        };
        footer_ui.label(RichText::new(message).size(11.0).color(color));
        footer_ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .size(11.0)
                    .color(pal.text_muted),
            );
            if !self.preview && !self.status.is_connected() {
                ui.label(
                    RichText::new("Será aplicada ao conectar")
                        .size(11.0)
                        .color(pal.text_muted),
                );
            }
        });
        self.settings_modal(&ctx, pal);
        self.color_modal(&ctx, pal);
        self.toast_ui(&ctx, pal);

        let animated = self.view == View::Detail
            && matches!(self.tab, Tab::Lighting | Tab::Effects)
            && self.config.lighting.enabled
            && matches!(
                self.config.lighting.effect,
                Effect::Breathing | Effect::Cycle | Effect::Wave
            );
        ctx.request_repaint_after(Duration::from_millis(if animated { 33 } else { 500 }));
    }
}

impl eframe::App for G213App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let dark = match self.config.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => true,
        };
        Palette::for_mode(dark).page.to_normalized_gamma_f32()
    }
}

impl Drop for G213App {
    fn drop(&mut self) {
        if self.dirty && !self.preview {
            let _ = self.config.save();
        }
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

fn modal_frame(pal: &Palette) -> Frame {
    Frame::new()
        .fill(pal.panel)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(widgets::CARD)
        .inner_margin(Margin::same(22))
}

fn swatch_row(ui: &mut Ui, pal: &Palette, current: Rgb) -> SwatchPick {
    let mut pick = SwatchPick::None;
    ui.horizontal_wrapped(|ui| {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut G213App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1120.0, 760.0))),
                events,
                ..Default::default()
            },
            |ui| app.render(ui),
        );
        output.textures_delta.clear();
        output
    }

    fn click_text(app: &mut G213App, ctx: &egui::Context, label: &str) {
        frame(app, ctx, vec![]);
        let output = frame(app, ctx, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == label
                {
                    Some(text.pos + text.galley.size() * 0.5)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("Visible control not found: {label}"));
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }

    #[test]
    fn gallery_zone_edit_undo_and_profile_save_flow() {
        let ctx = egui::Context::default();
        let mut app = G213App::with_context(&ctx, true);
        click_text(&mut app, &ctx, "Personalizar");
        assert_eq!(app.view, View::Detail);
        click_text(&mut app, &ctx, "Por zona");
        assert_eq!(app.config.lighting.layout, Layout::Zones);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            }],
        );
        assert_eq!(app.config.lighting.layout, Layout::Uniform);
        click_text(&mut app, &ctx, "Perfis");
        click_text(&mut app, &ctx, "Nome do perfil");
        frame(&mut app, &ctx, vec![egui::Event::Text("Trabalho".into())]);
        click_text(&mut app, &ctx, "Salvar perfil");
        assert_eq!(app.config.profiles.len(), 1);
        assert_eq!(app.config.profiles[0].name, "Trabalho");
        assert!(app.worker.is_none());
        app.save_if_due();
        assert!(app.dirty, "preview must never persist changes");
    }

    #[test]
    fn invalid_hex_cannot_be_applied_and_static_edits_replace_native_effect() {
        let ctx = egui::Context::default();
        let mut app = G213App::with_context(&ctx, true);
        app.view = View::Detail;
        app.config.lighting.effect = Effect::Cycle;
        app.open_color_editor(ColorTarget::Uniform);
        app.color_editor.as_mut().unwrap().hex = "aé123".into();
        click_text(&mut app, &ctx, "Aplicar");
        assert!(app.color_editor.is_some());
        assert_eq!(app.config.lighting.effect, Effect::Cycle);
        app.color_editor.as_mut().unwrap().hex = "#123456".into();
        app.color_editor.as_mut().unwrap().color = Rgb::new(0x12, 0x34, 0x56);
        click_text(&mut app, &ctx, "Aplicar");
        assert!(app.color_editor.is_none());
        assert_eq!(app.config.lighting.effect, Effect::Static);
        assert_eq!(app.config.lighting.color, Rgb::new(0x12, 0x34, 0x56));
    }
}
