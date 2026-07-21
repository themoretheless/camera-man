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
