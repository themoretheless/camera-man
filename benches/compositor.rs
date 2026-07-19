use std::env;
use std::hint::black_box;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use camera_man::benchmarking::{
    BaselineFile, BenchmarkReport, COMPOSITOR_BENCHMARK_SCHEMA_VERSION, CaseResult,
    DEFAULT_FIXTURE_PROFILE, OperationResult, REPRESENTATIVE_FIXTURE_PROFILE,
    capture_benchmark_environment, duration_statistics, representative_fixture_frames,
    seeded_case_order,
};
use camera_man::performance::{CopyStage, copy_ledger};
use camera_man::{
    CompositionLayout, Compositor, CropInsets, Frame, FrameView, PixelFormat, Rotation,
    ScalingFilter, SourceFit, SourceTransform, VideoFormat,
};

#[derive(Clone, Copy)]
struct Case {
    width: u32,
    height: u32,
    sources: usize,
    layout: CompositionLayout,
    filter: ScalingFilter,
    transform: TransformProfile,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum TransformProfile {
    #[default]
    Identity,
    Fill,
    CropRotateMirror,
    Opacity,
}

impl TransformProfile {
    const fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Fill => "fill",
            Self::CropRotateMirror => "crop_rotate_mirror",
            Self::Opacity => "opacity",
        }
    }

    fn transform(self) -> SourceTransform {
        match self {
            Self::Identity => SourceTransform::default(),
            Self::Fill => SourceTransform {
                fit: SourceFit::Fill,
                ..SourceTransform::default()
            },
            Self::CropRotateMirror => SourceTransform {
                crop: CropInsets {
                    left_per_mille: 100,
                    top_per_mille: 50,
                    right_per_mille: 100,
                    bottom_per_mille: 50,
                },
                rotation: Rotation::Degrees90,
                mirror_horizontal: true,
                ..SourceTransform::default()
            },
            Self::Opacity => SourceTransform {
                opacity_per_mille: 850,
                ..SourceTransform::default()
            },
        }
    }
}

#[derive(Clone, Copy)]
enum FixtureProfile {
    SyntheticSolid,
    RepresentativePatterned,
}

impl FixtureProfile {
    fn from_environment() -> Self {
        match env::var("CAMERAMAN_BENCH_FIXTURE").as_deref() {
            Ok(DEFAULT_FIXTURE_PROFILE) | Err(_) => Self::SyntheticSolid,
            Ok(REPRESENTATIVE_FIXTURE_PROFILE) => Self::RepresentativePatterned,
            Ok(value) => panic!(
                "CAMERAMAN_BENCH_FIXTURE must be `{DEFAULT_FIXTURE_PROFILE}` or `{REPRESENTATIVE_FIXTURE_PROFILE}`, got `{value}`"
            ),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::SyntheticSolid => DEFAULT_FIXTURE_PROFILE,
            Self::RepresentativePatterned => REPRESENTATIVE_FIXTURE_PROFILE,
        }
    }
}

impl Case {
    fn id(self) -> String {
        let base = format!(
            "{}p_{}_{}_{}",
            self.height,
            self.sources,
            self.layout.name(),
            filter_name(self.filter)
        );
        if self.transform == TransformProfile::Identity {
            base
        } else {
            format!("{base}_{}", self.transform.name())
        }
    }
}

