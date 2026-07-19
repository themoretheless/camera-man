use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::config::{
    VIRTUAL_CAMERA_DEFAULT_FPS, VIRTUAL_CAMERA_HEIGHT, VIRTUAL_CAMERA_MAX_FPS,
    VIRTUAL_CAMERA_MIN_FPS, VIRTUAL_CAMERA_WIDTH, VideoFormat,
};
use crate::error::CameraManError;
use crate::frame::PixelFormat;
use crate::media_contract::{Colorimetry, FrameContract};

pub const FIXED_OUTPUT_FORMAT: VideoFormat = VideoFormat {
    width: VIRTUAL_CAMERA_WIDTH,
    height: VIRTUAL_CAMERA_HEIGHT,
    fps: VIRTUAL_CAMERA_DEFAULT_FPS,
    pixel_format: PixelFormat::Bgra8,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyMode {
    InteractiveLatestFrame,
    BufferedQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameOwnershipMode {
    BorrowedUntilRelease,
    Owned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamCapability {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub min_fps: u32,
    pub max_fps: u32,
    pub colorimetry: Colorimetry,
    pub latency_mode: LatencyMode,
    pub ownership: FrameOwnershipMode,
}

pub const FIXED_OUTPUT_CAPABILITY: StreamCapability = StreamCapability {
    width: VIRTUAL_CAMERA_WIDTH,
    height: VIRTUAL_CAMERA_HEIGHT,
    pixel_format: PixelFormat::Bgra8,
    min_fps: VIRTUAL_CAMERA_MIN_FPS,
    max_fps: VIRTUAL_CAMERA_MAX_FPS,
    colorimetry: Colorimetry::BT709_FULL_OPAQUE,
    latency_mode: LatencyMode::InteractiveLatestFrame,
    ownership: FrameOwnershipMode::BorrowedUntilRelease,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamRequest {
    pub format: VideoFormat,
    pub colorimetry: Colorimetry,
    pub latency_mode: LatencyMode,
    pub ownership: FrameOwnershipMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedStream {
    pub format: VideoFormat,
    pub contract: FrameContract,
    pub latency_mode: LatencyMode,
    pub ownership: FrameOwnershipMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRejection {
    Dimensions,
    PixelFormat,
    FrameRate,
    Colorimetry,
    LatencyMode,
    Ownership,
}

impl CapabilityRejection {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Dimensions => "requested dimensions are not advertised",
            Self::PixelFormat => "requested pixel format is not advertised",
            Self::FrameRate => "requested frame rate is outside the advertised range",
            Self::Colorimetry => "requested color interpretation is not advertised",
            Self::LatencyMode => "requested latency mode is not implemented",
            Self::Ownership => "requested frame ownership mode is not implemented",
        }
    }
}

impl std::fmt::Display for CapabilityRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.reason())
    }
}

impl std::error::Error for CapabilityRejection {}

pub fn negotiate_stream(request: StreamRequest) -> Result<NegotiatedStream, CapabilityRejection> {
    let capability = FIXED_OUTPUT_CAPABILITY;
    if request.format.width != capability.width || request.format.height != capability.height {
        return Err(CapabilityRejection::Dimensions);
    }
    if request.format.pixel_format != capability.pixel_format {
        return Err(CapabilityRejection::PixelFormat);
    }
    if !(capability.min_fps..=capability.max_fps).contains(&request.format.fps) {
        return Err(CapabilityRejection::FrameRate);
    }
    if request.colorimetry != capability.colorimetry {
        return Err(CapabilityRejection::Colorimetry);
    }
    if request.latency_mode != capability.latency_mode {
        return Err(CapabilityRejection::LatencyMode);
    }
    if request.ownership != capability.ownership {
        return Err(CapabilityRejection::Ownership);
    }
    let mut contract = FrameContract::canonical_bgra(request.format.width, request.format.height);
    contract.colorimetry = request.colorimetry;
    Ok(NegotiatedStream {
        format: request.format,
        contract,
        latency_mode: request.latency_mode,
        ownership: request.ownership,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatSnapshot {
    pub epoch: u64,
    pub format: VideoFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatTransition {
    pub previous: FormatSnapshot,
    pub current: FormatSnapshot,
}

#[derive(Debug)]
pub struct FormatEpochCoordinator {
    state: Mutex<FormatSnapshot>,
}

impl Default for FormatEpochCoordinator {
    fn default() -> Self {
        Self::new(FIXED_OUTPUT_FORMAT)
    }
}

impl FormatEpochCoordinator {
    pub fn new(format: VideoFormat) -> Self {
        Self {
            state: Mutex::new(FormatSnapshot { epoch: 1, format }),
        }
    }

    pub fn snapshot(&self) -> FormatSnapshot {
        *self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Commits the format and its epoch only after all format-bound resources
    /// (pools, sampling maps and cached stale frames) reset successfully.
    pub fn renegotiate(
        &self,
        next: VideoFormat,
        reset_resources: impl FnOnce(FormatSnapshot) -> Result<(), CameraManError>,
    ) -> Result<Option<FormatTransition>, CameraManError> {
        validate_advertised_format(next)?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.format == next {
            return Ok(None);
        }
        let previous = *state;
        let candidate = FormatSnapshot {
            epoch: state.epoch.wrapping_add(1).max(1),
            format: next,
        };
        reset_resources(candidate)?;
        *state = candidate;
        Ok(Some(FormatTransition {
            previous,
            current: candidate,
        }))
    }
}

/// CameraMan intentionally advertises one tested geometry/pixel format. Frame
/// cadence may vary within the supported range and does not create an
/// untested CoreMedia format-description surface.
pub fn validate_advertised_format(format: VideoFormat) -> Result<(), CameraManError> {
    negotiate_stream(StreamRequest {
        format,
        colorimetry: FIXED_OUTPUT_CAPABILITY.colorimetry,
        latency_mode: FIXED_OUTPUT_CAPABILITY.latency_mode,
        ownership: FIXED_OUTPUT_CAPABILITY.ownership,
    })
    .map(|_| ())
    .map_err(|rejection| CameraManError::InvalidMediaContract(rejection.reason()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_reset_does_not_publish_new_epoch() {
        let coordinator = FormatEpochCoordinator::default();
        let before = coordinator.snapshot();
        let mut next = before.format;
        next.fps = 60;

        let result = coordinator.renegotiate(next, |_| {
            Err(CameraManError::VirtualCameraUnavailable(
                "test reset failure",
            ))
        });

        assert!(result.is_err());
        assert_eq!(coordinator.snapshot(), before);
    }

    #[test]
    fn successful_reset_and_epoch_are_committed_together() {
        let coordinator = FormatEpochCoordinator::default();
        let mut next = coordinator.snapshot().format;
        next.fps = 60;
        let mut reset_epoch = None;

        let transition = coordinator
            .renegotiate(next, |candidate| {
                reset_epoch = Some(candidate.epoch);
                Ok(())
            })
            .unwrap()
            .unwrap();

        assert_eq!(reset_epoch, Some(transition.current.epoch));
        assert_eq!(coordinator.snapshot(), transition.current);
    }

    #[test]
    fn does_not_claim_unimplemented_output_geometry() {
        let mut unsupported = FIXED_OUTPUT_FORMAT;
        unsupported.width = 1280;
        assert!(validate_advertised_format(unsupported).is_err());
    }

    #[test]
    fn typed_negotiation_accepts_only_the_complete_advertised_contract() {
        let request = StreamRequest {
            format: FIXED_OUTPUT_FORMAT,
            colorimetry: Colorimetry::BT709_FULL_OPAQUE,
            latency_mode: LatencyMode::InteractiveLatestFrame,
            ownership: FrameOwnershipMode::BorrowedUntilRelease,
        };
        assert_eq!(
            negotiate_stream(request).unwrap().format,
            FIXED_OUTPUT_FORMAT
        );

        let rejection = negotiate_stream(StreamRequest {
            latency_mode: LatencyMode::BufferedQuality,
            ..request
        })
        .unwrap_err();
        assert_eq!(rejection, CapabilityRejection::LatencyMode);
        assert!(rejection.reason().contains("latency"));
    }
}
