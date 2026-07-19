use crate::camera::FrameSource;
use crate::diagnostics::{DropReason, PipelineStage, record_drop, stage_span};
use crate::error::CameraManError;
use crate::frame::{CapturedFrame, Frame};
use crate::layout::CompositionLayout;
use crate::render::Compositor;
use crate::virtual_camera::VirtualCameraSink;
use std::time::{Duration, Instant};

/// Source, composition and sink counters/timings accumulated by a pipeline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PipelineMetrics {
    pub ticks_completed: u64,
    pub source_reads: u64,
    pub source_frames_received: u64,
    /// Missing source frames include both `Ok(None)` and source errors.
    pub dropped_source_frames: u64,
    pub source_errors: u64,
    /// Frames whose sequence equals the previous sequence from that source.
    pub stale_source_frames: u64,
    pub latency_samples: u64,
    pub capture_to_sink_duration: Duration,
    pub max_capture_to_sink_duration: Duration,
    pub source_read_duration: Duration,
    pub render_duration: Duration,
    pub sink_send_duration: Duration,
}

impl PipelineMetrics {
    fn accumulate(&mut self, other: Self) {
        self.ticks_completed = self.ticks_completed.saturating_add(other.ticks_completed);
        self.source_reads = self.source_reads.saturating_add(other.source_reads);
        self.source_frames_received = self
            .source_frames_received
            .saturating_add(other.source_frames_received);
        self.dropped_source_frames = self
            .dropped_source_frames
            .saturating_add(other.dropped_source_frames);
        self.source_errors = self.source_errors.saturating_add(other.source_errors);
        self.stale_source_frames = self
            .stale_source_frames
            .saturating_add(other.stale_source_frames);
        self.latency_samples = self.latency_samples.saturating_add(other.latency_samples);
        self.capture_to_sink_duration += other.capture_to_sink_duration;
        self.max_capture_to_sink_duration = self
            .max_capture_to_sink_duration
            .max(other.max_capture_to_sink_duration);
        self.source_read_duration += other.source_read_duration;
        self.render_duration += other.render_duration;
        self.sink_send_duration += other.sink_send_duration;
    }
}

/// Result of one successful source-compose-send tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderReport {
    pub tick_index: u64,
    pub source_count: usize,
    pub missing_source_count: usize,
    /// Errors from individual sources during this tick, by source index.
    /// A failing source degrades to an empty cell instead of aborting the tick.
    pub source_errors: Vec<(usize, CameraManError)>,
    pub output: Frame,
    pub metrics: PipelineMetrics,
}

/// Aggregate result from [`PipelineEngine::render_n`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunSummary {
    pub ticks: u64,
    pub frames_sent: u64,
    pub missing_sources_seen: u64,
    pub source_errors_seen: u64,
    pub metrics: PipelineMetrics,
}

/// Synchronous trait-driven source-compose-sink pipeline.
pub struct PipelineEngine<Sink: VirtualCameraSink> {
    compositor: Compositor,
    sources: Vec<Box<dyn FrameSource>>,
    sink: Sink,
    layout: CompositionLayout,
    connected: bool,
    tick_index: u64,
    metrics: PipelineMetrics,
    last_source_sequences: Vec<Option<u64>>,
}

