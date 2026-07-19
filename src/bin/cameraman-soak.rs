use std::collections::VecDeque;
use std::process::Command;
use std::time::{Duration, Instant};

use camera_man::benchmarking::{BenchmarkEnvironment, capture_benchmark_environment};
use camera_man::performance::{
    ProcessUsageRates, ProcessUsageSnapshot, percentile_index, process_usage_snapshot,
};
use camera_man::{
    CompositionLayout, Compositor, DiagnosticsSnapshot, DropReason, Frame, PipelineStage,
    PixelFormat, ScalingFilter, SharedFrameReader, SharedFrameSink, VideoFormat, VirtualCameraSink,
    new_generation_id, record_drop, stage_span,
};
use serde::Serialize;

const DEFAULT_SOAK_SECONDS: u64 = 8 * 60 * 60;
const DEFAULT_SAMPLE_SECONDS: u64 = 60;
const OUTPUT_FPS: u32 = 30;
const SOURCE_COUNT: usize = 4;
const TRANSPORT_SLOT_COUNT: usize = 3;
const MAX_PENDING_RENDER_JOBS: usize = 1;
const LATENCY_WINDOW_CAPACITY: usize = 2_048;
const WARMUP_FRAMES: u64 = 120;

#[derive(Debug, Serialize)]
struct HardwareProfile {
    os: String,
    architecture: String,
    cpu_brand: String,
    logical_cpus: usize,
    memory_bytes: u64,
}

#[derive(Debug, Serialize)]
struct QueueContract {
    transport_slots: usize,
    max_pending_render_jobs: usize,
}

#[derive(Debug, Default, Serialize)]
struct FrameValidation {
    reads: u64,
    read_misses: u64,
    sequence_errors: u64,
    generation_errors: u64,
    torn_frames: u64,
}

#[derive(Debug, Default, Serialize)]
struct LatencySummary {
    samples: u64,
    p50_nanos: u64,
    p95_nanos: u64,
    p99_nanos: u64,
    max_nanos: u64,
}

#[derive(Debug, Serialize)]
struct SoakSample {
    elapsed_seconds: u64,
    output_frames: u64,
    usage: ProcessUsageSnapshot,
}

#[derive(Debug, Serialize)]
struct SoakReport {
    schema_version: u32,
    workload: &'static str,
    hardware: HardwareProfile,
    environment_before: BenchmarkEnvironment,
    environment_after: BenchmarkEnvironment,
    format: VideoFormat,
    source_count: usize,
    queue_contract: QueueContract,
    warmup_frames: u64,
    requested_duration_seconds: u64,
    elapsed_seconds: f64,
    output_frames: u64,
    late_frames: u64,
    late_frame_ratio: f64,
    validation: FrameValidation,
    frame_latency: LatencySummary,
    max_observed_resident_bytes: u64,
    max_allocator_size_in_use_bytes: u64,
    max_allocator_size_allocated_bytes: u64,
    max_allocator_fragmentation_bytes: u64,
    max_resident_growth_bytes: u64,
    final_resident_growth_bytes: i64,
    final_physical_footprint_growth_bytes: i64,
    final_allocator_size_in_use_growth_bytes: i64,
    final_allocator_size_allocated_growth_bytes: i64,
    final_allocator_fragmentation_growth_bytes: i64,
    start_usage: ProcessUsageSnapshot,
    final_usage: ProcessUsageSnapshot,
    usage_delta: ProcessUsageSnapshot,
    usage_rates: ProcessUsageRates,
    samples: Vec<SoakSample>,
    diagnostics: DiagnosticsSnapshot,
}

