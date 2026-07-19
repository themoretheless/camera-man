use super::ui_output::{extension_status_color, extension_status_text};
use super::*;

impl CameraManApp {
    pub(super) fn show_setup_window(&mut self, ctx: &egui::Context) {
        if self.force_setup_fixture {
            self.show_setup = true;
        }
        if !self.show_setup {
            return;
        }
        let mut open = self.show_setup;
        let window = egui::Window::new(tr(self.locale, UiText::Setup))
            .open(&mut open)
            .default_width(520.0)
            .default_height(500.0)
            .min_width(420.0)
            .min_height(420.0)
            .collapsible(false)
            .resizable(true);
        let window = if self.force_setup_fixture {
            window.anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        } else {
            window
        };
        window.show(ctx, |ui| self.setup_window_ui(ui, ctx));
        self.show_setup = open;
    }

    fn setup_window_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .max_height(620.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.label(tr(self.locale, UiText::Language));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.locale, UiLocale::English, "English");
                    ui.selectable_value(&mut self.locale, UiLocale::Russian, "Русский");
                });
                if ui
                    .checkbox(
                        &mut self.high_contrast,
                        tr(self.locale, UiText::HighContrast),
                    )
                    .changed()
                {
                    configure_style(ctx, self.high_contrast || self.system_increase_contrast);
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                self.scene_file_ui(ui, ctx);

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                setup_section_heading(ui, tr(self.locale, UiText::SystemExtension));
                self.activation_assistant_ui(ui);

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                setup_section_heading(ui, tr(self.locale, UiText::VirtualCameraSelfTest));
                let test_running = matches!(
                    self.virtual_camera_self_test,
                    VirtualCameraSelfTest::Running { .. }
                );
                if ui
                    .add_enabled(
                        !self.running && !test_running,
                        egui::Button::new(if test_running {
                            tr(self.locale, UiText::Testing)
                        } else {
                            tr(self.locale, UiText::RunSelfTest)
                        })
                        .min_size(egui::vec2(150.0, 30.0)),
                    )
                    .on_disabled_hover_text(tr(self.locale, UiText::StopPreviewBeforeTesting))
                    .clicked()
                {
                    self.start_virtual_camera_self_test(ctx);
                }
                let mut cancel_self_test = false;
                match &self.virtual_camera_self_test {
                    VirtualCameraSelfTest::Idle => {
                        ui.label(
                            egui::RichText::new(tr(self.locale, UiText::NotRun)).color(COLOR_DIM),
                        );
                    }
                    VirtualCameraSelfTest::Running { target, .. } => {
                        ui.horizontal(|ui| {
                            progress_indicator(
                                ui,
                                self.reduce_motion,
                                tr(self.locale, UiText::Testing),
                            );
                            ui.label(match target {
                                Some(target) => format!(
                                    "{}: {} {}, {} {}",
                                    tr(self.locale, UiText::WaitingForConsumer),
                                    tr(self.locale, UiText::Generation),
                                    target.generation,
                                    tr(self.locale, UiText::Sequence),
                                    target.sequence
                                ),
                                None => {
                                    String::from(tr(self.locale, UiText::PublishingTestPattern))
                                }
                            });
                            if ui
                                .add_sized(
                                    [80.0, MIN_POINTER_TARGET],
                                    egui::Button::new(tr(self.locale, UiText::Cancel)),
                                )
                                .clicked()
                            {
                                cancel_self_test = true;
                            }
                        });
                    }
                    VirtualCameraSelfTest::Passed { consumer } => {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: PID {}, {} {}, {} {}",
                                tr(self.locale, UiText::SelfTestPassed),
                                consumer.pid,
                                tr(self.locale, UiText::Generation),
                                consumer.generation,
                                tr(self.locale, UiText::Sequence),
                                consumer.sequence
                            ))
                            .color(COLOR_OK),
                        );
                    }
                    VirtualCameraSelfTest::Failed(error) => {
                        ui.label(egui::RichText::new(error).color(COLOR_ERROR));
                    }
                }
                if cancel_self_test {
                    self.cancel_virtual_camera_self_test(ctx);
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                setup_section_heading(ui, tr(self.locale, UiText::Diagnostics));
                ui.monospace(format!(
                    "{}x{} BGRA @ {} fps",
                    VIRTUAL_CAMERA_WIDTH, VIRTUAL_CAMERA_HEIGHT, self.active_fps
                ));
                let endpoint = self.virtual_endpoint.clone();
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&endpoint).monospace()).wrap());
                    if ui
                        .add_sized(
                            [100.0, 28.0],
                            egui::Button::new(tr(self.locale, UiText::Copy)),
                        )
                        .on_hover_text(tr(self.locale, UiText::CopyTransportEndpoint))
                        .clicked()
                    {
                        ctx.copy_text(endpoint.clone());
                        self.set_event(tr(self.locale, UiText::TransportEndpointCopied), false);
                    }
                });
                if ui
                    .add_sized(
                        [190.0, 28.0],
                        egui::Button::new(tr(self.locale, UiText::CopyDiagnostics)),
                    )
                    .clicked()
                {
                    ctx.copy_text(self.diagnostics_summary());
                    self.set_event(tr(self.locale, UiText::DiagnosticsCopied), false);
                }
                if ui
                    .add_enabled(
                        !self.io_worker.is_running(),
                        egui::Button::new(tr(self.locale, UiText::ExportRedactedDiagnostics))
                            .min_size(egui::vec2(220.0, 28.0)),
                    )
                    .clicked()
                {
                    self.export_diagnostics();
                }
                ui.monospace(self.diagnostics_path.display().to_string());
                if self.io_worker.operation() == Some(IoOperation::DiagnosticsExport) {
                    ui.horizontal(|ui| {
                        progress_indicator(
                            ui,
                            self.reduce_motion,
                            tr(self.locale, UiText::ExportingDiagnostics),
                        );
                        ui.label(if self.io_worker.is_cancelling() {
                            tr(self.locale, UiText::CancellingDiagnosticsExport)
                        } else {
                            tr(self.locale, UiText::ExportingDiagnostics)
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
            });
    }

    fn activation_assistant_ui(&mut self, ui: &mut egui::Ui) {
        let extension_status = self.extension_status();
        ui.horizontal(|ui| {
            if extension_status == ExtensionActivationStatus::Requesting {
                progress_indicator(
                    ui,
                    self.reduce_motion,
                    tr(self.locale, UiText::RequestingActivation),
                );
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(extension_status_text(self.locale, &extension_status))
                        .color(extension_status_color(&extension_status)),
                )
                .wrap(),
            );
        });
        let can_request = self.extension_capable && self.extension_installer.can_activate();
        let button_label = match extension_status {
            ExtensionActivationStatus::Idle => tr(self.locale, UiText::InstallExtension),
            ExtensionActivationStatus::Requesting => tr(self.locale, UiText::Requesting),
            ExtensionActivationStatus::NeedsApproval => tr(self.locale, UiText::AwaitingApproval),
            ExtensionActivationStatus::Activated => tr(self.locale, UiText::Installed),
            ExtensionActivationStatus::WillCompleteAfterReboot => {
                tr(self.locale, UiText::RestartRequired)
            }
            ExtensionActivationStatus::Failed(_) => tr(self.locale, UiText::RetryActivation),
        };
        if ui
            .add_enabled(
                can_request,
                egui::Button::new(button_label)
                    .min_size(egui::vec2(ui.available_width().min(240.0), 32.0)),
            )
            .on_disabled_hover_text(if self.extension_capable {
                tr(self.locale, UiText::CurrentRequestMustFinish)
            } else {
                tr(self.locale, UiText::InstallSignedBundleFirst)
            })
            .clicked()
        {
            self.extension_status_override = None;
            self.extension_installer.activate();
        }
        if extension_status == ExtensionActivationStatus::NeedsApproval {
            ui.hyperlink_to(
                tr(self.locale, UiText::OpenApprovalInstructions),
                "https://support.apple.com/guide/mac-help/change-login-items-extension-settings-mtusr003/mac",
            );
        }
        ui.add_space(8.0);

        assistant_step(
            ui,
            matches!(self.launch_context, AppLaunchContext::ApplicationsBundle),
            format!(
                "{}: {}",
                tr(self.locale, UiText::ApplicationLocation),
                launch_context_label(self.locale, self.launch_context)
            ),
        );
        assistant_step(
            ui,
            self.signing_team_identifier.is_some(),
            format!(
                "{}: {}",
                tr(self.locale, UiText::SigningTeamId),
                self.signing_team_identifier
                    .as_deref()
                    .unwrap_or(tr(self.locale, UiText::Unavailable))
            ),
        );
        assistant_step(
            ui,
            self.extension_profile_present,
            format!(
                "{}: {}",
                tr(self.locale, UiText::ExtensionProfile),
                if self.extension_profile_present {
                    tr(self.locale, UiText::Embedded)
                } else {
                    tr(self.locale, UiText::MissingValue)
                }
            ),
        );
        let (profile_ok, profile_text, profile_details) = match &self.provisioning_profile_status {
            ProvisioningProfileStatus::Missing => (
                false,
                String::from(tr(self.locale, UiText::HostProfileMissing)),
                String::from(tr(self.locale, UiText::ProductionProfileHint)),
            ),
            ProvisioningProfileStatus::Valid {
                path,
                team_identifier,
            } => (
                true,
                String::from(tr(self.locale, UiText::HostProfileValid)),
                format!(
                    "{}\n{}: {}",
                    path.display(),
                    tr(self.locale, UiText::SigningTeamId),
                    team_identifier
                        .as_deref()
                        .unwrap_or(tr(self.locale, UiText::Unavailable))
                ),
            ),
            ProvisioningProfileStatus::Invalid { path, error } => (
                false,
                String::from(tr(self.locale, UiText::HostProfileInvalid)),
                format!("{}: {error}", path.display()),
            ),
        };
        assistant_step(ui, profile_ok, profile_text).on_hover_text(profile_details);
        assistant_step(
            ui,
            self.extension_capable,
            if self.extension_capable {
                String::from(tr(self.locale, UiText::ActivationCapabilityReady))
            } else {
                format!(
                    "{}: {}",
                    tr(self.locale, UiText::ActivationCapability),
                    self.extension_capability_reason
                        .as_deref()
                        .unwrap_or(tr(self.locale, UiText::Unavailable))
                )
            },
        );

        ui.hyperlink_to(
            tr(self.locale, UiText::SigningTroubleshooting),
            "https://github.com/themoretheless/camera-man#troubleshooting-no-matching-profile-found",
        );
    }
}

fn launch_context_label(locale: UiLocale, context: AppLaunchContext) -> &'static str {
    match context {
        AppLaunchContext::BareExecutable => tr(locale, UiText::BareExecutable),
        AppLaunchContext::DevelopmentBundle => tr(locale, UiText::DevelopmentBundle),
        AppLaunchContext::ApplicationsBundle => tr(locale, UiText::ApplicationsBundle),
    }
}

fn assistant_step(
    ui: &mut egui::Ui,
    complete: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    let icon = if complete { "✓" } else { "!" };
    let color = if complete { COLOR_OK } else { COLOR_WARNING };
    ui.horizontal(|ui| {
        ui.add_sized(
            [22.0, 22.0],
            egui::Label::new(egui::RichText::new(icon).strong().color(color)),
        );
        ui.label(text)
    })
    .inner
}

fn setup_section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong().size(16.0));
}
