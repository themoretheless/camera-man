use crate::error::CameraManError;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgra8,
}

impl PixelFormat {
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Bgra8 => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameMetadata {
    pub source_id: String,
    pub sequence: u64,
    pub timestamp_nanos: u128,
}

impl FrameMetadata {
    pub fn new(source_id: impl Into<String>, sequence: u64) -> Self {
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        Self {
            source_id: source_id.into(),
            sequence,
            timestamp_nanos,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    frame: Frame,
    metadata: FrameMetadata,
}

impl CapturedFrame {
    pub const fn new(frame: Frame, metadata: FrameMetadata) -> Self {
        Self { frame, metadata }
    }

    pub const fn frame(&self) -> &Frame {
        &self.frame
    }

    pub const fn metadata(&self) -> &FrameMetadata {
        &self.metadata
    }

    pub fn into_frame(self) -> Frame {
        self.frame
    }
}

impl Frame {
    pub fn new_checked(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        data: Vec<u8>,
    ) -> Result<Self, CameraManError> {
        if width == 0 || height == 0 {
            return Err(CameraManError::InvalidDimensions { width, height });
        }

        let expected = byte_len(width, height, pixel_format)?;
        let actual = data.len();
        if actual != expected {
            return Err(CameraManError::InvalidBufferLength { expected, actual });
        }

        Ok(Self {
            width,
            height,
            pixel_format,
            data,
        })
    }

    pub fn solid_bgra(width: u32, height: u32, bgra: [u8; 4]) -> Result<Self, CameraManError> {
        let len = byte_len(width, height, PixelFormat::Bgra8)?;
        let mut data = vec![0; len];
        for pixel in data.chunks_exact_mut(4) {
            pixel.copy_from_slice(&bgra);
        }
        Self::new_checked(width, height, PixelFormat::Bgra8, data)
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn pixel_offset(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let stride = self.width as usize * self.pixel_format.bytes_per_pixel();
        Some(y as usize * stride + x as usize * self.pixel_format.bytes_per_pixel())
    }

    pub fn bgra_at(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let offset = self.pixel_offset(x, y)?;
        Some([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ])
    }

    pub fn set_bgra(&mut self, x: u32, y: u32, bgra: [u8; 4]) {
        // Out-of-bounds writes are clipped in release builds, but they almost
        // always mean a renderer bug, so debug builds fail loudly. This silent
        // clipping is exactly what hid the zero-width-cell underflow bug.
        debug_assert!(
            x < self.width && y < self.height,
            "set_bgra out of bounds: ({x}, {y}) in {}x{}",
            self.width,
            self.height
        );
        if let Some(offset) = self.pixel_offset(x, y) {
            self.data[offset..offset + 4].copy_from_slice(&bgra);
        }
    }
}

fn byte_len(width: u32, height: u32, pixel_format: PixelFormat) -> Result<usize, CameraManError> {
    if width == 0 || height == 0 {
        return Err(CameraManError::InvalidDimensions { width, height });
    }

    // u64 math: the old u32 version rejected valid dimensions like 40000x40000
    // (6.4 GB) with a misleading InvalidDimensions error.
    let bytes = u64::from(width) * u64::from(height) * pixel_format.bytes_per_pixel() as u64;
    usize::try_from(bytes)
        .ok()
        .filter(|&bytes| bytes <= MAX_FRAME_BYTES)
        .ok_or(CameraManError::BufferTooLarge { bytes })
}

/// Hard cap on a single frame allocation (1 GiB). Anything bigger is almost
/// certainly a corrupted dimension coming from a capture backend.
const MAX_FRAME_BYTES: usize = 1 << 30;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_buffer_length() {
        let err = Frame::new_checked(2, 2, PixelFormat::Bgra8, vec![0; 15]).unwrap_err();
        assert_eq!(
            err,
            CameraManError::InvalidBufferLength {
                expected: 16,
                actual: 15
            }
        );
    }

    #[test]
    fn creates_solid_frame() {
        let frame = Frame::solid_bgra(2, 1, [10, 20, 30, 255]).unwrap();
        assert_eq!(frame.bgra_at(0, 0), Some([10, 20, 30, 255]));
        assert_eq!(frame.bgra_at(1, 0), Some([10, 20, 30, 255]));
    }
}
