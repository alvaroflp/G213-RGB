//! Device-centred macro editor with a trigger list and an inspector.

use super::*;
use crate::macros::{Binding, MAX_NAME, MAX_STEPS, Peripheral, Status, Step};

#[derive(Default)]
pub(super) struct Editor {
    pub(super) device: Peripheral,
    draft: Option<Binding>,
    error: Option<String>,
}

impl Editor {
    fn select(&mut self, trigger: u16, settings: &crate::macros::Settings) {
        self.error = None;
        self.draft = Some(
            settings
                .bindings
                .iter()
                .find(|binding| binding.device == self.device && binding.trigger == trigger)
                .cloned()
                .unwrap_or_else(|| Binding {
                    device: self.device,
                    trigger,
                    name: String::new(),
                    steps: vec![Step::Shortcut(String::new())],
                }),
        );
    }
}

impl G213App {
    pub(super) fn open_macros(&mut self, device: Peripheral) {
        if self.macro_editor.device != device {
            self.macro_editor = Editor {
                device,
                ..Editor::default()
            };
        }
        self.view = View::Macros;
    }

    fn macros_changed(&mut self) {
        if let Some(worker) = &self.macro_worker {
            worker.update(&self.config.macros);
        }
        self.touch();
    }

    pub(super) fn macros_ui(&mut self, ui: &mut Ui, pal: Palette) {
        ScrollArea::vertical().id_salt("macros-page").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(24.0);
            centered(ui, 1040.0, |ui| {
                let enabled = widgets::toggle_row(ui, &pal, "Executar macros",
                    "Executa a sequência ao soltar o gatilho. A ação original da tecla ou botão também é enviada.",
                    self.config.macros.enabled);
                if enabled != self.config.macros.enabled {
                    self.config.macros.enabled = enabled;
                    self.macros_changed();
                }
                let status = if self.preview {
                    "Prévia · execução desativada".into()
                } else {
                    match self.macro_worker.as_ref().map(crate::macros::Worker::status) {
                        Some(Status::Ready { keyboard, mouse }) => {
                            match (keyboard, mouse) {
                                (true, true) => "G213 e G203 prontos".into(),
                                (true, false) => "G213 pronto".into(),
                                (false, true) => "G203 pronto".into(),
                                _ => "Aguardando dispositivos".into(),
                            }
                        }
                        Some(Status::Waiting) => "Aguardando um dispositivo com macro configurada".into(),
                        Some(Status::Error(error)) => error,
                        _ => if enabled { "Selecione uma tecla ou botão e salve uma macro.".into() } else { "Execução pausada".into() },
                    }
                };
                ui.label(RichText::new(status).size(12.0).color(pal.text_muted));
                ui.add_space(10.0);
                widgets::divider(ui, &pal);
                ui.add_space(16.0);
                let width = ui.available_width();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(width * 0.53 - 16.0);
                        self.trigger_list(ui, pal);
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.set_width(width * 0.47 - 16.0);
                        self.binding_inspector(ui, pal);
                    });
                });
                ui.add_space(22.0);
            });
        });
    }

    fn trigger_list(&mut self, ui: &mut Ui, pal: Palette) {
        let device = self.macro_editor.device;
        let width = ui.available_width();
        if device == Peripheral::Mouse {
            self.mouse_trigger_map(ui, pal, width);
            return;
        }
        ui.vertical_centered(|ui| self.product.show(ui, width.min(440.0)));
        ui.add_space(16.0);
        widgets::section_label(ui, &pal, "Teclas de função");
        ui.horizontal_wrapped(|ui| {
            for &(label, code) in device.triggers() {
                let selected = self
                    .macro_editor
                    .draft
                    .as_ref()
                    .is_some_and(|d| d.trigger == code.code());
                let saved = self
                    .config
                    .macros
                    .bindings
                    .iter()
                    .any(|b| b.device == device && b.trigger == code.code());
                let button = egui::Button::new(label)
                    .min_size(vec2(52.0, 36.0))
                    .selected(selected)
                    .stroke(Stroke::new(
                        1.0,
                        if saved || selected {
                            theme::ACCENT
                        } else {
                            pal.border
                        },
                    ));
                if ui.add(button).clicked() {
                    self.macro_editor.select(code.code(), &self.config.macros);
                }
            }
        });
    }

    fn mouse_trigger_map(&mut self, ui: &mut Ui, pal: Palette, width: f32) {
        let device = Peripheral::Mouse;
        let image_width = width * 0.64;
        let image_height = image_width * 311.0 / 360.0;
        let stage = ui
            .allocate_exact_size(vec2(width, image_height + 24.0), Sense::hover())
            .0;
        let image = Rect::from_min_size(
            pos2(stage.right() - image_width, stage.top()),
            vec2(image_width, image_height),
        );
        let mut artwork = ui.new_child(UiBuilder::new().max_rect(image));
        self.mouse_image.show(&mut artwork, image_width);

        let controls_width = width * 0.46;
        let anchors = [(0.502, 0.26), (0.332, 0.39), (0.325, 0.46)];
        for (index, &(label, code)) in device.triggers().iter().enumerate() {
            let (nx, ny) = anchors[index];
            let point = image.min + vec2(image.width() * nx, image.height() * ny);
            let row = Rect::from_min_size(
                stage.min + vec2(0.0, 24.0 + index as f32 * 54.0),
                vec2(controls_width, 42.0),
            );
            let selected = self
                .macro_editor
                .draft
                .as_ref()
                .is_some_and(|d| d.trigger == code.code());
            let name = self
                .config
                .macros
                .bindings
                .iter()
                .find(|b| b.device == device && b.trigger == code.code())
                .map_or("Sem macro", |b| b.name.as_str());
            let color = if selected {
                theme::ACCENT
            } else {
                pal.text_muted
            };
            let start = pos2(row.right(), row.center().y);
            ui.painter().add(egui::Shape::line(
                vec![start, pos2((start.x + point.x) * 0.5, point.y), point],
                Stroke::new(1.0, if selected { color } else { pal.border }),
            ));
            let dot = ui
                .interact(
                    Rect::from_center_size(point, vec2(24.0, 24.0)),
                    ui.id().with(("mouse-trigger", code.code())),
                    Sense::click(),
                )
                .on_hover_text(label)
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            ui.painter().circle(
                point,
                if dot.hovered() { 7.0 } else { 5.0 },
                color,
                Stroke::new(1.5, pal.panel),
            );

            let item =
                egui::Button::new(RichText::new(format!("{label}  ·  {name}    ›")).size(12.0))
                    .selected(selected)
                    .truncate()
                    .min_size(row.size());
            let mut item_ui = ui.new_child(UiBuilder::new().max_rect(row));
            if item_ui.add(item).clicked() || dot.clicked() {
                self.macro_editor.select(code.code(), &self.config.macros);
            }
        }
    }

    fn binding_inspector(&mut self, ui: &mut Ui, pal: Palette) {
        widgets::panel_card(ui, &pal, None, |ui| {
            let Some(draft) = self.macro_editor.draft.as_mut() else {
                ui.heading("Macro");
                ui.label("Selecione uma tecla ou botão para editar a sequência.");
                return;
            };
            let trigger_label = draft
                .device
                .triggers()
                .iter()
                .find(|(_, code)| code.code() == draft.trigger)
                .map_or("Gatilho", |(label, _)| *label);
            ui.heading(format!("Macro · {trigger_label}"));
            ui.label("Nome");
            ui.add(
                TextEdit::singleline(&mut draft.name)
                    .char_limit(MAX_NAME)
                    .desired_width(ui.available_width())
                    .hint_text("Ex.: Reabrir aba"),
            );
            ui.add_space(8.0);
            widgets::section_label(ui, &pal, "Sequência");
            let mut remove = None;
            let mut reorder = None;
            let count = draft.steps.len();
            for (index, step) in draft.steps.iter_mut().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}", index + 1));
                        match step {
                            Step::Shortcut(shortcut) => {
                                ui.add(
                                    TextEdit::singleline(shortcut)
                                        .char_limit(48)
                                        .desired_width((ui.available_width() - 100.0).max(90.0))
                                        .hint_text("Ctrl+Shift+T"),
                                );
                            }
                            Step::Delay(ms) => {
                                ui.add(
                                    egui::DragValue::new(ms)
                                        .range(10..=5000)
                                        .speed(5)
                                        .suffix(" ms"),
                                );
                                ui.label("Pausa");
                            }
                        }
                        if ui
                            .add_enabled(index > 0, egui::Button::new("↑"))
                            .on_hover_text("Subir etapa")
                            .clicked()
                        {
                            reorder = Some((index, index - 1));
                        }
                        if ui
                            .add_enabled(index + 1 < count, egui::Button::new("↓"))
                            .on_hover_text("Descer etapa")
                            .clicked()
                        {
                            reorder = Some((index, index + 1));
                        }
                        if ui.button("×").on_hover_text("Remover etapa").clicked() {
                            remove = Some(index);
                        }
                    });
                });
            }
            if let Some((a, b)) = reorder {
                draft.steps.swap(a, b);
            }
            if let Some(index) = remove {
                draft.steps.remove(index);
            }
            ui.add_enabled_ui(draft.steps.len() < MAX_STEPS, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if widgets::button(ui, &pal, "Adicionar atalho", None, ButtonKind::Secondary)
                        .clicked()
                    {
                        draft.steps.push(Step::Shortcut(String::new()));
                    }
                    if widgets::button(ui, &pal, "Adicionar pausa", None, ButtonKind::Secondary)
                        .clicked()
                    {
                        draft.steps.push(Step::Delay(250));
                    }
                });
            });
            ui.label(RichText::new("Use Ctrl, Shift, Alt ou Super com uma letra, número ou tecla como F5, Enter ou Tab.")
                .size(11.5).color(pal.text_muted));
            ui.add_space(8.0);
            let index = self
                .config
                .macros
                .bindings
                .iter()
                .position(|b| b.device == draft.device && b.trigger == draft.trigger);
            let mut save = false;
            let mut delete = false;
            ui.horizontal_wrapped(|ui| {
                save =
                    widgets::button(ui, &pal, "Salvar macro", None, ButtonKind::Primary).clicked();
                if index.is_some() {
                    delete = widgets::button(ui, &pal, "Remover macro", None, ButtonKind::Ghost)
                        .clicked();
                }
            });
            if save {
                draft.name = draft.name.trim().to_owned();
                match draft.validate() {
                    Ok(()) => {
                        if let Some(index) = index {
                            self.config.macros.bindings[index] = draft.clone();
                        } else {
                            self.config.macros.bindings.push(draft.clone());
                        }
                        self.macro_editor.error = None;
                        self.macros_changed();
                        self.notify("Macro salva");
                    }
                    Err(error) => self.macro_editor.error = Some(error),
                }
            } else if delete && let Some(index) = index {
                let binding = self.config.macros.bindings.remove(index);
                self.macro_editor
                    .select(binding.trigger, &self.config.macros);
                self.macros_changed();
                self.notify("Macro removida");
            }
            if let Some(error) = &self.macro_editor.error {
                ui.colored_label(theme::STATUS_DANGER, error);
            }
        });
    }
}
