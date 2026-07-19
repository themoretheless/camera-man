use std::hint::black_box;
use std::time::{Duration, Instant};

use camera_man::{CompositionLayout, Compositor, Frame, PixelFormat, VideoFormat};
use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{PixelType, ResizeAlg, ResizeOptions, Resizer};

const ITERATIONS: u32 = 30;

fn main() {
    let input = Frame::new_checked(1920, 1080, PixelFormat::Bgra8, patterned_bgra()).unwrap();
    let frames = [Some(input.clone())];
    let compositor = Compositor::new(VideoFormat {
        width: 960,
        height: 540,
        fps: 30,
        pixel_format: PixelFormat::Bgra8,
    });
    let mut native_output = Frame::solid_bgra(960, 540, [0, 0, 0, 255]).unwrap();
    compositor
        .compose_into(&frames, CompositionLayout::Grid, &mut native_output)
        .unwrap();

    let native = measure(|| {
        compositor
            .compose_into(
                black_box(&frames),
                CompositionLayout::Grid,
                black_box(&mut native_output),
            )
            .unwrap();
        black_box(native_output.data()[0]);
    });

    let source = ImageRef::new(1920, 1080, input.data(), PixelType::U8x4).unwrap();
    let mut fast_output = Image::new(960, 540, PixelType::U8x4);
    let mut resizer = Resizer::new();
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Nearest);
    resizer.resize(&source, &mut fast_output, &options).unwrap();
    let fast = measure(|| {
        resizer
            .resize(
                black_box(&source),
                black_box(&mut fast_output),
                black_box(&options),
            )
            .unwrap();
        black_box(fast_output.buffer()[0]);
    });

    let mismatched_pixels = native_output
        .data()
        .chunks_exact(4)
        .zip(fast_output.buffer().chunks_exact(4))
        .filter(|(native, fast)| native != fast)
        .count();
    let ratio = fast.as_secs_f64() / native.as_secs_f64();
    let decision = if mismatched_pixels > 0 {
        "reject fast_image_resize: nearest pixel contract differs"
    } else if ratio <= 0.90 {
        "candidate: fast_image_resize clears the 10% adoption threshold"
    } else {
        "keep native compositor: measured gain is below 10%"
    };
    println!(
        "resize 1920x1080 -> 960x540 nearest: native={:.3}ms, fast_image_resize={:.3}ms, ratio={ratio:.3}, mismatched_pixels={mismatched_pixels}; {decision}",
        millis(native),
        millis(fast),
    );
}

fn patterned_bgra() -> Vec<u8> {
    let mut pixels = Vec::with_capacity(1920 * 1080 * 4);
    for y in 0_u32..1080 {
        for x in 0_u32..1920 {
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

fn measure(mut operation: impl FnMut()) -> Duration {
    let started = Instant::now();
    for _ in 0..ITERATIONS {
        operation();
    }
    started.elapsed() / ITERATIONS
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
