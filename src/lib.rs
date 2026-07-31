//! Pure-Rust frame, composition, pipeline and transport building blocks for
//! CameraMan.
//!
//! The core data flow is `FrameSource -> Compositor -> VirtualCameraSink`.
//! Platform capture, shared memory and CoreMediaIO integration live behind
//! those small interfaces. See `examples/custom_source.rs` and
//! `examples/custom_sink.rs` for minimal implementations.

pub mod app_group;
pub mod atomic_file;
pub mod backpressure;
pub mod benchmarking;
pub mod camera;
#[cfg(feature = "camera-capture")]
pub mod capture;
pub mod client_authorization;
pub mod config;
pub mod diagnostics;
pub mod error;
pub mod format_negotiation;
pub mod frame;
pub mod frame_integrity;
pub mod frame_transport;
#[cfg(feature = "gpu-compositor-experiment")]
pub mod gpu_experiment;
pub mod invalidation;
pub mod layout;
pub mod media_contract;
pub mod media_time;
#[cfg(all(target_os = "macos", feature = "metal-interop-experiment"))]
pub mod metal_interop;
pub mod panic_boundary;
pub mod parser_limits;
pub mod performance;
pub mod pipeline;
pub mod ppm;
pub mod provisioning_profile;
pub mod quality;
pub mod render;
pub mod scene_schema;
pub mod shared_memory_transport;
pub mod source_descriptor;
pub mod source_health;
pub mod source_transform;
pub mod stream_runtime;
pub mod system_extension;
pub mod transport;
pub mod virtual_camera;
pub mod wire;

