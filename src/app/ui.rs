use std::collections::HashSet;

use super::*;

impl eframe::App for CameraManApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        COLOR_WINDOW.to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let preferences = self.preferences();
        let preferences_path = self.preferences_path.clone();
        if let Err(error) = preferences.store(storage, preferences_path.as_deref()) {
            self.set_event(
                format!("Could not save preferences: {error}. Previous settings were preserved."),
                true,
            );
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_camera_discovery(ctx);
        self.poll_render_result(ctx);
        self.poll_file_operation();
        self.drive_virtual_camera_self_test(ctx);

        if self.running && self.fixture_state.is_none() {
            if self.media_clock.take_tick() {
                self.request_render(ctx, true, false);
            }
        } else if self.notice.as_ref().is_some_and(StatusEvent::is_visible) {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        if self.render_worker.has_work() {
            ctx.request_repaint_after(WORKER_POLL_INTERVAL);
        }
        if self.camera_discovery.is_running() {
            ctx.request_repaint_after(CAMERA_DISCOVERY_POLL_INTERVAL);
        }
        if self.io_worker.is_running() {
            ctx.request_repaint_after(WORKER_POLL_INTERVAL);
        }
        if matches!(
            self.extension_status(),
            ExtensionActivationStatus::Requesting | ExtensionActivationStatus::NeedsApproval
        ) {
            ctx.request_repaint_after(Duration::from_millis(500));
        }

        self.drive_ui_fixture_capture(ctx);
        self.handle_keyboard_shortcuts(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let before = self.scene_snapshot();

        egui::Panel::bottom("camera-man-status")
            .exact_size(STATUS_BAR_HEIGHT)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .fill(COLOR_PANEL),
            )
            .show(ui, |ui| self.status_ui(ui));

        egui::Panel::left("camera-man-sources")
            .default_size(SOURCES_PANEL_DEFAULT_WIDTH)
            .size_range(SOURCES_PANEL_MIN_WIDTH..=SOURCES_PANEL_MAX_WIDTH)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .fill(COLOR_PANEL),
            )
            .show(ui, |ui| self.sources_panel_ui(ui, &ctx));

        egui::Panel::right("camera-man-output")
            .default_size(OUTPUT_PANEL_DEFAULT_WIDTH)
            .size_range(OUTPUT_PANEL_MIN_WIDTH..=OUTPUT_PANEL_MAX_WIDTH)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .fill(COLOR_PANEL),
            )
            .show(ui, |ui| self.output_panel_ui(ui, &ctx));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(CENTRAL_HORIZONTAL_MARGIN as i8)
                    .fill(COLOR_WINDOW),
            )
            .show(ui, |ui| self.preview_ui(ui));

        self.show_setup_window(&ctx);
        let pointer_down = ctx.input(|input| input.pointer.any_down());
        self.finish_scene_history(before, pointer_down);
    }
}

