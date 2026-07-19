use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::CFRetained;
use objc2_core_video::{
    CVMetalTexture, CVMetalTextureCache, CVMetalTextureGetTexture, CVPixelBuffer,
    CVPixelBufferGetHeight, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    kCVPixelFormatType_32BGRA,
};
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice, MTLPixelFormat, MTLTexture};

use crate::error::CameraManError;

/// Rust-only proof of the IOSurface/CoreVideo/Metal bridge. Production keeps
/// the CPU compositor until benchmarks justify adopting this path.
pub struct MetalPixelBufferBridge {
    cache: CFRetained<CVMetalTextureCache>,
    _device: Retained<ProtocolObject<dyn MTLDevice>>,
}

impl MetalPixelBufferBridge {
    pub fn new() -> Result<Self, CameraManError> {
        let device = MTLCreateSystemDefaultDevice().ok_or(
            CameraManError::VirtualCameraUnavailable("Metal device is unavailable"),
        )?;
        let mut raw_cache: *mut CVMetalTextureCache = std::ptr::null_mut();
        // SAFETY: the out-parameter points to writable pointer storage and the
        // retained Metal device remains alive for the complete call.
        let status = unsafe {
            CVMetalTextureCache::create(None, None, &device, None, NonNull::from(&mut raw_cache))
        };
        if status != 0 {
            return Err(CameraManError::VirtualCameraUnavailable(
                "CVMetalTextureCache creation failed",
            ));
        }
        let cache = NonNull::new(raw_cache).ok_or(CameraManError::VirtualCameraUnavailable(
            "CVMetalTextureCache returned null",
        ))?;
        // SAFETY: successful `CVMetalTextureCacheCreate` returns a non-null
        // object at +1 ownership, which is transferred into `CFRetained`.
        let cache = unsafe { CFRetained::from_raw(cache) };
        Ok(Self {
            cache,
            _device: device,
        })
    }

    pub fn texture_from_bgra(
        &self,
        pixel_buffer: &CVPixelBuffer,
    ) -> Result<CFRetained<CVMetalTexture>, CameraManError> {
        if CVPixelBufferGetPixelFormatType(pixel_buffer) != kCVPixelFormatType_32BGRA {
            return Err(CameraManError::UnsupportedPixelFormat);
        }
        let width = CVPixelBufferGetWidth(pixel_buffer);
        let height = CVPixelBufferGetHeight(pixel_buffer);
        let mut raw_texture: *mut CVMetalTexture = std::ptr::null_mut();
        // SAFETY: all Core Video/Metal objects are retained for the call, the
        // BGRA format was checked, and the out-parameter is writable.
        let status = unsafe {
            CVMetalTextureCache::create_texture_from_image(
                None,
                &self.cache,
                pixel_buffer,
                None,
                MTLPixelFormat::BGRA8Unorm,
                width,
                height,
                0,
                NonNull::from(&mut raw_texture),
            )
        };
        if status != 0 {
            return Err(CameraManError::VirtualCameraUnavailable(
                "CVPixelBuffer could not be mapped as a Metal texture",
            ));
        }
        let texture = NonNull::new(raw_texture).ok_or(CameraManError::VirtualCameraUnavailable(
            "CVMetalTextureCache returned null",
        ))?;
        // SAFETY: successful `CVMetalTextureCacheCreateTextureFromImage`
        // returns this non-null texture at +1 ownership.
        Ok(unsafe { CFRetained::from_raw(texture) })
    }

    pub fn metal_texture(
        &self,
        texture: &CVMetalTexture,
    ) -> Result<Retained<ProtocolObject<dyn MTLTexture>>, CameraManError> {
        CVMetalTextureGetTexture(texture).ok_or(CameraManError::VirtualCameraUnavailable(
            "CVMetalTexture has no Metal texture",
        ))
    }

    pub fn flush(&self) {
        self.cache.flush(0);
    }
}
