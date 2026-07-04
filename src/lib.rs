pub mod camera;
pub mod capture;
pub mod config;
pub mod error;
pub mod frame;
pub mod frame_transport;
pub mod layout;
pub mod pipeline;
pub mod ppm;
pub mod render;
pub mod system_extension;
pub mod virtual_camera;

pub use camera::{CameraDevice, CameraDiscovery, FrameSource, SyntheticFrameSource};
pub use capture::{
    NokhwaCameraDiscovery, NokhwaFrameSource, ThreadedNokhwaFrameSource, capture_one_with_timeout,
};
pub use config::{VideoFormat, VirtualCameraConfig};
pub use error::CameraManError;
pub use frame::{CapturedFrame, Frame, FrameMetadata, PixelFormat};
pub use frame_transport::{
    FrameSpoolSink, TransportFrame, default_frame_spool_path, read_latest_frame,
};
pub use layout::{Cell, CompositionLayout, GridLayout, GridLayoutCalculator};
pub use pipeline::{PipelineEngine, RenderReport, RunSummary};
pub use ppm::{PpmSequenceSink, write_ppm};
pub use render::Compositor;
pub use system_extension::{EXTENSION_BUNDLE_ID, ExtensionActivationStatus, ExtensionInstaller};
pub use virtual_camera::{MemorySink, VirtualCameraSink};