impl CameraManApp {
    fn drive_ui_fixture_capture(&mut self, ctx: &egui::Context) {
        let screenshot = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot {
                    viewport_id, image, ..
                } if *viewport_id == egui::ViewportId::ROOT => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(screenshot) = screenshot {
            let Some(capture) = self.fixture_capture.take() else {
                return;
            };
            let path = capture.path;
            save_ui_fixture(&path, &screenshot, capture.target_size).unwrap_or_else(|error| {
                panic!("could not save UI fixture {}: {error}", path.display())
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let Some(capture) = self.fixture_capture.as_mut() else {
            return;
        };
        if capture.settle_frames > 0 {
            capture.settle_frames -= 1;
            ctx.request_repaint();
        } else if !capture.requested {
            capture.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            ctx.request_repaint();
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() || ctx.memory(|memory| memory.focused().is_some()) {
            return;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Space)) {
            self.toggle_running(ctx);
        }

        let command_z =
            ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Z));
        if command_z {
            let redo = ctx.input(|input| input.modifiers.shift);
            let restored = if redo {
                self.redo_scene_edit()
            } else {
                self.undo_scene_edit()
            };
            if restored {
                self.refresh_scene_render(ctx, SceneChange::SceneRestore);
            }
            return;
        }

        let direction = ctx.input(|input| {
            if !input.modifiers.command {
                None
            } else if input.key_pressed(egui::Key::ArrowUp) {
                Some(-1_isize)
            } else if input.key_pressed(egui::Key::ArrowDown) {
                Some(1_isize)
            } else {
                None
            }
        });
        let Some(direction) = direction else {
            return;
        };
        let ids = self.selected_source_ids();
        let Some(position) = self
            .selected_source_id
            .as_ref()
            .and_then(|id| ids.iter().position(|candidate| candidate == id))
        else {
            return;
        };
        let target = position
            .saturating_add_signed(direction)
            .min(ids.len().saturating_sub(1));
        let before = self.scene_snapshot();
        if let Some(change) = self.apply_scene_command(SceneEditCommand::MoveSource {
            from: position,
            to: target,
        }) {
            self.commit_scene_history(before);
            self.refresh_scene_render(ctx, change);
        }
    }

    fn sources_panel_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.add_space(8.0);
                ui.heading("CameraMan");
                ui.add_space(10.0);
                self.scene_selector_ui(ui, ctx);

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(tr(self.locale, UiText::Sources));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(self.locale, UiText::Synthetic))
                        .small()
                        .color(COLOR_DIM),
                );
                self.synthetic_sources_ui(ui, ctx);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr(self.locale, UiText::Real))
                        .small()
                        .color(COLOR_DIM),
                );
                self.real_sources_ui(ui, ctx);
                self.transform_inspector_ui(ui, ctx);
            });
    }

    fn synthetic_sources_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let selected_ids = self.selected_source_ids();
        let mut scene_change = None;
        let mut reorder = None;
        for index in 0..self.sources.len() {
            let id = self.sources[index].id.to_owned();
            let name = self.sources[index].name;
            let bgra = self.sources[index].bgra;
            let descriptor = SourceDescriptor::synthetic(&id);
            let stable_key = descriptor.stable_key();
            let mut checked = self.selected_sources.contains(&descriptor);
            let order = selected_ids
                .iter()
                .position(|selected| selected == &stable_key);
            let selected = self.selected_source_id.as_deref() == Some(stable_key.as_str());
            ui.push_id(("source", &id), |ui| {
                ui.horizontal(|ui| {
                    let (swatch, _) = ui.allocate_exact_size(
                        egui::vec2(14.0, MIN_POINTER_TARGET),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        egui::Rect::from_center_size(swatch.center(), egui::vec2(12.0, 12.0)),
                        3.0,
                        egui::Color32::from_rgb(bgra[2], bgra[1], bgra[0]),
                    );
                    let selection_label =
                        format!("{} {name}", tr(self.locale, UiText::SelectSource));
                    let selection = ui.add(egui::Checkbox::without_text(&mut checked));
                    selection.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            true,
                            checked,
                            &selection_label,
                        )
                    });
                    selection.on_hover_text(&selection_label);
                    let name_width = (ui.available_width() - 88.0).max(72.0);
                    if ui
                        .add_sized(
                            [name_width, MIN_POINTER_TARGET],
                            egui::Button::new(name).selected(selected).truncate(),
                        )
                        .on_hover_text(name)
                        .clicked()
                    {
                        self.selected_source_id = Some(stable_key.clone());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        health_label(
                            ui,
                            SourceHealthVector::synthetic(self.running && checked),
                            self.locale,
                        );
                        if let Some(position) = order {
                            ui.label(
                                egui::RichText::new(format!("#{}", position + 1))
                                    .small()
                                    .color(COLOR_DIM),
                            );
                        }
                    });
                });
                if selected && let Some(position) = order {
                    ui.horizontal(|ui| {
                        ui.add_space(42.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            reorder_buttons(
                                ui,
                                position,
                                selected_ids.len(),
                                self.locale,
                                &mut reorder,
                            );
                        });
                    });
                }
            });
            if checked != self.selected_sources.contains(&descriptor)
                && let Some(change) =
                    self.apply_scene_command(SceneEditCommand::SetSourceSelected {
                        source: descriptor,
                        selected: checked,
                    })
            {
                scene_change = Some(change);
            }
        }
        if let Some((from, to)) = reorder
            && let Some(change) =
                self.apply_scene_command(SceneEditCommand::MoveSource { from, to })
            && scene_change.is_none()
        {
            scene_change = Some(change);
        }
        if let Some(change) = scene_change {
            self.refresh_scene_render(ctx, change);
        }
    }

    fn real_sources_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.camera_discovery.is_running() {
            ui.horizontal(|ui| {
                progress_indicator(
                    ui,
                    self.reduce_motion,
                    tr(self.locale, UiText::DiscoveringCameras),
                );
                ui.label(
                    egui::RichText::new(tr(self.locale, UiText::DiscoveringCameras))
                        .color(COLOR_DIM),
                );
                if ui
                    .add_sized(
                        [72.0, MIN_POINTER_TARGET],
                        egui::Button::new(tr(self.locale, UiText::Cancel)),
                    )
                    .clicked()
                {
                    self.cancel_camera_discovery();
                }
            });
        } else if ui
            .add_sized(
                [ui.available_width(), 30.0],
                egui::Button::new(tr(self.locale, UiText::RefreshCameras)),
            )
            .clicked()
        {
            self.start_camera_discovery(true);
            ctx.request_repaint_after(CAMERA_DISCOVERY_POLL_INTERVAL);
        }

        if self.real_devices.is_empty() && !self.camera_discovery.is_running() {
            ui.label(egui::RichText::new(tr(self.locale, UiText::NoCameras)).color(COLOR_DIM));
        }

        let devices = self.real_devices.clone();
        let selected_ids = self.selected_source_ids();
        let mut selection_changed = false;
        let mut reorder = None;
        let mut retry_id = None;
        for device in devices {
            let descriptor = SourceDescriptor::camera(&device.id);
            let stable_key = descriptor.stable_key();
            let mut checked = self.selected_sources.contains(&descriptor);
            let order = selected_ids.iter().position(|id| id == &stable_key);
            let runtime = self
                .real_sources
                .iter()
                .find(|(id, _, _)| camera_locators_match(id, &device.id, &self.real_devices))
                .map(|(_, source, streak)| RealSourceRuntime {
                    failure_streak: *streak,
                    negotiated_format: source.negotiated_format(),
                    frame_gap_exceeded: source.frame_gap_exceeded(),
                });
            let health = real_source_health(checked, self.running, runtime);
            let selected = self.selected_source_id.as_deref() == Some(stable_key.as_str());
            ui.push_id(("real-source", &device.id), |ui| {
                ui.horizontal(|ui| {
                    let selection_label =
                        format!("{} {}", tr(self.locale, UiText::SelectSource), device.name);
                    let selection = ui.add(egui::Checkbox::without_text(&mut checked));
                    selection.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            true,
                            checked,
                            &selection_label,
                        )
                    });
                    selection.on_hover_text(&selection_label);
                    let name_width = (ui.available_width() - 108.0).max(72.0);
                    if ui
                        .add_sized(
                            [name_width, MIN_POINTER_TARGET],
                            egui::Button::new(&device.name)
                                .selected(selected)
                                .truncate(),
                        )
                        .on_hover_text(&device.name)
                        .clicked()
                    {
                        self.selected_source_id = Some(stable_key.clone());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        health_label(ui, health, self.locale);
                        if let Some(position) = order {
                            ui.label(
                                egui::RichText::new(format!("#{}", position + 1))
                                    .small()
                                    .color(COLOR_DIM),
                            );
                        }
                    });
                });
                if (selected && order.is_some()) || health.summary() == SourceHealthSummary::Retry {
                    ui.horizontal(|ui| {
                        ui.add_space(34.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if health.summary() == SourceHealthSummary::Retry {
                                let retry = ui.add_sized(
                                    [MIN_POINTER_TARGET, MIN_POINTER_TARGET],
                                    egui::Button::new("↻"),
                                );
                                let retry_label = tr(self.locale, UiText::RetryCamera);
                                retry.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        retry_label,
                                    )
                                });
                                if retry.on_hover_text(retry_label).clicked() {
                                    retry_id = Some(device.id.clone());
                                }
                            }
                            if selected && let Some(position) = order {
                                reorder_buttons(
                                    ui,
                                    position,
                                    selected_ids.len(),
                                    self.locale,
                                    &mut reorder,
                                );
                            }
                        });
                    });
                }
                if let Some(format) = runtime.and_then(|runtime| runtime.negotiated_format) {
                    ui.label(
                        egui::RichText::new(format!(
                            "    {}x{} · {} fps · decoded BGRA",
                            format.width, format.height, format.fps
                        ))
                        .small()
                        .color(COLOR_DIM),
                    );
                }
            });
            if checked != self.selected_sources.contains(&descriptor)
                && self
                    .apply_scene_command(SceneEditCommand::SetSourceSelected {
                        source: descriptor,
                        selected: checked,
                    })
                    .is_some()
            {
                selection_changed = true;
            }
        }

        let known = self
            .real_devices
            .iter()
            .map(|device| device.id.as_str())
            .collect::<HashSet<_>>();
        let missing_cameras = self
            .selected_sources
            .iter()
            .filter(|source| source.kind.is_camera() && !known.contains(source.locator.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for descriptor in missing_cameras {
            let stable_key = descriptor.stable_key();
            let id = descriptor.locator.clone();
            let order = selected_ids.iter().position(|id| id == &stable_key);
            let selected = self.selected_source_id.as_deref() == Some(stable_key.as_str());
            let mut checked = true;
            ui.push_id(("missing-source", &id), |ui| {
                ui.horizontal(|ui| {
                    let selection_label = format!("{} {id}", tr(self.locale, UiText::SelectSource));
                    let selection = ui.add(egui::Checkbox::without_text(&mut checked));
                    selection.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            true,
                            checked,
                            &selection_label,
                        )
                    });
                    selection.on_hover_text(&selection_label);
                    let name_width = (ui.available_width() - 108.0).max(72.0);
                    if ui
                        .add_sized(
                            [name_width, MIN_POINTER_TARGET],
                            egui::Button::new(&id).selected(selected).truncate(),
                        )
                        .on_hover_text(&id)
                        .clicked()
                    {
                        self.selected_source_id = Some(stable_key.clone());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        health_label(ui, SourceHealthVector::missing(), self.locale);
                        if let Some(position) = order {
                            ui.label(
                                egui::RichText::new(format!("#{}", position + 1))
                                    .small()
                                    .color(COLOR_DIM),
                            );
                        }
                    });
                });
                if selected && let Some(position) = order {
                    ui.horizontal(|ui| {
                        ui.add_space(34.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            reorder_buttons(
                                ui,
                                position,
                                selected_ids.len(),
                                self.locale,
                                &mut reorder,
                            );
                        });
                    });
                }
            });
            if !checked
                && self
                    .apply_scene_command(SceneEditCommand::SetSourceSelected {
                        source: descriptor,
                        selected: false,
                    })
                    .is_some()
            {
                selection_changed = true;
            }
        }

        if let Some((from, to)) = reorder {
            selection_changed |= self
                .apply_scene_command(SceneEditCommand::MoveSource { from, to })
                .is_some();
        }
        if let Some(id) = retry_id {
            self.retry_real_source(&id);
            selection_changed = true;
        }
        if selection_changed {
            self.refresh_scene_render(ctx, SceneChange::SourceTopology);
        }
    }

    fn preview_ui(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let target_ratio = VIRTUAL_CAMERA_WIDTH as f32 / VIRTUAL_CAMERA_HEIGHT as f32;
        let mut preview_size = egui::vec2(available.x, available.x / target_ratio);
        if preview_size.y > available.y {
            preview_size.y = available.y;
            preview_size.x = available.y * target_ratio;
        }
        preview_size.x = preview_size.x.max(1.0);
        preview_size.y = preview_size.y.max(1.0);

        ui.add_space(((available.y - preview_size.y) / 2.0).max(0.0));
        ui.horizontal_centered(|ui| {
            let (rect, response) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
            let (overlay_headline, overlay_recovery) = waiting_overlay_text(
                self.camera_not_responding,
                self.fixture_state == Some(UiFixtureState::Disconnected),
            );
            // The accessible name and the painted overlay must agree: a
            // not-responding camera must not be announced as a warm-up.
            let preview_state = if self.waiting_for_camera {
                tr(self.locale, overlay_headline)
            } else if self.running {
                tr(self.locale, UiText::PreviewRunning)
            } else if self.preview_texture.is_some() {
                tr(self.locale, UiText::Paused)
            } else {
                tr(self.locale, UiText::NoPreview)
            };
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Image, true, preview_state)
            });
            ui.painter()
                .rect_filled(rect, 4.0, COLOR_PREVIEW_BACKGROUND);
            if let Some(texture) = &self.preview_texture {
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                self.paint_preview_badges(ui, rect);
            } else if !self.waiting_for_camera {
                let empty = self.selected_count() == 0;
                paint_centered_message(
                    ui,
                    rect,
                    if empty {
                        tr(self.locale, UiText::NoSourcesSelected)
                    } else {
                        tr(self.locale, UiText::NoPreview)
                    },
                    empty.then(|| tr(self.locale, UiText::SelectSourceToBegin)),
                    COLOR_DIM,
                );
            }
            if self.waiting_for_camera {
                ui.painter()
                    .rect_filled(rect, 4.0, egui::Color32::from_black_alpha(150));
                paint_centered_message(
                    ui,
                    rect,
                    tr(self.locale, overlay_headline),
                    overlay_recovery.map(|text| tr(self.locale, text)),
                    COLOR_WARNING,
                );
            }
        });
    }

    fn paint_preview_badges(&self, ui: &egui::Ui, rect: egui::Rect) {
        let painter = ui.painter();
        let has_camera = self
            .selected_sources
            .iter()
            .any(|source| source.kind.is_camera());
        let has_synthetic = self
            .selected_sources
            .iter()
            .any(|source| source.kind.is_synthetic());
        let mode = match (has_synthetic, has_camera) {
            (true, true) => String::from("MIXED"),
            (false, true) => tr(self.locale, UiText::Real).to_uppercase(),
            _ => tr(self.locale, UiText::Synthetic).to_uppercase(),
        };
        let state = if self.running {
            tr(self.locale, UiText::Live)
        } else {
            tr(self.locale, UiText::Paused)
        };
        let text = format!("{state} · {mode}");
        let font = egui::FontId::proportional(12.0);
        let padding = egui::vec2(8.0, 4.0);
        let max_text_width = (rect.width() - padding.x * 2.0 - 34.0).max(1.0);
        let galley = painter.layout(text, font, egui::Color32::WHITE, max_text_width);
        let badge = egui::Rect::from_min_size(
            rect.min + egui::vec2(10.0, 10.0),
            galley.size() + padding * 2.0 + egui::vec2(12.0, 0.0),
        );
        painter.rect_filled(badge, 4.0, egui::Color32::from_black_alpha(180));
        painter.circle_filled(
            egui::pos2(badge.min.x + padding.x + 3.0, badge.center().y),
            3.0,
            if self.running { COLOR_ERROR } else { COLOR_DIM },
        );
        painter.galley(
            egui::pos2(badge.min.x + padding.x + 12.0, badge.min.y + padding.y),
            galley,
            egui::Color32::WHITE,
        );
    }

    fn status_ui(&mut self, ui: &mut egui::Ui) {
        let (color, text) = self.state_line();
        let drops = camera_man::drop_counters().snapshot();
        let total_drops = drops.source_missing
            + drops.late
            + drops.queue_replacement
            + drops.all_slots_busy
            + drops.stale
            + drops.pool_exhausted
            + drops.discontinuity;
        let latency = camera_man::latency_histograms().snapshot();
        let p95 = |stage| {
            latency
                .iter()
                .find(|sample| sample.stage == stage)
                .map_or(0.0, |sample| sample.p95_nanos as f64 / 1_000_000.0)
        };
        let compose_p95 = p95(PipelineStage::Compose);
        let publish_p95 = p95(PipelineStage::Publish);
        let details = format!(
            "compose p95 {compose_p95:.2} ms\npublish p95 {publish_p95:.2} ms\n\
             source missing {}\nlate {}\nqueue replacement {}\nall slots busy {}\nstale {}\n\
             pool exhausted {}\ndiscontinuity {}",
            drops.source_missing,
            drops.late,
            drops.queue_replacement,
            drops.all_slots_busy,
            drops.stale,
            drops.pool_exhausted,
            drops.discontinuity,
        );

        ui.horizontal(|ui| {
            let (dot, status_icon) =
                ui.allocate_exact_size(egui::vec2(10.0, MIN_POINTER_TARGET), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            status_icon
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &text));
            if self.sticky_error.is_some() {
                let dismiss_label = tr(self.locale, UiText::DismissError);
                let dismiss = ui.add_sized(
                    [MIN_POINTER_TARGET, MIN_POINTER_TARGET],
                    egui::Button::new("×"),
                );
                dismiss.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, dismiss_label)
                });
                if dismiss.on_hover_text(dismiss_label).clicked() {
                    self.sticky_error = None;
                }
            }
            ui.add(
                egui::Label::new(egui::RichText::new(&text).color(color))
                    .truncate()
                    .selectable(false),
            )
            .on_hover_text(&text);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("drops {total_drops}"))
                    .on_hover_text(&details);
                ui.separator();
                ui.label(format!("p95 {compose_p95:.1}/{publish_p95:.1} ms"))
                    .on_hover_text(&details);
                if self.missing_source_count > 0 {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!("missing {}", self.missing_source_count))
                            .color(COLOR_WARNING),
                    )
                    .on_hover_text(tr(self.locale, UiText::SourceMissingOrStale));
                }
                if self.running && self.measured_fps > 0.0 {
                    ui.separator();
                    ui.label(format!("{:.0} fps", self.measured_fps));
                }
            });
        });
    }
}

