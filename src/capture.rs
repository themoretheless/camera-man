use crate::camera::{CameraDevice, CameraDiscovery, FrameSource};
use crate::config::{VideoFormat, VirtualCameraConfig};
use crate::diagnostics::{PipelineStage, stage_span};
use crate::error::{CameraManError, CaptureErrorKind};
use crate::frame::{CapturedFrame, Frame, FrameMetadata, PixelFormat};
use crate::performance::{CopyStage, copy_ledger};
use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nokhwa::FormatDecoder;

static ACTIVE_CAMERA_IDS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

struct CameraLease {
    id: String,
}

impl Drop for CameraLease {
    fn drop(&mut self) {
        ACTIVE_CAMERA_IDS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
            .expect("camera lease mutex poisoned")
            .remove(&self.id);
    }
}

pub struct NokhwaCameraDiscovery;

const CAMERA_UID_PREFIX: &str = "uid:";

impl CameraDiscovery for NokhwaCameraDiscovery {
    fn list_devices(&self) -> Result<Vec<CameraDevice>, CameraManError> {
        let devices = nokhwa::query(nokhwa::utils::ApiBackend::Auto).map_err(nokhwa_error)?;
        Ok(devices.into_iter().map(camera_device_from_info).collect())
    }
}

pub struct NokhwaFrameSource {
    source_id: String,
    camera: nokhwa::Camera,
    sequence: u64,
}

