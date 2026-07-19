use super::*;

pub(super) fn print_status() {
    let config = VirtualCameraConfig::default();
    let transport = FrameTransportMode::from_environment();
    let transport_endpoint = FrameTransportSink::new(transport).description();
    println!("CameraMan (Rust)");
    println!("Virtual camera backend: Rust CoreMediaIO system-extension provider");
    println!(
        "Virtual camera: {} ({})",
        config.device_name, config.device_uid
    );
    println!(
        "Extension stream: {}x{} BGRA at {}-{} fps via {} ({transport_endpoint}) with placeholder fallback",
        config.format.width,
        config.format.height,
        camera_man::VIRTUAL_CAMERA_MIN_FPS,
        camera_man::VIRTUAL_CAMERA_MAX_FPS,
        transport.name(),
    );
    println!(
        "Install note: macOS still requires system-extension approval and proper release signing/notarization."
    );
    println!(
        "Output format: {}x{} {}fps {:?}",
        config.format.width, config.format.height, config.format.fps, config.format.pixel_format
    );
    println!(
        "Frame memory limit: {} MiB",
        default_frame_limits().max_frame_bytes() / (1024 * 1024)
    );
    println!("Run `cargo test` to verify the core.");
    println!("Run `cargo run` to launch the Rust desktop app.");
    println!("Run `cargo run -- demo` to render a synthetic composed frame.");
    println!("Run `cargo run -- pipeline-demo` to render through the pipeline.");
    println!("Run `cargo run -- list-cameras` to query real cameras.");
    println!("Run `cargo run --release -- bundle` to create an optimized CameraMan.app.");
}

pub(super) fn run_demo(output_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let format = VideoFormat {
        width: 960,
        height: 540,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    };
    let compositor = Compositor::new(format);
    let mut blue = SyntheticFrameSource::with_id("blue", 320, 240, [255, 40, 40, 255]);
    let mut green = SyntheticFrameSource::with_id("green", 320, 240, [40, 255, 40, 255]);
    let mut red = SyntheticFrameSource::with_id("red", 320, 240, [40, 40, 255, 255]);
    let frames = vec![
        blue.latest_frame()?,
        green.latest_frame()?,
        red.latest_frame()?,
    ];

    let output = compositor.compose_captured(&frames, CompositionLayout::Grid)?;
    let path = output_path.unwrap_or_else(|| PathBuf::from("target/camera-man-demo.ppm"));
    write_ppm(&path, &output)?;
    println!("Rendered demo frame: {}", path.display());
    Ok(())
}

pub(super) fn run_pipeline_demo(
    output_dir: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = output_dir.unwrap_or_else(|| PathBuf::from("target/camera-man-pipeline-demo"));
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

pub(super) fn list_cameras() -> Result<(), Box<dyn std::error::Error>> {
    let discovery = NokhwaCameraDiscovery;
    let devices = discovery.list_devices()?;
    if devices.is_empty() {
        println!("No cameras found.");
    } else {
        for device in devices {
            println!("{} {}", device.id, device.name);
        }
    }
    Ok(())
}

pub(super) fn capture_demo(output_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let frame = capture_one_with_timeout("0", Duration::from_secs(5))?.into_frame();
    let path = output_path.unwrap_or_else(|| PathBuf::from("target/camera-man-capture.ppm"));
    write_ppm(&path, &frame)?;
    println!("Captured real camera frame: {}", path.display());
    Ok(())
}
