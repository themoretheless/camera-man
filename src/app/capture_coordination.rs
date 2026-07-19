use super::*;

#[derive(Debug, Clone)]
pub(super) struct SourceSlot {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) bgra: [u8; 4],
    pub(super) selected: bool,
}

impl CameraManApp {
    pub(super) fn synthetic_frame(
        &self,
        source: &SourceSlot,
    ) -> Result<Option<CapturedFrame>, camera_man::CameraManError> {
        let bgra = animated_color(source.bgra, self.tick, self.reduce_motion);
        let mut frame_source =
            SyntheticFrameSource::with_id(source.id, SOURCE_WIDTH, SOURCE_HEIGHT, bgra);
        frame_source.latest_frame()
    }

    /// Returns the newest frame from every selected real camera, in
    /// selection order (so composition order matches check order, the same
    /// way Synthetic mode's cell order matches its fixed source list).
    ///
    /// Cameras are opened only when `allow_open` is true (Start pressed /
    /// already streaming), never as a side effect of switching modes or
    /// checking a box. A single failing camera degrades to `None` for that
    /// cell instead of aborting the whole composite (mirrors
    /// `PipelineEngine::render_once`); only when EVERY selected camera is
    /// currently failing the shared capture streak records one source-named
    /// error and eventually stops the preview, while the empty frame set keeps
    /// the waiting state explicit.
    ///
    /// Each source tracks its own consecutive-failure streak (the trailing
    /// `u32` in `real_sources`). The capture worker reconnects with bounded
    /// exponential backoff, while this coordinator keeps the source selected
    /// and exposes a stable RETRY state to the UI.
    pub(super) fn real_frames(&mut self, allow_open: bool) -> Vec<Option<CapturedFrame>> {
        // Release cameras that were unchecked (or disappeared) since the last tick.
        self.real_sources
            .retain(|(id, _, _)| self.selected_real_ids.contains(id));

        if allow_open {
            for id in &self.selected_real_ids {
                if !self.real_sources.iter().any(|(sid, _, _)| sid == id) {
                    self.real_sources
                        .push((id.clone(), ThreadedNokhwaFrameSource::open_id(id), 0));
                }
            }
        }

        let mut frames = Vec::with_capacity(self.selected_real_ids.len());
        let mut first_error = None;
        let mut ok_count = 0;
        // Collected during the loop and acted on after it: `real_sources` is
        // mutably borrowed by the loop, so `self.set_event` cannot be called
        // from inside it.
        let mut new_failures = Vec::new();
        let mut recovered_ids = Vec::new();
        for id in &self.selected_real_ids {
            let Some((_, source, streak)) =
                self.real_sources.iter_mut().find(|(sid, _, _)| sid == id)
            else {
                frames.push(None);
                continue;
            };
            match source.latest_frame() {
                Ok(frame) => {
                    if *streak > 0 && frame.is_some() {
                        recovered_ids.push(id.clone());
                    }
                    *streak = 0;
                    if frame.is_some() {
                        ok_count += 1;
                    }
                    frames.push(frame);
                }
                Err(error) => {
                    let source_name = self
                        .real_devices
                        .iter()
                        .find(|device| device.id == *id)
                        .map(|device| device.name.as_str())
                        .unwrap_or(id);
                    if *streak == 0 {
                        new_failures
                            .push(error.actionable_message(&format!("Read camera {source_name}")));
                    }
                    *streak = streak.saturating_add(1);
                    frames.push(None);
                    first_error.get_or_insert_with(|| (source_name.to_owned(), error));
                }
            }
        }

        if ok_count == 0 {
            if let Some((source_name, error)) = first_error {
                self.on_capture_error(
                    error.actionable_message(&format!("Read camera {source_name}")),
                );
            }
        } else {
            // Partial failure: at least one camera among several is broken.
            // Post each newly-failing camera's message once (not every tick)
            // so it can still expire, and let the working cameras keep
            // composing instead of aborting the whole preview.
            for message in new_failures {
                self.set_event(message, true);
            }
        }
        if !recovered_ids.is_empty() {
            self.capture_error_streak = 0;
            self.sticky_error = None;
            self.set_event(
                format!("{} camera(s) reconnected", recovered_ids.len()),
                false,
            );
        }

        frames
    }

    pub(super) fn retry_real_source(&mut self, id: &str) {
        self.real_sources
            .retain(|(source_id, _, _)| source_id != id);
        if self.running
            && self
                .selected_real_ids
                .iter()
                .any(|source_id| source_id == id)
        {
            self.real_sources
                .push((id.to_owned(), ThreadedNokhwaFrameSource::open_id(id), 0));
        }
        self.capture_error_streak = 0;
        self.sticky_error = None;
        self.set_event("Camera reconnect restarted", false);
    }

    pub(super) fn start_camera_discovery(&mut self, announce: bool) {
        match self.camera_discovery.start() {
            Ok(true) => self.announce_discovery_result = announce,
            Ok(false) => {}
            Err(error) => self.set_event(error.actionable_message("Start camera discovery"), true),
        }
    }

    pub(super) fn cancel_camera_discovery(&mut self) {
        if self.camera_discovery.cancel() {
            self.announce_discovery_result = false;
            self.set_event(tr(self.locale, UiText::DiscoveryCancelled), false);
        }
    }

    pub(super) fn poll_camera_discovery(&mut self, ctx: &egui::Context) {
        let Some(result) = self.camera_discovery.poll() else {
            return;
        };
        let announce = std::mem::take(&mut self.announce_discovery_result);
        match result {
            Ok(devices) => {
                self.real_devices = devices;
                let known_ids = self
                    .real_devices
                    .iter()
                    .map(|device| device.id.clone())
                    .collect::<Vec<_>>();
                // Preserve the user's scene when a camera disappears. The
                // missing-source policy remains in force until discovery sees
                // the same stable id again.
                self.real_sources
                    .retain(|(id, _, _)| known_ids.contains(id));
                if announce {
                    if self.real_devices.is_empty() {
                        self.set_event("No cameras found", true);
                    } else {
                        self.set_event(
                            format!("Found {} camera(s)", self.real_devices.len()),
                            false,
                        );
                    }
                }
            }
            Err(error) => {
                self.real_devices.clear();
                self.real_sources.clear();
                self.set_event(error.actionable_message("Discover cameras"), true);
            }
        }

        if self.input_mode == InputMode::Real && self.running {
            self.invalidate_render_epoch();
            self.request_render(ctx, self.running, true);
        }
    }
}