fn main() {
    let (duration, sample_period) = arguments();
    let format = VideoFormat {
        width: 1_920,
        height: 1_080,
        fps: OUTPUT_FPS,
        pixel_format: PixelFormat::Bgra8,
    };
    let compositor = Compositor::new(format).with_scaling_filter(ScalingFilter::Nearest);
    let frames = [
        Some(Frame::solid_bgra(640, 480, [25, 75, 200, 255]).unwrap()),
        Some(Frame::solid_bgra(640, 480, [80, 190, 45, 255]).unwrap()),
        Some(Frame::solid_bgra(640, 480, [210, 55, 90, 255]).unwrap()),
        Some(Frame::solid_bgra(640, 480, [150, 135, 35, 255]).unwrap()),
    ];
    let mut output = Frame::solid_bgra(format.width, format.height, [0, 0, 0, 255]).unwrap();
    let shared_memory_name = format!(
        "/cms-{}-{:04x}",
        std::process::id(),
        new_generation_id() & 0xffff
    );
    let mut sink = SharedFrameSink::new(&shared_memory_name, output.data().len());
    sink.set_target_fps(OUTPUT_FPS);
    sink.connect().expect("soak mmap writer must connect");
    let mut reader = SharedFrameReader::try_open(&shared_memory_name)
        .expect("soak mmap reader must open")
        .expect("soak mmap writer must be visible");
    warm_up(&compositor, &frames, &mut output, &mut sink, &mut reader);
    let environment_before =
        capture_benchmark_environment("sustained 1080p30 compositor and mmap soak");
    let frame_period = Duration::from_nanos(1_000_000_000 / u64::from(OUTPUT_FPS));
    let started = Instant::now();
    let finish = started + duration;
    let mut next_deadline = started;
    let mut next_sample = started;
    let start_usage = process_usage_snapshot();
    let mut max_observed_resident_bytes = start_usage.resident_size_bytes;
    let mut max_allocator_size_in_use_bytes = start_usage.allocator_size_in_use_bytes;
    let mut max_allocator_size_allocated_bytes = start_usage.allocator_size_allocated_bytes;
    let mut max_allocator_fragmentation_bytes = start_usage.allocator_fragmentation_bytes;
    let mut samples = Vec::new();
    let mut output_frames = 0_u64;
    let mut late_frames = 0_u64;
    let mut validation = FrameValidation::default();
    let mut last_sequence = None;
    let mut generation = None;
    let mut frame_latencies = VecDeque::with_capacity(LATENCY_WINDOW_CAPACITY);

    while Instant::now() < finish {
        let frame_started = Instant::now();
        {
            let _compose_span = stage_span(
                PipelineStage::Compose,
                output_frames,
                format.width,
                format.height,
            );
            compositor
                .compose_into(&frames, CompositionLayout::Grid, &mut output)
                .expect("soak compositor must remain valid");
        }
        sink.send(&output).expect("soak mmap publish must succeed");
        match reader
            .read_latest_borrowed()
            .expect("soak mmap consume must remain valid")
        {
            Some(borrowed) => {
                validation.reads = validation.reads.saturating_add(1);
                if last_sequence.is_some_and(|previous| borrowed.sequence != previous + 1) {
                    validation.sequence_errors = validation.sequence_errors.saturating_add(1);
                }
                if generation.is_some_and(|expected| borrowed.generation != expected) {
                    validation.generation_errors = validation.generation_errors.saturating_add(1);
                }
                last_sequence = Some(borrowed.sequence);
                generation.get_or_insert(borrowed.generation);
                let exact_match = borrowed.frame.width() == output.width()
                    && borrowed.frame.height() == output.height()
                    && borrowed.fps == OUTPUT_FPS
                    && borrowed.frame.data() == output.data();
                if !exact_match {
                    validation.torn_frames = validation.torn_frames.saturating_add(1);
                }
                std::hint::black_box(borrowed.frame.data()[borrowed.frame.data().len() / 2]);
            }
            None => validation.read_misses = validation.read_misses.saturating_add(1),
        }
        output_frames = output_frames.wrapping_add(1);
        if frame_latencies.len() == LATENCY_WINDOW_CAPACITY {
            frame_latencies.pop_front();
        }
        frame_latencies.push_back(frame_started.elapsed().as_nanos() as u64);

        let now = Instant::now();
        if now >= next_sample {
            let usage = process_usage_snapshot();
            max_observed_resident_bytes =
                max_observed_resident_bytes.max(usage.resident_size_bytes);
            max_allocator_size_in_use_bytes =
                max_allocator_size_in_use_bytes.max(usage.allocator_size_in_use_bytes);
            max_allocator_size_allocated_bytes =
                max_allocator_size_allocated_bytes.max(usage.allocator_size_allocated_bytes);
            max_allocator_fragmentation_bytes =
                max_allocator_fragmentation_bytes.max(usage.allocator_fragmentation_bytes);
            samples.push(SoakSample {
                elapsed_seconds: now.duration_since(started).as_secs(),
                output_frames,
                usage,
            });
            next_sample = now + sample_period;
        }

        next_deadline += frame_period;
        let now = Instant::now();
        if now < next_deadline {
            std::thread::sleep(next_deadline - now);
        } else {
            let missed = now.duration_since(next_deadline).as_nanos() / frame_period.as_nanos() + 1;
            let missed = missed.min(u128::from(u64::MAX)) as u64;
            late_frames = late_frames.saturating_add(missed);
            record_drop(DropReason::Late, missed, Some(output_frames));
            next_deadline += frame_period.saturating_mul(missed.min(u64::from(u32::MAX)) as u32);
        }
    }

    let measured_elapsed = started.elapsed();
    let final_usage = process_usage_snapshot();
    let usage_delta = final_usage.difference_since(start_usage);
    let usage_rates = usage_delta.rates_over(measured_elapsed);
    max_observed_resident_bytes = max_observed_resident_bytes.max(final_usage.resident_size_bytes);
    max_allocator_size_in_use_bytes =
        max_allocator_size_in_use_bytes.max(final_usage.allocator_size_in_use_bytes);
    max_allocator_size_allocated_bytes =
        max_allocator_size_allocated_bytes.max(final_usage.allocator_size_allocated_bytes);
    max_allocator_fragmentation_bytes =
        max_allocator_fragmentation_bytes.max(final_usage.allocator_fragmentation_bytes);
    let diagnostics = DiagnosticsSnapshot::capture(&[format], &[]);
    sink.disconnect();
    let environment_after =
        capture_benchmark_environment("sustained 1080p30 compositor and mmap soak");
    let late_frame_ratio = late_frames as f64 / output_frames.max(1) as f64;
    let sampled_resident_growth =
        max_observed_resident_bytes.saturating_sub(start_usage.resident_size_bytes);
    let high_water_growth = final_usage
        .rss_high_water_bytes
        .saturating_sub(start_usage.rss_high_water_bytes);
    let report = SoakReport {
        schema_version: 4,
        workload: "1080p30-4-source-compose-mmap-borrow",
        hardware: hardware_profile(),
        environment_before,
        environment_after,
        format,
        source_count: SOURCE_COUNT,
        queue_contract: QueueContract {
            transport_slots: TRANSPORT_SLOT_COUNT,
            max_pending_render_jobs: MAX_PENDING_RENDER_JOBS,
        },
        warmup_frames: WARMUP_FRAMES,
        requested_duration_seconds: duration.as_secs(),
        elapsed_seconds: measured_elapsed.as_secs_f64(),
        output_frames,
        late_frames,
        late_frame_ratio,
        validation,
        frame_latency: latency_summary(&frame_latencies),
        max_observed_resident_bytes,
        max_allocator_size_in_use_bytes,
        max_allocator_size_allocated_bytes,
        max_allocator_fragmentation_bytes,
        max_resident_growth_bytes: sampled_resident_growth.max(high_water_growth),
        final_resident_growth_bytes: signed_growth(
            final_usage.resident_size_bytes,
            start_usage.resident_size_bytes,
        ),
        final_physical_footprint_growth_bytes: signed_growth(
            final_usage.physical_footprint_bytes,
            start_usage.physical_footprint_bytes,
        ),
        final_allocator_size_in_use_growth_bytes: signed_growth(
            final_usage.allocator_size_in_use_bytes,
            start_usage.allocator_size_in_use_bytes,
        ),
        final_allocator_size_allocated_growth_bytes: signed_growth(
            final_usage.allocator_size_allocated_bytes,
            start_usage.allocator_size_allocated_bytes,
        ),
        final_allocator_fragmentation_growth_bytes: signed_growth(
            final_usage.allocator_fragmentation_bytes,
            start_usage.allocator_fragmentation_bytes,
        ),
        start_usage,
        final_usage,
        usage_delta,
        usage_rates,
        samples,
        diagnostics,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("soak report must serialize")
    );
}

