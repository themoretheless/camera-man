#![cfg(target_os = "macos")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use camera_man::{
    Frame, SharedFrameReader, SharedFrameSink, VIRTUAL_CAMERA_HEIGHT, VIRTUAL_CAMERA_WIDTH,
    VirtualCameraSink,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct ProcessMemory {
    directory: PathBuf,
    path: PathBuf,
}

impl ProcessMemory {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "cameraman-process-test-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("frames-v5.mmap");
        Self { directory, path }
    }
}

impl Drop for ProcessMemory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn child_process_reads_the_file_backed_ring_and_lifecycle_cleans_it() {
    let memory = ProcessMemory::new();
    let mut sink = SharedFrameSink::new_file(&memory.path, 1024);
    sink.connect().unwrap();
    sink.send(&Frame::solid_bgra(4, 3, [17, 17, 17, 255]).unwrap())
        .unwrap();

    let output = run_watcher(&memory.path, 250, 1);
    assert_success(output);
    sink.disconnect();
    assert!(!memory.path.exists());
}

#[test]
fn killed_writer_is_replaced_with_a_new_generation() {
    let memory = ProcessMemory::new();
    let ready = memory.directory.join("writer.ready");
    let mut child = Command::new(probe())
        .arg("flood")
        .arg(&memory.path)
        .arg(&ready)
        .spawn()
        .unwrap();
    wait_for_file(&ready, &mut child);

    let reader = wait_for_reader(&memory.path);
    let crashed_generation = reader_generation(&memory.path);
    assert!(reader.writer_is_alive());
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!reader.writer_is_alive());

    let capacity = VIRTUAL_CAMERA_WIDTH as usize * VIRTUAL_CAMERA_HEIGHT as usize * 4;
    let mut replacement = SharedFrameSink::new_file(&memory.path, capacity);
    replacement.connect().unwrap();
    replacement
        .send(
            &Frame::solid_bgra(
                VIRTUAL_CAMERA_WIDTH,
                VIRTUAL_CAMERA_HEIGHT,
                [99, 99, 99, 255],
            )
            .unwrap(),
        )
        .unwrap();
    let mut replacement_reader = SharedFrameReader::try_open_file(&memory.path)
        .unwrap()
        .unwrap();
    let frame = replacement_reader.read_latest().unwrap().unwrap();
    assert_ne!(frame.generation, crashed_generation);
    assert_eq!(frame.frame.bgra_at(0, 0), Some([99, 99, 99, 255]));
    replacement.disconnect();
}

