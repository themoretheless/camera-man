use camera_man::{CameraManError, Frame, VirtualCameraSink};

#[derive(Default)]
struct CountingSink {
    connected: bool,
    frames_sent: u64,
}

impl VirtualCameraSink for CountingSink {
    fn connect(&mut self) -> Result<(), CameraManError> {
        self.connected = true;
        Ok(())
    }

    fn send(&mut self, _frame: &Frame) -> Result<(), CameraManError> {
        if !self.connected {
            return Err(CameraManError::VirtualCameraUnavailable(
                "counting sink is disconnected",
            ));
        }
        self.frames_sent += 1;
        Ok(())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }
}

fn main() -> Result<(), CameraManError> {
    let frame = Frame::solid_bgra(2, 2, [20, 40, 60, 255])?;
    let mut sink = CountingSink::default();

    sink.connect()?;
    sink.send(&frame)?;
    sink.disconnect();

    assert_eq!(sink.frames_sent, 1);
    Ok(())
}
