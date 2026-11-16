//! Preview and confirm changes to the shared lighting of the keyboard and mouse.

use super::*;
use crate::{config::SolidLight, product::ProductImage};

pub(super) enum PeripheralDialog {
    Mouse,
    Sync(SyncDraft),
}

pub(super) struct SyncDraft {
    linked: bool,
    light: SolidLight,
    hex: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Decision {
    Pending,
    Cancel,
    Apply,
}

impl G213App {
    pub(super) fn open_sync(&mut self) {
        self.peripheral_dialog = Some(PeripheralDialog::Sync(SyncDraft {
            linked: self.config.sync_colors,
            light: self.config.shared,
            hex: self.config.shared.color.hex(),
        }));
    }

    pub(super) fn sync_modal(&mut self, ctx: &egui::Context, pal: Palette, mut draft: SyncDraft) {
        let response = egui::Modal::new(Id::new("color-sync"))
            .backdrop_color(Color32::from_black_alpha(105))
            .frame(modal_frame(&pal))
            .show(ctx, |ui| {
                ui.set_width(512.0);
                let mut decision = Decision::Pending;
                ui.horizontal(|ui| {
                    ui.heading("Sincronização de cores");
                    ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(ui, &pal, egui_phosphor::regular::X, "Fechar")
                            .clicked()
                        {
                            decision = Decision::Cancel;
                        }
                    });
                });

                ui.add_space(8.0);
                select_mode(ui, pal, &mut draft.linked);
                ui.add_space(4.0);

                if draft.linked {
                    shared_color_controls(ui, pal, &mut draft);
                } else {
                    ui.label(
                        RichText::new("Cada dispositivo usa suas próprias cores e efeitos.")
                            .size(12.5)
                            .color(pal.text_muted),
                    );
                }

                let valid = !draft.linked || Rgb::from_hex(&draft.hex).is_some();
                self.sync_preview(ui, pal, &draft, valid);

                ui.add_space(6.0);
                widgets::divider(ui, &pal);
                ui.add_space(4.0);
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(valid, |ui| {
                        if widgets::button(ui, &pal, "Aplicar", None, ButtonKind::Primary).clicked()
                        {
                            decision = Decision::Apply;
                        }
                    });
                    if widgets::button(ui, &pal, "Cancelar", None, ButtonKind::Ghost).clicked() {
                        decision = Decision::Cancel;
                    }
                });
                decision
            });

        match response.inner {
            Decision::Apply => {
                let shared = if draft.linked {
                    draft.light
                } else {
                    self.config.shared
                };
                if self.config.sync_colors != draft.linked || self.config.shared != shared {
                    self.config.sync_colors = draft.linked;
                    self.config.shared = shared;
                    self.touch();
                    self.notify(if draft.linked {
                        "Cor compartilhada salva"
                    } else {
                        "Cores individuais restauradas"
                    });
                }
            }
            Decision::Pending if !response.should_close() => {
                self.peripheral_dialog = Some(PeripheralDialog::Sync(draft));
            }
            _ => {}
        }
    }

    fn sync_preview(&self, ui: &mut Ui, pal: Palette, draft: &SyncDraft, valid: bool) {
        let (keyboard, mouse) = if draft.linked {
            let light = draft.light.as_lighting();
            (light.clone(), light)
        } else {
            (
                self.config.lighting.clone(),
                self.config.mouse.as_lighting(),
            )
        };
        let mouse_connection = self
            .mouse_worker
            .as_ref()
            .map_or(ConnectionState::NotFound, |worker| worker.status().state);

        ui.add_space(6.0);
        widgets::section_label(ui, &pal, "Prévia nos dispositivos");
        ui.add_enabled_ui(valid, |ui| {
            let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            ui.horizontal_top(|ui| {
                preview_card(
                    ui,
                    pal,
                    width,
                    "G213 Prodigy",
                    (&self.product, 88.0),
                    self.status.state,
                    &keyboard,
                );
                preview_card(
                    ui,
                    pal,
                    width,
                    "G203",
                    (&self.mouse_image, 56.0),
                    mouse_connection,
                    &mouse,
                );
            });
        });

        if !self.preview
            && (self.status.state != ConnectionState::Connected
                || mouse_connection != ConnectionState::Connected)
        {
            ui.label(RichText::new("A configuração também será aplicada quando os dispositivos estiverem disponíveis.")
                .size(11.5).color(pal.text_muted));
        }
    }
}