pub use app_group::{APP_GROUP_INFO_KEY, bundled_application_group, shared_frame_file_path};
pub use atomic_file::replace_file_atomically;
pub use camera::{CameraDevice, CameraDiscovery, FrameSource, SyntheticFrameSource};
#[cfg(feature = "camera-capture")]
pub use capture::{
    NokhwaCameraDiscovery, NokhwaFrameSource, ThreadedNokhwaFrameSource, capture_one_with_timeout,
};
pub use client_authorization::{
    ClientAuthorization, ClientDenialReason, ClientIdentity, authorize_client,
};
pub use config::{
    VIRTUAL_CAMERA_DEFAULT_FPS, VIRTUAL_CAMERA_DEVICE_NAME, VIRTUAL_CAMERA_DEVICE_UID,
    VIRTUAL_CAMERA_FPS_PRESETS, VIRTUAL_CAMERA_HEIGHT, VIRTUAL_CAMERA_MAX_FPS,
    VIRTUAL_CAMERA_MIN_FPS, VIRTUAL_CAMERA_STREAM_NAME, VIRTUAL_CAMERA_STREAM_UID,
    VIRTUAL_CAMERA_WIDTH, VideoFormat, VirtualCameraConfig,
};
pub use diagnostics::{
    BuildDiagnostics, DiagnosticEvent, DiagnosticEventKind, DiagnosticEventRing, DiagnosticFormat,
    DiagnosticsSnapshot, DropCounters, DropCountersSnapshot, DropReason, LatencyHistogramSnapshot,
    LatencyHistograms, PipelineStage, PlatformDiagnostics, StageSpan, diagnostic_events,
    drop_counters, latency_histograms, output_frame_count, record_drop, record_output_frame,
    redact_path, stage_span,
};
pub use error::{CameraManError, CaptureErrorKind, ErrorCode};
pub use format_negotiation::{
    CapabilityRejection, FIXED_OUTPUT_CAPABILITY, FIXED_OUTPUT_FORMAT, FormatEpochCoordinator,
    FormatSnapshot, FormatTransition, FrameOwnershipMode, LatencyMode, NegotiatedStream,
    StreamCapability, StreamRequest, negotiate_stream, validate_advertised_format,
};
pub use frame::{
    CapturedFrame, FRAME_MEMORY_LIMIT_ENV, Frame, FrameLimits, FrameMetadata, FrameView,
    PixelFormat, default_frame_limits,
};
pub use frame_integrity::{
    FrameDiscontinuity, FrameIntegritySnapshot, FrameIntegrityState, FrameObservation,
    IntegrityDropReason, classify_frame_integrity,
};
pub use frame_transport::{
    FrameSpoolSink, TransportFrame, default_frame_spool_path, read_latest_frame,
};
#[cfg(feature = "gpu-compositor-experiment")]
pub use gpu_experiment::{
    ExperimentalCompositor, GpuCompositorExperiment, GpuExperimentError, GpuProbe,
};
pub use invalidation::{InvalidationTargets, SceneChange};
pub use layout::{Cell, CompositionLayout, GridLayout, GridLayoutCalculator};
pub use media_contract::{
    AlphaMode, COLOR_CONTRACT_SCHEMA_VERSION, CleanAperture, ColorPrimaries, ColorRange,
    Colorimetry, FrameContract, MatrixCoefficients, PixelAspectRatio, Rotation, TransferFunction,
    TransformMetadata,
};
pub use media_time::{
    CaptureTimestamps, Clock, MediaTimestamp, MonotonicTimestampNanos, SystemClock,
    WallTimestampNanos, monotonic_time_nanos, new_generation_id, wall_time_nanos,
};
#[cfg(all(target_os = "macos", feature = "metal-interop-experiment"))]
pub use metal_interop::MetalPixelBufferBridge;
pub use panic_boundary::{contain_panic, contain_panic_unit, panic_message, report_line};
pub use parser_limits::{
    BENCHMARK_REPORT_PARSER_LIMITS, PREFERENCES_PARSER_LIMITS, PROFILE_PARSER_LIMITS,
    ParserLimitError, ParserLimits, SCENE_PARSER_LIMITS, read_bounded, validate_input_size,
    validate_json_envelope,
};
pub use performance::{
    CopyLedger, CopyLedgerPerOutputSnapshot, CopyLedgerSnapshot, CopyStage,
    CopyStagePerOutputSnapshot, CopyStageSnapshot, copy_ledger, copy_ledger_markdown,
};
pub use pipeline::{PipelineEngine, PipelineMetrics, RenderReport, RunSummary};
pub use ppm::{PpmMetadata, PpmSequenceSink, write_ppm, write_ppm_with_metadata};
pub use provisioning_profile::{ProfileMetadata, ProvisioningProfileError};
#[cfg(feature = "linear-light-experiment")]
pub use quality::resize_bilinear_linear_light;
pub use quality::{
    QualityConfidenceInterval, QualityGate, QualityMetrics, QualitySummary, compare_frames,
    representative_quality_fixture, summarize_quality,
};
pub use render::{Compositor, ScalingFilter};
pub use scene_schema::{
    LegacySceneInputMode, MissingSourcePolicy, SCENE_SCHEMA_VERSION, SceneDocument,
    SceneSchemaError,
};
pub use shared_memory_transport::{
    BorrowedTransportFrame, ConsumerProgress, ProducerProgress, SHARED_PROTOCOL_MAGIC,
    SHARED_PROTOCOL_SLOT_COUNT, SHARED_PROTOCOL_VERSION, SharedFrameEndpoint, SharedFrameReader,
    SharedFrameSink, SharedHeaderMetadata, SharedProtocolValidationError, SharedSlotMetadata,
    checked_mapped_len, checked_slot_offset, default_shared_frame_endpoint,
    default_shared_memory_name, validate_shared_header_metadata, validate_shared_slot_metadata,
};
pub use source_descriptor::{
    MAX_SOURCE_KEY_BYTES, MAX_SOURCE_LOCATOR_BYTES, SourceDescriptor, SourceDescriptorError,
    SourceKind,
};
pub use source_health::{
    ConsumerAckHealth, DropHealth, FormatHealth, FreshnessHealth, JitterHealth, ReconnectHealth,
    SourceHealthSummary, SourceHealthVector,
};
pub use source_transform::{CropInsets, SourceFit, SourceTransform, TRANSFORM_SCALE};
pub use stream_runtime::{
    DeadlinePacer, LifecycleError, PacingPlan, StartAction, StopAction, StreamLifecycle,
    StreamState, begin_start_reaping_finished_worker, reset_after_contained_panic,
};
pub use system_extension::{EXTENSION_BUNDLE_ID, ExtensionActivationStatus, ExtensionInstaller};
pub use transport::{
    FrameTransportMode, FrameTransportReader, FrameTransportSink, TransportFrameRef,
};
pub use virtual_camera::{MemorySink, VirtualCameraSink};
pub use wire::{WireFrameDescriptorDto, WireFrameTimingDto, WirePixelFormat};