impl<Sink> PipelineEngine<Sink>
where
    Sink: VirtualCameraSink,
{
    pub fn new(
        compositor: Compositor,
        sources: Vec<Box<dyn FrameSource>>,
        sink: Sink,
        layout: CompositionLayout,
    ) -> Self {
        let source_count = sources.len();
        Self {
            compositor,
            sources,
            sink,
            layout,
            connected: false,
            tick_index: 0,
            metrics: PipelineMetrics::default(),
            last_source_sequences: vec![None; source_count],
        }
    }

    pub fn start(&mut self) -> Result<(), CameraManError> {
        if self.connected {
            return Ok(());
        }
        self.sink.connect()?;
        self.connected = true;
        Ok(())
    }

    pub fn render_once(&mut self) -> Result<RenderReport, CameraManError> {
        if !self.connected {
            return Err(CameraManError::VirtualCameraUnavailable(
                "pipeline is not started",
            ));
        }

        // One dead camera must not kill the whole stream: a failing source
        // becomes an empty cell for this tick and is reported in source_errors.
        let mut frames = Vec::with_capacity(self.sources.len());
        let mut source_errors = Vec::new();
        let mut tick_metrics = PipelineMetrics::default();
        let mut oldest_monotonic_timestamp_nanos = None;
        for (index, source) in self.sources.iter_mut().enumerate() {
            let source_started = Instant::now();
            let result = source.latest_frame();
            tick_metrics.source_read_duration += source_started.elapsed();
            tick_metrics.source_reads = tick_metrics.source_reads.saturating_add(1);
            match result {
                Ok(Some(frame)) => {
                    tick_metrics.source_frames_received =
                        tick_metrics.source_frames_received.saturating_add(1);
                    let metadata = frame.metadata();
                    if self.last_source_sequences[index] == Some(metadata.sequence) {
                        tick_metrics.stale_source_frames =
                            tick_metrics.stale_source_frames.saturating_add(1);
                        record_drop(DropReason::Stale, 1, Some(metadata.sequence));
                    }
                    self.last_source_sequences[index] = Some(metadata.sequence);
                    oldest_monotonic_timestamp_nanos = Some(
                        oldest_monotonic_timestamp_nanos
                            .map_or(metadata.monotonic_timestamp().0, |oldest: u64| {
                                oldest.min(metadata.monotonic_timestamp().0)
                            }),
                    );
                    frames.push(Some(frame));
                }
                Ok(None) => {
                    tick_metrics.dropped_source_frames =
                        tick_metrics.dropped_source_frames.saturating_add(1);
                    record_drop(DropReason::SourceMissing, 1, Some(self.tick_index));
                    frames.push(None);
                }
                Err(error) => {
                    tick_metrics.dropped_source_frames =
                        tick_metrics.dropped_source_frames.saturating_add(1);
                    tick_metrics.source_errors = tick_metrics.source_errors.saturating_add(1);
                    record_drop(DropReason::SourceMissing, 1, Some(self.tick_index));
                    source_errors.push((index, error.context(format!("read source {index}"))));
                    frames.push(None);
                }
            }
        }

        let source_count = frames.len();
        let missing_source_count = frames.iter().filter(|frame| frame.is_none()).count();
        let render_started = Instant::now();
        let format = self.compositor.format();
        let output = {
            let _compose_span = stage_span(
                PipelineStage::Compose,
                self.tick_index,
                format.width,
                format.height,
            );
            self.compose_captured(&frames)
        };
        tick_metrics.render_duration += render_started.elapsed();
        let output = match output {
            Ok(output) => output,
            Err(error) => {
                self.metrics.accumulate(tick_metrics);
                return Err(error.context("compose pipeline frame"));
            }
        };
        let send_started = Instant::now();
        let send_result = self.sink.send(&output);
        tick_metrics.sink_send_duration += send_started.elapsed();
        if let Err(error) = send_result {
            self.metrics.accumulate(tick_metrics);
            return Err(error.context("send pipeline frame"));
        }
        if let Some(timestamp_nanos) = oldest_monotonic_timestamp_nanos {
            let latency = crate::frame::FrameMetadata::age_from_monotonic_nanos(timestamp_nanos);
            tick_metrics.latency_samples = 1;
            tick_metrics.capture_to_sink_duration = latency;
            tick_metrics.max_capture_to_sink_duration = latency;
        }
        tick_metrics.ticks_completed = 1;
        self.metrics.accumulate(tick_metrics);
        self.tick_index += 1;

        Ok(RenderReport {
            tick_index: self.tick_index,
            source_count,
            missing_source_count,
            source_errors,
            output,
            metrics: tick_metrics,
        })
    }

    pub fn render_n(&mut self, ticks: u64) -> Result<RunSummary, CameraManError> {
        let mut summary = RunSummary {
            ticks,
            frames_sent: 0,
            missing_sources_seen: 0,
            source_errors_seen: 0,
            metrics: PipelineMetrics::default(),
        };

        for _ in 0..ticks {
            let report = self.render_once()?;
            summary.frames_sent += 1;
            summary.missing_sources_seen += report.missing_source_count as u64;
            summary.source_errors_seen += report.source_errors.len() as u64;
            summary.metrics.accumulate(report.metrics);
        }

        Ok(summary)
    }

    pub fn stop(&mut self) {
        if self.connected {
            self.sink.disconnect();
        }
        self.connected = false;
    }

    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    pub const fn tick_index(&self) -> u64 {
        self.tick_index
    }

    pub const fn metrics(&self) -> PipelineMetrics {
        self.metrics
    }

    pub fn reset_metrics(&mut self) {
        self.metrics = PipelineMetrics::default();
    }

    pub fn sink(&self) -> &Sink {
        &self.sink
    }

    pub fn sink_mut(&mut self) -> &mut Sink {
        &mut self.sink
    }

    fn compose_captured(&self, frames: &[Option<CapturedFrame>]) -> Result<Frame, CameraManError> {
        self.compositor.compose_captured(frames, self.layout)
    }
}

