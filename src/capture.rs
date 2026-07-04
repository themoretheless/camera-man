use crate::camera::{CameraDevice, CameraDiscovery, FrameSource};
use crate::error::CameraManError;
use crate::frame::{CapturedFrame, Frame, FrameMetadata, PixelFormat};
use std::panic::{self, AssertUnwindSafe};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct NokhwaCameraDiscovery;

impl CameraDiscovery for NokhwaCameraDiscovery {
    fn list_devices(&self) -> Result<Vec<CameraDevice>, CameraManError> {
        let devices = nokhwa::query(nokhwa::utils::ApiBackend::Auto).map_err(nokhwa_error)?;
        Ok(devices
            .into_iter()
            .map(|info| CameraDevice {
                id: info.index().as_string(),
                name: info.human_name().to_string(),
            })
            .collect())
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
/// the camera call unblocks, without ever blocking the caller.
pub struct ThreadedNokhwaFrameSource {
    latest: Arc<Mutex<Option<CapturedFrame>>>,
    last_error: Arc<Mutex<Option<CameraManError>>>,
    /// Set once the worker has actually opened the device; `None` while still
    /// warming up. This is the camera's real negotiated rate, not a guess.
    negotiated_fps: Arc<Mutex<Option<u32>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl NokhwaFrameSource {
    pub fn open_index(index: u32) -> Result<Self, CameraManError> {
        Self::open_camera_index(
            nokhwa::utils::CameraIndex::Index(index),
            format!("camera-{index}"),
        )
    }

    pub fn open_id(id: &str) -> Result<Self, CameraManError> {
        let index = id
            .parse::<u32>()
            .map(nokhwa::utils::CameraIndex::Index)
            .unwrap_or_else(|_| nokhwa::utils::CameraIndex::String(id.to_string()));
        Self::open_camera_index(index, format!("camera-{id}"))
    }

    fn open_camera_index(
        index: nokhwa::utils::CameraIndex,
        source_id: String,
    ) -> Result<Self, CameraManError> {
        let requested = nokhwa::utils::RequestedFormat::new::<nokhwa::pixel_format::RgbFormat>(
            nokhwa::utils::RequestedFormatType::AbsoluteHighestFrameRate,
        );
        let mut camera = nokhwa::Camera::new(index, requested).map_err(nokhwa_error)?;
        camera.open_stream().map_err(nokhwa_error)?;
        Ok(Self {
            source_id,
            camera,
            sequence: 0,
        })
    }
}

impl NokhwaFrameSource {
    /// The frame rate actually negotiated with the device at open time (via
    /// `RequestedFormatType::AbsoluteHighestFrameRate`), not a guess.
    pub fn frame_rate(&self) -> u32 {
        self.camera.frame_rate()
    }
}

impl FrameSource for NokhwaFrameSource {
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
        let buffer = self.camera.frame().map_err(nokhwa_error)?;
        let image = buffer
            .decode_image::<nokhwa::pixel_format::RgbFormat>()
            .map_err(nokhwa_error)?;
        let width = image.width();
        let height = image.height();
        let rgb = image.as_raw();
        let mut bgra = Vec::with_capacity(rgb.len() / 3 * PixelFormat::Bgra8.bytes_per_pixel());

        for pixel in rgb.chunks_exact(3) {
            bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }

        self.sequence += 1;
        let frame = Frame::new_checked(width, height, PixelFormat::Bgra8, bgra)?;
        let metadata = FrameMetadata::new(self.source_id.clone(), self.sequence);
        Ok(Some(CapturedFrame::new(frame, metadata)))
    }
}

impl ThreadedNokhwaFrameSource {
    pub fn open_id(id: &str) -> Self {
        let latest = Arc::new(Mutex::new(None));
        let last_error = Arc::new(Mutex::new(None));
        let negotiated_fps = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));

        let worker_latest = Arc::clone(&latest);
        let worker_last_error = Arc::clone(&last_error);
        let worker_negotiated_fps = Arc::clone(&negotiated_fps);
        let worker_stop = Arc::clone(&stop);
        let worker_id = id.to_string();

        let worker = thread::Builder::new()
            .name(format!("camera-capture-{id}"))
            .spawn(move || {
                run_capture_worker(
                    &worker_id,
                    &worker_latest,
                    &worker_last_error,
                    &worker_negotiated_fps,
                    &worker_stop,
                );
            })
            .expect("failed to spawn camera capture thread");

        Self {
            latest,
            last_error,
            negotiated_fps,
            stop,
            worker: Some(worker),
        }
    }

    /// The camera's real negotiated frame rate, once known. `None` until the
    /// worker thread has finished opening the device.
    pub fn negotiated_fps(&self) -> Option<u32> {
        *self
            .negotiated_fps
            .lock()
            .expect("camera fps mutex poisoned")
    }
}

/// Body of the background capture loop. Runs until `stop` is set or the
/// initial `open_id` fails. A panic inside a single iteration (camera or
/// mutex failure) is caught so it surfaces as a normal error on the
/// `last_error` slot instead of silently killing the thread with no signal:
/// without this, `latest_frame()` would keep returning the last-known-good
/// frame forever and the preview would look frozen-but-healthy.
fn run_capture_worker(
    id: &str,
    latest: &Arc<Mutex<Option<CapturedFrame>>>,
    last_error: &Arc<Mutex<Option<CameraManError>>>,
    negotiated_fps: &Arc<Mutex<Option<u32>>>,
    stop: &Arc<AtomicBool>,
) {
    let mut source = match NokhwaFrameSource::open_id(id) {
        Ok(source) => source,
        Err(error) => {
            *last_error.lock().expect("camera error mutex poisoned") = Some(error);
            return;
        }
    };
    *negotiated_fps.lock().expect("camera fps mutex poisoned") = Some(source.frame_rate());

    while !stop.load(Ordering::Relaxed) {
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| source.latest_frame()));
        match outcome {
            Ok(Ok(frame)) => {
                *latest.lock().expect("camera frame mutex poisoned") = frame;
                *last_error.lock().expect("camera error mutex poisoned") = None;
            }
            Ok(Err(error)) => {
                *last_error.lock().expect("camera error mutex poisoned") = Some(error);
            }
            Err(panic_payload) => {
                let message = panic_message(&panic_payload);
                *last_error.lock().expect("camera error mutex poisoned") = Some(
                    CameraManError::capture(format!("capture worker panicked: {message}")),
                );
                // The camera object may be in an inconsistent state after a
                // panic unwound through it; stop rather than loop on a
                // possibly-corrupt source.
                return;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
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
    let (sender, receiver) = mpsc::channel();

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
        Err(mpsc::RecvTimeoutError::Timeout) => Err(CameraManError::capture(format!(
            "timed out after {}ms waiting for a frame",
            timeout.as_millis()
        ))),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(CameraManError::capture(
            "capture thread exited without sending a result",
        )),
    }
}

fn nokhwa_error(error: nokhwa::NokhwaError) -> CameraManError {
    CameraManError::capture(error.to_string())
}
