//! Device discovery and shared lighting controls.

use super::sync_ui::PeripheralDialog;
use super::*;
use crate::config::SolidLight;

impl G213App {
    pub(super) fn lighting_delivery(&self, ui: &mut Ui, pal: Palette, mouse: bool) {
        use crate::device::DeliveryState;
        let worker = if mouse {
            &self.mouse_worker
        } else {
            &self.worker
        };
        let Some(worker) = worker else {
            return;
        };
        let state = worker.delivery();
        let (label, color) = match &state {
            DeliveryState::Idle => return,
            DeliveryState::Pending if worker.status().is_connected() => {
                ("Envio pendente", pal.text_muted)
            }
            DeliveryState::Pending => ("Aguardando conexão para aplicar", pal.text_muted),
            DeliveryState::Sending => ("Enviando iluminação…", pal.text_muted),
            DeliveryState::Confirmed => ("Iluminação confirmada por USB", pal.text_muted),
            DeliveryState::Failed(_) => ("Falha ao aplicar iluminação", theme::STATUS_DANGER),
        };
        ui.push_id(("lighting-delivery", mouse), |ui| {
            ui.horizontal_wrapped(|ui| {
                let label = ui.label(RichText::new(label).size(12.0).color(color));
                if let DeliveryState::Failed(error) = state {
                    label.on_hover_text(error);
                    if widgets::button(
                        ui,
                        &pal,
                        "Reenviar",
                        Some(egui_phosphor::regular::ARROW_CLOCKWISE),
                        ButtonKind::Secondary,
                    )
                    .clicked()
                    {
                        let desired = if mouse {
                            self.config.mouse_lighting()
                        } else {
                            self.config.keyboard_lighting()
                        };
                        worker.retry(ApplyState::from(&desired));
                        ui.ctx().request_repaint();
                    }
                }
            });
        });
    }

    pub(super) fn peripheral_actions(&mut self, ui: &mut Ui, pal: Palette) {
        ui.horizontal(|ui| {
            if widgets::button(
                ui,
                &pal,
                "Buscar dispositivos",
                Some(egui_phosphor::regular::MAGNIFYING_GLASS),
                ButtonKind::Secondary,
            )
            .clicked()
            {
                self.refresh();
            }
            if widgets::button(
                ui,
                &pal,
                self.sync_label(),
                Some(egui_phosphor::regular::LINK),
                if self.config.sync_colors {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
            )
            .clicked()
            {
                self.open_sync();
            }
            if widgets::button(ui, &pal, "Macros", None, ButtonKind::Secondary).clicked() {
                self.open_macros(crate::macros::Peripheral::Keyboard);
            }
        });
        self.global_power_ui(ui, pal, "Iluminação dos dispositivos");
    }

    fn global_power_ui(&mut self, ui: &mut Ui, pal: Palette, title: &str) {
        let keyboard = self.config.keyboard_lighting().is_on();
        let mouse = self.config.mouse_lighting().is_on();
        let both_on = keyboard && mouse;
        let description = if keyboard != mouse {
            "Um dispositivo está aceso. Clique para acender ambos."
        } else {
            "Liga ou desliga teclado e mouse juntos."
        };
        let enabled = widgets::toggle_row(ui, &pal, title, description, both_on);
        if enabled != both_on {
            self.config.set_all_lights(enabled);
            self.touch();
        }
    }

    fn sync_label(&self) -> &'static str {
        if self.config.sync_colors {
            "Cores sincronizadas"
        } else {
            "Sincronizar cores"
        }
    }

