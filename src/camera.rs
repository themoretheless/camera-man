use crate::error::CameraManError;
use crate::frame::{CapturedFrame, Frame, FrameMetadata};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraDevice {
    pub id: String,
    pub name: String,
}

pub trait CameraDiscovery {
    fn list_devices(&self) -> Result<Vec<CameraDevice>, CameraManError>;
}

pub trait FrameSource {
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError>;
}

#[derive(Debug, Clone)]
pub struct SyntheticFrameSource {
    source_id: String,
    width: u32,
    height: u32,
    bgra: [u8; 4],
    sequence: u64,
}

impl SyntheticFrameSource {
    pub fn new(width: u32, height: u32, bgra: [u8; 4]) -> Self {
        Self::with_id("synthetic", width, height, bgra)
    }

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