impl<Sink: VirtualCameraSink> Drop for PipelineEngine<Sink> {
    fn drop(&mut self) {
        if self.connected {
            self.sink.disconnect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::SyntheticFrameSource;
    use crate::config::VideoFormat;
    use crate::error::ErrorCode;
    use crate::frame::PixelFormat;
    use crate::virtual_camera::MemorySink;

    #[test]
    fn renders_synthetic_sources_into_sink() {
        let compositor = Compositor::new(VideoFormat {
            width: 4,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let sources: Vec<Box<dyn FrameSource>> = vec![
            Box::new(SyntheticFrameSource::with_id(
                "left",
                1,
                1,
                [255, 0, 0, 255],
            )),
            Box::new(SyntheticFrameSource::with_id(
                "right",
                1,
                1,
                [0, 255, 0, 255],
            )),
        ];
        let mut engine = PipelineEngine::new(
            compositor,
            sources,
            MemorySink::default(),
            CompositionLayout::Row,
        );

        engine.start().unwrap();
        let report = engine.render_once().unwrap();

        assert_eq!(report.tick_index, 1);
        assert_eq!(report.source_count, 2);
        assert_eq!(report.missing_source_count, 0);
        assert_eq!(engine.sink().frames_sent(), 1);
        assert_eq!(report.output.bgra_at(0, 0), Some([255, 0, 0, 255]));
        assert_eq!(report.output.bgra_at(3, 0), Some([0, 255, 0, 255]));
        assert_eq!(report.metrics.ticks_completed, 1);
        assert_eq!(report.metrics.source_reads, 2);
        assert_eq!(report.metrics.source_frames_received, 2);
        assert_eq!(report.metrics.dropped_source_frames, 0);
        assert_eq!(engine.metrics(), report.metrics);
    }

    struct FailingSource;

    impl FrameSource for FailingSource {
        fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
            Err(CameraManError::capture("simulated failure"))
        }
    }

    struct EmptySource;

    impl FrameSource for EmptySource {
        fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
            Ok(None)
        }
    }

    struct RepeatingSource {
        frame: CapturedFrame,
    }

    impl FrameSource for RepeatingSource {
        fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
            Ok(Some(self.frame.clone()))
        }
    }

    #[test]
    fn failing_source_degrades_to_empty_cell() {
        let compositor = Compositor::new(VideoFormat {
            width: 4,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let sources: Vec<Box<dyn FrameSource>> = vec![
            Box::new(SyntheticFrameSource::new(1, 1, [255, 0, 0, 255])),
            Box::new(FailingSource),
        ];
        let mut engine = PipelineEngine::new(
            compositor,
            sources,
            MemorySink::default(),
            CompositionLayout::Row,
        );

        engine.start().unwrap();
        let report = engine.render_once().unwrap();

        assert_eq!(report.source_errors.len(), 1);
        assert_eq!(report.source_errors[0].0, 1);
        assert_eq!(
            report.source_errors[0].1.code(),
            ErrorCode::Capture(crate::error::CaptureErrorKind::Other)
        );
        assert!(
            report.source_errors[0]
                .1
                .diagnostic_message()
                .starts_with("read source 1:")
        );
        assert_eq!(report.missing_source_count, 1);
        assert_eq!(engine.sink().frames_sent(), 1);
        assert_eq!(report.metrics.source_frames_received, 1);
        assert_eq!(report.metrics.dropped_source_frames, 1);
        assert_eq!(report.metrics.source_errors, 1);
    }

    #[test]
    fn missing_source_frame_is_dropped_without_becoming_an_error() {
        let compositor = Compositor::new(VideoFormat {
            width: 2,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let sources: Vec<Box<dyn FrameSource>> = vec![Box::new(EmptySource)];
        let mut engine = PipelineEngine::new(
            compositor,
            sources,
            MemorySink::default(),
            CompositionLayout::Grid,
        );

        engine.start().unwrap();
        let report = engine.render_once().unwrap();

        assert_eq!(report.missing_source_count, 1);
        assert!(report.source_errors.is_empty());
        assert_eq!(report.metrics.dropped_source_frames, 1);
        assert_eq!(report.metrics.source_errors, 0);
    }

    #[test]
    fn repeated_sequence_is_counted_as_stale_and_latency_is_measured() {
        let compositor = Compositor::new(VideoFormat {
            width: 2,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let frame = Frame::solid_bgra(1, 1, [1, 2, 3, 255]).unwrap();
        let captured = CapturedFrame::new(frame, crate::frame::FrameMetadata::new("repeat", 7));
        let sources: Vec<Box<dyn FrameSource>> =
            vec![Box::new(RepeatingSource { frame: captured })];
        let mut engine = PipelineEngine::new(
            compositor,
            sources,
            MemorySink::default(),
            CompositionLayout::Grid,
        );

        engine.start().unwrap();
        let summary = engine.render_n(2).unwrap();

        assert_eq!(summary.metrics.stale_source_frames, 1);
        assert_eq!(summary.metrics.latency_samples, 2);
        assert!(
            summary.metrics.capture_to_sink_duration
                >= summary.metrics.max_capture_to_sink_duration
        );
    }

    #[test]
    fn render_before_start_is_an_error() {
        let compositor = Compositor::new(VideoFormat {
            width: 2,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let sources: Vec<Box<dyn FrameSource>> =
            vec![Box::new(SyntheticFrameSource::new(1, 1, [1, 2, 3, 255]))];
        let mut engine = PipelineEngine::new(
            compositor,
            sources,
            MemorySink::default(),
            CompositionLayout::Grid,
        );

        assert!(engine.render_once().is_err());
    }

    #[test]
    fn renders_multiple_ticks() {
        let compositor = Compositor::new(VideoFormat {
            width: 2,
            height: 2,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        });
        let sources: Vec<Box<dyn FrameSource>> =
            vec![Box::new(SyntheticFrameSource::new(1, 1, [10, 20, 30, 255]))];
        let mut engine = PipelineEngine::new(
            compositor,
            sources,
            MemorySink::default(),
            CompositionLayout::Grid,
        );

        engine.start().unwrap();
        let summary = engine.render_n(3).unwrap();

        assert_eq!(summary.frames_sent, 3);
        assert_eq!(summary.metrics.ticks_completed, 3);
        assert_eq!(summary.metrics.source_reads, 3);
        assert_eq!(summary.metrics.source_frames_received, 3);
        assert_eq!(engine.tick_index(), 3);
        assert_eq!(engine.sink().frames_sent(), 3);
        assert_eq!(engine.metrics(), summary.metrics);

        engine.reset_metrics();
        assert_eq!(engine.metrics(), PipelineMetrics::default());
    }
}