    pub(super) fn mouse_card(&mut self, ui: &mut Ui, pal: Palette) {
        let status = self.mouse_worker.as_ref().map(DeviceWorker::status);
        let state = status
            .as_ref()
            .map_or(ConnectionState::NotFound, |s| s.state);
        let (label, color) = match state {
            ConnectionState::Connected => ("Conectado", theme::STATUS_CONNECTED),
            ConnectionState::Searching => ("Buscando", theme::STATUS_CONNECTING),
            ConnectionState::PermissionDenied => ("Sem permissão", theme::STATUS_DANGER),
            ConnectionState::Error => ("Erro de acesso", theme::STATUS_DANGER),
            ConnectionState::NotFound => ("Desconectado", theme::STATUS_OFFLINE),
        };
        widgets::panel_card(ui, &pal, None, |ui| {
            ui.horizontal(|ui| {
                widgets::badge(ui, &pal, "MOUSE");
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    widgets::status_pill(ui, &pal, label, color);
                });
            });
            ui.vertical_centered(|ui| {
                self.mouse_image.show(ui, 218.0);
            });
            let name = status
                .as_ref()
                .and_then(|s| s.name.as_deref())
                .unwrap_or("Logitech G203");
            ui.label(RichText::new(name).font(theme::font(20.0, Weight::SemiBold)));
            ui.label(
                RichText::new("Prodigy / LIGHTSYNC · USB")
                    .size(13.0)
                    .color(pal.text_muted),
            );
            if state == ConnectionState::PermissionDenied {
                ui.label("Atualize a regra com ./install.sh e reconecte o mouse.");
            } else if let Some(error) = status
                .as_ref()
                .and_then(|s| s.detail.as_ref())
                .filter(|_| state == ConnectionState::Error)
            {
                ui.label(error);
            }
            self.lighting_delivery(ui, pal, true);
            widgets::divider(ui, &pal);
            ui.horizontal_wrapped(|ui| {
                if widgets::button(ui, &pal, "Personalizar mouse", None, ButtonKind::Primary)
                    .clicked()
                {
                    if self.config.sync_colors {
                        self.open_sync();
                    } else {
                        self.peripheral_dialog = Some(PeripheralDialog::Mouse);
                    }
                }
                if widgets::button(ui, &pal, "Macros do mouse", None, ButtonKind::Secondary)
                    .clicked()
                {
                    self.open_macros(crate::macros::Peripheral::Mouse);
                }
            });
        });
    }

    pub(super) fn sync_notice(&mut self, ui: &mut Ui, pal: Palette) {
        widgets::panel_card(ui, &pal, None, |ui| {
            ui.label("O teclado e o mouse estão configurados para usar a cor compartilhada.");
            if !self.preview {
                widgets::section_label(ui, &pal, "G213 · teclado");
                self.lighting_delivery(ui, pal, false);
                widgets::section_label(ui, &pal, "G203 · mouse");
                self.lighting_delivery(ui, pal, true);
            }
            if widgets::button(
                ui,
                &pal,
                "Editar cor compartilhada",
                None,
                ButtonKind::Secondary,
            )
            .clicked()
            {
                self.open_sync();
            }
        });
    }

    pub(super) fn peripherals_modal(&mut self, ctx: &egui::Context, pal: Palette) {
        let Some(dialog) = self.peripheral_dialog.take() else {
            return;
        };
        if let PeripheralDialog::Sync(draft) = dialog {
            self.sync_modal(ctx, pal, draft);
            return;
        }

        let response = egui::Modal::new(Id::new("mouse-lighting"))
            .frame(modal_frame(&pal))
            .show(ctx, |ui| {
                ui.set_width(390.0);
                ui.heading("Iluminação do G203");
                let mut light = self.config.mouse;
                solid_controls(ui, &pal, &mut light);
                if light != self.config.mouse {
                    self.config.mouse = light;
                    self.touch();
                }
                widgets::button(ui, &pal, "Fechar", None, ButtonKind::Secondary).clicked()
            });
        if !response.inner && !response.should_close() {
            self.peripheral_dialog = Some(PeripheralDialog::Mouse);
        }
    }

    pub(super) fn autostart_ui(&mut self, ui: &mut Ui, pal: Palette) {
        ui.add_enabled_ui(!self.preview, |ui| {
            let enabled = widgets::toggle_row(
                ui,
                &pal,
                "Iniciar com o sistema",
                "Abre o aplicativo ao entrar na sessão gráfica.",
                self.autostart,
            );
            if enabled != self.autostart {
                match crate::autostart::set_enabled(enabled) {
                    Ok(()) => {
                        self.autostart = enabled;
                        self.autostart_error = None;
                    }
                    Err(err) => self.autostart_error = Some(err.to_string()),
                }
            }
        });
        if let Some(error) = &self.autostart_error {
            ui.colored_label(
                theme::STATUS_DANGER,
                format!("Falha ao alterar a inicialização: {error}"),
            );
        }
    }
}

fn solid_controls(ui: &mut Ui, pal: &Palette, light: &mut SolidLight) {
    widgets::divider(ui, pal);
    light.enabled = widgets::toggle_row(
        ui,
        pal,
        "Iluminação do mouse",
        "Controla os LEDs do G203.",
        light.enabled,
    );
    ui.horizontal(|ui| {
        let mut rgb = [light.color.r, light.color.g, light.color.b];
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            light.color = Rgb::new(rgb[0], rgb[1], rgb[2]);
        }
        ui.label(format!("Cor base · {}", light.color.hex()));
    });
    widgets::slider_row(ui, pal, "Brilho", &mut light.brightness, 1..=100);
    ui.add(
        egui::Slider::new(&mut light.hue, 0..=359)
            .suffix("°")
            .text("Matiz"),
    );
    let output = light.as_lighting().preview_colors(0.0)[0];
    ui.colored_label(theme::color32(output), "●");
    ui.label(format!("Prévia · {}", output.hex()));
}
