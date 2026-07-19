use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::config::VIRTUAL_CAMERA_DEFAULT_FPS;
use crate::diagnostics::{PipelineStage, record_output_frame, stage_span};
use crate::error::CameraManError;
use crate::frame::{Frame, PixelFormat, default_frame_limits};
use crate::media_contract::{Colorimetry, FrameContract};
use crate::media_time::{monotonic_time_nanos, new_generation_id, wall_time_nanos};
use crate::performance::{CopyStage, copy_ledger};
use crate::virtual_camera::VirtualCameraSink;
use crate::wire::WirePixelFormat;

const MAGIC: &[u8; 4] = b"CMAN";
const VERSION: u32 = 4;
const HEADER_LEN: usize = 76;

pub fn default_frame_spool_path() -> PathBuf {
    std::env::var_os("CAMERAMAN_FRAME_SPOOL")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("cameraman-virtual-frame.bgra"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportFrame {
    pub frame: Frame,
    pub sequence: u64,
    /// Changes whenever a producer process creates a new transport writer.
    pub generation: u64,
    /// Boot-relative timestamp used for latency and stale-frame decisions.
    pub monotonic_timestamp_nanos: u64,
    /// Unix timestamp retained for logs and persisted diagnostics only.
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
    generation: u64,
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
            generation: new_generation_id(),
            target_fps: VIRTUAL_CAMERA_DEFAULT_FPS,
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
        {
            let _publish_span = stage_span(
                PipelineStage::Publish,
                self.sequence,
                frame.width(),
                frame.height(),
            );
            write_frame(
                &self.path,
                self.sequence,
                self.generation,
                self.target_fps,
                frame,
            )?;
        }
        self.sequence = self.sequence.wrapping_add(1);
        record_output_frame();
        Ok(())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }
}

pub fn read_latest_frame(path: impl AsRef<Path>) -> Result<Option<TransportFrame>, CameraManError> {
    let path = path.as_ref();
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let max_file_bytes = default_frame_limits()
        .max_frame_bytes()
        .checked_add(HEADER_LEN as u64)
        .ok_or(CameraManError::BufferTooLarge { bytes: u64::MAX })?;
    let metadata_len = file.metadata()?.len();
    if metadata_len > max_file_bytes {
        return Err(CameraManError::BufferTooLarge {
            bytes: metadata_len,
        });
    }
    let _consume_span = stage_span(PipelineStage::Consume, 0, 0, 0);

    let mut bytes = Vec::with_capacity(usize::try_from(metadata_len).unwrap_or(0));
    file.take(max_file_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_file_bytes {
        return Err(CameraManError::BufferTooLarge {
            bytes: bytes.len() as u64,
        });
    }
    parse_frame(&bytes)
}

fn write_frame(
    path: &Path,
    sequence: u64,
    generation: u64,
    fps: u32,
    frame: &Frame,
) -> Result<(), CameraManError> {
    if frame.pixel_format() != PixelFormat::Bgra8 {
        return Err(CameraManError::UnsupportedPixelFormat);
    }
    if !frame
        .contract()
        .is_canonical_bgra(frame.width(), frame.height())
    {
        return Err(CameraManError::InvalidMediaContract(
            "transport accepts only compositor-normalized frames",
        ));
    }

    let temp_path = temporary_path(path);
    let mut file = File::create(&temp_path)?;
    file.write_all(MAGIC)?;
    write_u32(&mut file, VERSION)?;
    write_u32(&mut file, frame.width())?;
    write_u32(&mut file, frame.height())?;
    write_u32(&mut file, WirePixelFormat::Bgra8.code())?;
    write_u32(&mut file, fps)?;
    write_u32(&mut file, frame.contract().colorimetry.wire_code())?;
    write_u64(&mut file, sequence)?;
    write_u64(&mut file, generation)?;
    write_u64(&mut file, monotonic_time_nanos())?;
    write_u128(&mut file, wall_time_nanos())?;
    write_u64(&mut file, frame.data().len() as u64)?;
    file.write_all(frame.data())?;
    copy_ledger().record_copy(CopyStage::FileSpool, frame.data().len());
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
    let color_contract = read_u32(bytes, 24)?;
    let sequence = read_u64(bytes, 28)?;
    let generation = read_u64(bytes, 36)?;
    let monotonic_timestamp_nanos = read_u64(bytes, 44)?;
    let timestamp_nanos = read_u128(bytes, 52)?;
    let data_len_u64 = read_u64(bytes, 68)?;
    let data_len = usize::try_from(data_len_u64).map_err(|_| CameraManError::BufferTooLarge {
        bytes: data_len_u64,
    })?;

    let Some(colorimetry) = Colorimetry::from_wire_code(color_contract) else {
        return Ok(None);
    };
    if version != VERSION
        || WirePixelFormat::from_code(pixel_format) != Some(WirePixelFormat::Bgra8)
        || colorimetry != Colorimetry::BT709_FULL_OPAQUE
    {
        return Ok(None);
    }
    if HEADER_LEN
        .checked_add(data_len)
        .is_none_or(|expected| bytes.len() != expected)
    {
        return Ok(None);
    }

    let mut contract = FrameContract::canonical_bgra(width, height);
    contract.colorimetry = colorimetry;
    let frame = Frame::new_checked_with_contract(
        width,
        height,
        PixelFormat::Bgra8,
        contract,
        bytes[HEADER_LEN..].to_vec(),
    )?;
    Ok(Some(TransportFrame {
        frame,
        sequence,
        generation,
        monotonic_timestamp_nanos,
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
            wall_time_nanos()
        ));
        assert_eq!(read_latest_frame(path).unwrap(), None);
    }

    #[test]
    fn sink_requires_connect() {
        let path = std::env::temp_dir().join(format!(
            "cameraman-unconnected-{}-{}.bgra",
            std::process::id(),
            wall_time_nanos()
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
            wall_time_nanos()
        ));
        let frame = Frame::solid_bgra(2, 1, [10, 20, 30, 255]).unwrap();
        let mut sink = FrameSpoolSink::new(&path);

        sink.connect().unwrap();
        sink.send(&frame).unwrap();

        let transported = read_latest_frame(&path).unwrap().unwrap();
        assert_eq!(transported.sequence, 0);
        assert_ne!(transported.generation, 0);
        assert_ne!(transported.monotonic_timestamp_nanos, 0);
        assert_eq!(transported.frame, frame);
        assert_eq!(transported.fps, VIRTUAL_CAMERA_DEFAULT_FPS);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn target_fps_is_carried_through_the_spool() {
        let path = std::env::temp_dir().join(format!(
            "cameraman-fps-{}-{}.bgra",
            std::process::id(),
            wall_time_nanos()
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
    fn rejects_a_corrupted_color_contract_without_exposing_pixels() {
        let path = std::env::temp_dir().join(format!(
            "cameraman-color-contract-{}-{}.bgra",
            std::process::id(),
            wall_time_nanos()
        ));
        let frame = Frame::solid_bgra(2, 1, [10, 20, 30, 255]).unwrap();
        let mut sink = FrameSpoolSink::new(&path);
        sink.connect().unwrap();
        sink.send(&frame).unwrap();

        let mut bytes = fs::read(&path).unwrap();
        bytes[24..28].copy_from_slice(&0_u32.to_le_bytes());
        fs::write(&path, bytes).unwrap();

        assert_eq!(read_latest_frame(&path).unwrap(), None);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn zero_fps_is_clamped_to_one() {
        let mut sink = FrameSpoolSink::new("/tmp/unused-in-this-test.bgra");
        sink.set_target_fps(0);
        assert_eq!(sink.target_fps(), 1);
    }
}