/// What one open camera's capture source currently reports about itself.
#[derive(Clone, Copy)]
pub(super) struct RealSourceRuntime {
    pub(super) failure_streak: u32,
    pub(super) negotiated_format: Option<VideoFormat>,
    pub(super) frame_gap_exceeded: bool,
}

/// Health for one real camera row. A read that hung after frames had arrived is
/// Stale like any other failure, but it must not claim a reconnect is running:
/// the worker is parked inside the backend and can reopen nothing. Arm order is
/// load-bearing, because a stalled read also drives the failure streak above
/// zero and would otherwise be described as `retrying`.
pub(super) fn real_source_health(
    checked: bool,
    running: bool,
    runtime: Option<RealSourceRuntime>,
) -> SourceHealthVector {
    match (checked, running, runtime) {
        (false, _, _) => SourceHealthVector::off(),
        (true, true, Some(runtime)) if runtime.frame_gap_exceeded => SourceHealthVector::stalled(),
        (true, true, Some(runtime)) if runtime.failure_streak > 0 => {
            SourceHealthVector::retrying(u64::from(runtime.failure_streak))
        }
        (
            true,
            true,
            Some(RealSourceRuntime {
                failure_streak: 0,
                negotiated_format: Some(_),
                ..
            }),
        ) => SourceHealthVector::connected(),
        (true, true, _) => SourceHealthVector::waiting(),
        (true, false, _) => SourceHealthVector::off(),
    }
}

