use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::CameraManError;
use crate::frame::{Frame, PixelFormat};
use crate::virtual_camera::VirtualCameraSink;

const MAGIC: &[u8; 4] = b"CMAN";
const VERSION: u32 = 2;
const PIXEL_FORMAT_BGRA8: u32 = 1;
const HEADER_LEN: usize = 56;
/// Used when a producer never called `set_target_fps`; keeps old behavior.
const DEFAULT_FPS: u32 = 30;

pub fn default_frame_spool_path() -> PathBuf {
    std::env::var_os("CAMERAMAN_FRAME_SPOOL")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("cameraman-virtual-frame.bgra"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportFrame {
    pub frame: Frame,
    pub sequence: u64,
    pub timestamp_nanos: u128,
    /// The producer's target frame rate at the time this frame was written.
    /// Lets the extension adapt its own cadence instead of assuming a fixed
    /// constant that may not match what the app is actually producing.
    pub fps: u32,
}

#[derive(Debug, Clone)]
pub struct FrameSpoolSink {
    path: PathBuf,
    connected: bool,
    sequence: u64,
    target_fps: u32,
}

impl Default for FrameSpoolSink {
    fn default() -> Self {
        Self::new(default_frame_spool_path())
    }
}

impl FrameSpoolSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            connected: false,
            sequence: 0,
            target_fps: DEFAULT_FPS,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Advertises the rate this sink is being fed at, so a reader (the CMIO
    /// extension) can match its own cadence instead of assuming a constant.
    /// Takes effect on the next `send`; safe to call at any time, including
    /// while connected.
    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps.max(1);
    }

    pub const fn target_fps(&self) -> u32 {
        self.target_fps
    }
}

impl VirtualCameraSink for FrameSpoolSink {
    fn connect(&mut self) -> Result<(), CameraManError> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        self.connected = true;
        Ok(())
    }

    fn send(&mut self, frame: &Frame) -> Result<(), CameraManError> {
        if !self.connected {
            return Err(CameraManError::VirtualCameraUnavailable(
                "frame spool sink is not connected",
            ));
        }
        write_frame(&self.path, self.sequence, self.target_fps, frame)?;
        self.sequence = self.sequence.wrapping_add(1);
        Ok(())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }
}

pub fn read_latest_frame(path: impl AsRef<Path>) -> Result<Option<TransportFrame>, CameraManError> {
    let path = path.as_ref();
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    parse_frame(&bytes)
}

fn write_frame(path: &Path, sequence: u64, fps: u32, frame: &Frame) -> Result<(), CameraManError> {
    if frame.pixel_format() != PixelFormat::Bgra8 {
        return Err(CameraManError::UnsupportedPixelFormat);
    }

    let temp_path = temporary_path(path);
    let mut file = File::create(&temp_path)?;
    file.write_all(MAGIC)?;
    write_u32(&mut file, VERSION)?;
    write_u32(&mut file, frame.width())?;
    write_u32(&mut file, frame.height())?;
    write_u32(&mut file, PIXEL_FORMAT_BGRA8)?;
    write_u32(&mut file, fps)?;
    write_u64(&mut file, sequence)?;
    write_u128(&mut file, now_nanos())?;
    write_u64(&mut file, frame.data().len() as u64)?;
    file.write_all(frame.data())?;
    file.flush()?;
    drop(file);
    fs::rename(temp_path, path)?;
    Ok(())
}

