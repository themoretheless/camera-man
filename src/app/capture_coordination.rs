use std::collections::HashSet;

use super::*;

#[derive(Debug, Clone)]
pub(super) struct SourceSlot {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) bgra: [u8; 4],
}

impl CameraManApp {
    pub(super) fn synthetic_frame(
        &self,
        source: &SourceSlot,
    ) -> Result<Option<CapturedFrame>, camera_man::CameraManError> {
        let bgra = animated_color(source.bgra, self.tick, self.reduce_motion);
        let source_id = SourceDescriptor::synthetic(source.id).stable_key();
        let mut frame_source =
            SyntheticFrameSource::with_id(source_id, SOURCE_WIDTH, SOURCE_HEIGHT, bgra);
        frame_source.latest_frame()
    }

    pub(super) fn selected_camera_ids(&self) -> Vec<String> {
        self.selected_sources
            .iter()
            .filter(|source| source.kind == SourceKind::Camera)
            .map(|source| source.locator.clone())
            .collect()
    }

    /// Resolves the ordered heterogeneous scene graph into one latest-frame
    /// vector without grouping sources by kind.
    pub(super) fn selected_frames(
        &mut self,
        allow_open: bool,
    ) -> Result<Vec<Option<CapturedFrame>>, camera_man::CameraManError> {
        let selected_sources = self.selected_sources.clone();
        let camera_ids = self.selected_camera_ids();
        let camera_frames = camera_ids
            .iter()
            .cloned()
            .zip(self.real_frames(&camera_ids, allow_open))
            .collect::<Vec<_>>();
        resolve_ordered_sources(&selected_sources, camera_frames, |locator| {
            self.sources
                .iter()
                .find(|source| source.id == locator)
                .map(|source| self.synthetic_frame(source))
                .unwrap_or(Ok(None))
        })
    }

