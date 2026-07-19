use crate::frame::PixelFormat;
use serde::{Deserialize, Serialize};

pub const VIRTUAL_CAMERA_DEVICE_UID: &str = "7F1D9A42-3C58-4E6B-9D0A-2B6F8C1E5A17";
pub const VIRTUAL_CAMERA_DEVICE_NAME: &str = "CameraMan Virtual Camera";
pub const VIRTUAL_CAMERA_STREAM_UID: &str = "A3B8C2D1-4E5F-4A6B-8C7D-9E0F1A2B3C4D";
pub const VIRTUAL_CAMERA_STREAM_NAME: &str = "CameraMan Output";
pub const VIRTUAL_CAMERA_WIDTH: u32 = 1920;
pub const VIRTUAL_CAMERA_HEIGHT: u32 = 1080;
pub const VIRTUAL_CAMERA_MIN_FPS: u32 = 15;
pub const VIRTUAL_CAMERA_DEFAULT_FPS: u32 = 30;
pub const VIRTUAL_CAMERA_MAX_FPS: u32 = 60;
pub const VIRTUAL_CAMERA_FPS_PRESETS: [u32; 3] =
    [24, VIRTUAL_CAMERA_DEFAULT_FPS, VIRTUAL_CAMERA_MAX_FPS];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoFormat {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub pixel_format: PixelFormat,
}

impl VideoFormat {
    pub const fn hd_1080p_bgra() -> Self {
        Self {
            width: VIRTUAL_CAMERA_WIDTH,
            height: VIRTUAL_CAMERA_HEIGHT,
            fps: VIRTUAL_CAMERA_DEFAULT_FPS,
            pixel_format: PixelFormat::Bgra8,
        }
    }

    pub const fn aspect_ratio(self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualCameraConfig {
    pub device_uid: &'static str,
    pub device_name: &'static str,
    pub format: VideoFormat,
}

impl Default for VirtualCameraConfig {
    fn default() -> Self {
        Self {
            device_uid: VIRTUAL_CAMERA_DEVICE_UID,
            device_name: VIRTUAL_CAMERA_DEVICE_NAME,
            format: VideoFormat::hd_1080p_bgra(),
        }
    }
}
