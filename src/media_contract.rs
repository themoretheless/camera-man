use serde::{Deserialize, Serialize};

use crate::error::CameraManError;

pub const COLOR_CONTRACT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorPrimaries {
    Bt709,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferFunction {
    Bt709,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatrixCoefficients {
    Bt709,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorRange {
    Full,
    Limited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaMode {
    Opaque,
    Straight,
    Premultiplied,
}

/// Complete interpretation of the color channels. Pixel byte order remains a
/// separate storage concern (`PixelFormat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Colorimetry {
    pub schema_version: u8,
    pub bit_depth: u8,
    pub primaries: ColorPrimaries,
    pub transfer: TransferFunction,
    pub matrix: MatrixCoefficients,
    pub range: ColorRange,
    pub alpha: AlphaMode,
}

impl Colorimetry {
    pub const BT709_FULL_OPAQUE: Self = Self {
        schema_version: COLOR_CONTRACT_SCHEMA_VERSION,
        bit_depth: 8,
        primaries: ColorPrimaries::Bt709,
        transfer: TransferFunction::Bt709,
        matrix: MatrixCoefficients::Bt709,
        range: ColorRange::Full,
        alpha: AlphaMode::Opaque,
    };

    /// Stable packed representation used by both mmap and spool transports.
    /// Enum discriminants are encoded explicitly rather than relying on Rust's
    /// in-memory representation.
    pub const fn wire_code(self) -> u32 {
        let primaries = match self.primaries {
            ColorPrimaries::Bt709 => 1,
        };
        let transfer = match self.transfer {
            TransferFunction::Bt709 => 1,
        };
        let matrix = match self.matrix {
            MatrixCoefficients::Bt709 => 1,
        };
        let range = match self.range {
            ColorRange::Full => 1,
            ColorRange::Limited => 2,
        };
        let alpha = match self.alpha {
            AlphaMode::Opaque => 1,
            AlphaMode::Straight => 2,
            AlphaMode::Premultiplied => 3,
        };
        self.schema_version as u32
            | ((self.bit_depth as u32) << 8)
            | (primaries << 16)
            | (transfer << 19)
            | (matrix << 22)
            | (range << 25)
            | (alpha << 27)
    }

    pub const fn from_wire_code(code: u32) -> Option<Self> {
        if code & 0xC000_0000 != 0 {
            return None;
        }
        let schema_version = (code & 0xff) as u8;
        let bit_depth = ((code >> 8) & 0xff) as u8;
        let primaries = match (code >> 16) & 0x7 {
            1 => ColorPrimaries::Bt709,
            _ => return None,
        };
        let transfer = match (code >> 19) & 0x7 {
            1 => TransferFunction::Bt709,
            _ => return None,
        };
        let matrix = match (code >> 22) & 0x7 {
            1 => MatrixCoefficients::Bt709,
            _ => return None,
        };
        let range = match (code >> 25) & 0x3 {
            1 => ColorRange::Full,
            2 => ColorRange::Limited,
            _ => return None,
        };
        let alpha = match (code >> 27) & 0x7 {
            1 => AlphaMode::Opaque,
            2 => AlphaMode::Straight,
            3 => AlphaMode::Premultiplied,
            _ => return None,
        };
        if schema_version != COLOR_CONTRACT_SCHEMA_VERSION || bit_depth != 8 {
            return None;
        }
        Some(Self {
            schema_version,
            bit_depth,
            primaries,
            transfer,
            matrix,
            range,
            alpha,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PixelAspectRatio {
    pub horizontal: u32,
    pub vertical: u32,
}

impl PixelAspectRatio {
    pub const SQUARE: Self = Self {
        horizontal: 1,
        vertical: 1,
    };

    pub fn new(horizontal: u32, vertical: u32) -> Result<Self, CameraManError> {
        if horizontal == 0 || vertical == 0 {
            return Err(CameraManError::InvalidMediaContract(
                "pixel aspect ratio terms must be non-zero",
            ));
        }
        Ok(Self {
            horizontal,
            vertical,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CleanAperture {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CleanAperture {
    pub const fn full_frame(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub fn validate(self, frame_width: u32, frame_height: u32) -> Result<(), CameraManError> {
        if self.width == 0 || self.height == 0 {
            return Err(CameraManError::InvalidMediaContract(
                "clean aperture must not be empty",
            ));
        }
        if self
            .x
            .checked_add(self.width)
            .is_none_or(|edge| edge > frame_width)
            || self
                .y
                .checked_add(self.height)
                .is_none_or(|edge| edge > frame_height)
        {
            return Err(CameraManError::InvalidMediaContract(
                "clean aperture exceeds frame bounds",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    #[default]
    Degrees0,
    Degrees90,
    Degrees180,
    Degrees270,
}

impl Rotation {
    pub const fn swaps_axes(self) -> bool {
        matches!(self, Self::Degrees90 | Self::Degrees270)
    }
}

/// Logical transform applied after clean-aperture cropping. The capture
/// backend records it but does not rewrite pixels; composition consumes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransformMetadata {
    pub rotation: Rotation,
    pub mirror_horizontal: bool,
    pub mirror_vertical: bool,
}

impl TransformMetadata {
    pub const IDENTITY: Self = Self {
        rotation: Rotation::Degrees0,
        mirror_horizontal: false,
        mirror_vertical: false,
    };

    pub const fn is_identity(self) -> bool {
        matches!(self.rotation, Rotation::Degrees0)
            && !self.mirror_horizontal
            && !self.mirror_vertical
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameContract {
    pub colorimetry: Colorimetry,
    pub pixel_aspect_ratio: PixelAspectRatio,
    pub clean_aperture: CleanAperture,
    pub transform: TransformMetadata,
}

impl FrameContract {
    pub const fn canonical_bgra(width: u32, height: u32) -> Self {
        Self {
            colorimetry: Colorimetry::BT709_FULL_OPAQUE,
            pixel_aspect_ratio: PixelAspectRatio::SQUARE,
            clean_aperture: CleanAperture::full_frame(width, height),
            transform: TransformMetadata::IDENTITY,
        }
    }

    pub fn validate(self, width: u32, height: u32) -> Result<(), CameraManError> {
        if self.colorimetry.schema_version != COLOR_CONTRACT_SCHEMA_VERSION {
            return Err(CameraManError::InvalidMediaContract(
                "unsupported color-contract schema version",
            ));
        }
        if self.colorimetry.bit_depth != 8 {
            return Err(CameraManError::InvalidMediaContract(
                "BGRA transport currently requires 8-bit channels",
            ));
        }
        if self.pixel_aspect_ratio.horizontal == 0 || self.pixel_aspect_ratio.vertical == 0 {
            return Err(CameraManError::InvalidMediaContract(
                "pixel aspect ratio terms must be non-zero",
            ));
        }
        self.clean_aperture.validate(width, height)
    }

    pub fn is_canonical_bgra(self, width: u32, height: u32) -> bool {
        self.colorimetry.schema_version == COLOR_CONTRACT_SCHEMA_VERSION
            && self.colorimetry.bit_depth == 8
            && self.colorimetry.primaries == ColorPrimaries::Bt709
            && self.colorimetry.transfer == TransferFunction::Bt709
            && self.colorimetry.matrix == MatrixCoefficients::Bt709
            && self.colorimetry.range == ColorRange::Full
            && self.colorimetry.alpha == AlphaMode::Opaque
            && self.pixel_aspect_ratio.horizontal == 1
            && self.pixel_aspect_ratio.vertical == 1
            && self.clean_aperture.x == 0
            && self.clean_aperture.y == 0
            && self.clean_aperture.width == width
            && self.clean_aperture.height == height
            && self.transform.is_identity()
    }

    /// Display-space dimensions after clean aperture, pixel aspect ratio and
    /// rotation are applied. Integer math rounds to the nearest pixel.
    pub fn display_dimensions(self) -> (u32, u32) {
        let aperture = self.clean_aperture;
        let adjusted_width = (u64::from(aperture.width)
            .saturating_mul(u64::from(self.pixel_aspect_ratio.horizontal))
            .saturating_add(u64::from(self.pixel_aspect_ratio.vertical) / 2)
            / u64::from(self.pixel_aspect_ratio.vertical))
        .max(1)
        .min(u64::from(u32::MAX)) as u32;
        if self.transform.rotation.swaps_axes() {
            (aperture.height, adjusted_width)
        } else {
            (adjusted_width, aperture.height)
        }
    }

    /// Maps a coordinate in the unscaled, transformed clean aperture back to
    /// a source-frame coordinate. Mirroring is defined in display space.
    pub fn source_coordinate(self, display_x: u32, display_y: u32) -> (u32, u32) {
        let aperture = self.clean_aperture;
        let (rotated_width, rotated_height) = if self.transform.rotation.swaps_axes() {
            (aperture.height, aperture.width)
        } else {
            (aperture.width, aperture.height)
        };
        let (display_width, display_height) = self.display_dimensions();
        let raw_x = (u64::from(display_x) * u64::from(rotated_width)
            / u64::from(display_width.max(1))) as u32;
        let raw_y = (u64::from(display_y) * u64::from(rotated_height)
            / u64::from(display_height.max(1))) as u32;
        let x = if self.transform.mirror_horizontal {
            rotated_width.saturating_sub(1).saturating_sub(raw_x)
        } else {
            raw_x
        };
        let y = if self.transform.mirror_vertical {
            rotated_height.saturating_sub(1).saturating_sub(raw_y)
        } else {
            raw_y
        };
        let (source_x, source_y) = match self.transform.rotation {
            Rotation::Degrees0 => (x, y),
            Rotation::Degrees90 => (y, aperture.height.saturating_sub(1).saturating_sub(x)),
            Rotation::Degrees180 => (
                aperture.width.saturating_sub(1).saturating_sub(x),
                aperture.height.saturating_sub(1).saturating_sub(y),
            ),
            Rotation::Degrees270 => (aperture.width.saturating_sub(1).saturating_sub(y), x),
        };
        (
            aperture.x.saturating_add(source_x.min(aperture.width - 1)),
            aperture.y.saturating_add(source_y.min(aperture.height - 1)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_contract_is_explicit_and_valid() {
        let contract = FrameContract::canonical_bgra(1920, 1080);
        assert!(contract.is_canonical_bgra(1920, 1080));
        assert_eq!(contract.validate(1920, 1080), Ok(()));
        assert_eq!(
            Colorimetry::from_wire_code(contract.colorimetry.wire_code()),
            Some(contract.colorimetry)
        );
        assert_eq!(contract.colorimetry.bit_depth, 8);
        assert_eq!(contract.colorimetry.schema_version, 1);
        assert_eq!(
            Colorimetry::from_wire_code(contract.colorimetry.wire_code() | 0x8000_0000),
            None
        );
    }

    #[test]
    fn rotation_maps_display_coordinates_back_to_source() {
        let mut contract = FrameContract::canonical_bgra(3, 2);
        contract.transform.rotation = Rotation::Degrees90;

        assert_eq!(contract.source_coordinate(0, 0), (0, 1));
        assert_eq!(contract.source_coordinate(1, 2), (2, 0));
    }

    #[test]
    fn rejects_aperture_outside_frame() {
        let mut contract = FrameContract::canonical_bgra(10, 10);
        contract.clean_aperture = CleanAperture {
            x: 9,
            y: 0,
            width: 2,
            height: 10,
        };
        assert!(contract.validate(10, 10).is_err());
    }
}