#[test]
fn multiple_fast_and_slow_process_readers_never_observe_tearing() {
    let memory = ProcessMemory::new();
    let mut sink = SharedFrameSink::new_file(&memory.path, 64 * 64 * 4);
    sink.connect().unwrap();
    let watchers = [0_u64, 1, 12]
        .into_iter()
        .map(|poll| spawn_watcher(&memory.path, 700, poll))
        .collect::<Vec<_>>();

    for value in 1..=120_u8 {
        sink.send(&Frame::solid_bgra(64, 64, [value, value, value, 255]).unwrap())
            .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    for watcher in watchers {
        assert_success(watcher.wait_with_output().unwrap());
    }
    sink.disconnect();
}

#[test]
fn producer_death_during_a_read_is_contained_by_the_reader_process() {
    let memory = ProcessMemory::new();
    let mut writer = spawn_flood_writer(&memory);
    let reader_ready = memory.directory.join("reader.ready");
    let mut reader = spawn_ready_watcher(&memory.path, &reader_ready, 400);
    wait_for_file(&reader_ready, &mut reader);

    writer.kill().unwrap();
    writer.wait().unwrap();

    assert_success(reader.wait_with_output().unwrap());
}

#[test]
fn replacing_a_mapped_file_keeps_the_existing_reader_on_its_inode() {
    let memory = ProcessMemory::new();
    let mut writer = spawn_flood_writer(&memory);
    let reader_ready = memory.directory.join("replace-reader.ready");
    let mut reader = spawn_ready_watcher(&memory.path, &reader_ready, 300);
    wait_for_file(&reader_ready, &mut reader);
    writer.kill().unwrap();
    writer.wait().unwrap();

    let old_path = memory.directory.join("frames-old.mmap");
    fs::rename(&memory.path, &old_path).unwrap();
    fs::copy(&old_path, &memory.path).unwrap();

    assert_success(reader.wait_with_output().unwrap());
}

#[test]
fn permission_change_after_map_does_not_revoke_the_existing_reader() {
    let memory = ProcessMemory::new();
    let mut writer = spawn_flood_writer(&memory);
    let reader_ready = memory.directory.join("permission-reader.ready");
    let mut reader = spawn_ready_watcher(&memory.path, &reader_ready, 300);
    wait_for_file(&reader_ready, &mut reader);
    writer.kill().unwrap();
    writer.wait().unwrap();

    fs::set_permissions(&memory.path, fs::Permissions::from_mode(0o000)).unwrap();
    let output = reader.wait_with_output().unwrap();
    fs::set_permissions(&memory.path, fs::Permissions::from_mode(0o600)).unwrap();

    assert_success(output);
}

#[test]
fn truncate_fault_is_contained_to_the_reader_child_process() {
    let memory = ProcessMemory::new();
    let mut writer = spawn_flood_writer(&memory);
    let reader_ready = memory.directory.join("truncate-reader.ready");
    let mut reader = spawn_ready_watcher(&memory.path, &reader_ready, 2_000);
    wait_for_file(&reader_ready, &mut reader);
    writer.kill().unwrap();
    writer.wait().unwrap();

    fs::OpenOptions::new()
        .write(true)
        .open(&memory.path)
        .unwrap()
        .set_len(0)
        .unwrap();
    let output = reader.wait_with_output().unwrap();

    assert_eq!(output.status.signal(), Some(libc::SIGBUS));
}

fn probe() -> &'static str {
    env!("CARGO_BIN_EXE_cameraman-mmap-probe")
}

fn spawn_watcher(path: &Path, duration_millis: u64, poll_millis: u64) -> Child {
    Command::new(probe())
        .arg("watch")
        .arg(path)
        .arg(duration_millis.to_string())
        .arg(poll_millis.to_string())
        .spawn()
        .unwrap()
}

fn spawn_ready_watcher(path: &Path, ready: &Path, duration_millis: u64) -> Child {
    Command::new(probe())
        .arg("watch")
        .arg(path)
        .arg(duration_millis.to_string())
        .arg("1")
        .arg(ready)
        .spawn()
        .unwrap()
}

fn spawn_flood_writer(memory: &ProcessMemory) -> Child {
    let ready = memory.directory.join("writer.ready");
    let mut child = Command::new(probe())
        .arg("flood")
        .arg(&memory.path)
        .arg(&ready)
        .spawn()
        .unwrap();
    wait_for_file(&ready, &mut child);
    child
}

fn run_watcher(path: &Path, duration_millis: u64, poll_millis: u64) -> Output {
    spawn_watcher(path, duration_millis, poll_millis)
        .wait_with_output()
        .unwrap()
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "watcher failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn wait_for_file(path: &Path, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if path.is_file() {
            return;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("writer exited before ready: {status}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("writer readiness timed out");
}

fn wait_for_reader(path: &Path) -> SharedFrameReader {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(reader) = SharedFrameReader::try_open_file(path).unwrap() {
            return reader;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("reader open timed out");
}

fn reader_generation(path: &Path) -> u64 {
    let mut reader = wait_for_reader(path);
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(frame) = reader.read_latest().unwrap() {
            return frame.generation;
        }
        std::thread::yield_now();
    }
    panic!("writer did not publish before timeout");
}