fn parse_frame(bytes: &[u8]) -> Result<Option<TransportFrame>, CameraManError> {
    if bytes.len() < HEADER_LEN || &bytes[0..4] != MAGIC {
        return Ok(None);
    }

    let version = read_u32(bytes, 4)?;
    let width = read_u32(bytes, 8)?;
    let height = read_u32(bytes, 12)?;
    let pixel_format = read_u32(bytes, 16)?;
    let fps = read_u32(bytes, 20)?;
    let sequence = read_u64(bytes, 24)?;
    let timestamp_nanos = read_u128(bytes, 32)?;
    let data_len = read_u64(bytes, 48)? as usize;

    if version != VERSION || pixel_format != PIXEL_FORMAT_BGRA8 {
        return Ok(None);
    }
    if bytes.len() != HEADER_LEN + data_len {
        return Ok(None);
    }

    let frame = Frame::new_checked(
        width,
        height,
        PixelFormat::Bgra8,
        bytes[HEADER_LEN..].to_vec(),
    )?;
    Ok(Some(TransportFrame {
        frame,
        sequence,
        timestamp_nanos,
        fps: fps.max(1),
    }))
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cameraman-virtual-frame.bgra");
    path.with_file_name(format!("{file_name}.{}.tmp", std::process::id()))
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn write_u32(file: &mut File, value: u32) -> Result<(), CameraManError> {
    Ok(file.write_all(&value.to_le_bytes())?)
}

fn write_u64(file: &mut File, value: u64) -> Result<(), CameraManError> {
    Ok(file.write_all(&value.to_le_bytes())?)
}

fn write_u128(file: &mut File, value: u128) -> Result<(), CameraManError> {
    Ok(file.write_all(&value.to_le_bytes())?)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, CameraManError> {
    let mut value = [0; 4];
    value.copy_from_slice(read_exact(bytes, offset, 4)?);
    Ok(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, CameraManError> {
    let mut value = [0; 8];
    value.copy_from_slice(read_exact(bytes, offset, 8)?);
    Ok(u64::from_le_bytes(value))
}

fn read_u128(bytes: &[u8], offset: usize) -> Result<u128, CameraManError> {
    let mut value = [0; 16];
    value.copy_from_slice(read_exact(bytes, offset, 16)?);
    Ok(u128::from_le_bytes(value))
}

fn read_exact(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], CameraManError> {
    bytes
        .get(offset..offset + len)
        .ok_or(CameraManError::InvalidBufferLength {
            expected: offset + len,
            actual: bytes.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_spool_returns_none() {
        let path = std::env::temp_dir().join(format!(
            "cameraman-missing-{}-{}.bgra",
            std::process::id(),
            now_nanos()
        ));
        assert_eq!(read_latest_frame(path).unwrap(), None);
    }

    #[test]
    fn sink_requires_connect() {
        let path = std::env::temp_dir().join(format!(
            "cameraman-unconnected-{}-{}.bgra",
            std::process::id(),
            now_nanos()
        ));
        let mut sink = FrameSpoolSink::new(path);
        let frame = Frame::solid_bgra(1, 1, [1, 2, 3, 255]).unwrap();
        assert!(sink.send(&frame).is_err());
    }

    #[test]
    fn frame_spool_round_trips_bgra_frame() {
        let path = std::env::temp_dir().join(format!(
            "cameraman-roundtrip-{}-{}.bgra",
            std::process::id(),
            now_nanos()
        ));
        let frame = Frame::solid_bgra(2, 1, [10, 20, 30, 255]).unwrap();
        let mut sink = FrameSpoolSink::new(&path);

        sink.connect().unwrap();
        sink.send(&frame).unwrap();

        let transported = read_latest_frame(&path).unwrap().unwrap();
        assert_eq!(transported.sequence, 0);
        assert_eq!(transported.frame, frame);
        assert_eq!(transported.fps, DEFAULT_FPS);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn target_fps_is_carried_through_the_spool() {
        let path = std::env::temp_dir().join(format!(
            "cameraman-fps-{}-{}.bgra",
            std::process::id(),
            now_nanos()
        ));
        let frame = Frame::solid_bgra(1, 1, [1, 2, 3, 255]).unwrap();
        let mut sink = FrameSpoolSink::new(&path);
        sink.set_target_fps(60);
        assert_eq!(sink.target_fps(), 60);

        sink.connect().unwrap();
        sink.send(&frame).unwrap();

        let transported = read_latest_frame(&path).unwrap().unwrap();
        assert_eq!(transported.fps, 60);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn zero_fps_is_clamped_to_one() {
        let mut sink = FrameSpoolSink::new("/tmp/unused-in-this-test.bgra");
        sink.set_target_fps(0);
        assert_eq!(sink.target_fps(), 1);
    }
}
