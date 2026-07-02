use crate::error::CameraManError;
use crate::frame::Frame;
use crate::virtual_camera::VirtualCameraSink;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub fn write_ppm(path: impl AsRef<Path>, frame: &Frame) -> Result<(), CameraManError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // BufWriter matters here: the unbuffered version issued one syscall per
    // pixel (~2 million writes for a 1080p frame).
    let mut file = BufWriter::new(File::create(path)?);
    writeln!(file, "P6")?;
    writeln!(file, "{} {}", frame.width(), frame.height())?;
    writeln!(file, "255")?;
    for pixel in frame.data().chunks_exact(4) {
        file.write_all(&[pixel[2], pixel[1], pixel[0]])?;
    }
    file.flush()?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PpmSequenceSink {
    directory: PathBuf,
    prefix: String,
    connected: bool,
    frames_sent: usize,
}

impl PpmSequenceSink {
    pub fn new(directory: impl Into<PathBuf>, prefix: impl Into<String>) -> Self {
        Self {
            directory: directory.into(),
            prefix: prefix.into(),
            connected: false,
            frames_sent: 0,
        }
    }

    pub const fn frames_sent(&self) -> usize {
        self.frames_sent
    }

    pub fn next_path(&self) -> PathBuf {
        self.directory
            .join(format!("{}-{:06}.ppm", self.prefix, self.frames_sent + 1))
    }
}

impl VirtualCameraSink for PpmSequenceSink {
    fn connect(&mut self) -> Result<(), CameraManError> {
        std::fs::create_dir_all(&self.directory)?;
        self.connected = true;
        Ok(())
    }

    fn send(&mut self, frame: &Frame) -> Result<(), CameraManError> {
        if !self.connected {
            return Err(CameraManError::VirtualCameraUnavailable(
                "ppm sequence sink is not connected",
            ));
        }
        let path = self.next_path();
        write_ppm(path, frame)?;
        self.frames_sent += 1;
        Ok(())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    #[test]
    fn writes_ppm_file() {
        let path = std::env::temp_dir().join(format!("camera-man-test-{}.ppm", std::process::id()));
        let frame = Frame::solid_bgra(1, 1, [1, 2, 3, 255]).unwrap();

        write_ppm(&path, &frame).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(bytes.starts_with(b"P6\n1 1\n255\n"));
        assert_eq!(&bytes[11..], &[3, 2, 1]);
    }
}
