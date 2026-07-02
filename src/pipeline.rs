use crate::camera::FrameSource;
use crate::error::CameraManError;
use crate::frame::{CapturedFrame, Frame};
use crate::layout::CompositionLayout;
use crate::render::Compositor;
use crate::virtual_camera::VirtualCameraSink;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderReport {
    pub tick_index: u64,
    pub source_count: usize,
    pub missing_source_count: usize,
    /// Errors from individual sources during this tick, by source index.
    /// A failing source degrades to an empty cell instead of aborting the tick.
    pub source_errors: Vec<(usize, CameraManError)>,
    pub output: Frame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunSummary {
    pub ticks: u64,
    pub frames_sent: u64,
    pub missing_sources_seen: u64,
    pub source_errors_seen: u64,
}

pub struct PipelineEngine<Sink: VirtualCameraSink> {
    compositor: Compositor,
    sources: Vec<Box<dyn FrameSource>>,
    sink: Sink,
    layout: CompositionLayout,
    connected: bool,
    tick_index: u64,
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
        Self {
            compositor,
            sources,
            sink,
            layout,
            connected: false,
            tick_index: 0,
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
        for (index, source) in self.sources.iter_mut().enumerate() {
            match source.latest_frame() {
                Ok(frame) => frames.push(frame),
                Err(error) => {
                    source_errors.push((index, error));
                    frames.push(None);
                }
            }
        }

        let source_count = frames.len();
        let missing_source_count = frames.iter().filter(|frame| frame.is_none()).count();
        let output = self.compose_captured(&frames)?;
        self.sink.send(&output)?;
        self.tick_index += 1;

        Ok(RenderReport {
            tick_index: self.tick_index,
            source_count,
            missing_source_count,
            source_errors,
            output,
        })
    }

    pub fn render_n(&mut self, ticks: u64) -> Result<RunSummary, CameraManError> {
        let mut summary = RunSummary {
            ticks,
            frames_sent: 0,
            missing_sources_seen: 0,
            source_errors_seen: 0,
        };

        for _ in 0..ticks {
            let report = self.render_once()?;
            summary.frames_sent += 1;
            summary.missing_sources_seen += report.missing_source_count as u64;
            summary.source_errors_seen += report.source_errors.len() as u64;
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
    }

    struct FailingSource;

    impl FrameSource for FailingSource {
        fn latest_frame(&mut self) -> Result<Option<CapturedFrame>, CameraManError> {
            Err(CameraManError::Capture(String::from("simulated failure")))
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
        assert_eq!(report.missing_source_count, 1);
        assert_eq!(engine.sink().frames_sent(), 1);
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
        assert_eq!(engine.tick_index(), 3);
        assert_eq!(engine.sink().frames_sent(), 3);
    }
}
