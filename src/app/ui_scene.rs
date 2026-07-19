use super::*;

impl CameraManApp {
    pub(super) fn scene_selector_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(tr(self.locale, UiText::Scene));
        let selected = self
            .active_scene_name
            .as_deref()
            .unwrap_or(tr(self.locale, UiText::Unsaved));
        let mut apply_index = None;
        egui::ComboBox::from_id_salt("scene-selector")
            .selected_text(selected)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (index, scene) in self.scenes.iter().enumerate() {
                    if ui
                        .selectable_label(
                            self.active_scene_name.as_deref() == Some(scene.name.as_str()),
                            &scene.name,
                        )
                        .clicked()
                    {
                        apply_index = Some(index);
                    }
                }
            });
        if let Some(index) = apply_index {
            match self.apply_scene(index) {
                Ok(()) => {
                    self.refresh_scene_render(ctx, SceneChange::SceneRestore);
                    self.set_event(tr(self.locale, UiText::SceneApplied), false);
                }
                Err(error) => self.set_event(error, true),
            }
        }

        ui.add(egui::TextEdit::singleline(&mut self.scene_name_input).desired_width(f32::INFINITY))
            .on_hover_text(tr(self.locale, UiText::NamedScene));
        if ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new(tr(self.locale, UiText::Save)),
            )
            .clicked()
        {
            match self.save_current_scene() {
                Ok(()) => self.set_event(tr(self.locale, UiText::SceneSaved), false),
                Err(error) => self.set_event(error, true),
            }
        }
        ui.horizontal(|ui| {
            let undo_label = tr(self.locale, UiText::Undo);
            let undo = ui
                .add_enabled(
                    !self.undo_stack.is_empty(),
                    egui::Button::new("↶").min_size(egui::vec2(36.0, 28.0)),
                )
                .on_hover_text(format!("{undo_label} (Command-Z)"));
            undo.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, undo_label)
            });
            if undo.clicked() && self.undo_scene_edit() {
                self.refresh_scene_render(ctx, SceneChange::SceneRestore);
            }
            let redo_label = tr(self.locale, UiText::Redo);
            let redo = ui
                .add_enabled(
                    !self.redo_stack.is_empty(),
                    egui::Button::new("↷").min_size(egui::vec2(36.0, 28.0)),
                )
                .on_hover_text(format!("{redo_label} (Command-Shift-Z)"));
            redo.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, redo_label)
            });
            if redo.clicked() && self.redo_scene_edit() {
                self.refresh_scene_render(ctx, SceneChange::SceneRestore);
            }
        });
    }

    pub(super) fn transform_inspector_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let selected_ids = self.selected_source_ids();
        if self
            .selected_source_id
            .as_ref()
            .is_none_or(|id| !selected_ids.contains(id))
        {
            self.selected_source_id = selected_ids.first().cloned();
        }
        let Some(source_id) = self.selected_source_id.clone() else {
            return;
        };

        ui.separator();
        ui.label(tr(self.locale, UiText::Transform));
        ui.add(egui::Label::new(egui::RichText::new(&source_id).monospace()).truncate())
            .on_hover_text(&source_id);
        let mut transform = self
            .source_transforms
            .get(&source_id)
            .copied()
            .unwrap_or_default();
        let before = transform;

        ui.columns(2, |columns| {
            columns[0].label(tr(self.locale, UiText::Scaling));
            egui::ComboBox::from_id_salt("transform-fit")
                .width(columns[0].available_width())
                .selected_text(match transform.fit {
                    SourceFit::Fit => tr(self.locale, UiText::Fit),
                    SourceFit::Fill => tr(self.locale, UiText::Fill),
                })
                .show_ui(&mut columns[0], |ui| {
                    ui.selectable_value(
                        &mut transform.fit,
                        SourceFit::Fit,
                        tr(self.locale, UiText::Fit),
                    );
                    ui.selectable_value(
                        &mut transform.fit,
                        SourceFit::Fill,
                        tr(self.locale, UiText::Fill),
                    );
                });
            columns[1].label(tr(self.locale, UiText::Rotation));
            egui::ComboBox::from_id_salt("transform-rotation")
                .width(columns[1].available_width())
                .selected_text(rotation_label(transform.rotation))
                .show_ui(&mut columns[1], |ui| {
                    for rotation in [
                        Rotation::Degrees0,
                        Rotation::Degrees90,
                        Rotation::Degrees180,
                        Rotation::Degrees270,
                    ] {
                        ui.selectable_value(
                            &mut transform.rotation,
                            rotation,
                            rotation_label(rotation),
                        );
                    }
                });
        });
        ui.checkbox(
            &mut transform.mirror_horizontal,
            tr(self.locale, UiText::MirrorHorizontal),
        );
        ui.checkbox(
            &mut transform.mirror_vertical,
            tr(self.locale, UiText::MirrorVertical),
        );

        ui.label(format!(
            "{}: {:.0}%",
            tr(self.locale, UiText::Opacity),
            f32::from(transform.opacity_per_mille) / 10.0
        ));
        ui.add(egui::Slider::new(&mut transform.opacity_per_mille, 0..=1_000).show_value(false));

        ui.label(tr(self.locale, UiText::Position));
        normalized_slider(ui, "X", &mut transform.position_x_per_mille, -1_000..=1_000);
        normalized_slider(ui, "Y", &mut transform.position_y_per_mille, -1_000..=1_000);

        ui.label(tr(self.locale, UiText::Crop));
        crop_slider(ui, "L", &mut transform.crop.left_per_mille);
        crop_slider(ui, "T", &mut transform.crop.top_per_mille);
        crop_slider(ui, "R", &mut transform.crop.right_per_mille);
        crop_slider(ui, "B", &mut transform.crop.bottom_per_mille);

        if ui
            .add_sized(
                [96.0, 28.0],
                egui::Button::new(tr(self.locale, UiText::Reset)),
            )
            .on_hover_text(tr(self.locale, UiText::ResetTransform))
            .clicked()
        {
            transform = SourceTransform::default();
        }

        transform = transform.sanitized();
        if transform != before
            && let Some(change) = self.apply_scene_command(SceneEditCommand::SetTransform {
                source_id,
                transform,
            })
        {
            self.refresh_scene_render(ctx, change);
        }
    }

    pub(super) fn scene_file_ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.label(tr(self.locale, UiText::SceneFile));
        let mut scene_path = self.scene_path.to_string_lossy().into_owned();
        if ui
            .add(
                egui::TextEdit::singleline(&mut scene_path)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace),
            )
            .changed()
        {
            self.scene_path = PathBuf::from(scene_path);
            self.import_preview = None;
        }
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    [86.0, 28.0],
                    egui::Button::new(tr(self.locale, UiText::Validate)),
                )
                .clicked()
            {
                match self.validate_scene_import() {
                    Ok(()) => self.set_event(tr(self.locale, UiText::SceneValid), false),
                    Err(error) => self.set_event(error, true),
                }
            }
            if ui
                .add_sized(
                    [82.0, 28.0],
                    egui::Button::new(tr(self.locale, UiText::Export)),
                )
                .clicked()
            {
                match self.export_current_scene() {
                    Ok(()) => self.set_event(tr(self.locale, UiText::SceneExported), false),
                    Err(error) => self.set_event(error, true),
                }
            }
        });
        if let Some(scene) = &self.import_preview {
            ui.label(
                egui::RichText::new(format!(
                    "{} · {} source(s) · schema {}",
                    scene.name,
                    scene.source_ids.len(),
                    scene.schema_version
                ))
                .color(COLOR_OK),
            );
            if ui
                .add_sized(
                    [96.0, 28.0],
                    egui::Button::new(tr(self.locale, UiText::Import)),
                )
                .clicked()
            {
                match self.accept_scene_import() {
                    Ok(()) => {
                        self.set_event(tr(self.locale, UiText::SceneImported), false);
                    }
                    Err(error) => self.set_event(error, true),
                }
            }
        }
    }
}

fn normalized_slider(
    ui: &mut egui::Ui,
    axis: &str,
    value: &mut i16,
    range: std::ops::RangeInclusive<i16>,
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [42.0, 18.0],
            egui::Label::new(format!("{axis} {:.0}%", f32::from(*value) / 10.0)),
        );
        ui.add(egui::Slider::new(value, range).show_value(false));
    });
}

fn crop_slider(ui: &mut egui::Ui, edge: &str, value: &mut u16) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [42.0, 18.0],
            egui::Label::new(format!("{edge} {:.0}%", f32::from(*value) / 10.0)),
        );
        ui.add(egui::Slider::new(value, 0..=950).show_value(false));
    });
}

const fn rotation_label(rotation: Rotation) -> &'static str {
    match rotation {
        Rotation::Degrees0 => "0°",
        Rotation::Degrees90 => "90°",
        Rotation::Degrees180 => "180°",
        Rotation::Degrees270 => "270°",
    }
}
