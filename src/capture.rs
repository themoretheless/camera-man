use crate::camera::{CameraDevice, CameraDiscovery, FrameSource};
use crate::config::{VideoFormat, VirtualCameraConfig};
use crate::diagnostics::{PipelineStage, stage_span};
use crate::error::{CameraManError, CaptureErrorKind};
use crate::frame::{CapturedFrame, Frame, FrameMetadata, PixelFormat};
use crate::panic_boundary::panic_message;
use crate::performance::{CopyStage, copy_ledger};
use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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

pub const CAMERA_OPEN_TIMEOUT_ENV: &str = "CAMERAMAN_CAMERA_OPEN_TIMEOUT_MS";
const DEFAULT_CAMERA_OPEN_TIMEOUT: Duration = Duration::from_secs(20);

/// Deadline for one pre-streaming phase: waiting for the camera lease, opening
/// the device, or reading its first frame. The open phase is a whole
/// negotiation, not a single device open: `open_camera_index` tries every
/// decodable format candidate in turn (seven with nokhwa's RGBA decoder) and
/// then starts the stream, and the budget covers that sequence as a whole.
/// The 20 s default is an estimate for a slow but healthy device (external USB,
/// Continuity Camera wake, a first-run TCC prompt), not a measured figure;
/// `CAMERAMAN_CAMERA_OPEN_TIMEOUT_MS` overrides it. Exceeding it reports a
/// failure but does not stop the worker, so a late success still recovers on
/// its own.
pub fn camera_open_timeout() -> Duration {
    static TIMEOUT: OnceLock<Duration> = OnceLock::new();
    *TIMEOUT
        .get_or_init(|| parse_open_timeout(std::env::var(CAMERA_OPEN_TIMEOUT_ENV).ok().as_deref()))
}

fn parse_open_timeout(value: Option<&str>) -> Duration {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_CAMERA_OPEN_TIMEOUT)
}

impl CameraDiscovery for NokhwaCameraDiscovery {
    fn list_devices(&self) -> Result<Vec<CameraDevice>, CameraManError> {
        let devices = nokhwa::query(nokhwa::utils::ApiBackend::Auto).map_err(nokhwa_error)?;
        Ok(devices.into_iter().map(camera_device_from_info).collect())
    }
}

struct NokhwaFrameSource {
    source_id: String,
    camera: nokhwa::Camera,
    sequence: u64,
}

/// What a capture worker is doing before it has published a frame. The worker
/// cannot report a backend call that never returns, because it is parked inside
/// that call; publishing the phase lets the reader report it instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerPhase {
    /// An earlier worker still holds this camera id's lease.
    AwaitingLease,
    /// Inside the backend open call.
    Opening,
    /// Device open, first frame not read yet.
    AwaitingFirstFrame,
    /// At least one frame published; failures now speak through `last_error`.
    Streaming,
}

#[derive(Debug, Clone, Copy)]
struct WorkerProgress {
    phase: WorkerPhase,
    since: Instant,
}

/// Slots shared between one capture worker and its reader. The worker owns
/// every write; the reader only observes.
#[derive(Clone)]
struct CaptureSlots {
    latest: Arc<Mutex<Option<CapturedFrame>>>,
    last_error: Arc<Mutex<Option<CameraManError>>>,
    /// Set once the worker has actually opened the device; `None` while still
    /// warming up. This is the camera's real negotiated rate, not a guess.
    negotiated_format: Arc<Mutex<Option<VideoFormat>>>,
    progress: Arc<Mutex<WorkerProgress>>,
    stop: Arc<AtomicBool>,
}

