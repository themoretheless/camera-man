use crate::error::CameraManError;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use crate::media_contract::FrameContract;
use crate::media_time::{
    CaptureTimestamps, MonotonicTimestampNanos, SystemClock, WallTimestampNanos,
    monotonic_time_nanos,
};

pub const FRAME_MEMORY_LIMIT_ENV: &str = "CAMERAMAN_MAX_FRAME_BYTES";
const MIN_DEFAULT_FRAME_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DEFAULT_FRAME_BYTES: u64 = 1024 * 1024 * 1024;
const FRAME_MEMORY_FRACTION: u64 = 16;

/// Maximum memory allowed for one tightly packed frame.
///
/// CameraMan's default is one sixteenth of physical memory, clamped to
/// 64 MiB..=1 GiB. `CAMERAMAN_MAX_FRAME_BYTES` overrides that process-wide
/// policy, while `Frame::new_checked_with_limits` accepts an explicit policy
/// for embedded callers and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLimits {
    max_frame_bytes: u64,
}

impl FrameLimits {
    pub const fn new(max_frame_bytes: u64) -> Self {
        Self {
            max_frame_bytes: if max_frame_bytes == 0 {
                1
            } else {
                max_frame_bytes
            },
        }
    }

    pub const fn for_physical_memory(physical_memory_bytes: u64) -> Self {
        let suggested = physical_memory_bytes / FRAME_MEMORY_FRACTION;
        let max_frame_bytes = if suggested < MIN_DEFAULT_FRAME_BYTES {
            MIN_DEFAULT_FRAME_BYTES
        } else if suggested > MAX_DEFAULT_FRAME_BYTES {
            MAX_DEFAULT_FRAME_BYTES
        } else {
            suggested
        };
        Self { max_frame_bytes }
    }

    pub const fn max_frame_bytes(self) -> u64 {
        self.max_frame_bytes
    }

    fn detect() -> Self {
        if let Some(max_frame_bytes) = std::env::var(FRAME_MEMORY_LIMIT_ENV)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|bytes| *bytes > 0)
        {
            return Self::new(max_frame_bytes);
        }
        physical_memory_bytes()
            .map(Self::for_physical_memory)
            .unwrap_or_else(|| Self::new(MAX_DEFAULT_FRAME_BYTES))
    }
}

impl Default for FrameLimits {
    fn default() -> Self {
        default_frame_limits()
    }
}

pub fn default_frame_limits() -> FrameLimits {
    static LIMITS: OnceLock<FrameLimits> = OnceLock::new();
    *LIMITS.get_or_init(FrameLimits::detect)
}

/// Pixel memory layout used by a [`Frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    /// Four tightly packed bytes per pixel in blue, green, red, alpha order.
    Bgra8,
}

impl PixelFormat {
    /// Number of tightly packed bytes occupied by one pixel.
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Bgra8 => 4,
        }
    }
}

/// Source identity and capture timing carried separately from pixel storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameMetadata {
    pub source_id: String,
    pub sequence: u64,
    pub timestamps: CaptureTimestamps,
}

impl FrameMetadata {
    pub fn new(source_id: impl Into<String>, sequence: u64) -> Self {
        Self {
            source_id: source_id.into(),
            sequence,
            timestamps: CaptureTimestamps::now(&SystemClock),
        }
    }

    /// Current age of this capture timestamp, saturating for clock skew.
    pub fn age(&self) -> Duration {
        Self::age_from_monotonic_nanos(self.timestamps.monotonic.0)
    }

    pub const fn monotonic_timestamp(&self) -> MonotonicTimestampNanos {
        self.timestamps.monotonic
    }

    pub const fn wall_timestamp(&self) -> WallTimestampNanos {
        self.timestamps.wall
    }

    pub(crate) fn age_from_monotonic_nanos(timestamp_nanos: u64) -> Duration {
        Duration::from_nanos(monotonic_time_nanos().saturating_sub(timestamp_nanos))
    }
}

/// Validated, tightly packed pixel data with copy-on-write clone semantics.
///
/// Cloning a frame shares its immutable `Arc<Vec<u8>>`; requesting mutable
/// data detaches only when another clone still references the same pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    contract: FrameContract,
    data: Arc<Vec<u8>>,
}

/// Borrowed pixel view that can represent padded source rows without copying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameView<'a> {
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    contract: FrameContract,
    row_stride: usize,
    active_row_bytes: usize,
    data: &'a [u8],
}

