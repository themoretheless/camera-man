use std::time::Duration;

use crate::config::VideoFormat;
use crate::error::CameraManError;
use crate::frame::{CapturedFrame, Frame, FrameMetadata};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraDevice {
    pub id: String,
    pub name: String,
    /// Previous/runtime locators that should migrate to `id` when discovered.
    pub aliases: Vec<String>,
}

impl CameraDevice {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            aliases: Vec::new(),
        }
    }

    pub fn with_aliases(
        id: impl Into<String>,
        name: impl Into<String>,
        aliases: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            aliases,
        }
    }
}

/// Enumerates camera-like inputs available to a backend.
pub trait CameraDiscovery {
    /// Returns a point-in-time device list. Discovery may block and callers
    /// that own a UI thread should run it on a worker.
    fn list_devices(&self) -> Result<Vec<CameraDevice>, CameraManError>;
}

/// Produces the newest frame for one logical input.
///
/// `Ok(None)` means the source is healthy but has no frame ready. It must not
/// be used for failures; return `Err` so pipeline metrics and UI recovery can
/// distinguish a dropped frame from a broken source.
///
/// A complete implementation is available in `examples/custom_source.rs`.
pub trait FrameSource {
    /// Reads or clones the newest available frame.
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError>;
}

/// A live capture source plus the device state its owner can read.
///
/// Kept apart from [`FrameSource`] on purpose: a caller that only composes
/// frames needs neither negotiated format nor lease history, so sources that
/// cannot answer these questions — a synthetic pattern, a file — stay plain
/// `FrameSource` values. Implement this to add a camera backend the app can
/// drive without knowing the backend's type, as
/// `tests/camera_backend_seam.rs` does against the published API only.
pub trait CameraRuntime: FrameSource + Send {
    /// The device's real format, or `None` until the backend has finished
    /// opening it.
    fn negotiated_format(&self) -> Option<VideoFormat>;

    /// The device's real frame rate, once known.
    fn negotiated_fps(&self) -> Option<u32> {
        self.negotiated_format().map(|format| format.fps)
    }

    /// How long this source has gone without advancing, either before its first
    /// frame or between frames, or `Duration::ZERO` while it is healthy or still
    /// within budget.
    fn stall_age(&self) -> Duration;

    /// Whether a source that had already streamed has passed its frame-gap
    /// deadline: open and negotiated, but no new frame is arriving.
    fn frame_gap_exceeded(&self) -> bool;

    /// Hands out the frame-producing half of this source. A `dyn CameraRuntime`
    /// value resolves only its own methods, so an owner holding one still needs
    /// this to reach [`FrameSource::latest_frame`].
    fn as_frame_source(&mut self) -> &mut dyn FrameSource;
}

/// Deterministic solid-color source used by demos and tests.
#[derive(Debug, Clone)]
pub struct SyntheticFrameSource {
    source_id: String,
    width: u32,
    height: u32,
    bgra: [u8; 4],
    sequence: u64,
}

impl SyntheticFrameSource {
    /// Creates a source with the stable id `synthetic`.
    pub fn new(width: u32, height: u32, bgra: [u8; 4]) -> Self {
        Self::with_id("synthetic", width, height, bgra)
    }

    /// Creates a source with an explicit metadata id.
    pub fn with_id(source_id: impl Into<String>, width: u32, height: u32, bgra: [u8; 4]) -> Self {
        Self {
            source_id: source_id.into(),
            width,
            height,
            bgra,
            sequence: 0,
        }
    }
}

impl FrameSource for SyntheticFrameSource {
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
        self.sequence += 1;
        let frame = Frame::solid_bgra(self.width, self.height, self.bgra)?;
        let metadata = FrameMetadata::new(self.source_id.clone(), self.sequence);
        Ok(Some(CapturedFrame::new(frame, metadata)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::PixelFormat;

    /// A camera backend that knows nothing about the capture worker. The app
    /// must be able to drive it through the same handle it uses for the nokhwa
    /// source, which is what keeps a second backend from needing app changes.
    struct ScriptedCameraSource {
        reads: usize,
        format: Option<VideoFormat>,
    }

    impl FrameSource for ScriptedCameraSource {
        fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
            self.reads += 1;
            if self.reads < 2 {
                return Ok(None);
            }
            let frame = Frame::solid_bgra(2, 2, [0, 0, 255, 255])?;
            Ok(Some(CapturedFrame::new(
                frame,
                FrameMetadata::new("scripted".to_owned(), self.reads as u64),
            )))
        }
    }

    impl CameraRuntime for ScriptedCameraSource {
        fn negotiated_format(&self) -> Option<VideoFormat> {
            self.format
        }

        fn stall_age(&self) -> Duration {
            Duration::from_millis(self.reads as u64)
        }

        fn frame_gap_exceeded(&self) -> bool {
            self.reads > 3
        }

        fn as_frame_source(&mut self) -> &mut dyn FrameSource {
            self
        }
    }

    fn assert_send<T: Send>(_: &T) {}

    #[test]
    fn a_backend_can_be_driven_through_the_handle_the_app_stores() {
        let mut slots: Vec<(String, Box<dyn CameraRuntime>, u32)> = vec![(
            "scripted".to_owned(),
            Box::new(ScriptedCameraSource {
                reads: 0,
                format: Some(VideoFormat {
                    width: 2,
                    height: 2,
                    fps: 7,
                    pixel_format: PixelFormat::Bgra8,
                }),
            }),
            0,
        )];
        assert_send(&slots);

        let first = slots[0].1.as_frame_source().latest_frame().unwrap();
        assert!(first.is_none(), "Ok(None) means nothing is ready yet");
        let second = slots[0].1.as_frame_source().latest_frame().unwrap();
        assert!(second.is_some());

        assert_eq!(
            slots[0].1.negotiated_fps(),
            Some(7),
            "the frame rate is derived from the negotiated format"
        );
        assert_eq!(slots[0].1.stall_age(), Duration::from_millis(2));
        assert!(!slots[0].1.frame_gap_exceeded());
    }

    #[test]
    fn a_source_without_device_state_stays_a_plain_frame_source() {
        let mut source = SyntheticFrameSource::new(2, 2, [255, 0, 0, 255]);
        assert!(source.latest_frame().unwrap().is_some());
    }
}
