use super::*;

/// What one composed output depends on: the frames it was built from, the
/// settings that shaped it, and whether it was published. Two jobs with equal
/// fingerprints produce equal pixels, which is what lets a submission be
/// skipped.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct RenderFingerprint {
    frames: Vec<Option<FrameFingerprint>>,
    layout: CompositionLayout,
    format: VideoFormat,
    scaling_filter: ScalingFilter,
    source_transforms: Vec<SourceTransform>,
    publish_virtual: bool,
}

/// The identity of one captured frame: which source produced it, and the two
/// counters that say it is not a repeat of the previous tick.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct FrameFingerprint {
    source_id: String,
    sequence: u64,
    monotonic_timestamp_nanos: u64,
}

impl RenderFingerprint {
    pub(super) fn new(
        frames: &[Option<CapturedFrame>],
        layout: CompositionLayout,
        format: VideoFormat,
        scaling_filter: ScalingFilter,
        source_transforms: Vec<SourceTransform>,
        publish_virtual: bool,
    ) -> Self {
        Self {
            frames: frames
                .iter()
                .map(|frame| {
                    frame.as_ref().map(|frame| FrameFingerprint {
                        source_id: frame.metadata().source_id.clone(),
                        sequence: frame.metadata().sequence,
                        monotonic_timestamp_nanos: frame.metadata().monotonic_timestamp().0,
                    })
                })
                .collect(),
            layout,
            format,
            scaling_filter,
            source_transforms,
            publish_virtual,
        }
    }
}

/// The app's side of the render worker handshake: which frame the worker should
/// be building next, and what to do with the frame it returns. `request_render`
/// submits a job only when the current [`RenderFingerprint`] differs from the
/// last one, which is what keeps a repaint loop running faster than the target
/// frame rate from queueing identical frames behind the worker.
/// `poll_render_result` drains the worker, discards results from a superseded
/// render epoch, then credits the frame counters, the measured frame rate and the
/// preview texture. `invalidate_render_epoch` is how every other module abandons
/// work already given to the worker, and `refresh_scene_render` is the one call
/// the scene panels make after they change the composition.
impl CameraManApp {
    pub(super) fn request_render(
        &mut self,
        ctx: &egui::Context,
        allow_open_camera: bool,
        force_preview: bool,
    ) {
        self.waiting_for_camera = false;
        // `real_frames` reassigns this on every tick that reaches it; the reset
        // covers the early return below, where no camera is polled at all.
        self.camera_not_responding = false;
        self.recompute_active_fps();
        let source_ids = self.selected_source_ids();
        if source_ids.is_empty() {
            self.invalidate_render_epoch();
            self.preview = None;
            self.preview_texture = None;
            if self.running {
                self.stop_streaming();
                self.set_event("No sources left selected, preview stopped", true);
            }
            return;
        }
        let frames = self.selected_frames(allow_open_camera);

        let frames = match frames {
            Ok(frames) => frames,
            Err(error) => {
                self.invalidate_render_epoch();
                self.on_capture_error(error.actionable_message("Prepare preview sources"));
                return;
            }
        };
        let only_cameras = self
            .selected_sources
            .iter()
            .all(|source| source.kind.is_camera());
        let every_frame_missing = frames.iter().all(Option::is_none);
        self.waiting_for_camera = self.running && only_cameras && every_frame_missing;
        let prepared = self.prepare_sources(source_ids, frames);
        let frames = prepared.frames;

        if only_cameras && (frames.is_empty() || frames.iter().all(Option::is_none)) {
            // Camera thread is still warming up (or closed): keep the previous
            // texture on screen and report the waiting state only for an
            // active preview. A paused real-mode selection has not opened the
            // device yet and must not look like a hung capture.
            if prepared.suppress_output {
                self.disconnect_virtual_output();
            }
            return;
        }

        let format = VideoFormat {
            width: VIRTUAL_CAMERA_WIDTH,
            height: VIRTUAL_CAMERA_HEIGHT,
            fps: self.active_fps,
            pixel_format: PixelFormat::Bgra8,
        };
        let live = self.running;
        let preview_due = force_preview || self.last_preview_upload.elapsed() >= PREVIEW_INTERVAL;
        let publish_virtual = live && self.virtual_output_enabled && !prepared.suppress_output;
        let fingerprint = RenderFingerprint::new(
            &frames,
            self.output.layout,
            format,
            self.output.scaling_filter,
            prepared.transforms.clone(),
            publish_virtual,
        );
        if !force_preview && self.last_render_fingerprint.as_ref() == Some(&fingerprint) {
            return;
        }
        self.last_render_fingerprint = Some(fingerprint);
        self.render_worker.submit(
            RenderJob::new(
                frames,
                self.output.layout,
                format,
                self.render_epoch,
                live,
                publish_virtual,
                force_preview,
            )
            .with_scaling_filter(self.output.scaling_filter)
            .with_source_transforms(prepared.transforms)
            .with_preview_size(preview_due.then_some((PREVIEW_MAX_WIDTH, PREVIEW_MAX_HEIGHT))),
        );
        ctx.request_repaint_after(WORKER_POLL_INTERVAL);
    }