/// One validated frame paired with source metadata.
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
    /// Validates dimensions, byte length and the process-wide memory policy.
    pub fn new_checked(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        data: Vec<u8>,
    ) -> Result<Self, CameraManError> {
        Self::new_checked_with_limits(width, height, pixel_format, data, default_frame_limits())
    }

    /// Validates a frame against an explicit caller-provided memory policy.
    pub fn new_checked_with_limits(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        data: Vec<u8>,
        limits: FrameLimits,
    ) -> Result<Self, CameraManError> {
        if width == 0 || height == 0 {
            return Err(CameraManError::InvalidDimensions { width, height });
        }

        let expected = byte_len(width, height, pixel_format, limits)?;
        let actual = data.len();
        if actual != expected {
            return Err(CameraManError::InvalidBufferLength { expected, actual });
        }

        let contract = FrameContract::canonical_bgra(width, height);
        Ok(Self {
            width,
            height,
            pixel_format,
            contract,
            data: Arc::new(data),
        })
    }

    pub fn new_checked_with_contract(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        contract: FrameContract,
        data: Vec<u8>,
    ) -> Result<Self, CameraManError> {
        contract.validate(width, height)?;
        let mut frame = Self::new_checked(width, height, pixel_format, data)?;
        frame.contract = contract;
        Ok(frame)
    }

    /// Allocates a tightly packed solid BGRA frame under the default limits.
    pub fn solid_bgra(width: u32, height: u32, bgra: [u8; 4]) -> Result<Self, CameraManError> {
        Self::solid_bgra_with_limits(width, height, bgra, default_frame_limits())
    }

    /// Allocates a solid BGRA frame under explicit limits.
    pub fn solid_bgra_with_limits(
        width: u32,
        height: u32,
        bgra: [u8; 4],
        limits: FrameLimits,
    ) -> Result<Self, CameraManError> {
        let len = byte_len(width, height, PixelFormat::Bgra8, limits)?;
        let mut data = vec![0; len];
        for pixel in data.chunks_exact_mut(4) {
            pixel.copy_from_slice(&bgra);
        }
        Ok(Self {
            width,
            height,
            pixel_format: PixelFormat::Bgra8,
            contract: FrameContract::canonical_bgra(width, height),
            data: Arc::new(data),
        })
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

    pub const fn contract(&self) -> FrameContract {
        self.contract
    }

    pub fn set_contract(&mut self, contract: FrameContract) -> Result<(), CameraManError> {
        contract.validate(self.width, self.height)?;
        self.contract = contract;
        Ok(())
    }

    /// Immutable tightly packed pixel bytes.
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// Mutable pixel bytes, detaching copy-on-write storage when shared.
    pub fn data_mut(&mut self) -> &mut [u8] {
        Arc::make_mut(&mut self.data).as_mut_slice()
    }

    /// Borrows this tightly packed frame as a stride-aware view.
    pub fn as_view(&self) -> FrameView<'_> {
        let row_stride = self.width as usize * self.pixel_format.bytes_per_pixel();
        FrameView {
            width: self.width,
            height: self.height,
            pixel_format: self.pixel_format,
            contract: self.contract,
            row_stride,
            active_row_bytes: row_stride,
            data: self.data(),
        }
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

    /// Writes a pixel and returns structured coordinates when out of bounds.
    pub fn try_set_bgra(&mut self, x: u32, y: u32, bgra: [u8; 4]) -> Result<(), CameraManError> {
        let Some(offset) = self.pixel_offset(x, y) else {
            return Err(CameraManError::PixelOutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        };
        Arc::make_mut(&mut self.data)[offset..offset + 4].copy_from_slice(&bgra);
        Ok(())
    }

    /// Resilient writer that clips out-of-bounds coordinates in release and
    /// asserts in debug. Use [`Frame::try_set_bgra`] when failure must be
    /// reported to the caller.
    pub fn set_bgra(&mut self, x: u32, y: u32, bgra: [u8; 4]) {
        let result = self.try_set_bgra(x, y, bgra);
        debug_assert!(
            result.is_ok(),
            "set_bgra out of bounds: ({x}, {y}) in {}x{}",
            self.width,
            self.height
        );
    }
}

impl<'a> FrameView<'a> {
    /// Validates dimensions, stride and accessible row bytes without copying.
    /// Extra bytes at the end of each row are treated as padding.
    pub fn new_checked(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        row_stride: usize,
        data: &'a [u8],
    ) -> Result<Self, CameraManError> {
        if width == 0 || height == 0 {
            return Err(CameraManError::InvalidDimensions { width, height });
        }
        let active_row_bytes_u64 = u64::from(width) * pixel_format.bytes_per_pixel() as u64;
        let active_row_bytes =
            usize::try_from(active_row_bytes_u64).map_err(|_| CameraManError::BufferTooLarge {
                bytes: active_row_bytes_u64,
            })?;
        if row_stride < active_row_bytes {
            return Err(CameraManError::InvalidRowStride {
                minimum: active_row_bytes,
                actual: row_stride,
            });
        }
        let required = (height as usize - 1)
            .checked_mul(row_stride)
            .and_then(|prefix| prefix.checked_add(active_row_bytes))
            .ok_or(CameraManError::BufferTooLarge { bytes: u64::MAX })?;
        if data.len() < required {
            return Err(CameraManError::InvalidBufferLength {
                expected: required,
                actual: data.len(),
            });
        }

        Ok(Self {
            width,
            height,
            pixel_format,
            contract: FrameContract::canonical_bgra(width, height),
            row_stride,
            active_row_bytes,
            data,
        })
    }

    pub fn new_checked_with_contract(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        contract: FrameContract,
        row_stride: usize,
        data: &'a [u8],
    ) -> Result<Self, CameraManError> {
        contract.validate(width, height)?;
        let mut view = Self::new_checked(width, height, pixel_format, row_stride, data)?;
        view.contract = contract;
        Ok(view)
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    pub const fn pixel_format(self) -> PixelFormat {
        self.pixel_format
    }

    pub const fn contract(self) -> FrameContract {
        self.contract
    }

    pub const fn row_stride(self) -> usize {
        self.row_stride
    }

    pub const fn data(self) -> &'a [u8] {
        self.data
    }

    /// Active pixels for one row, excluding trailing padding.
    pub fn row(self, y: u32) -> Option<&'a [u8]> {
        if y >= self.height {
            return None;
        }
        let start = y as usize * self.row_stride;
        Some(&self.data[start..start + self.active_row_bytes])
    }

    pub fn bgra_at(self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || self.pixel_format != PixelFormat::Bgra8 {
            return None;
        }
        let row = self.row(y)?;
        let offset = x as usize * 4;
        Some([
            row[offset],
            row[offset + 1],
            row[offset + 2],
            row[offset + 3],
        ])
    }

    /// Copies active row bytes into a tightly packed owned frame.
    pub fn to_owned_tightly_packed(self) -> Result<Frame, CameraManError> {
        self.to_owned_tightly_packed_with_limits(default_frame_limits())
    }

    pub fn to_owned_tightly_packed_with_limits(
        self,
        limits: FrameLimits,
    ) -> Result<Frame, CameraManError> {
        let tight_len = byte_len(self.width, self.height, self.pixel_format, limits)?;
        let mut pixels = Vec::with_capacity(tight_len);
        for y in 0..self.height {
            pixels.extend_from_slice(self.row(y).expect("row validated at construction"));
        }
        let mut frame = Frame::new_checked_with_limits(
            self.width,
            self.height,
            self.pixel_format,
            pixels,
            limits,
        )?;
        frame.set_contract(self.contract)?;
        Ok(frame)
    }
}

