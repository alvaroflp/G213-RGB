//! Named lighting snapshots and built-in presets.

use super::*;

impl G213App {
    pub(super) fn profiles_tab(&mut self, ui: &mut Ui, pal: Palette) {
        centered(ui, 740.0, |ui| {
            ui.label(
                RichText::new("Um perfil para cada momento.")
                    .font(theme::font(26.0, Weight::SemiBold)),
            );
            ui.label(
                RichText::new("Guarde suas cores e efeitos. Troque quando quiser.")
                    .size(13.0)
                    .color(pal.text_muted),
            );
            ui.add_space(16.0);
            widgets::panel_card(
                ui,
                &pal,
                Some((egui_phosphor::regular::PLUS, "Salvar iluminação atual")),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut self.profile_name)
                                .margin(vec2(10.0, 7.0))
                                .hint_text("Nome do perfil")
                                .char_limit(40)
                                .desired_width((ui.available_width() - 110.0).max(160.0)),
                        );
                        if widgets::button(ui, &pal, "Salvar perfil", None, ButtonKind::Primary)
                            .clicked()
                        {
                            match self.config.save_profile(&self.profile_name) {
                                Ok(()) => {
                                    self.profile_name.clear();
                                    self.profile_error = None;
                                    self.touch();
                                    self.notify("Perfil adicionado");
                                }
                                Err(error) => self.profile_error = Some(error.into()),
                            }
                        }
                    });
                    if let Some(error) = &self.profile_error {
                        ui.colored_label(theme::STATUS_DANGER, error);
                    }
                },
            );
            ui.add_space(16.0);
            widgets::section_label(ui, &pal, "SEUS PERFIS");
            if self.config.profiles.is_empty() {
                ui.label(
                    RichText::new("Seu primeiro perfil começa com a iluminação atual.")
                        .size(13.0)
                        .color(pal.text_muted),
                );
            }
            let mut remove = None;
            for (index, profile) in self.config.profiles.clone().iter().enumerate() {
                if self.profile_row(ui, pal, profile, true) {
                    remove = Some(index);
                }
            }
            if let Some(index) = remove {
                self.deleted_profile = Some(self.config.profiles.remove(index));
                self.touch();
            }
            if self.deleted_profile.is_some()
                && widgets::button(
                    ui,
                    &pal,
                    "Desfazer exclusão do perfil",
                    None,
                    ButtonKind::Secondary,
                )
                .clicked()
            {
                let profile = self.deleted_profile.as_ref().unwrap();
                if self.config.profiles.len() < 32
                    && !self
                        .config
                        .profiles
                        .iter()
                        .any(|p| p.name.to_lowercase() == profile.name.to_lowercase())
                {
                    self.config
                        .profiles
                        .push(self.deleted_profile.take().unwrap());
                    self.touch();
                } else {
                    self.notify("Remova o perfil de mesmo nome ou libere espaço para restaurar.");
                }
            }
            ui.add_space(16.0);
            widgets::section_label(ui, &pal, "PARA EXPERIMENTAR");
            for profile in config::builtin_profiles() {
                self.profile_row(ui, pal, &profile, false);
            }
            self.history_actions(ui, pal);
        });
    }

    fn profile_row(
        &mut self,
        ui: &mut Ui,
        pal: Palette,
        profile: &config::Profile,
        removable: bool,
    ) -> bool {
        let mut remove = false;
        let active = self.config.lighting == profile.lighting;
        widgets::panel_card(ui, &pal, None, |ui| {
            ui.horizontal(|ui| {
                let colors = profile.lighting.preview_colors(0.0);
                for color in colors {
                    let (rect, _) = ui.allocate_exact_size(vec2(8.0, 30.0), Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(3), theme::color32(color));
                }
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(&profile.name).font(theme::font(14.0, Weight::SemiBold)),
                    );
                    ui.label(
                        RichText::new(profile.lighting.effect.label())
                            .size(11.5)
                            .color(pal.text_muted),
                    );
                });
                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                    if removable
                        && widgets::icon_button(
                            ui,
                            &pal,
                            egui_phosphor::regular::TRASH,
                            "Excluir perfil",
                        )
                        .clicked()
                    {
                        remove = true;
                    }
                    if widgets::button(
                        ui,
                        &pal,
                        if active { "Selecionado" } else { "Usar perfil" },
                        None,
                        if active {
                            ButtonKind::Ghost
                        } else {
                            ButtonKind::Secondary
                        },
                    )
                    .clicked()
                    {
                        self.config.lighting = profile.lighting.clone();
                        self.touch();
                    }
                });
            });
        });
        remove
    }
}
