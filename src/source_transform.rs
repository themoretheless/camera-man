use serde::{Deserialize, Serialize};

use crate::error::CameraManError;
use crate::media_contract::{CleanAperture, FrameContract, Rotation};

pub const TRANSFORM_SCALE: u16 = 1_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFit {
    #[default]
    Fit,
    Fill,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CropInsets {
    pub left_per_mille: u16,
    pub top_per_mille: u16,
    pub right_per_mille: u16,
    pub bottom_per_mille: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceTransform {
    pub crop: CropInsets,
    pub fit: SourceFit,
    pub rotation: Rotation,
    pub mirror_horizontal: bool,
    pub mirror_vertical: bool,
    pub opacity_per_mille: u16,
    pub position_x_per_mille: i16,
    pub position_y_per_mille: i16,
}

impl Default for SourceTransform {
    fn default() -> Self {
        Self {
            crop: CropInsets::default(),
            fit: SourceFit::Fit,
            rotation: Rotation::Degrees0,
            mirror_horizontal: false,
            mirror_vertical: false,
            opacity_per_mille: TRANSFORM_SCALE,
            position_x_per_mille: 0,
            position_y_per_mille: 0,
        }
    }
}

impl SourceTransform {
    pub const fn is_identity(self) -> bool {
        self.crop.left_per_mille == 0
            && self.crop.top_per_mille == 0
            && self.crop.right_per_mille == 0
            && self.crop.bottom_per_mille == 0
            && matches!(self.fit, SourceFit::Fit)
            && matches!(self.rotation, Rotation::Degrees0)
            && !self.mirror_horizontal
            && !self.mirror_vertical
            && self.opacity_per_mille == TRANSFORM_SCALE
            && self.position_x_per_mille == 0
            && self.position_y_per_mille == 0
    }

    pub fn validate(self) -> Result<(), CameraManError> {
        if self.crop.left_per_mille >= TRANSFORM_SCALE
            || self.crop.right_per_mille >= TRANSFORM_SCALE
            || self.crop.top_per_mille >= TRANSFORM_SCALE
            || self.crop.bottom_per_mille >= TRANSFORM_SCALE
            || self.crop.left_per_mille + self.crop.right_per_mille >= TRANSFORM_SCALE
            || self.crop.top_per_mille + self.crop.bottom_per_mille >= TRANSFORM_SCALE
        {
            return Err(CameraManError::InvalidMediaContract(
                "source crop must leave a non-empty image",
            ));
        }
        if self.opacity_per_mille > TRANSFORM_SCALE
            || !(-1_000..=1_000).contains(&self.position_x_per_mille)
            || !(-1_000..=1_000).contains(&self.position_y_per_mille)
        {
            return Err(CameraManError::InvalidMediaContract(
                "source opacity or position is outside its normalized range",
            ));
        }
        Ok(())
    }

    pub fn sanitized(mut self) -> Self {
        self.crop.left_per_mille = self.crop.left_per_mille.min(999);
        self.crop.right_per_mille = self
            .crop
            .right_per_mille
            .min(999 - self.crop.left_per_mille);
        self.crop.top_per_mille = self.crop.top_per_mille.min(999);
        self.crop.bottom_per_mille = self
            .crop
            .bottom_per_mille
            .min(999 - self.crop.top_per_mille);
        self.opacity_per_mille = self.opacity_per_mille.min(TRANSFORM_SCALE);
        self.position_x_per_mille = self.position_x_per_mille.clamp(-1_000, 1_000);
        self.position_y_per_mille = self.position_y_per_mille.clamp(-1_000, 1_000);
        self
    }

    pub fn apply_to_contract(
        self,
        mut contract: FrameContract,
        frame_width: u32,
        frame_height: u32,
    ) -> Result<FrameContract, CameraManError> {
        self.validate()?;
        contract.validate(frame_width, frame_height)?;
        let base = contract.clean_aperture;
        let left = scale_dimension(base.width, self.crop.left_per_mille);
        let top = scale_dimension(base.height, self.crop.top_per_mille);
        let right = scale_dimension(base.width, self.crop.right_per_mille);
        let bottom = scale_dimension(base.height, self.crop.bottom_per_mille);
        contract.clean_aperture = CleanAperture {
            x: base.x + left,
            y: base.y + top,
            width: base.width - left - right,
            height: base.height - top - bottom,
        };
        contract.transform.rotation = add_rotation(contract.transform.rotation, self.rotation);
        contract.transform.mirror_horizontal ^= self.mirror_horizontal;
        contract.transform.mirror_vertical ^= self.mirror_vertical;
        contract.validate(frame_width, frame_height)?;
        Ok(contract)
    }
}

const fn scale_dimension(value: u32, per_mille: u16) -> u32 {
    (value as u64 * per_mille as u64 / TRANSFORM_SCALE as u64) as u32
}

const fn add_rotation(left: Rotation, right: Rotation) -> Rotation {
    let quarter_turns = rotation_quarter_turns(left) + rotation_quarter_turns(right);
    match quarter_turns % 4 {
        0 => Rotation::Degrees0,
        1 => Rotation::Degrees90,
        2 => Rotation::Degrees180,
        _ => Rotation::Degrees270,
    }
}

const fn rotation_quarter_turns(rotation: Rotation) -> u8 {
    match rotation {
        Rotation::Degrees0 => 0,
        Rotation::Degrees90 => 1,
        Rotation::Degrees180 => 2,
        Rotation::Degrees270 => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_and_rotation_compose_with_capture_metadata() {
        let mut contract = FrameContract::canonical_bgra(100, 80);
        contract.transform.rotation = Rotation::Degrees90;
        let transformed = SourceTransform {
            crop: CropInsets {
                left_per_mille: 100,
                top_per_mille: 250,
                right_per_mille: 200,
                bottom_per_mille: 0,
            },
            rotation: Rotation::Degrees270,
            ..SourceTransform::default()
        }
        .apply_to_contract(contract, 100, 80)
        .unwrap();
        assert_eq!(transformed.clean_aperture.x, 10);
        assert_eq!(transformed.clean_aperture.y, 20);
        assert_eq!(transformed.clean_aperture.width, 70);
        assert_eq!(transformed.clean_aperture.height, 60);
        assert_eq!(transformed.transform.rotation, Rotation::Degrees0);
    }

    #[test]
    fn sanitization_never_leaves_an_empty_crop() {
        let transform = SourceTransform {
            crop: CropInsets {
                left_per_mille: u16::MAX,
                right_per_mille: u16::MAX,
                top_per_mille: u16::MAX,
                bottom_per_mille: u16::MAX,
            },
            opacity_per_mille: u16::MAX,
            position_x_per_mille: i16::MIN,
            position_y_per_mille: i16::MAX,
            ..SourceTransform::default()
        }
        .sanitized();
        assert_eq!(transform.validate(), Ok(()));
    }
}
