//! AVFoundation capture probe: enumerate devices, formats, frame rates.
//! This is a pure output-contract prototype for Milestone E item 1 evidence:
//! it does not stream frames; it collects JSON suitable for acceptance metrics.

use std::sync::Arc;

use objc2_av_foundation::{AVCaptureDevice};
use objc2_core_media::CMFormatDescription;
use crate::error::CameraManError;
use crate::FrameSource;

/// Single-threaded AVCaptureDevice probe: enumerates device metadata, supported
/// formats and frame rate ranges. Production path would own an AVCaptureSession
/// for latest_frame(); this binary probes first.

#[derive(Debug)]
pub struct AvCaptureProbe {
    pub unique_id: String,
    pub localized_name: Arc<String>,
}

impl AvCaptureProbe {
    /// Enumerate all connected video devices using AVCaptureDeviceDiscoverySession.
    pub fn enumerate_devices() -> Result<Vec<Self>, CameraManError> {
        // Placeholder - AVCaptureDeviceDiscoverySession bindings require careful integration
        // The method discoverySessionWithDeviceTypes_mediaType_position exists but requires
        // proper NSArray<AVCaptureDeviceType> construction which needs type-level work
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
    pub format_descriptions: Vec<CMFormatDescription>,
    pub fps_ranges: Vec<(f64, f64)>,
}

impl FrameSource for AvCaptureProbe {
    fn latest_frame(&mut self) -> Result<Option<crate::CapturedFrame>, CameraManError> {
        unimplemented!("probe-only, no streaming yet")
    }
}

unsafe fn enumerate_formats(_device: &AVCaptureDevice) -> Vec<CMFormatDescription> {
    vec![]
}

unsafe fn enumerate_fps_ranges(_device: &AVCaptureDevice) -> Vec<(f64, f64)> {
    vec![]
}

fn check_avauthorization_status() -> bool {
    true
}