/// Wording for every surface that speaks while no camera frame is on screen:
/// the painted overlay, the preview's accessible name and the status line. A
/// camera whose open never returned must not read as a warm-up on any of them.
pub(super) fn waiting_overlay_text(
    not_responding: bool,
    disconnected: bool,
) -> (UiText, Option<UiText>) {
    if not_responding {
        return (UiText::CameraNotResponding, Some(UiText::RetryCamera));
    }
    if disconnected {
        return (UiText::CameraDisconnected, Some(UiText::ReconnectCamera));
    }
    (UiText::WaitingForCamera, None)
}

fn paint_centered_message(
    ui: &egui::Ui,
    rect: egui::Rect,
    title: &str,
    detail: Option<&str>,
    color: egui::Color32,
) {
    let text = detail.map_or_else(|| title.to_owned(), |detail| format!("{title}\n{detail}"));
    let wrap_width = (rect.width() - 24.0).max(1.0);
    let galley = ui
        .painter()
        .layout(text, egui::FontId::proportional(14.0), color, wrap_width);
    let position = rect.center() - galley.size() / 2.0;
    ui.painter().galley(position, galley, color);
}

fn save_ui_fixture(
    path: &Path,
    screenshot: &egui::ColorImage,
    target_size: [u32; 2],
) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let width = u32::try_from(screenshot.size[0])
        .map_err(|_| String::from("fixture width exceeds PNG limits"))?;
    let height = u32::try_from(screenshot.size[1])
        .map_err(|_| String::from("fixture height exceeds PNG limits"))?;
    let mut rgba = Vec::with_capacity(screenshot.pixels.len() * 4);
    for pixel in &screenshot.pixels {
        rgba.extend_from_slice(&pixel.to_array());
    }
    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| String::from("fixture pixels do not match its dimensions"))?;
    let image = if [width, height] == target_size {
        image
    } else {
        image::imageops::resize(
            &image,
            target_size[0],
            target_size[1],
            image::imageops::FilterType::Lanczos3,
        )
    };
    image.save(path).map_err(|error| error.to_string())
}

