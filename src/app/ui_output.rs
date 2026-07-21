use super::*;

impl CameraManApp {
    pub(super) fn output_panel_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.add_space(8.0);
                ui.heading(tr(self.locale, UiText::Output));
                ui.add_space(8.0);

                ui.monospace(format!(
                    "{}x{} BGRA · {} fps",
                    VIRTUAL_CAMERA_WIDTH, VIRTUAL_CAMERA_HEIGHT, self.active_fps
                ))
                .on_hover_text(
                    "The CMIO stream currently exposes the implemented 1080p BGRA profile. "
                        .to_owned()
                        + "Unsupported renegotiation is rejected atomically.",
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let (icon, label, fill) = if self.running {
                        ("■", tr(self.locale, UiText::Stop), COLOR_STOP)
                    } else {
                        ("▶", tr(self.locale, UiText::Start), COLOR_ACCENT)
                    };
                    let primary = egui::Button::new(
                        egui::RichText::new(format!("{icon}  {label}"))
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .fill(fill)
                    .min_size(egui::vec2(112.0, 36.0));
                    let can_start = self.running || self.selected_count() > 0;
                    if ui
                        .add_enabled(can_start, primary)
                        .on_hover_text(tr(self.locale, UiText::StartStop))
                        .on_disabled_hover_text(tr(self.locale, UiText::SelectAtLeastOneSource))
                        .clicked()
                    {
                        self.toggle_running(ctx);
                    }

                    let can_render = !self.running
                        && !self.selected_sources.is_empty()
                        && self
                            .selected_sources
                            .iter()
                            .all(|source| source.kind == SourceKind::Synthetic);
                    let render = ui.add_enabled(
                        can_render,
                        egui::Button::new("↻").min_size(egui::vec2(36.0, 36.0)),
                    );
                    let render_label = tr(self.locale, UiText::Render);
                    render.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            can_render,
                            render_label,
                        )
                    });
                    if render.on_hover_text(render_label).clicked() {
                        self.request_render(ctx, false, true);
                    }
                });

                ui.add_space(14.0);
                ui.label(tr(self.locale, UiText::TargetFps));
                let mut fps_mode = self.fps_mode;
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut fps_mode,
                        FpsMode::Auto,
                        tr(self.locale, UiText::Auto),
                    );
                    for &preset in &VIRTUAL_CAMERA_FPS_PRESETS {
                        ui.selectable_value(
                            &mut fps_mode,
                            FpsMode::Fixed(preset),
                            preset.to_string(),
                        );
                    }
                });
                if let Some(change) =
                    self.apply_scene_command(SceneEditCommand::SetFrameRate(fps_mode))
                {
                    self.refresh_scene_render(ctx, change);
                }
                if let Some(warning) = self.fixed_fps_warning() {
                    ui.label(egui::RichText::new(warning).color(COLOR_WARNING).small());
                }

                ui.add_space(14.0);
                ui.label(tr(self.locale, UiText::Layout));
                let mut selected_layout = self.layout;
                egui::ComboBox::from_id_salt("output-layout")
                    .selected_text(layout_label(self.locale, selected_layout))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for candidate in [
                            CompositionLayout::Grid,
                            CompositionLayout::Row,
                            CompositionLayout::Column,
                            CompositionLayout::PictureInPicture,
                        ] {
                            ui.selectable_value(
                                &mut selected_layout,
                                candidate,
                                layout_label(self.locale, candidate),
                            );
                        }
                    });
                if let Some(change) =
                    self.apply_scene_command(SceneEditCommand::SetLayout(selected_layout))
                {
                    self.refresh_scene_render(ctx, change);
                }

                ui.add_space(10.0);
                ui.label(tr(self.locale, UiText::Scaling));
                let mut scaling_filter = self.scaling_filter;
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut scaling_filter,
                        ScalingFilter::Nearest,
                        tr(self.locale, UiText::Fast),
                    );
                    ui.selectable_value(
                        &mut scaling_filter,
                        ScalingFilter::Bilinear,
                        tr(self.locale, UiText::Smooth),
                    );
                });
                if let Some(change) =
                    self.apply_scene_command(SceneEditCommand::SetScalingFilter(scaling_filter))
                {
                    self.refresh_scene_render(ctx, change);
                }

                ui.add_space(10.0);
                ui.label(tr(self.locale, UiText::MissingPolicy));
                let mut missing_source_policy = self.missing_source_policy;
                egui::ComboBox::from_id_salt("missing-source-policy")
                    .selected_text(missing_policy_label(self.locale, missing_source_policy))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for policy in [
                            MissingSourcePolicy::Placeholder,
                            MissingSourcePolicy::FreezeBriefly,
                            MissingSourcePolicy::HideCell,
                            MissingSourcePolicy::StopOutput,
                        ] {
                            ui.selectable_value(
                                &mut missing_source_policy,
                                policy,
                                missing_policy_label(self.locale, policy),
                            );
                        }
                    });
                if let Some(change) = self.apply_scene_command(
                    SceneEditCommand::SetMissingSourcePolicy(missing_source_policy),
                ) {
                    self.refresh_scene_render(ctx, change);
                }

                ui.add_space(14.0);
                let mut output_enabled = self.virtual_output_enabled;
                if ui
                    .checkbox(&mut output_enabled, tr(self.locale, UiText::VirtualCamera))
                    .changed()
                {
                    let mut workflow = WorkflowState {
                        running: self.running,
                        selected_sources: self.selected_count(),
                        virtual_output_enabled: self.virtual_output_enabled,
                    };
                    if let AppEvent::VirtualOutputChanged {
                        enabled,
                        disconnect_transport,
                    } = reduce(&mut workflow, AppCommand::SetVirtualOutput(output_enabled))
                    {
                        self.virtual_output_enabled = enabled;
                        if disconnect_transport {
                            self.disconnect_virtual_output();
                        }
                    }
                }

                ui.add_space(14.0);
                if ui
                    .add_enabled(
                        self.preview.is_some() && !self.io_worker.is_running(),
                        egui::Button::new(tr(self.locale, UiText::ExportFrame))
                            .min_size(egui::vec2(ui.available_width(), 30.0)),
                    )
                    .on_disabled_hover_text(tr(self.locale, UiText::NothingToExport))
                    .clicked()
                {
                    self.export_preview();
                }
                if self.io_worker.operation() == Some(IoOperation::FrameExport) {
                    ui.horizontal(|ui| {
                        progress_indicator(
                            ui,
                            self.reduce_motion,
                            tr(self.locale, UiText::ExportingFrame),
                        );
                        ui.label(if self.io_worker.is_cancelling() {
                            tr(self.locale, UiText::CancellingFrameExport)
                        } else {
                            tr(self.locale, UiText::ExportingFrame)
                        });
                        if ui
                            .add_enabled(
                                self.io_worker.can_cancel(),
                                egui::Button::new(tr(self.locale, UiText::Cancel))
                                    .min_size(egui::vec2(MIN_POINTER_TARGET, MIN_POINTER_TARGET)),
                            )
                            .clicked()
                        {
                            self.cancel_file_operation();
                        }
                    });
                }
                let mut export_path = self.export_path.to_string_lossy().into_owned();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut export_path)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace),
                    )
                    .on_hover_text(tr(self.locale, UiText::FrameExportPath))
                    .changed()
                {
                    self.export_path = PathBuf::from(export_path);
                }

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 30.0],
                        egui::Button::new(format!("⚙  {}", tr(self.locale, UiText::Setup))),
                    )
                    .clicked()
                {
                    self.show_setup = true;
                }
                let extension_status = self.extension_status();
                ui.label(
                    egui::RichText::new(extension_status_text(self.locale, &extension_status))
                        .color(extension_status_color(&extension_status))
                        .small(),
                );
            });
    }
}