fn main() {
    let iterations = env::var("CAMERAMAN_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|iterations| *iterations > 0)
        .unwrap_or(100);
    let warmup_iterations = env::var("CAMERAMAN_BENCH_WARMUP_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(5);
    let requested_case_rotation = env::var("CAMERAMAN_BENCH_CASE_ROTATION")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let case_order_mode =
        env::var("CAMERAMAN_BENCH_CASE_ORDER").unwrap_or_else(|_| String::from("rotation"));
    let case_order_seed = env::var("CAMERAMAN_BENCH_CASE_SEED").ok().map(|value| {
        value
            .parse::<u64>()
            .expect("CAMERAMAN_BENCH_CASE_SEED must be an unsigned integer")
    });
    let mode = env::var("CAMERAMAN_BENCH_MODE").unwrap_or_else(|_| String::from("matrix"));
    let fixture_profile = FixtureProfile::from_environment();
    let background_load_note = env::var("CAMERAMAN_BENCH_BACKGROUND_LOAD")
        .unwrap_or_else(|_| String::from("not recorded"));
    let environment_before = capture_benchmark_environment(&background_load_note);
    let hardware = hardware_label();
    let mut cases = cases_for_mode(&mode);
    let (case_rotation, recorded_seed) = match case_order_mode.as_str() {
        "rotation" => {
            assert!(
                case_order_seed.is_none(),
                "CAMERAMAN_BENCH_CASE_SEED requires CAMERAMAN_BENCH_CASE_ORDER=random"
            );
            let rotation = normalized_rotation(requested_case_rotation, cases.len());
            cases.rotate_left(rotation);
            (rotation, None)
        }
        "random" => {
            assert_eq!(
                requested_case_rotation, 0,
                "CAMERAMAN_BENCH_CASE_ROTATION cannot be combined with random order"
            );
            let seed = case_order_seed.unwrap_or(0x4341_4d45_5241_4d41);
            let order = seeded_case_order(cases.len(), seed);
            cases = order.into_iter().map(|index| cases[index]).collect();
            (0, Some(seed))
        }
        value => panic!("CAMERAMAN_BENCH_CASE_ORDER must be rotation or random, got `{value}`"),
    };
    let fixture_frames = fixture_frames(fixture_profile, 8);
    let results = cases
        .into_iter()
        .map(|case| {
            run_case(
                case,
                fixture_profile,
                &fixture_frames,
                warmup_iterations,
                iterations,
            )
        })
        .collect::<Vec<_>>();
    let preparation_results = match fixture_profile {
        FixtureProfile::SyntheticSolid => Vec::new(),
        FixtureProfile::RepresentativePatterned => {
            vec![run_padded_materialization(warmup_iterations, iterations)]
        }
    };
    let environment_after = capture_benchmark_environment(background_load_note);
    let report = BenchmarkReport {
        schema_version: COMPOSITOR_BENCHMARK_SCHEMA_VERSION,
        hardware,
        target_arch: std::env::consts::ARCH.to_owned(),
        os_version: command_output("sw_vers", &["-productVersion"]),
        compiler: command_output("rustc", &["-V"]),
        power_source: environment_before.power_source.clone(),
        fixture_profile: fixture_profile.name().to_owned(),
        build_hash: env!("CAMERAMAN_BUILD_HASH").to_owned(),
        iterations,
        warmup_iterations,
        case_rotation,
        case_order_mode,
        case_order_seed: recorded_seed,
        environment_before: Some(environment_before),
        environment_after: Some(environment_after),
        preparation_results,
        results,
    };

    for result in &report.results {
        println!(
            "{:<34} avg={:>8.3}ms p50={:>8.3}ms p95={:>8.3}ms p99={:>8.3}ms sd={:>7.3}ms {:>8.1} MPix/s checksum={}",
            result.case_id,
            result.average_ms,
            result.p50_ms,
            result.p95_ms,
            result.p99_ms,
            result.standard_deviation_ms,
            result.megapixels_per_second,
            result.checksum
        );
    }

    let json = serde_json::to_string_pretty(&report).expect("benchmark report must serialize");
    if let Some(path) = env::var_os("CAMERAMAN_BENCH_JSON") {
        std::fs::write(&path, &json).expect("benchmark JSON path must be writable");
        println!("benchmark_json={}", Path::new(&path).display());
    }

    if env::var("CAMERAMAN_BENCH_COMPARE").is_ok_and(|value| value != "0") {
        compare_with_baseline(&report);
    }
}

fn normalized_rotation(requested: usize, case_count: usize) -> usize {
    if case_count == 0 {
        0
    } else {
        requested % case_count
    }
}

fn cases_for_mode(mode: &str) -> Vec<Case> {
    if mode == "transforms" {
        return [ScalingFilter::Nearest, ScalingFilter::Bilinear]
            .into_iter()
            .flat_map(|filter| {
                [
                    TransformProfile::Identity,
                    TransformProfile::Fill,
                    TransformProfile::CropRotateMirror,
                    TransformProfile::Opacity,
                ]
                .into_iter()
                .map(move |transform| Case {
                    width: 1920,
                    height: 1080,
                    sources: 4,
                    layout: CompositionLayout::Grid,
                    filter,
                    transform,
                })
            })
            .collect();
    }
    let mut cases = Vec::new();
    for (width, height) in [(1280, 720), (1920, 1080)] {
        for sources in [1, 2, 4, 8] {
            for layout in [CompositionLayout::Grid, CompositionLayout::PictureInPicture] {
                for filter in [ScalingFilter::Nearest, ScalingFilter::Bilinear] {
                    let case = Case {
                        width,
                        height,
                        sources,
                        layout,
                        filter,
                        transform: TransformProfile::Identity,
                    };
                    let selected = match mode {
                        "matrix" | "all" => true,
                        "quick" => matches!(
                            (height, sources, layout, filter),
                            (720, 1, CompositionLayout::Grid, ScalingFilter::Nearest)
                                | (1080, 4, CompositionLayout::Grid, ScalingFilter::Nearest)
                                | (
                                    1080,
                                    4,
                                    CompositionLayout::PictureInPicture,
                                    ScalingFilter::Bilinear
                                )
                        ),
                        "nearest" => filter == ScalingFilter::Nearest,
                        "bilinear" => filter == ScalingFilter::Bilinear,
                        _ => panic!(
                            "CAMERAMAN_BENCH_MODE must be matrix, quick, nearest, bilinear, transforms or all"
                        ),
                    };
                    if selected {
                        cases.push(case);
                    }
                }
            }
        }
    }
    cases
}

fn run_case(
    case: Case,
    fixture_profile: FixtureProfile,
    fixture_frames: &[Option<Frame>],
    warmup_iterations: u32,
    iterations: u32,
) -> CaseResult {
    let format = VideoFormat {
        width: case.width,
        height: case.height,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    };
    let compositor = Compositor::new(format).with_scaling_filter(case.filter);
    let frames = fixture_frames[..case.sources].to_vec();
    let transforms = vec![case.transform.transform(); case.sources];
    let mut output = Frame::solid_bgra(case.width, case.height, [0, 0, 0, 255]).unwrap();
    compose_case(&compositor, &frames, &transforms, case, &mut output);
    for _ in 0..warmup_iterations {
        compose_case(
            &compositor,
            black_box(&frames),
            &transforms,
            case,
            black_box(&mut output),
        );
        black_box(&output);
    }

    let ledger_before = copy_ledger().snapshot();
    let mut samples = Vec::with_capacity(iterations as usize);
    let mut checksum = 0_u8;
    for _ in 0..iterations {
        let started = Instant::now();
        compose_case(
            &compositor,
            black_box(&frames),
            &transforms,
            case,
            black_box(&mut output),
        );
        samples.push(started.elapsed());
        checksum = checksum.wrapping_add(output.data()[output.data().len() / 2]);
        black_box(&output);
    }
    let statistics = duration_statistics(&samples).expect("benchmark has measured samples");
    let megapixels_per_second =
        f64::from(case.width) * f64::from(case.height) * f64::from(iterations)
            / statistics.total_seconds
            / 1_000_000.0;
    let copy_ledger_delta = copy_ledger().snapshot().difference_since(&ledger_before);

    CaseResult {
        case_id: case.id(),
        width: case.width,
        height: case.height,
        sources: case.sources,
        layout: case.layout.name().to_owned(),
        scaling_filter: filter_name(case.filter).to_owned(),
        average_ms: statistics.average_ms,
        standard_deviation_ms: statistics.standard_deviation_ms,
        p50_ms: statistics.p50_ms,
        p95_ms: statistics.p95_ms,
        p99_ms: statistics.p99_ms,
        min_ms: statistics.min_ms,
        max_ms: statistics.max_ms,
        megapixels_per_second,
        checksum: checksum.wrapping_add(fixture_profile.name().len() as u8),
        copy_ledger_delta: Some(copy_ledger_delta),
        samples_ms: samples.into_iter().map(millis).collect(),
    }
}

fn compose_case(
    compositor: &Compositor,
    frames: &[Option<Frame>],
    transforms: &[SourceTransform],
    case: Case,
    output: &mut Frame,
) {
    compositor
        .compose_into_with_transforms(frames, transforms, case.layout, output)
        .unwrap();
}

fn fixture_frames(profile: FixtureProfile, count: usize) -> Vec<Option<Frame>> {
    if matches!(profile, FixtureProfile::RepresentativePatterned) {
        return representative_fixture_frames(count).expect("representative fixture must fit");
    }
    (0..count)
        .map(|index| {
            let value = (index as u8).wrapping_mul(29).wrapping_add(17);
            Some(Frame::solid_bgra(640, 480, [value, 255 - value, value / 2, 255]).unwrap())
        })
        .collect()
}

fn run_padded_materialization(warmup_iterations: u32, iterations: u32) -> OperationResult {
    const WIDTH: u32 = 641;
    const HEIGHT: u32 = 479;
    const PADDING: usize = 64;
    let active_row_bytes = WIDTH as usize * 4;
    let row_stride = active_row_bytes + PADDING;
    let mut padded = vec![0_u8; row_stride * HEIGHT as usize];
    for y in 0..HEIGHT as usize {
        for x in 0..active_row_bytes {
            padded[y * row_stride + x] = (x as u8)
                .wrapping_mul(17)
                .wrapping_add((y as u8).wrapping_mul(3));
        }
        padded[y * row_stride + active_row_bytes..(y + 1) * row_stride].fill(0xa5);
    }
    let view = FrameView::new_checked(WIDTH, HEIGHT, PixelFormat::Bgra8, row_stride, &padded)
        .expect("padded fixture must be valid");
    for _ in 0..warmup_iterations {
        black_box(view.to_owned_tightly_packed().unwrap());
    }
    let ledger_before = copy_ledger().snapshot();
    let mut samples = Vec::with_capacity(iterations as usize);
    let mut checksum = 0_u8;
    let tight_bytes = active_row_bytes * HEIGHT as usize;
    for _ in 0..iterations {
        let started = Instant::now();
        let frame = view.to_owned_tightly_packed().unwrap();
        samples.push(started.elapsed());
        copy_ledger().record_allocation(CopyStage::CaptureDecode, tight_bytes);
        copy_ledger().record_copy(CopyStage::CaptureDecode, tight_bytes);
        checksum = checksum.wrapping_add(frame.data()[frame.data().len() / 2]);
        black_box(frame);
    }
    operation_result(
        "capture_padded_641x479_to_tight",
        u64::from(WIDTH) * u64::from(HEIGHT) * 4,
        checksum,
        samples,
        copy_ledger().snapshot().difference_since(&ledger_before),
    )
}

fn operation_result(
    operation_id: &str,
    bytes_per_operation: u64,
    checksum: u8,
    samples: Vec<Duration>,
    copy_ledger_delta: camera_man::performance::CopyLedgerSnapshot,
) -> OperationResult {
    let statistics = duration_statistics(&samples).expect("operation has measured samples");
    OperationResult {
        operation_id: operation_id.to_owned(),
        average_ms: statistics.average_ms,
        standard_deviation_ms: statistics.standard_deviation_ms,
        p50_ms: statistics.p50_ms,
        p95_ms: statistics.p95_ms,
        p99_ms: statistics.p99_ms,
        min_ms: statistics.min_ms,
        max_ms: statistics.max_ms,
        bytes_per_operation,
        checksum,
        copy_ledger_delta,
        samples_ms: samples.into_iter().map(millis).collect(),
    }
}

fn compare_with_baseline(report: &BenchmarkReport) {
    let path = env::var_os("CAMERAMAN_BENCH_BASELINE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("benchmarks/baselines.json"));
    let bytes = std::fs::read(&path).expect("performance baseline must be readable");
    let baseline: BaselineFile =
        serde_json::from_slice(&bytes).expect("performance baseline must be valid JSON");
    assert_eq!(baseline.schema_version, 1, "unsupported baseline schema");
    if let Some(required) = baseline.required_power_source.as_deref() {
        assert_eq!(
            report.power_source, required,
            "baseline comparison requires `{required}` but benchmark ran on `{}`",
            report.power_source
        );
    }
    if let Some(required) = baseline.required_fixture_profile.as_deref() {
        assert_eq!(
            report.fixture_profile, required,
            "baseline comparison requires fixture `{required}` but benchmark used `{}`",
            report.fixture_profile
        );
    }

    let mut compared = 0;
    let mut regressions = Vec::new();
    for result in &report.results {
        let Some(saved) = baseline
            .baselines
            .iter()
            .find(|saved| saved.hardware == report.hardware && saved.case_id == result.case_id)
        else {
            continue;
        };
        compared += 1;
        let allowed = saved.average_ms * (1.0 + baseline.max_regression_percent / 100.0);
        if result.average_ms > allowed {
            regressions.push(format!(
                "{}: {:.3}ms > {:.3}ms allowed (baseline {:.3}ms)",
                result.case_id, result.average_ms, allowed, saved.average_ms
            ));
        }
    }
    assert!(
        compared > 0,
        "no baseline entries matched hardware `{}`",
        report.hardware
    );
    assert!(
        regressions.is_empty(),
        "performance regression(s):\n{}",
        regressions.join("\n")
    );
    println!(
        "baseline=pass compared={compared} max_regression_percent={}",
        baseline.max_regression_percent
    );
}

fn filter_name(filter: ScalingFilter) -> &'static str {
    match filter {
        ScalingFilter::Nearest => "nearest",
        ScalingFilter::Bilinear => "bilinear",
    }
}

fn hardware_label() -> String {
    if let Ok(value) = env::var("CAMERAMAN_BENCH_HARDWARE")
        && !value.trim().is_empty()
    {
        return value;
    }
    let model = command_output("sysctl", &["-n", "machdep.cpu.brand_string"]);
    format!("{} | {model}", std::env::consts::ARCH)
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("unknown"))
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