fn byte_len(
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    limits: FrameLimits,
) -> Result<usize, CameraManError> {
    if width == 0 || height == 0 {
        return Err(CameraManError::InvalidDimensions { width, height });
    }

    // u64 math: the old u32 version rejected valid dimensions like 40000x40000
    // (6.4 GB) with a misleading InvalidDimensions error.
    let bytes = checked_frame_byte_len(width, height, pixel_format.bytes_per_pixel())
        .ok_or(CameraManError::BufferTooLarge { bytes: u64::MAX })?;
    if bytes > limits.max_frame_bytes() {
        return Err(CameraManError::FrameLimitExceeded {
            bytes,
            limit: limits.max_frame_bytes(),
        });
    }
    usize::try_from(bytes)
        .ok()
        .ok_or(CameraManError::BufferTooLarge { bytes })
}

#[doc(hidden)]
pub fn checked_frame_byte_len(width: u32, height: u32, bytes_per_pixel: usize) -> Option<u64> {
    u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(u64::try_from(bytes_per_pixel).ok()?)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn frame_length_is_total_and_overflow_safe() {
        let width = kani::any::<u32>();
        let height = kani::any::<u32>();
        let bytes_per_pixel = kani::any::<usize>();
        let _ = checked_frame_byte_len(width, height, bytes_per_pixel);
    }

    #[kani::proof]
    fn oversized_frame_length_is_rejected() {
        assert!(checked_frame_byte_len(u32::MAX, u32::MAX, 4).is_none());
    }
}