fn warm_up(
    compositor: &Compositor,
    frames: &[Option<Frame>],
    output: &mut Frame,
    sink: &mut SharedFrameSink,
    reader: &mut SharedFrameReader,
) {
    for sequence in 0..WARMUP_FRAMES {
        {
            let _compose_span = stage_span(
                PipelineStage::Compose,
                sequence,
                output.width(),
                output.height(),
            );
            compositor
                .compose_into(frames, CompositionLayout::Grid, output)
                .expect("soak warmup composition must remain valid");
        }
        sink.send(output).expect("soak warmup publish must succeed");
        let borrowed = reader
            .read_latest_borrowed()
            .expect("soak warmup consume must remain valid")
            .expect("soak warmup frame must be visible");
        assert_eq!(borrowed.frame.data(), output.data());
        std::hint::black_box(borrowed.frame.data()[borrowed.frame.data().len() / 2]);
    }
}

fn latency_summary(samples: &VecDeque<u64>) -> LatencySummary {
    let mut sorted = samples.iter().copied().collect::<Vec<_>>();
    sorted.sort_unstable();
    LatencySummary {
        samples: sorted.len() as u64,
        p50_nanos: percentile(&sorted, 50),
        p95_nanos: percentile(&sorted, 95),
        p99_nanos: percentile(&sorted, 99),
        max_nanos: sorted.last().copied().unwrap_or(0),
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    percentile_index(sorted.len(), percentile).map_or(0, |index| sorted[index])
}

fn signed_growth(current: u64, start: u64) -> i64 {
    let difference = i128::from(current) - i128::from(start);
    difference.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn hardware_profile() -> HardwareProfile {
    HardwareProfile {
        os: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        cpu_brand: sysctl_value("machdep.cpu.brand_string")
            .unwrap_or_else(|| String::from("unknown")),
        logical_cpus: std::thread::available_parallelism().map_or(1, usize::from),
        memory_bytes: sysctl_value("hw.memsize")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
    }
}

#[cfg(target_os = "macos")]
fn sysctl_value(name: &str) -> Option<String> {
    let output = Command::new("/usr/sbin/sysctl")
        .args(["-n", name])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(not(target_os = "macos"))]
fn sysctl_value(_name: &str) -> Option<String> {
    None
}

fn arguments() -> (Duration, Duration) {
    let mut duration_seconds = std::env::var("CAMERAMAN_SOAK_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_SOAK_SECONDS);
    let mut sample_seconds = std::env::var("CAMERAMAN_SOAK_SAMPLE_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_SAMPLE_SECONDS);
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--duration-seconds" => {
                duration_seconds = parse_positive(arguments.next(), "--duration-seconds")
            }
            "--sample-seconds" => {
                sample_seconds = parse_positive(arguments.next(), "--sample-seconds")
            }
            "--help" | "-h" => {
                println!(
                    "Usage: cameraman-soak [--duration-seconds N] [--sample-seconds N]\n\
                     Defaults to an 8-hour run; the report is emitted as JSON on stdout."
                );
                std::process::exit(0);
            }
            unknown => panic!("unknown soak argument: {unknown}"),
        }
    }
    (
        Duration::from_secs(duration_seconds),
        Duration::from_secs(sample_seconds),
    )
}

fn parse_positive(value: Option<String>, option: &str) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| panic!("{option} requires a positive integer"))
}
