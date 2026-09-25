use super::*;

/// The frame rate the app runs at and the measurement of the rate it actually
/// achieved. The target is resolved from `output.fps_mode` plus whatever the
/// open cameras negotiated; the measurement is a sliding window over rendered
/// jobs, reset whenever the stream stops.
impl CameraManApp {
    /// Recomputes `active_fps` from the current mode and open real cameras.
    /// The render worker receives the format with each latest-only job, so
    /// changing this value requires no synchronous renderer or sink work.
    pub(super) fn recompute_active_fps(&mut self) {
        let fps = match self.output.fps_mode {
            FpsMode::Fixed(fps) => fps,
            FpsMode::Auto => self
                .real_sources
                .iter()
                .filter_map(|(_, source, _)| source.negotiated_fps())
                .max()
                .unwrap_or(VIRTUAL_CAMERA_DEFAULT_FPS),
        }
        .clamp(1, VIRTUAL_CAMERA_MAX_FPS);

        if fps == self.active_fps {
            return;
        }
        self.active_fps = fps;
        if self.running {
            self.media_clock.set_fps(fps);
        }
    }

    pub(super) fn fixed_fps_warning(&self) -> Option<String> {
        let FpsMode::Fixed(target_fps) = self.output.fps_mode else {
            return None;
        };
        let slowest_camera_fps = self
            .real_sources
            .iter()
            .filter_map(|(_, source, _)| source.negotiated_fps())
            .min()?;
        (target_fps > slowest_camera_fps).then(|| {
            format!(
                "Target is above the slowest camera ({slowest_camera_fps} fps); frames may repeat"
            )
        })
    }

    pub(super) fn update_fps(&mut self, completed_frames: u64) {
        self.fps_window_frames = self
            .fps_window_frames
            .saturating_add(u32::try_from(completed_frames).unwrap_or(u32::MAX));
        let elapsed = self.fps_window_start.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.measured_fps = self.fps_window_frames as f32 / elapsed.as_secs_f32();
            self.fps_window_start = Instant::now();
            self.fps_window_frames = 0;
        }
    }
}