pub(super) const fn extension_status_color(status: &ExtensionActivationStatus) -> egui::Color32 {
    match status {
        ExtensionActivationStatus::Activated
        | ExtensionActivationStatus::WillCompleteAfterReboot => COLOR_OK,
        ExtensionActivationStatus::NeedsApproval => COLOR_WARNING,
        ExtensionActivationStatus::Failed(_) => COLOR_ERROR,
        ExtensionActivationStatus::Requesting => COLOR_ACCENT,
        ExtensionActivationStatus::Idle => COLOR_DIM,
    }
}

pub(super) fn extension_status_text(
    locale: UiLocale,
    status: &ExtensionActivationStatus,
) -> String {
    match status {
        ExtensionActivationStatus::Idle => String::from(tr(locale, UiText::ActivationNotRequested)),
        ExtensionActivationStatus::Requesting => {
            String::from(tr(locale, UiText::RequestingActivation))
        }
        ExtensionActivationStatus::NeedsApproval => {
            String::from(tr(locale, UiText::ApproveInSettings))
        }
        ExtensionActivationStatus::Activated => {
            String::from(tr(locale, UiText::ExtensionActivated))
        }
        ExtensionActivationStatus::WillCompleteAfterReboot => {
            String::from(tr(locale, UiText::ActivatesAfterReboot))
        }
        ExtensionActivationStatus::Failed(message) => {
            format!("{}: {message}", tr(locale, UiText::ActivationFailed))
        }
    }
}

fn layout_label(locale: UiLocale, layout: CompositionLayout) -> &'static str {
    match layout {
        CompositionLayout::Grid => tr(locale, UiText::Grid),
        CompositionLayout::Row => tr(locale, UiText::Row),
        CompositionLayout::Column => tr(locale, UiText::Column),
        CompositionLayout::PictureInPicture => "PiP",
    }
}

fn missing_policy_label(locale: UiLocale, policy: MissingSourcePolicy) -> &'static str {
    match policy {
        MissingSourcePolicy::Placeholder => tr(locale, UiText::Placeholder),
        MissingSourcePolicy::FreezeBriefly => tr(locale, UiText::FreezeBriefly),
        MissingSourcePolicy::HideCell => tr(locale, UiText::HideCell),
        MissingSourcePolicy::StopOutput => tr(locale, UiText::StopOutput),
    }
}