impl Default for CaptureSlots {
    fn default() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
            last_error: Arc::new(Mutex::new(None)),
            negotiated_format: Arc::new(Mutex::new(None)),
            // The pre-streaming clock starts when the source is created, not
            // when its worker thread happens to be scheduled.
            progress: Arc::new(Mutex::new(WorkerProgress {
                phase: WorkerPhase::AwaitingLease,
                since: Instant::now(),
            })),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl CaptureSlots {
    fn publish_phase(&self, phase: WorkerPhase) {
        *self
            .progress
            .lock()
            .expect("camera progress mutex poisoned") = WorkerProgress {
            phase,
            since: Instant::now(),
        };
    }

    fn progress(&self) -> WorkerProgress {
        *self
            .progress
            .lock()
            .expect("camera progress mutex poisoned")
    }
}

/// How long a pre-streaming phase has run once it is past its deadline. `None`
/// while the phase is still within budget or the device is streaming, so a
/// healthy warm-up is never reported as broken and `Ok(None)` keeps its
/// documented "no frame ready" meaning.
fn open_stall_age(phase: WorkerPhase, elapsed: Duration, timeout: Duration) -> Option<Duration> {
    (phase != WorkerPhase::Streaming && elapsed >= timeout).then_some(elapsed)
}

/// Turns a pre-streaming phase that outlived its deadline into a real failure.
fn stalled_open_error(
    id: &str,
    phase: WorkerPhase,
    elapsed: Duration,
    timeout: Duration,
) -> Option<CameraManError> {
    let millis = open_stall_age(phase, elapsed, timeout)?.as_millis();
    let message = match phase {
        // Already rejected by `open_stall_age`.
        WorkerPhase::Streaming => return None,
        WorkerPhase::AwaitingLease => format!(
            "camera {id} is still held by an earlier open that did not return after {millis}ms"
        ),
        WorkerPhase::Opening => format!("opening camera {id} did not return after {millis}ms"),
        WorkerPhase::AwaitingFirstFrame => {
            format!("camera {id} delivered no first frame after {millis}ms")
        }
    };
    Some(CameraManError::capture_with_kind(
        CaptureErrorKind::Timeout,
        message,
    ))
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
///
/// The worker's pre-frame phase (lease wait, open, first read) is published and
/// bounded by `camera_open_timeout()`. An exceeded deadline is reported here,
/// by the reader, because the worker itself is blocked inside the backend; it
/// is a report, not a cancellation. The abandoned open keeps its camera lease,
/// so no replacement worker can double-open the device, and that id stays
/// unavailable until the driver returns. A replacement built through
/// `reopen_id_with_target` inherits that stall so the lease wait it is left
/// with does not read as a fresh warm-up.
pub struct ThreadedNokhwaFrameSource {
    id: String,
    slots: CaptureSlots,
    open_timeout: Duration,
    /// Stall already accumulated by the worker this source replaced. That
    /// worker keeps the camera's lease until the backend returns, so its stall
    /// is this source's lease wait rather than a fresh warm-up.
    inherited_stall: Duration,
    worker: Option<JoinHandle<()>>,
}

impl NokhwaFrameSource {
    fn open_id_with_target(id: &str, target: VideoFormat) -> Result<Self, CameraManError> {
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
    fn video_format(&self) -> VideoFormat {
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
        Self::reopen_id_with_target(id, target, Duration::ZERO)
    }

    /// Replacement for a camera whose previous worker was already stalled by
    /// `stalled_open_age`. That worker keeps the camera's lease until the
    /// backend returns, so its stall carries into this source's lease wait:
    /// retrying a wedged camera keeps reporting it as unresponsive instead of
    /// showing a fresh warm-up for another full deadline. Acquiring the lease
    /// republishes `Opening` with a full budget of its own.
    pub fn reopen_id_with_target(id: &str, target: VideoFormat, inherited_stall: Duration) -> Self {
        let slots = CaptureSlots::default();
        let worker_slots = slots.clone();
        let worker_id = id.to_string();

        let worker = thread::Builder::new()
            .name(format!("camera-capture-{id}"))
            .spawn(move || {
                run_capture_worker(
                    &worker_id,
                    &worker_slots,
                    target,
                    NokhwaFrameSource::open_id_with_target,
                );
            })
            .expect("failed to spawn camera capture thread");

        Self {
            id: id.to_string(),
            slots,
            open_timeout: camera_open_timeout(),
            inherited_stall,
            worker: Some(worker),
        }
    }

    /// Reader over prebuilt slots with no worker behind them, so the real
    /// `latest_frame` path can be exercised without a device.
    #[cfg(test)]
    fn detached(
        id: &str,
        slots: CaptureSlots,
        open_timeout: Duration,
        inherited_stall: Duration,
    ) -> Self {
        Self {
            id: id.to_string(),
            slots,
            open_timeout,
            inherited_stall,
            worker: None,
        }
    }

    /// Wall time the current pre-frame phase has consumed. A lease wait adds
    /// the stall inherited from the worker that still holds that lease.
    fn pre_frame_elapsed(&self, progress: WorkerProgress) -> Duration {
        let elapsed = progress.since.elapsed();
        if progress.phase == WorkerPhase::AwaitingLease {
            return elapsed.saturating_add(self.inherited_stall);
        }
        elapsed
    }

    /// How long this source has been stalled before its first frame, or
    /// `Duration::ZERO` while it is healthy or still within budget. A
    /// replacement for the same camera inherits it, because a stalled worker
    /// holds the camera's lease until the backend returns.
    pub fn stalled_open_age(&self) -> Duration {
        let progress = self.slots.progress();
        open_stall_age(
            progress.phase,
            self.pre_frame_elapsed(progress),
            self.open_timeout,
        )
        .unwrap_or(Duration::ZERO)
    }

    /// The camera's real negotiated frame rate, once known. `None` until the
    /// worker thread has finished opening the device.
    pub fn negotiated_fps(&self) -> Option<u32> {
        self.negotiated_format().map(|format| format.fps)
    }

    pub fn negotiated_format(&self) -> Option<VideoFormat> {
        *self
            .slots
            .negotiated_format
            .lock()
            .expect("camera format mutex poisoned")
    }
}

/// The capture worker's view of a backend. Implemented by `NokhwaFrameSource`;
/// tests substitute a source whose open blocks.
trait OpenableSource: FrameSource {
    fn negotiated_format(&self) -> VideoFormat;
}

impl OpenableSource for NokhwaFrameSource {
    fn negotiated_format(&self) -> VideoFormat {
        self.video_format()
    }
}

/// Body of the background capture loop. Runs until `stop` is set, reopening
/// the device with bounded exponential backoff after open or stream failures.
/// A panic inside a single iteration (camera or mutex failure) is caught so it
/// surfaces as a normal error on the
/// `last_error` slot instead of silently killing the thread with no signal:
/// without this, `latest_frame()` would keep returning the last-known-good
/// frame forever and the preview would look frozen-but-healthy. The open call
/// is contained for the same reason, and a panicking open ends the worker:
/// retrying it would re-enter the same panic while holding the camera lease
/// against a replacement that might succeed.
/// Each pre-frame phase is published to `slots` before the call that can block
/// inside it, because a worker parked in the backend cannot report itself.
fn run_capture_worker<S: OpenableSource>(
    id: &str,
    slots: &CaptureSlots,
    target: VideoFormat,
    mut open: impl FnMut(&str, VideoFormat) -> Result<S, CameraManError>,
) {
    let stop = slots.stop.as_ref();
    let Some(_lease) = wait_for_camera_lease(id, stop) else {
        return;
    };
    let mut backoff = ReconnectBackoff::default();
    while !stop.load(Ordering::Relaxed) {
        slots.publish_phase(WorkerPhase::Opening);
        // An uncontained panic here would leave no signal at all: the worker is
        // gone, so the watchdog would go on reporting the open as still running,
        // which is false. Returning also releases the camera lease, which a
        // dead worker would have released anyway.
        let opened = match panic::catch_unwind(AssertUnwindSafe(|| open(id, target))) {
            Ok(opened) => opened,
            Err(panic_payload) => {
                let message = panic_message(panic_payload);
                *slots
                    .last_error
                    .lock()
                    .expect("camera error mutex poisoned") = Some(CameraManError::capture(
                    format!("camera open panicked: {message}"),
                ));
                return;
            }
        };
        let mut source = match opened {
            Ok(source) => source,
            Err(error) => {
                *slots
                    .last_error
                    .lock()
                    .expect("camera error mutex poisoned") = Some(error);
                *slots
                    .negotiated_format
                    .lock()
                    .expect("camera format mutex poisoned") = None;
                if !sleep_until_retry(stop, backoff.next_delay()) {
                    return;
                }
                continue;
            }
        };
        slots.publish_phase(WorkerPhase::AwaitingFirstFrame);
        *slots
            .negotiated_format
            .lock()
            .expect("camera format mutex poisoned") = Some(source.negotiated_format());
        // A successful open is not recovery yet. Keep any prior error visible
        // until this session has delivered a genuinely new frame.

        let mut failure_streak = 0_u8;
        while !stop.load(Ordering::Relaxed) {
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| source.latest_frame()));
            match outcome {
                Ok(Ok(frame)) => {
                    failure_streak = 0;
                    backoff.observe_frame(frame.is_some());
                    if frame.is_some() {
                        slots.publish_phase(WorkerPhase::Streaming);
                    }
                    *slots.latest.lock().expect("camera frame mutex poisoned") = frame;
                    *slots
                        .last_error
                        .lock()
                        .expect("camera error mutex poisoned") = None;
                }
                Ok(Err(error)) => {
                    failure_streak = failure_streak.saturating_add(1);
                    *slots
                        .last_error
                        .lock()
                        .expect("camera error mutex poisoned") = Some(error);
                    if failure_streak >= 3 {
                        break;
                    }
                    if !sleep_until_retry(stop, Duration::from_millis(50)) {
                        return;
                    }
                }
                Err(panic_payload) => {
                    let message = panic_message(panic_payload);
                    *slots
                        .last_error
                        .lock()
                        .expect("camera error mutex poisoned") = Some(CameraManError::capture(
                        format!("capture worker panicked: {message}"),
                    ));
                    break;
                }
            }
        }
        *slots
            .negotiated_format
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

impl FrameSource for ThreadedNokhwaFrameSource {
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
        if let Some(error) = self
            .slots
            .last_error
            .lock()
            .expect("camera error mutex poisoned")
            .clone()
        {
            return Err(error);
        }
        // Error first, watchdog second: a concrete failure always outranks the
        // deadline, which only speaks where nothing else can.
        let progress = self.slots.progress();
        if let Some(error) = stalled_open_error(
            &self.id,
            progress.phase,
            self.pre_frame_elapsed(progress),
            self.open_timeout,
        ) {
            return Err(error);
        }
        Ok(self
            .slots
            .latest
            .lock()
            .expect("camera frame mutex poisoned")
            .clone())
    }
}

impl Drop for ThreadedNokhwaFrameSource {
    fn drop(&mut self) {
        self.slots.stop.store(true, Ordering::Relaxed);
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

/// One frame from `id`, waited for up to `timeout`.
///
/// Runs on the same leased capture worker as the streaming path, so a one-shot
/// can never open a camera id that a capture worker holds, and vice versa. A
/// timeout drops the source, which sets `stop`: a worker still parked inside
/// the backend keeps the camera's lease until that open returns and then exits
/// without reopening, so a timed-out one-shot leaves no work that opens a
/// device later. A timeout therefore means "camera still busy", not "camera
/// free again".
///
/// `timeout` is the caller's budget only. The source's own pre-frame watchdog
/// (`camera_open_timeout()`) names the phase that stalled (lease wait, open,
/// first frame), which is the more useful report; whichever deadline expires
/// first speaks. The source's clock starts after this one (thread spawn, lease
/// acquisition), so a caller that wants the watchdog's message must budget
/// strictly more than `camera_open_timeout()`, not the same value.
pub fn capture_one_with_timeout(
    id: &str,
    timeout: Duration,
) -> Result<CapturedFrame, CameraManError> {
    let mut source = ThreadedNokhwaFrameSource::open_id(id);
    first_frame_within(&mut source, id, timeout)
}

/// Polls `source` until it publishes a frame, reports an error, or `timeout`
/// expires. A source error outranks the deadline, so a named failure is never
/// downgraded to a generic timeout.
fn first_frame_within(
    source: &mut impl FrameSource,
    id: &str,
    timeout: Duration,
) -> Result<CapturedFrame, CameraManError> {
    const POLL_INTERVAL: Duration = Duration::from_millis(20);
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(frame) = source.latest_frame()? {
            return Ok(frame);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CameraManError::capture_with_kind(
                CaptureErrorKind::Timeout,
                format!(
                    "camera {id} delivered no frame within {}ms",
                    timeout.as_millis()
                ),
            ));
        }
        thread::sleep(remaining.min(POLL_INTERVAL));
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
    use crate::camera::SyntheticFrameSource;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

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

    const TEST_TARGET: VideoFormat = VideoFormat {
        width: 1280,
        height: 720,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    };

    /// Never produces a frame, so a worker built on it stays in the
    /// pre-streaming phases the watchdog covers.
    struct SilentSource;

    impl FrameSource for SilentSource {
        fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
            Ok(None)
        }
    }

    impl OpenableSource for SilentSource {
        fn negotiated_format(&self) -> VideoFormat {
            TEST_TARGET
        }
    }

    /// Publishes nothing for the first `silent_polls` reads, then a real frame,
    /// like a worker that is still warming up when the reader first polls it.
    struct WarmingSource {
        silent_polls: u32,
        polls: u32,
        inner: SyntheticFrameSource,
    }

    impl FrameSource for WarmingSource {
        fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
            self.polls += 1;
            if self.polls <= self.silent_polls {
                return Ok(None);
            }
            self.inner.latest_frame()
        }
    }

    /// Reports the same failure on every read, like a source whose worker
    /// published an open failure or a watchdog timeout.
    struct FailingSource;

    impl FrameSource for FailingSource {
        fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
            Err(CameraManError::capture_with_kind(
                CaptureErrorKind::Timeout,
                "opening camera uid:desk did not return after 20000ms",
            ))
        }
    }

