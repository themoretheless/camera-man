use serde::{Deserialize, Serialize};

use crate::frame::PixelFormat;

/// Numeric values used by mmap/file protocols. These values are versioned at
/// the wire boundary and never leak into the domain `PixelFormat` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum WirePixelFormat {
    Bgra8 = 1,
}

impl WirePixelFormat {
    pub const fn code(self) -> u32 {
        self as u32
    }

    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(Self::Bgra8),
            _ => None,
        }
    }
}

impl From<PixelFormat> for WirePixelFormat {
    fn from(value: PixelFormat) -> Self {
        match value {
            PixelFormat::Bgra8 => Self::Bgra8,
        }
    }
}

impl From<WirePixelFormat> for PixelFormat {
    fn from(value: WirePixelFormat) -> Self {
        match value {
            WirePixelFormat::Bgra8 => Self::Bgra8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireFrameTimingDto {
    pub generation: u64,
    pub sequence: u64,
    pub monotonic_timestamp_nanos: u64,
    pub wall_timestamp_nanos: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireFrameDescriptorDto {
    pub schema_version: u32,
    pub width: u32,
    pub height: u32,
    pub pixel_format: WirePixelFormat,
    pub fps: u32,
    pub data_len: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_wire_value_does_not_change_domain_enum() {
        assert_eq!(WirePixelFormat::from_code(99), None);
        assert_eq!(WirePixelFormat::from(PixelFormat::Bgra8).code(), 1);
    }
}