    /// Returns the newest frame from every selected real camera, in
    /// selection order, which is later merged with the other source kinds by
    /// the scene graph's single composition order.
    ///
    /// Cameras are opened only when `allow_open` is true (Start pressed /
    /// already streaming), never as a side effect of switching modes or
    /// checking a box. A single failing camera degrades to `None` for that
    /// cell instead of aborting the whole composite (mirrors
    /// `PipelineEngine::render_once`); only an all-camera scene with no healthy
    /// camera records the failure in the shared capture streak. A mixed scene
    /// keeps rendering its non-camera cells.
    ///
    /// Each source tracks its own consecutive-failure streak (the trailing
    /// `u32` in `real_sources`). The capture worker reconnects with bounded
    /// exponential backoff, while this coordinator keeps the source selected
    /// and exposes a stable RETRY state to the UI.
    ///
    /// A camera whose open or first frame never returns reports a capture
    /// timeout through this same `Err` path, so it advances the streak like any
    /// other failure instead of resetting it through an indefinite `Ok(None)`.
    fn real_frames(
        &mut self,
        selected_camera_ids: &[String],
        allow_open: bool,
    ) -> Vec<Option<CapturedFrame>> {
        // Release cameras that were unchecked (or disappeared) since the last tick.
        let devices = &self.real_devices;
        self.real_sources.retain(|(runtime_id, _, _)| {
            selected_camera_ids
                .iter()
                .any(|selected_id| camera_locators_match(runtime_id, selected_id, devices))
        });

        if allow_open {
            let capture_target = VideoFormat {
                width: VIRTUAL_CAMERA_WIDTH,
                height: VIRTUAL_CAMERA_HEIGHT,
                fps: self.active_fps,
                pixel_format: PixelFormat::Bgra8,
            };
            for id in selected_camera_ids {
                if !self.real_sources.iter().any(|(runtime_id, _, _)| {
                    camera_locators_match(runtime_id, id, &self.real_devices)
                }) {
                    self.real_sources.push((
                        id.clone(),
                        ThreadedNokhwaFrameSource::open_id_with_target(id, capture_target),
                        0,
                    ));
                }
            }
        }

        let mut frames = Vec::with_capacity(selected_camera_ids.len());
        let mut first_error = None;
        let mut ok_count = 0;
        // Collected during the loop and acted on after it: `real_sources` is
        // mutably borrowed by the loop, so `self.set_event` cannot be called
        // from inside it.
        let mut new_failures = Vec::new();
        let mut recovered_ids = Vec::new();
        let mut open_timed_out = false;
        for id in selected_camera_ids {
            let Some((_, source, streak)) =
                self.real_sources.iter_mut().find(|(runtime_id, _, _)| {
                    camera_locators_match(runtime_id, id, &self.real_devices)
                })
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
                    open_timed_out |= error.code() == ErrorCode::Capture(CaptureErrorKind::Timeout);
                    let source_name = self
                        .real_devices
                        .iter()
                        .find(|device| device_has_locator(device, id))
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

        self.camera_not_responding = open_timed_out;

        if let Some((source_name, error)) =
            first_error.filter(|_| camera_failure_is_scene_wide(&self.selected_sources, ok_count))
        {
            self.on_capture_error(error.actionable_message(&format!("Read camera {source_name}")));
        } else {
            // A mixed scene keeps rendering its healthy non-camera cells even
            // when every camera is down. Each newly failing camera is still
            // announced once and remains available for bounded reconnect.
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
        // Reopen through the existing runtime locator when it is a migrated
        // alias. Its capture lease then remains identical to the retiring
        // worker's lease, so Retry cannot open the same physical device twice.
        let previous = self
            .real_sources
            .iter()
            .find(|(runtime_id, _, _)| camera_locators_match(runtime_id, id, &self.real_devices));
        let reopen_id = previous
            .map(|(runtime_id, _, _)| runtime_id.clone())
            .unwrap_or_else(|| id.to_owned());
        // A wedged worker keeps the camera's lease until the backend returns,
        // so the replacement inherits its stall instead of spending another
        // full deadline looking like a fresh warm-up.
        let inherited_stall =
            previous.map_or(Duration::ZERO, |(_, source, _)| source.stalled_open_age());
        self.real_sources.retain(|(runtime_id, _, _)| {
            !camera_locators_match(runtime_id, id, &self.real_devices)
        });
        if self.running
            && self
                .selected_sources
                .iter()
                .any(|source| source.kind == SourceKind::Camera && source.locator == id)
        {
            let capture_target = VideoFormat {
                width: VIRTUAL_CAMERA_WIDTH,
                height: VIRTUAL_CAMERA_HEIGHT,
                fps: self.active_fps,
                pixel_format: PixelFormat::Bgra8,
            };
            self.real_sources.push((
                reopen_id.clone(),
                ThreadedNokhwaFrameSource::reopen_id_with_target(
                    &reopen_id,
                    capture_target,
                    inherited_stall,
                ),
                0,
            ));
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
                self.migrate_camera_aliases(&devices);
                self.real_devices = devices;
                // Preserve the user's scene when a camera disappears. The
                // missing-source policy remains in force until discovery sees
                // the same stable id again.
                self.real_sources.retain(|(runtime_id, _, _)| {
                    self.real_devices
                        .iter()
                        .any(|device| device_has_locator(device, runtime_id))
                });
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

        if self.running {
            self.invalidate_render_epoch();
            self.request_render(ctx, self.running, true);
        }
    }

    fn migrate_camera_aliases(&mut self, devices: &[CameraDevice]) {
        let remapped = remap_camera_aliases(
            &mut self.selected_sources,
            &mut self.source_transforms,
            devices,
        );
        if let Some(selected) = self.selected_source_id.as_mut()
            && let Some(current) = remapped.get(selected)
        {
            *selected = current.clone();
        }
        for (legacy, current) in &remapped {
            if let Some(frame) = self.last_good_frames.remove(legacy) {
                self.last_good_frames
                    .entry(current.clone())
                    .or_insert(frame);
            }
        }
        for scene in &mut self.scenes {
            remap_camera_aliases(&mut scene.sources, &mut scene.source_transforms, devices);
        }
    }
}

fn device_has_locator(device: &CameraDevice, locator: &str) -> bool {
    device.id == locator || device.aliases.iter().any(|alias| alias == locator)
}

pub(super) fn camera_locators_match(left: &str, right: &str, devices: &[CameraDevice]) -> bool {
    left == right
        || devices
            .iter()
            .any(|device| device_has_locator(device, left) && device_has_locator(device, right))
}

fn resolve_ordered_sources<T, E>(
    descriptors: &[SourceDescriptor],
    mut cameras: Vec<(String, Option<T>)>,
    mut synthetic_frame: impl FnMut(&str) -> Result<Option<T>, E>,
) -> Result<Vec<Option<T>>, E> {
    descriptors
        .iter()
        .map(|descriptor| match descriptor.kind {
            SourceKind::Synthetic => synthetic_frame(&descriptor.locator),
            SourceKind::Camera => Ok(cameras
                .iter_mut()
                .find(|(locator, _)| locator == &descriptor.locator)
                .and_then(|(_, frame)| frame.take())),
        })
        .collect()
}

fn camera_failure_is_scene_wide(
    sources: &[SourceDescriptor],
    camera_frames_received: usize,
) -> bool {
    camera_frames_received == 0
        && !sources.is_empty()
        && sources
            .iter()
            .all(|source| source.kind == SourceKind::Camera)
}

fn remap_camera_aliases(
    sources: &mut Vec<SourceDescriptor>,
    transforms: &mut BTreeMap<String, SourceTransform>,
    devices: &[CameraDevice],
) -> HashMap<String, String> {
    let mut remapped = HashMap::new();
    for source in sources
        .iter_mut()
        .filter(|source| source.kind == SourceKind::Camera)
    {
        let Some(current_id) = devices.iter().find_map(|device| {
            device
                .aliases
                .iter()
                .any(|alias| alias == &source.locator)
                .then_some(device.id.clone())
        }) else {
            continue;
        };
        let legacy_key = source.stable_key();
        source.locator = current_id;
        let current_key = source.stable_key();
        if let Some(transform) = transforms.remove(&legacy_key) {
            transforms.entry(current_key.clone()).or_insert(transform);
        }
        remapped.insert(legacy_key, current_key);
    }

    let mut seen = HashSet::new();
    sources.retain(|source| seen.insert(source.stable_key()));
    remapped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heterogeneous_sources_keep_scene_order_and_missing_cells() {
        let descriptors = [
            SourceDescriptor::camera("built-in"),
            SourceDescriptor::synthetic("video-like"),
            SourceDescriptor::camera("missing"),
        ];
        let cameras = vec![(String::from("built-in"), Some("camera"))];

        assert_eq!(
            resolve_ordered_sources(&descriptors, cameras, |locator| {
                Ok::<_, ()>((locator == "video-like").then_some("video"))
            })
            .unwrap(),
            [Some("camera"), Some("video"), None]
        );
    }

    #[test]
    fn camera_failure_is_scene_wide_only_for_an_all_camera_scene() {
        assert!(camera_failure_is_scene_wide(
            &[SourceDescriptor::camera("built-in")],
            0
        ));
        assert!(!camera_failure_is_scene_wide(
            &[
                SourceDescriptor::camera("built-in"),
                SourceDescriptor::synthetic("video-like"),
            ],
            0
        ));
        assert!(!camera_failure_is_scene_wide(
            &[SourceDescriptor::camera("built-in")],
            1
        ));
    }

    #[test]
    fn discovery_migrates_legacy_camera_ids_and_transform_keys() {
        let mut sources = vec![
            SourceDescriptor::camera("0"),
            SourceDescriptor::synthetic("desk"),
        ];
        let transform = SourceTransform {
            mirror_horizontal: true,
            ..SourceTransform::default()
        };
        let mut transforms = BTreeMap::from([(String::from("camera:0"), transform)]);
        let devices = [CameraDevice::with_aliases(
            "uid:stable-camera",
            "Camera",
            vec![String::from("0")],
        )];

        let remapped = remap_camera_aliases(&mut sources, &mut transforms, &devices);

        assert_eq!(sources[0], SourceDescriptor::camera("uid:stable-camera"));
        assert_eq!(remapped["camera:0"], "camera:uid:stable-camera");
        assert!(transforms["camera:uid:stable-camera"].mirror_horizontal);
    }

    #[test]
    fn legacy_and_stable_camera_locators_share_one_runtime_identity() {
        let devices = [CameraDevice::with_aliases(
            "uid:stable-camera",
            "Camera",
            vec![String::from("0")],
        )];

        assert!(camera_locators_match("0", "uid:stable-camera", &devices));
        assert!(!camera_locators_match("1", "uid:stable-camera", &devices));
    }
}