fn health_label(ui: &mut egui::Ui, health: SourceHealthVector, locale: UiLocale) {
    let summary = health.summary();
    let (text, color) = match summary {
        SourceHealthSummary::Ready => (compact_status(locale, UiText::Ok), COLOR_OK),
        SourceHealthSummary::Waiting => (compact_status(locale, UiText::Wait), COLOR_WARNING),
        SourceHealthSummary::Retry => (compact_status(locale, UiText::Retry), COLOR_ERROR),
        SourceHealthSummary::Missing => (compact_status(locale, UiText::Missing), COLOR_ERROR),
        SourceHealthSummary::Off => (compact_status(locale, UiText::Off), COLOR_DIM),
    };
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(12.0, 18.0), egui::Sense::hover());
        let center = rect.center();
        match summary {
            SourceHealthSummary::Ready => {
                ui.painter().circle_filled(center, 4.0, color);
            }
            SourceHealthSummary::Waiting => {
                ui.painter()
                    .circle_stroke(center, 4.0, egui::Stroke::new(1.5, color));
                ui.painter().line_segment(
                    [center, center + egui::vec2(2.5, -2.5)],
                    egui::Stroke::new(1.5, color),
                );
            }
            SourceHealthSummary::Retry | SourceHealthSummary::Missing => {
                ui.painter().line_segment(
                    [
                        center + egui::vec2(0.0, -4.0),
                        center + egui::vec2(0.0, 1.0),
                    ],
                    egui::Stroke::new(1.8, color),
                );
                ui.painter()
                    .circle_filled(center + egui::vec2(0.0, 4.0), 1.2, color);
            }
            SourceHealthSummary::Off => {
                ui.painter()
                    .circle_stroke(center, 4.0, egui::Stroke::new(1.2, color));
            }
        }
        let health_text = format!(
            "{}: {text}\n{}",
            tr(locale, UiText::SourceHealth),
            health.description()
        );
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &health_text));
        response.on_hover_text(&health_text);
        ui.label(egui::RichText::new(text).small().color(color))
            .on_hover_text(health_text);
    });
}

