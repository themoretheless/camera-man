use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::config::VideoFormat;
use crate::media_time::{monotonic_time_nanos, wall_time_nanos};
use crate::performance::{CopyLedgerPerOutputSnapshot, copy_ledger, percentile_index};

const PIPELINE_STAGE_COUNT: usize = 6;
const DROP_REASON_COUNT: usize = 7;
const LATENCY_WINDOW_CAPACITY: usize = 2_048;
const EVENT_RING_CAPACITY: usize = 256;
const FEEDBACK_EVENT_INTERVAL_NANOS: u64 = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum PipelineStage {
    Capture = 0,
    Compose = 1,
    Publish = 2,
    Consume = 3,
    PixelBuffer = 4,
    SendSample = 5,
}

impl PipelineStage {
    pub const ALL: [Self; PIPELINE_STAGE_COUNT] = [
        Self::Capture,
        Self::Compose,
        Self::Publish,
        Self::Consume,
        Self::PixelBuffer,
        Self::SendSample,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Compose => "compose",
            Self::Publish => "publish",
            Self::Consume => "consume",
            Self::PixelBuffer => "pixel_buffer",
            Self::SendSample => "send_sample",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }

    const fn signpost_code(self) -> u32 {
        0x100 + self as u32
    }
}

/// One stable tracing interval mirrored to a native macOS signpost.
pub struct StageSpan {
    stage: PipelineStage,
    started: Instant,
    _tracing: tracing::span::EnteredSpan,
    _signpost: NativeSignpost,
}

pub fn stage_span(stage: PipelineStage, frame_sequence: u64, width: u32, height: u32) -> StageSpan {
    let tracing = match stage {
        PipelineStage::Capture => {
            tracing::info_span!("capture", stage = "capture", frame_sequence, width, height)
                .entered()
        }
        PipelineStage::Compose => {
            tracing::info_span!("compose", stage = "compose", frame_sequence, width, height)
                .entered()
        }
        PipelineStage::Publish => {
            tracing::info_span!("publish", stage = "publish", frame_sequence, width, height)
                .entered()
        }
        PipelineStage::Consume => {
            tracing::info_span!("consume", stage = "consume", frame_sequence, width, height)
                .entered()
        }
        PipelineStage::PixelBuffer => tracing::info_span!(
            "pixel_buffer",
            stage = "pixel_buffer",
            frame_sequence,
            width,
            height
        )
        .entered(),
        PipelineStage::SendSample => tracing::info_span!(
            "send_sample",
            stage = "send_sample",
            frame_sequence,
            width,
            height
        )
        .entered(),
    };
    StageSpan {
        stage,
        started: Instant::now(),
        _tracing: tracing,
        _signpost: NativeSignpost::begin(stage, frame_sequence, width, height),
    }
}

impl Drop for StageSpan {
    fn drop(&mut self) {
        latency_histograms().record(self.stage, self.started.elapsed().as_nanos() as u64);
    }
}

#[cfg(target_os = "macos")]
struct NativeSignpost {
    code: u32,
    frame_sequence: usize,
    dimensions: usize,
}

