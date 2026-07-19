use std::error::Error;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use camera_man::benchmarking::{
    BenchmarkEnvironment, DurationStatistics, REPRESENTATIVE_FIXTURE_PROFILE,
    capture_benchmark_environment, duration_statistics, representative_fixture_frames,
};
use camera_man::performance::{
    CopyLedgerPerOutputSnapshot, CopyLedgerSnapshot, ProcessUsageRates, ProcessUsageSnapshot,
    copy_ledger, process_usage_snapshot,
};
use camera_man::{
    CompositionLayout, Compositor, Frame, PixelFormat, ProducerProgress, ScalingFilter,
    SharedFrameReader, SharedFrameSink, VideoFormat, VirtualCameraSink, new_generation_id,
};
use serde::Serialize;

const DEFAULT_ITERATIONS: u32 = 100;
const DEFAULT_WARMUPS: u32 = 10;
const WORKER_TIMEOUT: Duration = Duration::from_secs(30);
const WIDTH: u32 = 1_920;
const HEIGHT: u32 = 1_080;
const FPS: u32 = 30;

#[derive(Debug)]
struct Arguments {
    iterations: u32,
    warmups: u32,
    output: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct StageMeasurements {
    compose_service: DurationStatistics,
    mmap_publish_service: DurationStatistics,
    cross_process_queue_wait: DurationStatistics,
    fixture_ready_to_extension_input_ack: DurationStatistics,
}

#[derive(Debug, Serialize)]
struct E2eReport {
    schema_version: u32,
    build_hash: String,
    target_arch: String,
    os_version: String,
    pipeline_boundary: &'static str,
    excluded_boundaries: [&'static str; 4],
    measurement_limitations: [&'static str; 2],
    fixture_profile: &'static str,
    output_format: VideoFormat,
    source_count: usize,
    warmup_iterations: u32,
    measured_iterations: u32,
    elapsed_seconds: f64,
    throughput_frames_per_second: f64,
    worker_pid: u32,
    stages: StageMeasurements,
    checksum: u8,
    copy_ledger_delta: CopyLedgerSnapshot,
    copy_ledger_per_output: CopyLedgerPerOutputSnapshot,
    parent_usage_start: ProcessUsageSnapshot,
    parent_usage_finish: ProcessUsageSnapshot,
    parent_usage_delta: ProcessUsageSnapshot,
    parent_usage_rates: ProcessUsageRates,
    environment_before: BenchmarkEnvironment,
    environment_after: BenchmarkEnvironment,
}

struct TransactionSample {
    compose: Duration,
    publish: Duration,
    queue_wait: Duration,
    end_to_end: Duration,
    checksum: u8,
}

struct TemporaryDirectory(PathBuf);

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct WorkerProcess(Option<Child>);

impl WorkerProcess {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn id(&self) -> u32 {
        self.0.as_ref().expect("worker is present").id()
    }

    fn child_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("worker is present")
    }

    fn wait(mut self) -> std::io::Result<std::process::ExitStatus> {
        self.0.take().expect("worker is present").wait()
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let raw_arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if raw_arguments.first().map(String::as_str) == Some("worker") {
        return worker(&raw_arguments);
    }
    run_benchmark(arguments(&raw_arguments)?)
}

fn run_benchmark(arguments: Arguments) -> Result<(), Box<dyn Error>> {
    let format = VideoFormat {
        width: WIDTH,
        height: HEIGHT,
        fps: FPS,
        pixel_format: PixelFormat::Bgra8,
    };
    let compositor = Compositor::new(format).with_scaling_filter(ScalingFilter::Bilinear);
    let frames = representative_fixture_frames(4)?;
    let mut output = Frame::solid_bgra(WIDTH, HEIGHT, [0, 0, 0, 255])?;
    let temp = TemporaryDirectory(std::env::temp_dir().join(format!(
        "cameraman-e2e-{}-{}",
        std::process::id(),
        new_generation_id()
    )));
    fs::create_dir_all(&temp.0)?;
    let transport_path = temp.0.join("frames.mmap");
    let ready_path = temp.0.join("worker.ready");
    let mut sink = SharedFrameSink::new_file(&transport_path, output.data().len());
    sink.set_target_fps(FPS);
    sink.connect()?;

    let expected_frames = arguments
        .iterations
        .checked_add(arguments.warmups)
        .ok_or("iteration count overflow")?;
    let executable = std::env::current_exe()?;
    let mut worker = WorkerProcess::new(
        Command::new(executable)
            .arg("worker")
            .arg(&transport_path)
            .arg(&ready_path)
            .arg(expected_frames.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let worker_pid = worker.id();
    wait_for_worker_ready(worker.child_mut(), &ready_path)?;

    let environment_before =
        capture_benchmark_environment("separate-process mmap end-to-end benchmark");
    for _ in 0..arguments.warmups {
        black_box(run_transaction(
            &compositor,
            &frames,
            &mut output,
            &mut sink,
        )?);
    }

    let ledger_before = copy_ledger().snapshot();
    let usage_start = process_usage_snapshot();
    let measured_started = Instant::now();
    let mut samples = Vec::with_capacity(arguments.iterations as usize);
    for _ in 0..arguments.iterations {
        samples.push(run_transaction(
            &compositor,
            &frames,
            &mut output,
            &mut sink,
        )?);
    }
    let measured_elapsed = measured_started.elapsed();
    let usage_finish = process_usage_snapshot();
    let ledger_delta = copy_ledger().snapshot().difference_since(&ledger_before);

    let status = worker.wait()?;
    if !status.success() {
        return Err(format!("consumer worker failed with {status}").into());
    }
    let environment_after =
        capture_benchmark_environment("separate-process mmap end-to-end benchmark");
    sink.disconnect();

    let compose = samples
        .iter()
        .map(|sample| sample.compose)
        .collect::<Vec<_>>();
    let publish = samples
        .iter()
        .map(|sample| sample.publish)
        .collect::<Vec<_>>();
    let queue_wait = samples
        .iter()
        .map(|sample| sample.queue_wait)
        .collect::<Vec<_>>();
    let end_to_end = samples
        .iter()
        .map(|sample| sample.end_to_end)
        .collect::<Vec<_>>();
    let usage_delta = usage_finish.difference_since(usage_start);
    let report = E2eReport {
        schema_version: 1,
        build_hash: env!("CAMERAMAN_BUILD_HASH").to_owned(),
        target_arch: std::env::consts::ARCH.to_owned(),
        os_version: command_output("sw_vers", &["-productVersion"]),
        pipeline_boundary: "representative fixture ready -> compose -> file-backed mmap publish -> separate-process borrowed extension input acknowledgement",
        excluded_boundaries: [
            "physical camera driver and device capture",
            "CoreVideo pixel-buffer pool upload",
            "CoreMediaIO sample-buffer delivery",
            "third-party application display or encode",
        ],
        measurement_limitations: [
            "producer polls the mmap acknowledgement, so queue wait includes polling overhead",
            "process usage contains the producer process only; sustained combined behavior belongs to the soak profile",
        ],
        fixture_profile: REPRESENTATIVE_FIXTURE_PROFILE,
        output_format: format,
        source_count: frames.len(),
        warmup_iterations: arguments.warmups,
        measured_iterations: arguments.iterations,
        elapsed_seconds: measured_elapsed.as_secs_f64(),
        throughput_frames_per_second: f64::from(arguments.iterations)
            / measured_elapsed.as_secs_f64(),
        worker_pid,
        stages: StageMeasurements {
            compose_service: required_statistics(&compose)?,
            mmap_publish_service: required_statistics(&publish)?,
            cross_process_queue_wait: required_statistics(&queue_wait)?,
            fixture_ready_to_extension_input_ack: required_statistics(&end_to_end)?,
        },
        checksum: samples.iter().fold(0_u8, |checksum, sample| {
            checksum.wrapping_add(sample.checksum)
        }),
        copy_ledger_per_output: ledger_delta.per_output_frame(u64::from(arguments.iterations)),
        copy_ledger_delta: ledger_delta,
        parent_usage_start: usage_start,
        parent_usage_finish: usage_finish,
        parent_usage_delta: usage_delta,
        parent_usage_rates: usage_delta.rates_over(measured_elapsed),
        environment_before,
        environment_after,
    };
    let json = format!("{}\n", serde_json::to_string_pretty(&report)?);
    if let Some(path) = arguments.output {
        fs::write(&path, json)?;
        println!("e2e_benchmark={}", path.display());
    } else {
        print!("{json}");
    }
    Ok(())
}

fn run_transaction(
    compositor: &Compositor,
    frames: &[Option<Frame>],
    output: &mut Frame,
    sink: &mut SharedFrameSink,
) -> Result<TransactionSample, Box<dyn Error>> {
    let end_to_end_started = Instant::now();
    let compose_started = Instant::now();
    compositor.compose_into(
        black_box(frames),
        CompositionLayout::Grid,
        black_box(output),
    )?;
    let compose = compose_started.elapsed();

    let publish_started = Instant::now();
    sink.send(black_box(output))?;
    let publish = publish_started.elapsed();
    let progress = sink
        .producer_progress()
        .ok_or("producer progress missing after publish")?;
    let wait_started = Instant::now();
    wait_for_acknowledgement(sink, progress)?;
    let queue_wait = wait_started.elapsed();
    Ok(TransactionSample {
        compose,
        publish,
        queue_wait,
        end_to_end: end_to_end_started.elapsed(),
        checksum: output.data()[output.data().len() / 2],
    })
}

fn wait_for_acknowledgement(
    sink: &SharedFrameSink,
    expected: ProducerProgress,
) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + WORKER_TIMEOUT;
    let mut attempts = 0_u32;
    while Instant::now() < deadline {
        if sink.consumer_progress().is_some_and(|progress| {
            progress.generation == expected.generation && progress.sequence == expected.sequence
        }) {
            return Ok(());
        }
        attempts = attempts.wrapping_add(1);
        if attempts.is_multiple_of(128) {
            std::thread::yield_now();
        } else {
            std::hint::spin_loop();
        }
    }
    Err(format!(
        "timed out waiting for generation {} sequence {}",
        expected.generation, expected.sequence
    )
    .into())
}

fn wait_for_worker_ready(child: &mut Child, ready_path: &Path) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + WORKER_TIMEOUT;
    while Instant::now() < deadline {
        if ready_path.is_file() {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(format!("consumer worker exited before readiness with {status}").into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Err("timed out waiting for consumer worker readiness".into())
}

fn worker(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let transport_path = arguments
        .get(1)
        .map(Path::new)
        .ok_or("worker requires transport path")?;
    let ready_path = arguments
        .get(2)
        .map(Path::new)
        .ok_or("worker requires ready path")?;
    let expected_frames = arguments
        .get(3)
        .ok_or("worker requires frame count")?
        .parse::<u32>()?;
    let deadline = Instant::now() + WORKER_TIMEOUT;
    let mut reader = loop {
        if let Some(reader) = SharedFrameReader::try_open_file(transport_path)? {
            break reader;
        }
        if Instant::now() >= deadline {
            return Err("worker timed out opening mmap transport".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    fs::write(ready_path, b"ready")?;

    let mut consumed = 0_u32;
    let deadline = Instant::now() + WORKER_TIMEOUT;
    while consumed < expected_frames {
        if let Some(frame) = reader.read_latest_borrowed()? {
            black_box(frame.frame.data()[frame.frame.data().len() / 2]);
            consumed += 1;
        } else if Instant::now() >= deadline {
            return Err(format!(
                "worker consumed {consumed} of {expected_frames} frames before timeout"
            )
            .into());
        } else {
            std::thread::yield_now();
        }
    }
    Ok(())
}

fn required_statistics(samples: &[Duration]) -> Result<DurationStatistics, Box<dyn Error>> {
    duration_statistics(samples).ok_or_else(|| "measurement has no samples".into())
}

fn arguments(arguments: &[String]) -> Result<Arguments, Box<dyn Error>> {
    let mut iterations = DEFAULT_ITERATIONS;
    let mut warmups = DEFAULT_WARMUPS;
    let mut output = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--iterations" => {
                iterations = parse_positive(arguments.get(index + 1), "--iterations")?;
                index += 2;
            }
            "--warmups" => {
                warmups = parse_positive(arguments.get(index + 1), "--warmups")?;
                index += 2;
            }
            "--output" => {
                output = Some(PathBuf::from(
                    arguments.get(index + 1).ok_or("--output requires a path")?,
                ));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: cameraman-e2e-benchmark [--iterations N] [--warmups N] [--output PATH]"
                );
                std::process::exit(0);
            }
            value => return Err(format!("unknown option: {value}").into()),
        }
    }
    Ok(Arguments {
        iterations,
        warmups,
        output,
    })
}

fn parse_positive(value: Option<&String>, option: &str) -> Result<u32, Box<dyn Error>> {
    value
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{option} requires a positive integer").into())
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| String::from("unknown"))
}