    /// Polls `ready` until it holds or `budget` expires, so a test can observe a
    /// worker's progress without pinning a wall-clock duration to it.
    fn wait_until(budget: Duration, ready: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            if ready() {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        ready()
    }

    #[test]
    fn open_timeout_defaults_generously_and_honors_the_env_override() {
        assert_eq!(parse_open_timeout(None), Duration::from_secs(20));
        assert_eq!(
            parse_open_timeout(Some(" 2500 ")),
            Duration::from_millis(2500)
        );
        assert_eq!(parse_open_timeout(Some("0")), Duration::from_secs(20));
        assert_eq!(parse_open_timeout(Some("abc")), Duration::from_secs(20));
    }

    #[test]
    fn a_stalled_open_reports_a_timeout_instead_of_a_missing_frame() {
        let error = stalled_open_error(
            "uid:desk",
            WorkerPhase::Opening,
            Duration::from_secs(30),
            Duration::from_secs(20),
        )
        .expect("an open past its deadline must be a failure");

        assert_eq!(error.code(), ErrorCode::Capture(CaptureErrorKind::Timeout));
        assert!(error.diagnostic_message().contains("uid:desk"));
    }

    #[test]
    fn a_warm_up_within_the_deadline_is_not_a_failure() {
        for phase in [WorkerPhase::Opening, WorkerPhase::AwaitingFirstFrame] {
            assert!(
                stalled_open_error(
                    "uid:desk",
                    phase,
                    Duration::from_secs(19),
                    Duration::from_secs(20)
                )
                .is_none()
            );
        }
    }

    #[test]
    fn a_streaming_worker_is_never_reported_as_stalled() {
        assert!(
            stalled_open_error(
                "uid:desk",
                WorkerPhase::Streaming,
                Duration::from_secs(600),
                Duration::from_secs(20)
            )
            .is_none()
        );
    }

    #[test]
    fn a_lease_wait_past_the_deadline_names_the_earlier_open() {
        let error = stalled_open_error(
            "uid:desk",
            WorkerPhase::AwaitingLease,
            Duration::from_secs(21),
            Duration::from_secs(20),
        )
        .expect("a lease wait past its deadline must be a failure");

        assert_eq!(error.code(), ErrorCode::Capture(CaptureErrorKind::Timeout));
        assert!(
            error
                .diagnostic_message()
                .contains("still held by an earlier open")
        );
    }

    #[test]
    fn a_worker_publishes_the_opening_phase_before_calling_the_backend() {
        let id = "camera-worker-publishes-opening";
        let slots = CaptureSlots::default();
        let (entered_open, open_entered) = mpsc::channel();
        let (release_open, open_released) = mpsc::channel();
        let worker_slots = slots.clone();
        let worker = thread::spawn(move || {
            run_capture_worker(id, &worker_slots, TEST_TARGET, move |_, _| {
                entered_open.send(()).expect("test receiver dropped");
                let _ = open_released.recv();
                Ok(SilentSource)
            });
        });

        open_entered.recv().expect("worker never reached open");
        assert_eq!(slots.progress().phase, WorkerPhase::Opening);

        slots.stop.store(true, Ordering::Relaxed);
        let _ = release_open.send(());
        worker.join().expect("capture worker panicked");
    }

    #[test]
    fn an_abandoned_open_keeps_its_camera_lease() {
        let id = "camera-abandoned-open-keeps-lease";
        let slots = CaptureSlots::default();
        let (entered_open, open_entered) = mpsc::channel();
        let (release_open, open_released) = mpsc::channel();
        let worker_slots = slots.clone();
        let worker = thread::spawn(move || {
            run_capture_worker(id, &worker_slots, TEST_TARGET, move |_, _| {
                entered_open.send(()).expect("test receiver dropped");
                let _ = open_released.recv();
                Ok(SilentSource)
            });
        });

        open_entered.recv().expect("worker never reached open");
        assert!(
            try_acquire_camera_lease(id).is_none(),
            "a worker parked inside open must keep the device reserved"
        );

        slots.stop.store(true, Ordering::Relaxed);
        let _ = release_open.send(());
        worker.join().expect("capture worker panicked");
        assert!(try_acquire_camera_lease(id).is_some());
    }

    #[test]
    fn a_reader_whose_worker_is_stuck_opening_returns_an_error_not_no_frame() {
        let slots = CaptureSlots::default();
        slots.publish_phase(WorkerPhase::Opening);
        // A zero deadline expires the phase without waiting on a real clock.
        let mut source = ThreadedNokhwaFrameSource::detached(
            "uid:desk",
            slots.clone(),
            Duration::ZERO,
            Duration::ZERO,
        );

        let error = source
            .latest_frame()
            .expect_err("a stuck open must not read as a missing frame");
        assert_eq!(error.code(), ErrorCode::Capture(CaptureErrorKind::Timeout));

        slots.publish_phase(WorkerPhase::Streaming);
        assert!(source.latest_frame().unwrap().is_none());
    }

    #[test]
    fn a_replacement_for_a_wedged_camera_does_not_restart_as_a_warm_up() {
        // The wedged worker still holds the lease, so the replacement opens in
        // `AwaitingLease` with its predecessor's stall carried over.
        let slots = CaptureSlots::default();
        let mut replacement = ThreadedNokhwaFrameSource::detached(
            "uid:desk",
            slots.clone(),
            Duration::from_secs(20),
            Duration::from_secs(30),
        );

        let error = replacement
            .latest_frame()
            .expect_err("a retry blocked by a wedged worker must not read as a warm-up");
        assert_eq!(error.code(), ErrorCode::Capture(CaptureErrorKind::Timeout));
        assert!(
            error
                .diagnostic_message()
                .contains("still held by an earlier open")
        );

        // Acquiring the lease means the earlier open returned: the inherited
        // stall must not follow the worker into its own open call.
        slots.publish_phase(WorkerPhase::Opening);
        assert!(replacement.latest_frame().unwrap().is_none());
        assert_eq!(replacement.stalled_open_age(), Duration::ZERO);
    }

    #[test]
    fn a_one_shot_returns_the_first_frame_its_source_publishes() {
        let mut source = WarmingSource {
            silent_polls: 2,
            polls: 0,
            inner: SyntheticFrameSource::with_id("camera-0", 8, 8, [1, 2, 3, 255]),
        };

        let frame = first_frame_within(&mut source, "camera-0", Duration::from_secs(5))
            .expect("an empty read is a retry, not a failure");

        assert_eq!(frame.metadata().source_id, "camera-0");
        assert_eq!(
            source.polls, 3,
            "the wait must end on the first real frame, not keep polling"
        );
    }

    #[test]
    fn a_one_shot_reports_the_sources_error_instead_of_waiting_out_its_deadline() {
        let started = Instant::now();

        let error = first_frame_within(&mut FailingSource, "uid:desk", Duration::from_secs(30))
            .expect_err("a source that reports a failure must not read as a slow warm-up");

        assert_eq!(error.code(), ErrorCode::Capture(CaptureErrorKind::Timeout));
        assert!(
            error
                .diagnostic_message()
                .contains("opening camera uid:desk did not return")
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_one_shot_that_never_receives_a_frame_times_out_naming_the_camera() {
        let error = first_frame_within(&mut SilentSource, "uid:desk", Duration::ZERO)
            .expect_err("a source that never publishes must not block forever");

        assert_eq!(error.code(), ErrorCode::Capture(CaptureErrorKind::Timeout));
        assert!(error.diagnostic_message().contains("uid:desk"));
    }

    #[test]
    fn a_stopped_worker_does_not_open_the_camera_again_after_its_in_flight_open_returns() {
        let id = "camera-stopped-worker-does-not-reopen";
        let opens = Arc::new(AtomicUsize::new(0));
        let slots = CaptureSlots::default();
        let (entered_open, open_entered) = mpsc::channel();
        let (release_open, open_released) = mpsc::channel();
        let worker_slots = slots.clone();
        let counter = Arc::clone(&opens);
        let worker = thread::spawn(move || {
            run_capture_worker(id, &worker_slots, TEST_TARGET, move |_, _| {
                counter.fetch_add(1, Ordering::Relaxed);
                entered_open.send(()).expect("test receiver dropped");
                let _ = open_released.recv();
                Ok(SilentSource)
            });
        });

        open_entered.recv().expect("worker never reached open");
        // What a timed-out one-shot does when it drops its source.
        slots.stop.store(true, Ordering::Relaxed);
        let _ = release_open.send(());
        worker.join().expect("capture worker panicked");

        assert_eq!(
            opens.load(Ordering::Relaxed),
            1,
            "a stopped worker must not reopen the camera once its abandoned open returns"
        );
        assert!(
            try_acquire_camera_lease(id).is_some(),
            "the lease is released when the worker finishes, not when the caller gave up"
        );
    }

    #[test]
    fn a_panicking_open_is_reported_instead_of_leaving_the_worker_silently_dead() {
        let id = "camera-open-panics";
        let slots = CaptureSlots::default();
        let worker_slots = slots.clone();
        let worker = thread::spawn(move || {
            run_capture_worker(
                id,
                &worker_slots,
                TEST_TARGET,
                |_, _| -> Result<SilentSource, CameraManError> { panic!("backend open exploded") },
            );
        });

        worker
            .join()
            .expect("a panicking open must not unwind the worker thread");

        let error = slots
            .last_error
            .lock()
            .expect("camera error mutex poisoned")
            .clone()
            .expect("a panicking open must publish an error, not stay silent");
        assert!(
            error.diagnostic_message().contains("backend open exploded"),
            "the panic message must reach the reader: {}",
            error.diagnostic_message()
        );
        assert!(
            try_acquire_camera_lease(id).is_some(),
            "a worker that gave up on a panicking open must release the camera"
        );
    }

    #[test]
    fn a_one_shot_waits_for_the_camera_lease_instead_of_opening_the_device() {
        let id = "camera-one-shot-waits-for-the-lease";
        // Never released: the one-shot must be unable to reach a backend even if
        // its worker is preempted between its stop check and its lease attempt.
        // The id is unique to this test, so holding it forever is inert.
        std::mem::forget(
            try_acquire_camera_lease(id).expect("the test must start with the camera free"),
        );

        let error = capture_one_with_timeout(id, Duration::from_millis(100))
            .expect_err("a one-shot must not open a camera id another path holds");

        assert_eq!(error.code(), ErrorCode::Capture(CaptureErrorKind::Timeout));
        assert!(
            error
                .diagnostic_message()
                .contains("delivered no frame within 100ms"),
            "a leased one-shot reports its own deadline, not a backend open failure: {}",
            error.diagnostic_message()
        );
    }

    #[test]
    fn a_second_capture_path_cannot_open_a_camera_the_first_still_holds() {
        let id = "camera-second-path-waits-for-the-first";
        let opens_b = Arc::new(AtomicUsize::new(0));
        let slots_a = CaptureSlots::default();
        let slots_b = CaptureSlots::default();

        let (entered_a, a_entered) = mpsc::channel();
        let (release_a, a_released) = mpsc::channel();
        let worker_slots_a = slots_a.clone();
        let worker_a = thread::spawn(move || {
            run_capture_worker(id, &worker_slots_a, TEST_TARGET, move |_, _| {
                entered_a.send(()).expect("test receiver dropped");
                let _ = a_released.recv();
                Ok(SilentSource)
            });
        });
        a_entered.recv().expect("first worker never reached open");

        let (release_b, b_released) = mpsc::channel();
        let worker_slots_b = slots_b.clone();
        let counter_b = Arc::clone(&opens_b);
        let worker_b = thread::spawn(move || {
            run_capture_worker(id, &worker_slots_b, TEST_TARGET, move |_, _| {
                counter_b.fetch_add(1, Ordering::Relaxed);
                let _ = b_released.recv();
                Ok(SilentSource)
            });
        });

        thread::sleep(Duration::from_millis(50));
        let opened_while_first_holds = opens_b.load(Ordering::Relaxed);

        slots_a.stop.store(true, Ordering::Relaxed);
        let _ = release_a.send(());
        worker_a.join().expect("first capture worker panicked");

        let opened_after_release = wait_until(Duration::from_millis(500), || {
            opens_b.load(Ordering::Relaxed) == 1
        });

        slots_b.stop.store(true, Ordering::Relaxed);
        let _ = release_b.send(());
        worker_b.join().expect("second capture worker panicked");

        assert_eq!(
            opened_while_first_holds, 0,
            "a second capture path must not open a camera id the first still holds"
        );
        assert!(
            opened_after_release,
            "the second path must open once the first releases the lease"
        );
    }
}