#[cfg(target_os = "macos")]
impl NativeSignpost {
    fn begin(stage: PipelineStage, frame_sequence: u64, width: u32, height: u32) -> Self {
        let code = stage.signpost_code();
        let frame_sequence = frame_sequence as usize;
        let dimensions = ((width as usize) << 32) | height as usize;
        // SAFETY: the System signpost function accepts only scalar values and
        // does not retain or dereference any Rust memory.
        unsafe {
            kdebug_signpost_start(code, frame_sequence, dimensions, 0, 0);
        }
        Self {
            code,
            frame_sequence,
            dimensions,
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativeSignpost {
    fn drop(&mut self) {
        // SAFETY: this closes the scalar-only signpost opened by `begin`; no
        // pointers or borrowed Rust values cross the FFI boundary.
        unsafe {
            kdebug_signpost_end(self.code, self.frame_sequence, self.dimensions, 0, 0);
        }
    }
}

#[cfg(target_os = "macos")]
#[link(name = "System")]
unsafe extern "C" {
    fn kdebug_signpost_start(
        code: u32,
        arg1: usize,
        arg2: usize,
        arg3: usize,
        arg4: usize,
    ) -> libc::c_int;
    fn kdebug_signpost_end(
        code: u32,
        arg1: usize,
        arg2: usize,
        arg3: usize,
        arg4: usize,
    ) -> libc::c_int;
}

#[cfg(not(target_os = "macos"))]
struct NativeSignpost;

#[cfg(not(target_os = "macos"))]
impl NativeSignpost {
    fn begin(_stage: PipelineStage, _frame_sequence: u64, _width: u32, _height: u32) -> Self {
        Self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum DropReason {
    SourceMissing = 0,
    Late = 1,
    QueueReplacement = 2,
    AllSlotsBusy = 3,
    Stale = 4,
    PoolExhausted = 5,
    Discontinuity = 6,
}

impl DropReason {
    pub const ALL: [Self; DROP_REASON_COUNT] = [
        Self::SourceMissing,
        Self::Late,
        Self::QueueReplacement,
        Self::AllSlotsBusy,
        Self::Stale,
        Self::PoolExhausted,
        Self::Discontinuity,
    ];

    const fn index(self) -> usize {
        self as usize
    }
}

pub struct DropCounters {
    counters: [AtomicU64; DROP_REASON_COUNT],
}

impl Default for DropCounters {
    fn default() -> Self {
        Self {
            counters: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropCountersSnapshot {
    pub source_missing: u64,
    pub late: u64,
    pub queue_replacement: u64,
    pub all_slots_busy: u64,
    pub stale: u64,
    pub pool_exhausted: u64,
    pub discontinuity: u64,
}

impl DropCounters {
    pub fn increment(&self, reason: DropReason, count: u64) {
        self.counters[reason.index()].fetch_add(count, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> DropCountersSnapshot {
        let load = |reason: DropReason| self.counters[reason.index()].load(Ordering::Relaxed);
        DropCountersSnapshot {
            source_missing: load(DropReason::SourceMissing),
            late: load(DropReason::Late),
            queue_replacement: load(DropReason::QueueReplacement),
            all_slots_busy: load(DropReason::AllSlotsBusy),
            stale: load(DropReason::Stale),
            pool_exhausted: load(DropReason::PoolExhausted),
            discontinuity: load(DropReason::Discontinuity),
        }
    }
}

pub fn drop_counters() -> &'static DropCounters {
    static COUNTERS: OnceLock<DropCounters> = OnceLock::new();
    COUNTERS.get_or_init(DropCounters::default)
}

pub fn record_drop(reason: DropReason, count: u64, frame_sequence: Option<u64>) {
    if count == 0 {
        return;
    }
    drop_counters().increment(reason, count);
    let now = monotonic_time_nanos();
    if !diagnostic_feedback_gate().should_emit(reason.index(), now) {
        return;
    }
    diagnostic_events().record(DiagnosticEvent {
        monotonic_timestamp_nanos: now,
        kind: DiagnosticEventKind::Drop,
        drop_reason: Some(reason),
        frame_sequence,
        value: count,
    });
}

struct DiagnosticFeedbackGate {
    last_emitted_nanos: [AtomicU64; DROP_REASON_COUNT],
    minimum_interval_nanos: u64,
}

impl DiagnosticFeedbackGate {
    fn new(minimum_interval_nanos: u64) -> Self {
        Self {
            last_emitted_nanos: std::array::from_fn(|_| AtomicU64::new(0)),
            minimum_interval_nanos,
        }
    }

    fn should_emit(&self, key: usize, now_nanos: u64) -> bool {
        let Some(last) = self.last_emitted_nanos.get(key) else {
            return false;
        };
        let mut observed = last.load(Ordering::Relaxed);
        loop {
            if observed != 0 && now_nanos.saturating_sub(observed) < self.minimum_interval_nanos {
                return false;
            }
            match last.compare_exchange_weak(
                observed,
                now_nanos.max(1),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(changed) => observed = changed,
            }
        }
    }
}

fn diagnostic_feedback_gate() -> &'static DiagnosticFeedbackGate {
    static GATE: OnceLock<DiagnosticFeedbackGate> = OnceLock::new();
    GATE.get_or_init(|| DiagnosticFeedbackGate::new(FEEDBACK_EVENT_INTERVAL_NANOS))
}

#[derive(Default)]
struct LatencyWindow {
    samples: VecDeque<u64>,
}

impl LatencyWindow {
    fn record(&mut self, value_nanos: u64) {
        if self.samples.len() == LATENCY_WINDOW_CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(value_nanos);
    }

    fn snapshot(&self, stage: PipelineStage) -> LatencyHistogramSnapshot {
        let mut samples = self.samples.iter().copied().collect::<Vec<_>>();
        samples.sort_unstable();
        LatencyHistogramSnapshot {
            stage,
            samples: samples.len() as u64,
            p50_nanos: percentile(&samples, 50),
            p95_nanos: percentile(&samples, 95),
            p99_nanos: percentile(&samples, 99),
            max_nanos: samples.last().copied().unwrap_or(0),
        }
    }
}

pub struct LatencyHistograms {
    windows: [Mutex<LatencyWindow>; PIPELINE_STAGE_COUNT],
}

impl Default for LatencyHistograms {
    fn default() -> Self {
        Self {
            windows: std::array::from_fn(|_| Mutex::new(LatencyWindow::default())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatencyHistogramSnapshot {
    pub stage: PipelineStage,
    pub samples: u64,
    pub p50_nanos: u64,
    pub p95_nanos: u64,
    pub p99_nanos: u64,
    pub max_nanos: u64,
}

impl LatencyHistograms {
    pub fn record(&self, stage: PipelineStage, value_nanos: u64) {
        let mut window = self.windows[stage.index()]
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        window.record(value_nanos);
    }

    pub fn snapshot(&self) -> Vec<LatencyHistogramSnapshot> {
        PipelineStage::ALL
            .into_iter()
            .map(|stage| {
                self.windows[stage.index()]
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .snapshot(stage)
            })
            .collect()
    }
}

pub fn latency_histograms() -> &'static LatencyHistograms {
    static HISTOGRAMS: OnceLock<LatencyHistograms> = OnceLock::new();
    HISTOGRAMS.get_or_init(LatencyHistograms::default)
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    percentile_index(sorted.len(), percentile).map_or(0, |index| sorted[index])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticEventKind {
    Drop,
    WriterRestart,
    FormatEpoch,
    TransportConnected,
    TransportDisconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticEvent {
    pub monotonic_timestamp_nanos: u64,
    pub kind: DiagnosticEventKind,
    pub drop_reason: Option<DropReason>,
    pub frame_sequence: Option<u64>,
    pub value: u64,
}

#[derive(Default)]
pub struct DiagnosticEventRing {
    events: Mutex<VecDeque<DiagnosticEvent>>,
}

impl DiagnosticEventRing {
    pub fn record(&self, event: DiagnosticEvent) {
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if events.len() == EVENT_RING_CAPACITY {
            events.pop_front();
        }
        events.push_back(event);
    }

    pub fn snapshot(&self) -> Vec<DiagnosticEvent> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .copied()
            .collect()
    }
}

pub fn diagnostic_events() -> &'static DiagnosticEventRing {
    static EVENTS: OnceLock<DiagnosticEventRing> = OnceLock::new();
    EVENTS.get_or_init(DiagnosticEventRing::default)
}

static OUTPUT_FRAMES: AtomicU64 = AtomicU64::new(0);

pub fn record_output_frame() {
    OUTPUT_FRAMES.fetch_add(1, Ordering::Relaxed);
}

pub fn output_frame_count() -> u64 {
    OUTPUT_FRAMES.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildDiagnostics {
    pub package_version: String,
    pub commit_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformDiagnostics {
    pub os: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticFormat {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub pixel_format: crate::frame::PixelFormat,
}

impl From<VideoFormat> for DiagnosticFormat {
    fn from(value: VideoFormat) -> Self {
        Self {
            width: value.width,
            height: value.height,
            fps: value.fps,
            pixel_format: value.pixel_format,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticsSnapshot {
    pub schema_version: u32,
    pub generated_wall_time_nanos: u128,
    pub build: BuildDiagnostics,
    pub platform: PlatformDiagnostics,
    pub formats: Vec<DiagnosticFormat>,
    pub output_frames: u64,
    pub drops: DropCountersSnapshot,
    pub latency: Vec<LatencyHistogramSnapshot>,
    pub copies: CopyLedgerPerOutputSnapshot,
    pub recent_events: Vec<DiagnosticEvent>,
    pub redacted_paths: Vec<String>,
}

impl DiagnosticsSnapshot {
    pub fn capture(formats: &[VideoFormat], paths: &[&Path]) -> Self {
        let output_frames = output_frame_count();
        Self {
            schema_version: 1,
            generated_wall_time_nanos: wall_time_nanos(),
            build: BuildDiagnostics {
                package_version: env!("CARGO_PKG_VERSION").to_owned(),
                commit_hash: env!("CAMERAMAN_BUILD_HASH").to_owned(),
            },
            platform: PlatformDiagnostics {
                os: std::env::consts::OS.to_owned(),
                architecture: std::env::consts::ARCH.to_owned(),
            },
            formats: formats.iter().copied().map(Into::into).collect(),
            output_frames,
            drops: drop_counters().snapshot(),
            latency: latency_histograms().snapshot(),
            copies: copy_ledger().snapshot().per_output_frame(output_frames),
            recent_events: diagnostic_events().snapshot(),
            redacted_paths: paths.iter().map(|path| redact_path(path)).collect(),
        }
    }

    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

pub fn redact_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("<redacted>/{name}"))
        .unwrap_or_else(|| String::from("<redacted>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_histogram_reports_requested_percentiles() {
        let histograms = LatencyHistograms::default();
        for value in 1..=100 {
            histograms.record(PipelineStage::Compose, value);
        }
        let compose = histograms
            .snapshot()
            .into_iter()
            .find(|snapshot| snapshot.stage == PipelineStage::Compose)
            .unwrap();
        assert_eq!(compose.samples, 100);
        assert_eq!(compose.p50_nanos, 50);
        assert_eq!(compose.p95_nanos, 95);
        assert_eq!(compose.p99_nanos, 99);
        assert_eq!(compose.max_nanos, 100);
    }

    #[test]
    fn event_ring_keeps_only_the_newest_bounded_events() {
        let ring = DiagnosticEventRing::default();
        for sequence in 0..EVENT_RING_CAPACITY as u64 + 10 {
            ring.record(DiagnosticEvent {
                monotonic_timestamp_nanos: sequence,
                kind: DiagnosticEventKind::Drop,
                drop_reason: Some(DropReason::Late),
                frame_sequence: Some(sequence),
                value: 1,
            });
        }
        let events = ring.snapshot();
        assert_eq!(events.len(), EVENT_RING_CAPACITY);
        assert_eq!(events.first().unwrap().frame_sequence, Some(10));
    }

    #[test]
    fn feedback_gate_limits_duplicate_events_without_losing_counters() {
        let gate = DiagnosticFeedbackGate::new(100);
        assert!(gate.should_emit(DropReason::Late.index(), 1_000));
        assert!(!gate.should_emit(DropReason::Late.index(), 1_050));
        assert!(gate.should_emit(DropReason::Late.index(), 1_100));
        assert!(gate.should_emit(DropReason::Stale.index(), 1_050));
    }

    #[test]
    fn diagnostics_json_is_versioned_and_does_not_expose_source_paths() {
        let secret = Path::new("/Users/alice/Private/CameraMan.app");
        let snapshot = DiagnosticsSnapshot::capture(&[VideoFormat::hd_1080p_bgra()], &[secret]);
        let json = snapshot.to_json_pretty().unwrap();
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("<redacted>/CameraMan.app"));
        assert!(!json.contains("/Users/alice"));
    }

    #[test]
    fn stage_names_are_stable() {
        assert_eq!(
            PipelineStage::ALL.map(PipelineStage::name),
            [
                "capture",
                "compose",
                "publish",
                "consume",
                "pixel_buffer",
                "send_sample"
            ]
        );
    }
}