#[cfg(unix)]
fn physical_memory_bytes() -> Option<u64> {
    // `sysconf` has no Rust wrapper. Negative values mean the query failed;
    // checked multiplication also rejects an impossible platform response.
    // SAFETY: both `sysconf` selectors are constants and the call takes no
    // pointers or ownership from Rust.
    let (pages, page_size) = unsafe {
        (
            libc::sysconf(libc::_SC_PHYS_PAGES),
            libc::sysconf(libc::_SC_PAGESIZE),
        )
    };
    (pages > 0 && page_size > 0)
        .then(|| (pages as u64).checked_mul(page_size as u64))
        .flatten()
}

#[cfg(not(unix))]
fn physical_memory_bytes() -> Option<u64> {
    None
}

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

    #[test]
    fn clone_shares_pixels_until_mutated() {
        let original = Frame::solid_bgra(2, 1, [10, 20, 30, 255]).unwrap();
        let mut cloned = original.clone();

        assert!(Arc::ptr_eq(&original.data, &cloned.data));

        cloned.set_bgra(0, 0, [1, 2, 3, 255]);

        assert!(!Arc::ptr_eq(&original.data, &cloned.data));
        assert_eq!(original.bgra_at(0, 0), Some([10, 20, 30, 255]));
        assert_eq!(cloned.bgra_at(0, 0), Some([1, 2, 3, 255]));
    }

    #[test]
    fn frame_limits_follow_system_memory_budget() {
        assert_eq!(
            FrameLimits::for_physical_memory(512 * 1024 * 1024).max_frame_bytes(),
            MIN_DEFAULT_FRAME_BYTES
        );
        assert_eq!(
            FrameLimits::for_physical_memory(4 * 1024 * 1024 * 1024).max_frame_bytes(),
            256 * 1024 * 1024
        );
        assert_eq!(
            FrameLimits::for_physical_memory(128 * 1024 * 1024 * 1024).max_frame_bytes(),
            MAX_DEFAULT_FRAME_BYTES
        );
    }

    #[test]
    fn explicit_frame_limit_rejects_before_buffer_validation() {
        let error = Frame::new_checked_with_limits(
            2,
            2,
            PixelFormat::Bgra8,
            Vec::new(),
            FrameLimits::new(15),
        )
        .unwrap_err();

        assert_eq!(
            error,
            CameraManError::FrameLimitExceeded {
                bytes: 16,
                limit: 15
            }
        );
    }

    #[test]
    fn padded_frame_view_converts_to_tightly_packed_pixels() {
        let padded = [
            1, 2, 3, 255, 4, 5, 6, 255, 99, 99, 99, 99, 10, 20, 30, 255, 40, 50, 60, 255, 88, 88,
            88, 88,
        ];
        let view = FrameView::new_checked(2, 2, PixelFormat::Bgra8, 12, &padded).unwrap();

        assert_eq!(view.row_stride(), 12);
        assert_eq!(view.row(0).unwrap().len(), 8);
        assert_eq!(view.bgra_at(1, 1), Some([40, 50, 60, 255]));

        let owned = view
            .to_owned_tightly_packed_with_limits(FrameLimits::new(16))
            .unwrap();
        assert_eq!(owned.data().len(), 16);
        assert_eq!(owned.bgra_at(0, 1), Some([10, 20, 30, 255]));
    }

    #[test]
    fn frame_view_rejects_short_stride() {
        let error = FrameView::new_checked(2, 1, PixelFormat::Bgra8, 7, &[0; 8]).unwrap_err();
        assert_eq!(
            error,
            CameraManError::InvalidRowStride {
                minimum: 8,
                actual: 7
            }
        );
    }

    #[test]
    fn checked_pixel_writer_reports_coordinates() {
        let mut frame = Frame::solid_bgra(2, 1, [0, 0, 0, 255]).unwrap();

        let error = frame.try_set_bgra(2, 0, [1, 2, 3, 255]).unwrap_err();

        assert_eq!(
            error,
            CameraManError::PixelOutOfBounds {
                x: 2,
                y: 0,
                width: 2,
                height: 1
            }
        );
    }
}
