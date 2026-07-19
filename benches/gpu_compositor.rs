use std::hint::black_box;
use std::time::{Duration, Instant};

use camera_man::performance::percentile_index;
use camera_man::{
    CompositionLayout, Compositor, Frame, GpuCompositorExperiment, PixelFormat, VideoFormat,
};

const WARMUP_ITERATIONS: u32 = 3;
const DEFAULT_ITERATIONS: u32 = 30;

fn main() {
    let Ok(gpu) = GpuCompositorExperiment::new() else {
        println!("wgpu compositor experiment unavailable; CPU fallback remains active");
        return;
    };
    let format = VideoFormat {
        width: 1_280,
        height: 720,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    };
    let input = Frame::solid_bgra(1_920, 1_080, [16, 64, 192, 255]).unwrap();
    let cpu = Compositor::new(format);
    let iterations = std::env::var("CAMERAMAN_GPU_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_ITERATIONS);

    let cpu_output = cpu
        .compose(&[Some(input.clone())], CompositionLayout::Grid)
        .unwrap();
    let gpu_output = gpu.compose_single(&input, format).unwrap();
    assert_eq!(
        gpu_output, cpu_output,
        "GPU output diverged from CPU golden"
    );

    let (cpu_average, cpu_p95) = measure(iterations, || {
        let output = cpu
            .compose(black_box(&[Some(input.clone())]), CompositionLayout::Grid)
            .unwrap();
        black_box(output.data()[output.data().len() / 2]);
    });
    let (gpu_average, gpu_p95) = measure(iterations, || {
        let output = gpu.compose_single(black_box(&input), format).unwrap();
        black_box(output.data()[output.data().len() / 2]);
    });
    let ratio = gpu_average.as_secs_f64() / cpu_average.as_secs_f64();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let decision = if cfg!(debug_assertions) {
        "debug timings are non-comparable; no adoption decision"
    } else if ratio <= 0.90 {
        "isolated release result clears 10%; production still requires saved end-to-end evidence"
    } else {
        "keep CPU path; GPU+readback does not clear the adoption threshold"
    };

    println!(
        "wgpu compositor experiment: profile={profile}, adapter={}, backend={:?}, iterations={iterations}, CPU avg={:.3}ms p95={:.3}ms, GPU+readback avg={:.3}ms p95={:.3}ms, ratio={ratio:.3}; {decision}",
        gpu.adapter().adapter_name,
        gpu.adapter().backend,
        millis(cpu_average),
        millis(cpu_p95),
        millis(gpu_average),
        millis(gpu_p95),
    );
}

fn measure(iterations: u32, mut operation: impl FnMut()) -> (Duration, Duration) {
    for _ in 0..WARMUP_ITERATIONS {
        operation();
    }
    let mut samples = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let started = Instant::now();
        operation();
        samples.push(started.elapsed());
    }
    let average = samples.iter().copied().sum::<Duration>() / iterations;
    samples.sort_unstable();
    let p95 = samples[percentile_index(samples.len(), 95).unwrap()];
    (average, p95)
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