/// Captures on a background thread so the caller (typically the UI render
/// loop) never blocks on `nokhwa::Camera::frame()`.
///
/// Caveat (не проверено across all backends): nokhwa's per-frame read can
/// block indefinitely if the device stalls, and there is no cancellation
/// hook. `stop` is only observed BETWEEN frames, so a wedged camera can keep
/// the worker thread (and the open device) alive past `Drop`. To avoid
/// freezing the dropping thread on that same wedge, `Drop` does not join the
/// worker directly; it hands the `JoinHandle` to a short-lived reaper thread
/// that joins in the background, so the OS thread is still reclaimed once
/// the camera call unblocks, without ever blocking the caller. A process-local
/// camera-id lease makes a replacement worker wait off-thread until the old
/// worker releases the device, preventing Stop -> Start reopen races.
pub struct ThreadedNokhwaFrameSource {
    latest: Arc<Mutex<Option<CapturedFrame>>>,
    last_error: Arc<Mutex<Option<CameraManError>>>,
    /// Set once the worker has actually opened the device; `None` while still
    /// warming up. This is the camera's real negotiated rate, not a guess.
    negotiated_format: Arc<Mutex<Option<VideoFormat>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl NokhwaFrameSource {
    pub fn open_index(index: u32) -> Result<Self, CameraManError> {
        Self::open_camera_index(
            nokhwa::utils::CameraIndex::Index(index),
            format!("camera-{index}"),
            VirtualCameraConfig::default().format,
        )
    }

    pub fn open_id(id: &str) -> Result<Self, CameraManError> {
        Self::open_id_with_target(id, VirtualCameraConfig::default().format)
    }

    pub fn open_id_with_target(id: &str, target: VideoFormat) -> Result<Self, CameraManError> {
        let index = camera_index_from_id(id);
        Self::open_camera_index(index, format!("camera-{id}"), target)
    }

    fn open_camera_index(
        index: nokhwa::utils::CameraIndex,
        source_id: String,
        target: VideoFormat,
    ) -> Result<Self, CameraManError> {
        let mut last_error = None;
        let mut camera = None;
        for request_type in camera_request_candidates(target) {
            let requested = nokhwa::utils::RequestedFormat::new::<nokhwa::pixel_format::RgbAFormat>(
                request_type,
            );
            match nokhwa::Camera::new(index.clone(), requested) {
                Ok(opened) => {
                    camera = Some(opened);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        let mut camera = camera.ok_or_else(|| {
            nokhwa_error(last_error.unwrap_or_else(|| {
                nokhwa::NokhwaError::OpenDeviceError(
                    index.as_string(),
                    String::from("no compatible decodable camera format"),
                )
            }))
            .context(format!("open {source_id}"))
        })?;
        camera
            .open_stream()
            .map_err(nokhwa_error)
            .map_err(|error| error.context(format!("start stream {source_id}")))?;
        Ok(Self {
            source_id,
            camera,
            sequence: 0,
        })
    }
}

fn camera_request_candidates(target: VideoFormat) -> Vec<nokhwa::utils::RequestedFormatType> {
    let resolution = nokhwa::utils::Resolution::new(target.width.max(1), target.height.max(1));
    nokhwa::pixel_format::RgbAFormat::FORMATS
        .iter()
        .map(|frame_format| {
            nokhwa::utils::RequestedFormatType::Closest(nokhwa::utils::CameraFormat::new(
                resolution,
                *frame_format,
                target.fps.max(1),
            ))
        })
        .chain([
            nokhwa::utils::RequestedFormatType::HighestFrameRate(target.fps.max(1)),
            nokhwa::utils::RequestedFormatType::AbsoluteHighestResolution,
        ])
        .collect()
}

fn camera_device_from_info(info: nokhwa::utils::CameraInfo) -> CameraDevice {
    let legacy_id = info.index().as_string();
    let unique_id = info.misc();
    if unique_id.trim().is_empty() {
        CameraDevice::new(legacy_id, info.human_name())
    } else {
        CameraDevice::with_aliases(
            format!("{CAMERA_UID_PREFIX}{unique_id}"),
            info.human_name(),
            vec![legacy_id],
        )
    }
}

fn camera_index_from_id(id: &str) -> nokhwa::utils::CameraIndex {
    if let Some(unique_id) = id.strip_prefix(CAMERA_UID_PREFIX) {
        return nokhwa::utils::CameraIndex::String(unique_id.to_owned());
    }
    id.parse::<u32>()
        .map(nokhwa::utils::CameraIndex::Index)
        .unwrap_or_else(|_| nokhwa::utils::CameraIndex::String(id.to_owned()))
}

impl NokhwaFrameSource {
    /// The frame rate actually negotiated with the device at open time, not a guess.
    pub fn frame_rate(&self) -> u32 {
        self.camera.frame_rate()
    }

    pub fn video_format(&self) -> VideoFormat {
        let resolution = self.camera.resolution();
        VideoFormat {
            width: resolution.x(),
            height: resolution.y(),
            fps: self.camera.frame_rate().max(1),
            pixel_format: PixelFormat::Bgra8,
        }
    }
}

impl FrameSource for NokhwaFrameSource {
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
        let _capture_span = stage_span(PipelineStage::Capture, self.sequence.wrapping_add(1), 0, 0);
        let buffer = self
            .camera
            .frame()
            .map_err(nokhwa_error)
            .map_err(|error| error.context(format!("read {}", self.source_id)))?;
        let image = buffer
            .decode_image::<nokhwa::pixel_format::RgbAFormat>()
            .map_err(nokhwa_error)
            .map_err(|error| error.context(format!("decode {}", self.source_id)))?;
        let width = image.width();
        let height = image.height();
        let mut bgra = image.into_raw();
        rgba_to_bgra_in_place(&mut bgra);
        copy_ledger().record_allocation(CopyStage::CaptureDecode, bgra.len());
        copy_ledger().record_copy(CopyStage::CaptureDecode, bgra.len());

        self.sequence += 1;
        let frame = Frame::new_checked(width, height, PixelFormat::Bgra8, bgra)?;
        let metadata = FrameMetadata::new(self.source_id.clone(), self.sequence);
        Ok(Some(CapturedFrame::new(frame, metadata)))
    }
}

fn rgba_to_bgra_in_place(pixels: &mut [u8]) {
    debug_assert!(pixels.len().is_multiple_of(4));
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

impl ThreadedNokhwaFrameSource {
    pub fn open_id(id: &str) -> Self {
        Self::open_id_with_target(id, VirtualCameraConfig::default().format)
    }

    pub fn open_id_with_target(id: &str, target: VideoFormat) -> Self {
        let latest = Arc::new(Mutex::new(None));
        let last_error = Arc::new(Mutex::new(None));
        let negotiated_format = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));

        let worker_latest = Arc::clone(&latest);
        let worker_last_error = Arc::clone(&last_error);
        let worker_negotiated_format = Arc::clone(&negotiated_format);
        let worker_stop = Arc::clone(&stop);
        let worker_id = id.to_string();

        let worker = thread::Builder::new()
            .name(format!("camera-capture-{id}"))
            .spawn(move || {
                run_capture_worker(
                    &worker_id,
                    &worker_latest,
                    &worker_last_error,
                    &worker_negotiated_format,
                    &worker_stop,
                    target,
                );
            })
            .expect("failed to spawn camera capture thread");

        Self {
            latest,
            last_error,
            negotiated_format,
            stop,
            worker: Some(worker),
        }
    }

    /// The camera's real negotiated frame rate, once known. `None` until the
    /// worker thread has finished opening the device.
    pub fn negotiated_fps(&self) -> Option<u32> {
        self.negotiated_format().map(|format| format.fps)
    }

    pub fn negotiated_format(&self) -> Option<VideoFormat> {
        *self
            .negotiated_format
            .lock()
            .expect("camera format mutex poisoned")
    }
}

/// Body of the background capture loop. Runs until `stop` is set, reopening
/// the device with bounded exponential backoff after open or stream failures.
/// A panic inside a single iteration (camera or mutex failure) is caught so it
/// surfaces as a normal error on the
/// `last_error` slot instead of silently killing the thread with no signal:
/// without this, `latest_frame()` would keep returning the last-known-good
/// frame forever and the preview would look frozen-but-healthy.
fn run_capture_worker(
    id: &str,
    latest: &Arc<Mutex<Option<CapturedFrame>>>,
    last_error: &Arc<Mutex<Option<CameraManError>>>,
    negotiated_format: &Arc<Mutex<Option<VideoFormat>>>,
    stop: &Arc<AtomicBool>,
    target: VideoFormat,
) {
    let Some(_lease) = wait_for_camera_lease(id, stop) else {
        return;
    };
    let mut backoff = ReconnectBackoff::default();
    while !stop.load(Ordering::Relaxed) {
        let mut source = match NokhwaFrameSource::open_id_with_target(id, target) {
            Ok(source) => source,
            Err(error) => {
                *last_error.lock().expect("camera error mutex poisoned") = Some(error);
                *negotiated_format
                    .lock()
                    .expect("camera format mutex poisoned") = None;
                if !sleep_until_retry(stop, backoff.next_delay()) {
                    return;
                }
                continue;
            }
        };
        *negotiated_format
            .lock()
            .expect("camera format mutex poisoned") = Some(source.video_format());
        // A successful open is not recovery yet. Keep any prior error visible
        // until this session has delivered a genuinely new frame.

        let mut failure_streak = 0_u8;
        while !stop.load(Ordering::Relaxed) {
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| source.latest_frame()));
            match outcome {
                Ok(Ok(frame)) => {
                    failure_streak = 0;
                    backoff.observe_frame(frame.is_some());
                    *latest.lock().expect("camera frame mutex poisoned") = frame;
                    *last_error.lock().expect("camera error mutex poisoned") = None;
                }
                Ok(Err(error)) => {
                    failure_streak = failure_streak.saturating_add(1);
                    *last_error.lock().expect("camera error mutex poisoned") = Some(error);
                    if failure_streak >= 3 {
                        break;
                    }
                    if !sleep_until_retry(stop, Duration::from_millis(50)) {
                        return;
                    }
                }
                Err(panic_payload) => {
                    let message = panic_message(&panic_payload);
                    *last_error.lock().expect("camera error mutex poisoned") = Some(
                        CameraManError::capture(format!("capture worker panicked: {message}")),
                    );
                    break;
                }
            }
        }
        *negotiated_format
            .lock()
            .expect("camera format mutex poisoned") = None;
        if !stop.load(Ordering::Relaxed) && !sleep_until_retry(stop, backoff.next_delay()) {
            return;
        }
    }
}

