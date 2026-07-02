use crate::error::CameraManError;
use crate::frame::Frame;

pub trait VirtualCameraSink {
    fn connect(&mut self) -> Result<(), CameraManError>;
    fn send(&mut self, frame: &Frame) -> Result<(), CameraManError>;
    fn disconnect(&mut self);
}

#[derive(Debug, Default)]
pub struct MemorySink {
    connected: bool,
    frames_sent: usize,
    last_frame: Option<Frame>,
}

impl MemorySink {
    pub const fn frames_sent(&self) -> usize {
        self.frames_sent
    }

    pub fn last_frame(&self) -> Option<&Frame> {
        self.last_frame.as_ref()
    }
}

impl VirtualCameraSink for MemorySink {
    fn connect(&mut self) -> Result<(), CameraManError> {
        self.connected = true;
        Ok(())
    }

    fn send(&mut self, frame: &Frame) -> Result<(), CameraManError> {
        if !self.connected {
            return Err(CameraManError::VirtualCameraUnavailable(
                "sink is not connected",
            ));
        }
        self.frames_sent += 1;
        self.last_frame = Some(frame.clone());
        Ok(())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }
}

#[derive(Debug, Default)]
pub struct UnsupportedVirtualCameraSink;

impl VirtualCameraSink for UnsupportedVirtualCameraSink {
    fn connect(&mut self) -> Result<(), CameraManError> {
        Err(CameraManError::VirtualCameraUnavailable(
            "the macOS CoreMediaIO backend has not been implemented in Rust yet",
        ))
    }

    fn send(&mut self, _frame: &Frame) -> Result<(), CameraManError> {
        Err(CameraManError::VirtualCameraUnavailable(
            "the macOS CoreMediaIO backend has not been implemented in Rust yet",
        ))
    }

    fn disconnect(&mut self) {}
}
