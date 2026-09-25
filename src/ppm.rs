use crate::error::CameraManError;
use crate::frame::{Frame, FrameMetadata};
use crate::virtual_camera::VirtualCameraSink;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PpmMetadata {
    pub source_id: String,
    pub sequence: u64,
    pub wall_timestamp_nanos: u128,
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
}

impl PpmMetadata {
    pub fn from_frame(metadata: &FrameMetadata, frame: &Frame) -> Self {
        Self {
            source_id: metadata.source_id.clone(),
            sequence: metadata.sequence,
            wall_timestamp_nanos: metadata.wall_timestamp().0,
            width: frame.width(),
            height: frame.height(),
            pixel_format: format!("{:?}", frame.pixel_format()),
        }
    }
}

pub fn write_ppm(path: impl AsRef<Path>, frame: &Frame) -> Result<(), CameraManError> {
    let path = path.as_ref();
    let result = (|| {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // BufWriter matters here: the unbuffered version issued one syscall
        // per pixel (~2 million writes for a 1080p frame).
        let mut file = BufWriter::new(File::create(path)?);
        writeln!(file, "P6")?;
        writeln!(file, "{} {}", frame.width(), frame.height())?;
        writeln!(file, "255")?;
        for pixel in frame.data().chunks_exact(4) {
            file.write_all(&[pixel[2], pixel[1], pixel[0]])?;
        }
        file.flush()?;
        Ok(())
    })();
    result.map_err(|error: CameraManError| error.context(format!("write {}", path.display())))
}

pub fn write_ppm_with_metadata(
    path: impl AsRef<Path>,
    frame: &Frame,
    metadata: &FrameMetadata,
) -> Result<(), CameraManError> {
    let path = path.as_ref();
    write_ppm(path, frame)?;
    let sidecar_path = path.with_extension("json");
    let sidecar = PpmMetadata::from_frame(metadata, frame);
    let result = (|| {
        let file = BufWriter::new(File::create(&sidecar_path)?);
        serde_json::to_writer_pretty(file, &sidecar).map_err(|error| CameraManError::Io {
            kind: std::io::ErrorKind::InvalidData,
            message: error.to_string(),
        })?;
        Ok(())
    })();
    result
        .map_err(|error: CameraManError| error.context(format!("write {}", sidecar_path.display())))
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

    fn path_for(&self, metadata: &FrameMetadata) -> PathBuf {
        self.directory.join(format!(
            "{}-{:06}-{}.ppm",
            self.prefix,
            metadata.sequence,
            metadata.wall_timestamp().0
        ))
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
        let metadata = FrameMetadata::new(&self.prefix, self.frames_sent as u64 + 1);
        let path = self.path_for(&metadata);
        write_ppm_with_metadata(path, frame, &metadata)?;
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
    use crate::error::ErrorCode;
    use crate::frame::Frame;

    #[test]
    fn writes_ppm_file() {
        let path = std::env::temp_dir().join(format!("camera-man-test-{}.ppm", std::process::id()));
        let frame = Frame::solid_bgra(1, 1, [10, 66, 65, 255]).unwrap();

        write_ppm(&path, &frame).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(bytes, include_bytes!("../tests/golden/ppm-writer.ppm"));
    }

    #[test]
    fn write_failure_keeps_io_code_and_path_context() {
        let path = std::env::temp_dir().join(format!(
            "camera-man-ppm-directory-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        let frame = Frame::solid_bgra(1, 1, [1, 2, 3, 255]).unwrap();

        let error = write_ppm(&path, &frame).unwrap_err();
        let _ = std::fs::remove_dir_all(&path);

        assert_eq!(error.code(), ErrorCode::Io);
        assert!(
            error
                .diagnostic_message()
                .contains(&path.display().to_string())
        );
    }

    #[test]
    fn sequence_sink_writes_timestamped_ppm_and_json_sidecar() {
        let directory = std::env::temp_dir().join(format!(
            "camera-man-ppm-sequence-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        let frame = Frame::solid_bgra(1, 1, [1, 2, 3, 255]).unwrap();
        let mut sink = PpmSequenceSink::new(&directory, "capture");

        sink.connect().unwrap();
        sink.send(&frame).unwrap();
        sink.disconnect();

        let files = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        let ppm_path = files
            .iter()
            .find(|path| path.extension().is_some_and(|extension| extension == "ppm"))
            .unwrap();
        let json_path = files
            .iter()
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .unwrap();
        let metadata: PpmMetadata =
            serde_json::from_slice(&std::fs::read(json_path).unwrap()).unwrap();

        assert!(
            ppm_path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .contains("capture-000001-")
        );
        assert_eq!(metadata.source_id, "capture");
        assert_eq!(metadata.sequence, 1);
        assert_eq!(metadata.width, 1);
        assert_eq!(metadata.pixel_format, "Bgra8");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
