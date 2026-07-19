use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};

use camera_man::{
    Frame, SharedFrameReader, SharedFrameSink, VIRTUAL_CAMERA_HEIGHT, VIRTUAL_CAMERA_WIDTH,
    VirtualCameraSink,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("watch") => watch(
            required_path(&args, 1)?,
            parse_u64(&args, 2, "duration milliseconds")?,
            parse_u64(&args, 3, "poll milliseconds")?,
            args.get(4).map(Path::new),
        ),
        Some("flood") => flood(required_path(&args, 1)?, required_path(&args, 2)?),
        _ => Err("usage: cameraman-mmap-probe watch <path> <duration-ms> <poll-ms> [ready-path] | flood <path> <ready-path>".into()),
    }
}

fn watch(
    path: &Path,
    duration_millis: u64,
    poll_millis: u64,
    ready_path: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_millis(duration_millis);
    let mut reader = None;
    let mut observed = 0_u64;
    let mut last_sequence = None;
    while Instant::now() < deadline {
        if reader.is_none() {
            reader = SharedFrameReader::try_open_file(path)?;
        }
        if let Some(active_reader) = reader.as_mut()
            && let Some(frame) = active_reader.read_latest_borrowed()?
        {
            let first = frame.frame.data().get(..4).ok_or("empty published frame")?;
            if !frame
                .frame
                .data()
                .chunks_exact(4)
                .all(|pixel| pixel == first && pixel[3] == 255)
            {
                return Err("torn frame observed".into());
            }
            observed += 1;
            last_sequence = Some(frame.sequence);
            if observed == 1
                && let Some(ready_path) = ready_path
            {
                std::fs::write(ready_path, b"ready")?;
            }
        }
        if poll_millis == 0 {
            std::thread::yield_now();
        } else {
            std::thread::sleep(Duration::from_millis(poll_millis));
        }
    }
    if observed == 0 {
        return Err("no frame observed".into());
    }
    println!(
        "observed={observed} last_sequence={}",
        last_sequence.unwrap_or(0)
    );
    Ok(())
}

fn flood(path: &Path, ready_path: &Path) -> Result<(), Box<dyn Error>> {
    let capacity = VIRTUAL_CAMERA_WIDTH as usize * VIRTUAL_CAMERA_HEIGHT as usize * 4;
    let mut sink = SharedFrameSink::new_file(path, capacity);
    sink.connect()?;
    std::fs::write(ready_path, b"ready")?;
    let mut value = 1_u8;
    loop {
        let frame = Frame::solid_bgra(
            VIRTUAL_CAMERA_WIDTH,
            VIRTUAL_CAMERA_HEIGHT,
            [value, value, value, 255],
        )?;
        sink.send(&frame)?;
        value = value.wrapping_add(1).max(1);
    }
}

fn required_path(args: &[String], index: usize) -> Result<&Path, Box<dyn Error>> {
    args.get(index)
        .map(Path::new)
        .ok_or_else(|| "missing path argument".into())
}

fn parse_u64(args: &[String], index: usize, name: &str) -> Result<u64, Box<dyn Error>> {
    args.get(index)
        .ok_or_else(|| format!("missing {name}").into())
        .and_then(|value| value.parse::<u64>().map_err(Into::into))
}
