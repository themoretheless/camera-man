use crate::frame::PixelFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoFormat {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub pixel_format: PixelFormat,
}

impl VideoFormat {
    pub const fn hd_1080p_bgra() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
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
            device_uid: "7F1D9A42-3C58-4E6B-9D0A-2B6F8C1E5A17",
            device_name: "CameraMan Virtual Camera",
            format: VideoFormat::hd_1080p_bgra(),
        }
    }
}