fn compact_status(locale: UiLocale, text: UiText) -> &'static str {
    if locale == UiLocale::PseudoLong {
        tr(UiLocale::English, text)
    } else {
        tr(locale, text)
    }
}

fn reorder_buttons(
    ui: &mut egui::Ui,
    position: usize,
    count: usize,
    locale: UiLocale,
    action: &mut Option<(usize, usize)>,
) {
    let can_move_later = position + 1 < count;
    let move_later_label = tr(locale, UiText::MoveLater);
    let move_later = ui.add_enabled(
        can_move_later,
        egui::Button::new("v").min_size(egui::vec2(MIN_POINTER_TARGET, MIN_POINTER_TARGET)),
    );
    move_later.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, can_move_later, move_later_label)
    });
    if move_later
        .on_hover_text(format!("{move_later_label} (Command-Down)"))
        .clicked()
    {
        *action = Some((position, position + 1));
    }
    let can_move_earlier = position > 0;
    let move_earlier_label = tr(locale, UiText::MoveEarlier);
    let move_earlier = ui.add_enabled(
        can_move_earlier,
        egui::Button::new("^").min_size(egui::vec2(MIN_POINTER_TARGET, MIN_POINTER_TARGET)),
    );
    move_earlier.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            can_move_earlier,
            move_earlier_label,
        )
    });
    if move_earlier
        .on_hover_text(format!("{move_earlier_label} (Command-Up)"))
        .clicked()
    {
        *action = Some((position, position - 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_not_responding_camera_overrides_the_warm_up_overlay() {
        assert_eq!(
            waiting_overlay_text(true, false),
            (UiText::CameraNotResponding, Some(UiText::RetryCamera))
        );
        assert_eq!(
            waiting_overlay_text(true, true),
            (UiText::CameraNotResponding, Some(UiText::RetryCamera))
        );
        assert_eq!(
            waiting_overlay_text(false, true),
            (UiText::CameraDisconnected, Some(UiText::ReconnectCamera))
        );
        assert_eq!(
            waiting_overlay_text(false, false),
            (UiText::WaitingForCamera, None)
        );
    }

    const TEST_FORMAT: VideoFormat = VideoFormat {
        width: 1280,
        height: 720,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    };

    fn streaming_runtime(failure_streak: u32, frame_gap_exceeded: bool) -> RealSourceRuntime {
        RealSourceRuntime {
            failure_streak,
            negotiated_format: Some(TEST_FORMAT),
            frame_gap_exceeded,
        }
    }

    #[test]
    fn a_stalled_read_reports_stale_without_claiming_a_reconnect_is_running() {
        let health = real_source_health(true, true, Some(streaming_runtime(0, true)));

        assert_eq!(health.freshness, camera_man::FreshnessHealth::Stale);
        assert_eq!(health.reconnect, camera_man::ReconnectHealth::Connected);
        assert_eq!(health.summary(), SourceHealthSummary::Retry);
        assert!(health.description().contains("freshness=Stale"));
    }

    #[test]
    fn a_stalled_read_outranks_the_failure_streak_in_the_source_row() {
        assert_eq!(
            real_source_health(true, true, Some(streaming_runtime(3, true))),
            SourceHealthVector::stalled()
        );
    }

    #[test]
    fn a_running_camera_with_current_frames_still_reports_connected() {
        assert_eq!(
            real_source_health(true, true, Some(streaming_runtime(0, false))),
            SourceHealthVector::connected()
        );
    }
}
