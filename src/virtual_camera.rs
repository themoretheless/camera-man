use crate::error::CameraManError;
use crate::frame::Frame;

/// Lifecycle boundary for a composed-frame destination.
///
/// Call `connect` before `send`; `disconnect` must be idempotent so pipeline
/// stop and drop paths can both release resources safely. A complete custom
/// implementation is available in `examples/custom_sink.rs`.
pub trait VirtualCameraSink {
    /// Acquires the destination and prepares it to receive frames.
    fn connect(&mut self) -> Result<(), CameraManError>;
    /// Publishes one complete tightly packed frame.
    fn send(&mut self, frame: &Frame) -> Result<(), CameraManError>;
    /// Releases destination resources. Repeated calls must be harmless.
    fn disconnect(&mut self);
}

/// In-memory sink for tests and small embedded examples.
#[derive(Debug, Default)]
pub struct MemorySink {
    connected: bool,
    frames_sent: usize,
    last_frame: Option<Frame>,
}

impl MemorySink {
    /// Number of frames accepted since construction.
    pub const fn frames_sent(&self) -> usize {
        self.frames_sent
    }

    /// Newest accepted frame, if any.
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