fn select_mode(ui: &mut Ui, pal: Palette, linked: &mut bool) {
    let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
    ui.horizontal(|ui| {
        for (value, title) in [(false, "Cores individuais"), (true, "Cor compartilhada")] {
            let selected = *linked == value;
            let button =
                egui::Button::new(RichText::new(title).font(theme::font(13.0, Weight::Medium)))
                    .selected(selected)
                    .stroke(Stroke::new(
                        1.0,
                        if selected { theme::ACCENT } else { pal.border },
                    ))
                    .fill(if selected {
                        pal.accent_tint()
                    } else {
                        pal.input
                    })
                    .min_size(vec2(width, 36.0));
            if ui
                .add(button)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *linked = value;
            }
        }
    });
}

fn shared_color_controls(ui: &mut Ui, pal: Palette, draft: &mut SyncDraft) {
    widgets::section_label(ui, &pal, "Cor base");
    ui.horizontal(|ui| {
        let mut rgb = [
            draft.light.color.r,
            draft.light.color.g,
            draft.light.color.b,
        ];
        if ui
            .color_edit_button_srgb(&mut rgb)
            .on_hover_text("Abrir seletor de cor")
            .changed()
        {
            draft.light.color = Rgb::new(rgb[0], rgb[1], rgb[2]);
            draft.hex = draft.light.color.hex();
        }
        if ui
            .add(
                TextEdit::singleline(&mut draft.hex)
                    .desired_width(90.0)
                    .hint_text("#RRGGBB")
                    .char_limit(7)
                    .font(FontId::monospace(13.0)),
            )
            .changed()
            && let Some(color) = Rgb::from_hex(&draft.hex)
        {
            draft.light.color = color;
        }
        ui.add_space(6.0);
        for color in SWATCHES {
            if widgets::swatch(ui, &pal, color, draft.light.color == color).clicked() {
                draft.light.color = color;
                draft.hex = color.hex();
            }
        }
    });
    if Rgb::from_hex(&draft.hex).is_none() {
        ui.colored_label(
            theme::STATUS_DANGER,
            "Informe seis dígitos hexadecimais, como #3B82F6.",
        );
    }
    ui.add_space(4.0);
    widgets::slider_row(ui, &pal, "Brilho", &mut draft.light.brightness, 1..=100);
    ui.add(
        egui::Slider::new(&mut draft.light.hue, 0..=359)
            .suffix("°")
            .text("Matiz"),
    );
}

fn preview_card(
    ui: &mut Ui,
    pal: Palette,
    width: f32,
    name: &str,
    artwork: (&ProductImage, f32),
    connection: ConnectionState,
    light: &Lighting,
) {
    let border = Stroke::new(1.0, pal.border);
    Frame::new()
        .fill(pal.input)
        .stroke(border)
        .corner_radius(widgets::CARD)
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(width - 26.0);
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        vec2(94.0, 50.0),
                        ELayout::centered_and_justified(egui::Direction::TopDown),
                        |ui| artwork.0.show(ui, artwork.1),
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new(name).font(theme::font(13.0, Weight::SemiBold)));
                        let (label, dot) = match connection {
                            ConnectionState::Connected => ("Conectado", theme::STATUS_CONNECTED),
                            ConnectionState::PermissionDenied => {
                                ("Sem permissão", theme::STATUS_DANGER)
                            }
                            ConnectionState::Error => ("Falha de acesso", theme::STATUS_DANGER),
                            ConnectionState::Searching => ("Buscando", theme::STATUS_CONNECTING),
                            ConnectionState::NotFound => ("Desconectado", pal.text_muted),
                        };
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            let rect = ui.allocate_exact_size(vec2(6.0, 14.0), Sense::hover()).0;
                            ui.painter().circle_filled(rect.center(), 3.0, dot);
                            ui.label(RichText::new(label).color(pal.text_muted).size(11.0));
                        });
                    });
                });

                let (track, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
                let colors = light.preview_colors(0.0);
                ui.painter()
                    .rect_filled(track, widgets::CONTROL, theme::color32(colors[0]));
                for (index, &color) in colors.iter().enumerate().skip(1) {
                    let segment = track.width() / colors.len() as f32;
                    let left = track.left() + segment * index as f32;
                    let rect = Rect::from_min_max(pos2(left, track.top()), track.right_bottom());
                    if color != colors[index - 1] {
                        ui.painter()
                            .rect_filled(rect, widgets::CONTROL, theme::color32(color));
                    }
                }
                ui.painter()
                    .rect_stroke(track, widgets::CONTROL, border, StrokeKind::Inside);
                let summary = if !light.is_on() {
                    "Iluminação apagada".to_owned()
                } else if light.effect != Effect::Static {
                    light.effect.label().to_owned()
                } else if light.layout == Layout::Zones {
                    "5 zonas RGB".to_owned()
                } else {
                    colors[0].hex()
                };
                ui.label(RichText::new(summary).size(11.5).color(pal.text_muted));
            });
        });
}
