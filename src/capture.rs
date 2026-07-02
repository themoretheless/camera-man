use crate::camera::{CameraDevice, CameraDiscovery, FrameSource};
use crate::error::CameraManError;
use crate::frame::{CapturedFrame, Frame, FrameMetadata, PixelFormat};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
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

pub struct ThreadedNokhwaFrameSource {
    latest: Arc<Mutex<Option<CapturedFrame>>>,
    last_error: Arc<Mutex<Option<String>>>,
    stop: Arc<AtomicBool>,
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
        let stop = Arc::new(AtomicBool::new(false));

        let worker_latest = Arc::clone(&latest);
        let worker_last_error = Arc::clone(&last_error);
        let worker_stop = Arc::clone(&stop);
        let worker_id = id.to_string();

        thread::spawn(move || {
            let mut source = match NokhwaFrameSource::open_id(&worker_id) {
                Ok(source) => source,
                Err(error) => {
                    *worker_last_error
                        .lock()
                        .expect("camera error mutex poisoned") = Some(error.to_string());
                    return;
                }
            };

            while !worker_stop.load(Ordering::Relaxed) {
                match source.latest_frame() {
                    Ok(frame) => {
                        *worker_latest.lock().expect("camera frame mutex poisoned") = frame;
                        *worker_last_error
                            .lock()
                            .expect("camera error mutex poisoned") = None;
                    }
                    Err(error) => {
                        *worker_last_error
                            .lock()
                            .expect("camera error mutex poisoned") = Some(error.to_string());
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });

        Self {
            latest,
            last_error,
            stop,
        }
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
            return Err(CameraManError::Capture(error));
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
    }
}

pub fn capture_one_with_timeout(
    id: &str,
    timeout: Duration,
) -> Result<CapturedFrame, CameraManError> {
    let id = id.to_string();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let result = (|| {
            let mut source = NokhwaFrameSource::open_id(&id)?;
            source.latest_frame()?.ok_or(CameraManError::EmptyInput)
        })();
        let _ = sender.send(result);
    });

    receiver.recv_timeout(timeout).map_err(|_| {
        CameraManError::Capture(format!("timed out after {}ms", timeout.as_millis()))
    })?
}

fn nokhwa_error(error: nokhwa::NokhwaError) -> CameraManError {
    CameraManError::Capture(error.to_string())
}
