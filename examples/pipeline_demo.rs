use std::path::PathBuf;

use camera_man::{
    CompositionLayout, Compositor, FrameSource, PipelineEngine, PixelFormat, PpmSequenceSink,
    SyntheticFrameSource, VideoFormat,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/camera-man-pipeline-demo"));
    let compositor = Compositor::new(VideoFormat {
        width: 960,
        height: 540,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    });
    let sources: Vec<Box<dyn FrameSource>> = vec![
        Box::new(SyntheticFrameSource::with_id(
            "blue",
            320,
            240,
            [255, 40, 40, 255],
        )),
        Box::new(SyntheticFrameSource::with_id(
            "green",
            320,
            240,
            [40, 255, 40, 255],
        )),
        Box::new(SyntheticFrameSource::with_id(
            "red",
            320,
            240,
            [40, 40, 255, 255],
        )),
    ];
    let sink = PpmSequenceSink::new(&output_dir, "frame");
    let mut pipeline = PipelineEngine::new(compositor, sources, sink, CompositionLayout::Grid);

    pipeline.start()?;
    let summary = pipeline.render_n(3)?;
    pipeline.stop();

    println!(
        "Rendered {} pipeline frames into {}",
        summary.frames_sent,
        output_dir.display()
    );
    println!(
        "Pipeline totals: source {:?}, render {:?}, sink {:?}, dropped source frames {}",
        summary.metrics.source_read_duration,
        summary.metrics.render_duration,
        summary.metrics.sink_send_duration,
        summary.metrics.dropped_source_frames,
    );
    Ok(())
}