    pub(super) fn poll_render_result(&mut self, ctx: &egui::Context) {
        let Some(result) = self.render_worker.take_result() else {
            return;
        };

        self.frames_rendered = self.frames_rendered.saturating_add(result.rendered_jobs);
        self.virtual_frames_sent = self
            .virtual_frames_sent
            .saturating_add(result.virtual_frames_sent);
        self.tick = self.tick.wrapping_add(result.rendered_jobs);
        self.update_virtual_camera_self_test(result.producer_progress, result.consumer_progress);
        if self.running && result.live_rendered_jobs > 0 {
            self.update_fps(result.live_rendered_jobs);
        }

        if result.epoch != self.render_epoch {
            return;
        }

        match result.frame {
            Ok(frame) => {
                self.capture_error_streak = 0;
                if let Some(image) = result.preview_image {
                    self.update_texture(ctx, image);
                }
                if let Some(previous) = self.preview.replace(frame) {
                    self.render_worker.recycle_frame(previous);
                }
            }
            Err(error) => {
                self.last_render_fingerprint = None;
                self.set_event(error.actionable_message("Compose preview"), true);
            }
        }
        if let Some(error) = result.transport_error {
            self.last_render_fingerprint = None;
            self.set_event(
                error.actionable_message("Publish virtual camera frame"),
                true,
            );
        }
    }

    pub(super) fn invalidate_render_epoch(&mut self) {
        self.render_epoch = self.render_epoch.wrapping_add(1);
        self.last_render_fingerprint = None;
    }

    pub(super) fn refresh_scene_render(&mut self, ctx: &egui::Context, change: SceneChange) {
        let targets = change.invalidates();
        if !targets.preview && !targets.output {
            return;
        }
        let selected_camera_ids = self.selected_camera_ids();
        let devices = &self.real_devices;
        self.real_sources.retain(|(runtime_id, _, _)| {
            selected_camera_ids
                .iter()
                .any(|selected_id| camera_locators_match(runtime_id, selected_id, devices))
        });
        self.invalidate_render_epoch();
        self.recompute_active_fps();
        self.request_render(ctx, self.running, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_fingerprint_deduplicates_only_the_same_capture_and_configuration() {
        let format = VideoFormat::hd_1080p_bgra();
        let frame = Frame::solid_bgra(2, 2, [1, 2, 3, 255]).unwrap();
        let first = CapturedFrame::new(frame.clone(), FrameMetadata::new("camera", 7));
        let next = CapturedFrame::new(frame, FrameMetadata::new("camera", 8));
        let fingerprint = |frame: CapturedFrame, publish_virtual| {
            RenderFingerprint::new(
                &[Some(frame)],
                CompositionLayout::Grid,
                format,
                ScalingFilter::Nearest,
                vec![SourceTransform::default()],
                publish_virtual,
            )
        };

        let baseline = fingerprint(first.clone(), true);
        assert!(baseline == fingerprint(first.clone(), true));
        assert!(baseline != fingerprint(next, true));
        assert!(baseline != fingerprint(first, false));
    }
}
