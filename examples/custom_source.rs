use camera_man::{CameraManError, CapturedFrame, Frame, FrameMetadata, FrameSource, PixelFormat};

struct CounterSource {
    sequence: u64,
}

impl FrameSource for CounterSource {
    fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
        self.sequence += 1;
        let pixels = vec![20, 40, 60, 255];
        let frame = Frame::new_checked(1, 1, PixelFormat::Bgra8, pixels)?;
        let metadata = FrameMetadata::new("counter", self.sequence);
        Ok(Some(CapturedFrame::new(frame, metadata)))
    }
}

fn main() -> Result<(), CameraManError> {
    let mut source = CounterSource { sequence: 0 };
    let captured = source.latest_frame()?.ok_or(CameraManError::EmptyInput)?;

    assert_eq!(captured.metadata().sequence, 1);
    assert_eq!(captured.frame().bgra_at(0, 0), Some([20, 40, 60, 255]));
    Ok(())
}
