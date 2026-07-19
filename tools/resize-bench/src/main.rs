use std::hint::black_box;
use std::process::Command;
use std::time::{Duration, Instant};

use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{PixelType, ResizeAlg, ResizeOptions, Resizer};

const SOURCE_WIDTH: u32 = 1_920;
const SOURCE_HEIGHT: u32 = 1_080;
const DEST_WIDTH: u32 = 960;
const DEST_HEIGHT: u32 = 540;

fn main() {
    let iterations = std::env::var("CAMERAMAN_RESIZE_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(50);
    let input = patterned_bgra();
    let x_offsets = (0..DEST_WIDTH)
        .map(|x| sample_coordinate(x, SOURCE_WIDTH, DEST_WIDTH) as usize * 4)
        .collect::<Vec<_>>();
    let y_rows = (0..DEST_HEIGHT)
        .map(|y| {
            sample_coordinate(y, SOURCE_HEIGHT, DEST_HEIGHT) as usize * SOURCE_WIDTH as usize * 4
        })
        .collect::<Vec<_>>();

    let mut native_output = vec![0_u8; DEST_WIDTH as usize * DEST_HEIGHT as usize * 4];
    resize_cached_nearest(&input, &mut native_output, &x_offsets, &y_rows);
    let native = measure(iterations, || {
        resize_cached_nearest(
            black_box(&input),
            black_box(&mut native_output),
            black_box(&x_offsets),
            black_box(&y_rows),
        );
    });

    let source = ImageRef::new(SOURCE_WIDTH, SOURCE_HEIGHT, &input, PixelType::U8x4).unwrap();
    let mut fast_output = Image::new(DEST_WIDTH, DEST_HEIGHT, PixelType::U8x4);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Nearest);
    let mut resizer = Resizer::new();
    resizer.resize(&source, &mut fast_output, &options).unwrap();
    let fast = measure(iterations, || {
        resizer
            .resize(
                black_box(&source),
                black_box(&mut fast_output),
                black_box(&options),
            )
            .unwrap();
    });

    let mismatched_pixels = native_output
        .chunks_exact(4)
        .zip(fast_output.buffer().chunks_exact(4))
        .filter(|(native, fast)| native != fast)
        .count();
    let ratio = fast.as_secs_f64() / native.as_secs_f64();
    let decision = if mismatched_pixels > 0 {
        "reject fast_image_resize: nearest pixel contract differs"
    } else if ratio <= 0.90 {
        "fast_image_resize clears the 10% adoption threshold"
    } else {
        "keep the native scaler on this target"
    };
    println!(
        "hardware={} target={} native={:.3}ms fast_image_resize={:.3}ms ratio={ratio:.3} mismatched_pixels={mismatched_pixels} decision={decision}",
        cpu_model(),
        std::env::consts::ARCH,
        millis(native),
        millis(fast)
    );
}

fn resize_cached_nearest(input: &[u8], output: &mut [u8], x_offsets: &[usize], y_rows: &[usize]) {
    let output_stride = DEST_WIDTH as usize * 4;
    for (destination, source_row) in output.chunks_exact_mut(output_stride).zip(y_rows) {
        for (x, source_x) in x_offsets.iter().enumerate() {
            let source_offset = source_row + source_x;
            destination[x * 4..x * 4 + 4].copy_from_slice(&input[source_offset..source_offset + 4]);
        }
    }
}

fn sample_coordinate(dest: u32, source_size: u32, dest_size: u32) -> u32 {
    let coordinate = u64::from(dest) * u64::from(source_size) / u64::from(dest_size.max(1));
    (coordinate as u32).min(source_size.saturating_sub(1))
}

fn patterned_bgra() -> Vec<u8> {
    let mut pixels = Vec::with_capacity(SOURCE_WIDTH as usize * SOURCE_HEIGHT as usize * 4);
    for y in 0..SOURCE_HEIGHT {
        for x in 0..SOURCE_WIDTH {
            pixels.extend_from_slice(&[
                x.wrapping_mul(17).wrapping_add(y * 3) as u8,
                y.wrapping_mul(11).wrapping_add(x * 5) as u8,
                x.wrapping_add(y.wrapping_mul(7)) as u8,
                255,
            ]);
        }
    }
    pixels
}

fn measure(iterations: u32, mut operation: impl FnMut()) -> Duration {
    operation();
    let started = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    started.elapsed() / iterations
}

fn cpu_model() -> String {
    Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("unknown-cpu"))
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
