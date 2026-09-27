//! AVFoundation capture probe: enumerate devices, formats, frame rates.
//! This is a pure output-contract prototype for Milestone E item 1 evidence:
//! it does not stream frames; it collects JSON suitable for acceptance metrics.

use std::sync::Arc;

use objc2_av_foundation::{AVCaptureDeviceType};
use crate::error::CameraManError;
use crate::{CameraRuntime, FrameSource};

/// Single-threaded AVCaptureDevice probe: enumerates device metadata, supported
/// formats and frame rate ranges. Production path would own an AVCaptureSession
/// for latest_frame(); this binary probes first.

#[derive(Debug)]
pub struct AvCaptureProbe {
    pub unique_id: String,
    pub localized_name: Arc<String>,
}

impl AvCaptureProbe {
    /// Enumerate all connected video devices (placeholder: AVCaptureDeviceDiscoverySession).
    pub fn enumerate_devices() -> Result<Vec<Self>, CameraManError> {
        // Placeholder for AVCaptureDevice discoverySessionWithDeviceTypes:mediaType:position:
        Ok(vec![])
    }

    pub fn probe_device(&self) -> DeviceInfo {
        DeviceInfo {
            unique_id: self.unique_id.clone(),
            localized_name: self.localized_name.as_str().to_string(),
            format_descriptions: vec![],
            fps_ranges: vec![],
        }
    }
}

#[derive(Debug)]
pub struct DeviceInfo {
    pub unique_id: String,
    pub localized_name: String,
    pub format_descriptions: Vec<u64>,
    pub fps_ranges: Vec<(f64, f64)>,
}

impl FrameSource for AvCaptureProbe {
    fn latest_frame(&mut self) -> Result<Option<crate::CapturedFrame>, CameraManError> {
        unimplemented!("probe-only, no streaming yet")
    }
}

impl CameraRuntime for AvCaptureProbe {
    fn negotiated_format(&self) -> Option<crate::VideoFormat> {
        None
    }

    fn stall_age(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }

    fn frame_gap_exceeded(&self) -> bool {
        false
    }

    fn as_frame_source(&mut self) -> &mut dyn FrameSource {
        self
    }
}

fn enumerate_device_types() -> &'static [AVCaptureDeviceType] {
    // Placeholder for types: BuiltInWideAngleCamera etc.
    &[]
}

fn check_avauthorization_status() -> bool {
    // Placeholder: AVCaptureDevice authorizationStatusForMediaType(…)
    true
}
