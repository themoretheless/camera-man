use std::error::Error;
use std::hint::black_box;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use camera_man::benchmarking::{DurationStatistics, duration_statistics};
use camera_man::{Frame, SharedFrameReader, SharedFrameSink, VirtualCameraSink, new_generation_id};
use serde::Serialize;

const DEFAULT_WIDTH: u32 = 640;
const DEFAULT_HEIGHT: u32 = 360;
const DEFAULT_ITERATIONS: u32 = 200;
const WARMUP_ITERATIONS: u32 = 20;

#[derive(Debug, Serialize)]
struct TransportComparison {
    schema_version: u32,
    workload: &'static str,
    width: u32,
    height: u32,
    bytes_per_frame: usize,
    iterations: u32,
    warmup_iterations: u32,
    shared_mmap_copy_and_borrow: DurationStatistics,
    mutex_copy_and_borrow: DurationStatistics,
    shared_to_mutex_average_ratio: f64,
    checksum: u8,
    conclusion: &'static str,
}

struct MutexLatestFrame {
    slot: Mutex<Vec<u8>>,
}

impl MutexLatestFrame {
    fn new(capacity: usize) -> Self {
        Self {
            slot: Mutex::new(vec![0; capacity]),
        }
    }

    fn publish_and_read(&self, bytes: &[u8]) -> u8 {
        let mut slot = self.slot.lock().expect("reference mutex poisoned");
        slot.copy_from_slice(bytes);
        black_box(slot[slot.len() / 2])
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let iterations = std::env::var("CAMERAMAN_TRANSPORT_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_ITERATIONS)
        .max(10);
    let frame = patterned_frame(DEFAULT_WIDTH, DEFAULT_HEIGHT)?;
    let name = format!(
        "/ctb-{}-{:04x}",
        std::process::id(),
        new_generation_id() & 0xffff
    );
    let mut shared = SharedFrameSink::new(&name, frame.data().len());
    shared.connect()?;
    let mut reader = SharedFrameReader::try_open(&name)?.ok_or("mmap reader did not open")?;
    let reference = MutexLatestFrame::new(frame.data().len());

    for _ in 0..WARMUP_ITERATIONS {
        let _ = measure_shared(&mut shared, &mut reader, &frame)?;
        black_box(reference.publish_and_read(frame.data()));
    }

    let mut shared_samples = Vec::with_capacity(iterations as usize);
    let mut mutex_samples = Vec::with_capacity(iterations as usize);
    let mut checksum = 0_u8;
    for iteration in 0..iterations {
        if iteration % 2 == 0 {
            let (duration, value) = measure_shared(&mut shared, &mut reader, &frame)?;
            shared_samples.push(duration);
            checksum ^= value;
            let started = Instant::now();
            checksum ^= reference.publish_and_read(frame.data());
            mutex_samples.push(started.elapsed());
        } else {
            let started = Instant::now();
            checksum ^= reference.publish_and_read(frame.data());
            mutex_samples.push(started.elapsed());
            let (duration, value) = measure_shared(&mut shared, &mut reader, &frame)?;
            shared_samples.push(duration);
            checksum ^= value;
        }
    }
    shared.disconnect();

    let shared_statistics = duration_statistics(&shared_samples).ok_or("missing mmap samples")?;
    let mutex_statistics = duration_statistics(&mutex_samples).ok_or("missing mutex samples")?;
    let report = TransportComparison {
        schema_version: 1,
        workload: "same-process full-frame copy plus latest-frame read",
        width: frame.width(),
        height: frame.height(),
        bytes_per_frame: frame.data().len(),
        iterations,
        warmup_iterations: WARMUP_ITERATIONS,
        shared_mmap_copy_and_borrow: shared_statistics,
        mutex_copy_and_borrow: mutex_statistics,
        shared_to_mutex_average_ratio: shared_statistics.average_ms
            / mutex_statistics.average_ms.max(f64::EPSILON),
        checksum,
        conclusion: "keep mmap only at the required process boundary; do not introduce a general lock-free queue without a measured deadline failure",
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn measure_shared(
    sink: &mut SharedFrameSink,
    reader: &mut SharedFrameReader,
    frame: &Frame,
) -> Result<(Duration, u8), Box<dyn Error>> {
    let started = Instant::now();
    sink.send(frame)?;
    let borrowed = reader
        .read_latest_borrowed()?
        .ok_or("published mmap frame was not visible")?;
    let value = black_box(borrowed.frame.data()[borrowed.frame.data().len() / 2]);
    Ok((started.elapsed(), value))
}

fn patterned_frame(width: u32, height: u32) -> Result<Frame, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            bytes.extend_from_slice(&[
                (x.wrapping_mul(13) ^ y.wrapping_mul(7)) as u8,
                (x.wrapping_mul(3).wrapping_add(y.wrapping_mul(11))) as u8,
                (x ^ y.rotate_left(3)) as u8,
                255,
            ]);
        }
    }
    Ok(Frame::new_checked(
        width,
        height,
        camera_man::PixelFormat::Bgra8,
        bytes,
    )?)
}