struct ReconnectBackoff {
    next: Duration,
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self {
            next: Duration::from_millis(100),
        }
    }
}

impl ReconnectBackoff {
    fn next_delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = (self.next * 2).min(Duration::from_secs(5));
        delay
    }

    fn reset(&mut self) {
        self.next = Duration::from_millis(100);
    }

    fn observe_frame(&mut self, received: bool) {
        if received {
            self.reset();
        }
    }
}

fn sleep_until_retry(stop: &AtomicBool, duration: Duration) -> bool {
    let deadline = std::time::Instant::now() + duration;
    while std::time::Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        thread::sleep(
            deadline
                .saturating_duration_since(std::time::Instant::now())
                .min(Duration::from_millis(25)),
        );
    }
    !stop.load(Ordering::Relaxed)
}

fn wait_for_camera_lease(id: &str, stop: &AtomicBool) -> Option<CameraLease> {
    loop {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        if let Some(lease) = try_acquire_camera_lease(id) {
            return Some(lease);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn try_acquire_camera_lease(id: &str) -> Option<CameraLease> {
    let mut active = ACTIVE_CAMERA_IDS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .expect("camera lease mutex poisoned");
    active
        .insert(id.to_owned())
        .then(|| CameraLease { id: id.to_owned() })
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        String::from("unknown panic payload")
    }
}

impl FrameSource for ThreadedNokhwaFrameSource {
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
        if let Some(error) = self
            .last_error
            .lock()
            .expect("camera error mutex poisoned")
            .clone()
        {
            return Err(error);
        }
        Ok(self
            .latest
            .lock()
            .expect("camera frame mutex poisoned")
            .clone())
    }
}

impl Drop for ThreadedNokhwaFrameSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Join off the calling thread: see the struct doc for why we cannot
        // join synchronously here without risking a UI freeze.
        if let Some(worker) = self.worker.take() {
            let _ = thread::Builder::new()
                .name(String::from("camera-capture-reaper"))
                .spawn(move || {
                    let _ = worker.join();
                });
        }
    }
}

pub fn capture_one_with_timeout(
    id: &str,
    timeout: Duration,
) -> Result<CapturedFrame, CameraManError> {
    let id = id.to_string();
    let (sender, receiver) = mpsc::sync_channel(crate::backpressure::CAPTURE_RESULTS.capacity);

    // If this times out, the spawned thread is left detached: nokhwa has no
    // cancellation hook, so the camera stays open until `open_id`/`frame()`
    // unblocks on its own and the thread exits (send() then fails silently
    // because the receiver is gone). Callers should treat a timeout here as
    // "camera may still be warming up", not "camera is free again".
    thread::spawn(move || {
        let result = (|| {
            let mut source = NokhwaFrameSource::open_id(&id)?;
            source.latest_frame()?.ok_or(CameraManError::EmptyInput)
        })();
        let _ = sender.send(result);
    });

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(CameraManError::capture_with_kind(
            CaptureErrorKind::Timeout,
            format!(
                "timed out after {}ms waiting for a frame",
                timeout.as_millis()
            ),
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(CameraManError::capture(
            "capture thread exited without sending a result",
        )),
    }
}

fn nokhwa_error(error: nokhwa::NokhwaError) -> CameraManError {
    let message = error.to_string();
    let kind = match error {
        nokhwa::NokhwaError::UnsupportedOperationError(_)
        | nokhwa::NokhwaError::NotImplementedError(_) => CaptureErrorKind::Unsupported,
        _ => CaptureErrorKind::classify(&message),
    };
    CameraManError::capture_with_kind(kind, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rgba_to_bgra_without_reallocating() {
        let mut pixels = vec![1, 2, 3, 4, 10, 20, 30, 40];
        let allocation = pixels.as_ptr();

        rgba_to_bgra_in_place(&mut pixels);

        assert_eq!(pixels, [3, 2, 1, 4, 30, 20, 10, 40]);
        assert_eq!(pixels.as_ptr(), allocation);
    }
    use crate::error::ErrorCode;

    #[test]
    fn maps_typed_nokhwa_unsupported_error_without_text_heuristics() {
        let error = nokhwa_error(nokhwa::NokhwaError::NotImplementedError(String::from(
            "manual exposure",
        )));

        assert_eq!(
            error.code(),
            ErrorCode::Capture(CaptureErrorKind::Unsupported)
        );
    }

    #[test]
    fn classifies_representative_avfoundation_messages() {
        let samples = [
            (
                "AVFoundation authorization denied",
                CaptureErrorKind::PermissionDenied,
            ),
            ("device is already in use", CaptureErrorKind::DeviceBusy),
            ("no such device", CaptureErrorKind::DeviceNotFound),
            ("capture stream stopped", CaptureErrorKind::Disconnected),
        ];

        for (message, expected) in samples {
            assert_eq!(CaptureErrorKind::classify(message), expected);
        }
    }

    #[test]
    fn stable_uid_and_legacy_index_open_through_distinct_locators() {
        assert_eq!(camera_index_from_id("0").as_string(), "0");
        assert_eq!(
            camera_index_from_id("uid:avfoundation-device").as_string(),
            "avfoundation-device"
        );
    }

    #[test]
    fn format_negotiation_tries_exact_output_before_broad_fallbacks() {
        let target = VideoFormat {
            width: 1920,
            height: 1080,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        };
        let candidates = camera_request_candidates(target);

        let nokhwa::utils::RequestedFormatType::Closest(first) = candidates[0] else {
            panic!("first camera request must be target-specific");
        };
        assert_eq!(
            first.resolution(),
            nokhwa::utils::Resolution::new(1920, 1080)
        );
        assert_eq!(first.frame_rate(), 30);
        assert_eq!(
            candidates[candidates.len() - 2],
            nokhwa::utils::RequestedFormatType::HighestFrameRate(30)
        );
        assert_eq!(
            candidates.last(),
            Some(&nokhwa::utils::RequestedFormatType::AbsoluteHighestResolution)
        );
    }

    #[test]
    fn camera_lease_serializes_reopen_for_the_same_id() {
        let id = "camera-lease-serializes-reopen";
        let first = try_acquire_camera_lease(id).unwrap();
        assert!(try_acquire_camera_lease(id).is_none());

        drop(first);

        assert!(try_acquire_camera_lease(id).is_some());
    }

    #[test]
    fn waiting_camera_lease_honors_stop() {
        let id = "camera-lease-honors-stop";
        let _first = try_acquire_camera_lease(id).unwrap();
        let stop = AtomicBool::new(true);

        assert!(wait_for_camera_lease(id, &stop).is_none());
    }

    #[test]
    fn reconnect_backoff_is_bounded_and_resettable() {
        let mut backoff = ReconnectBackoff::default();
        let delays = (0..8).map(|_| backoff.next_delay()).collect::<Vec<_>>();
        assert_eq!(delays[0], Duration::from_millis(100));
        assert_eq!(delays[6], Duration::from_secs(5));
        assert_eq!(delays[7], Duration::from_secs(5));
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
    }

    #[test]
    fn reconnect_backoff_resets_only_after_a_real_frame() {
        let mut backoff = ReconnectBackoff::default();
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));

        backoff.observe_frame(false);
        assert_eq!(backoff.next_delay(), Duration::from_millis(200));

        backoff.observe_frame(true);
        assert_eq!(backoff.next_delay(), Duration::from_millis(100));
    }
}
