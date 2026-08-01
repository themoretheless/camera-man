use std::ffi::CString;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::CameraManError;
use crate::frame::Frame;
use crate::media_contract::Colorimetry;
use crate::media_time::new_generation_id;
use crate::virtual_camera::VirtualCameraSink;

use super::protocol::DEFAULT_SLOT_CAPACITY;
use super::slot_state_machine::{SLOT_READING, SLOT_READY, reading_slot_state, slot_state};
use super::{SharedFrameReader, SharedFrameSink};

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

/// Hang guard only: the concurrent reader exits the moment it observes the
/// final frame, and the writer holds the sink until the reader acknowledges,
/// so this deadline never decides pass or fail on a healthy machine.
const CONCURRENT_READ_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

struct TestMemory {
    name: String,
}

impl TestMemory {
    fn new() -> Self {
        let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            name: format!("/cameraman-test-{}-{id}", std::process::id()),
        }
    }
}

impl Drop for TestMemory {
    fn drop(&mut self) {
        if let Ok(name) = CString::new(self.name.as_str()) {
            // SAFETY: `name` is a live NUL-terminated POSIX shm name and
            // unlinking a missing test object is harmless.
            unsafe {
                libc::shm_unlink(name.as_ptr());
            }
        }
    }
}

#[test]
fn reader_waits_until_writer_exists() {
    let memory = TestMemory::new();
    assert!(SharedFrameReader::try_open(&memory.name).unwrap().is_none());
}

#[test]
fn sink_requires_connect() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    let frame = Frame::solid_bgra(2, 2, [1, 2, 3, 255]).unwrap();

    assert!(matches!(
        sink.send(&frame),
        Err(CameraManError::VirtualCameraUnavailable(
            "shared-memory sink is not connected"
        ))
    ));
}

#[test]
fn round_trips_latest_frame_without_a_file() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    sink.set_target_fps(60);
    sink.connect().unwrap();
    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    let frame = Frame::solid_bgra(4, 3, [1, 2, 3, 255]).unwrap();

    sink.send(&frame).unwrap();
    let transported = reader.read_latest().unwrap().unwrap();

    assert_eq!(transported.frame, frame);
    assert_eq!(
        transported.frame.contract().colorimetry,
        Colorimetry::BT709_FULL_OPAQUE
    );
    assert_eq!(transported.sequence, 0);
    assert_ne!(transported.generation, 0);
    assert_ne!(transported.monotonic_timestamp_nanos, 0);
    assert_eq!(transported.fps, 60);
    assert_eq!(reader.read_latest().unwrap(), None);
}

#[test]
fn reader_acknowledges_the_exact_writer_generation_and_sequence() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    sink.connect().unwrap();
    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    sink.send(&Frame::solid_bgra(2, 2, [1, 2, 3, 255]).unwrap())
        .unwrap();

    let producer = sink.producer_progress().unwrap();
    assert_eq!(sink.consumer_progress(), None);
    let consumed = reader.read_latest().unwrap().unwrap();
    let consumer = sink.consumer_progress().unwrap();

    assert_eq!(producer.generation, consumed.generation);
    assert_eq!(producer.sequence, consumed.sequence);
    assert_eq!(consumer.generation, producer.generation);
    assert_eq!(consumer.sequence, producer.sequence);
    assert_eq!(consumer.pid, std::process::id());
    assert_ne!(consumer.heartbeat_monotonic_nanos, 0);
}

#[test]
fn reader_resets_sequence_cache_when_writer_generation_changes() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    sink.connect().unwrap();
    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    let first = Frame::solid_bgra(2, 2, [1, 2, 3, 255]).unwrap();
    sink.send(&first).unwrap();
    let first_generation = reader.read_latest().unwrap().unwrap().generation;

    let mapping = sink.mapping.as_ref().unwrap();
    let next_generation = new_generation_id();
    mapping
        .header()
        .writer_generation
        .store(next_generation, Ordering::Release);
    mapping.header().next_sequence.store(0, Ordering::Release);
    let second = Frame::solid_bgra(2, 2, [4, 5, 6, 255]).unwrap();
    sink.send(&second).unwrap();

    let transported = reader.read_latest().unwrap().unwrap();
    assert_eq!(transported.sequence, 0);
    assert_eq!(transported.generation, next_generation);
    assert_ne!(transported.generation, first_generation);
    assert_eq!(transported.frame, second);
}

#[test]
fn borrowed_frame_holds_and_releases_the_mmap_slot() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    sink.connect().unwrap();
    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    let frame = Frame::solid_bgra(2, 2, [7, 8, 9, 255]).unwrap();
    sink.send(&frame).unwrap();

    let mapping = sink.mapping.as_ref().unwrap();
    let active = mapping.header().active_slot.load(Ordering::Acquire) as usize;
    let reader_slot_data = reader
        .mapping
        .slot_data_ptr(active, reader.slot_capacity)
        .cast_const();
    {
        let borrowed = reader.read_latest_borrowed().unwrap().unwrap();
        assert_eq!(borrowed.frame.bgra_at(1, 1), Some([7, 8, 9, 255]));
        assert_eq!(borrowed.frame.data().as_ptr(), reader_slot_data);
        assert_eq!(
            slot_state(mapping.header().slots[active].state.load(Ordering::Acquire)),
            SLOT_READING
        );
    }
    assert_eq!(
        slot_state(mapping.header().slots[active].state.load(Ordering::Acquire)),
        SLOT_READY
    );
}

#[test]
fn app_group_style_mmap_file_uses_the_same_slot_protocol() {
    let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "cameraman-mmap-test-{}-{id}/frames.mmap",
        std::process::id()
    ));
    let mut sink = SharedFrameSink::new_file(&path, 1024);
    sink.connect().unwrap();
    let mut reader = SharedFrameReader::try_open_file(&path).unwrap().unwrap();
    let frame = Frame::solid_bgra(4, 3, [9, 8, 7, 255]).unwrap();

    sink.send(&frame).unwrap();

    assert_eq!(reader.read_latest().unwrap().unwrap().frame, frame);
    sink.disconnect();
    assert!(!path.exists());
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}

#[test]
fn round_trips_a_full_hd_bgra_frame() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, DEFAULT_SLOT_CAPACITY);
    sink.connect().unwrap();
    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    let frame = Frame::solid_bgra(1920, 1080, [7, 11, 13, 255]).unwrap();

    sink.send(&frame).unwrap();
    let transported = reader.read_latest().unwrap().unwrap();

    assert_eq!(transported.frame.width(), 1920);
    assert_eq!(transported.frame.height(), 1080);
    assert_eq!(transported.frame.data().len(), DEFAULT_SLOT_CAPACITY);
    assert_eq!(transported.frame.bgra_at(0, 0), Some([7, 11, 13, 255]));
    assert_eq!(
        transported.frame.bgra_at(1919, 1079),
        Some([7, 11, 13, 255])
    );
}

#[test]
fn publishes_newest_of_multiple_frames() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    sink.connect().unwrap();
    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();

    for value in 0..10_u8 {
        sink.send(&Frame::solid_bgra(2, 2, [value, 0, 0, 255]).unwrap())
            .unwrap();
    }
    let transported = reader.read_latest().unwrap().unwrap();

    assert_eq!(transported.sequence, 9);
    assert_eq!(transported.frame.bgra_at(0, 0), Some([9, 0, 0, 255]));
}

#[test]
fn rejects_frames_larger_than_a_slot() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 8);
    sink.connect().unwrap();
    let frame = Frame::solid_bgra(2, 2, [1, 2, 3, 255]).unwrap();

    assert!(matches!(
        sink.send(&frame),
        Err(CameraManError::BufferTooLarge { bytes: 16 })
    ));
}

#[test]
fn prevents_two_writers_from_owning_the_same_region() {
    let memory = TestMemory::new();
    let mut first = SharedFrameSink::new(&memory.name, 1024);
    let mut second = SharedFrameSink::new(&memory.name, 1024);
    first.connect().unwrap();

    assert!(matches!(
        second.connect(),
        Err(CameraManError::VirtualCameraUnavailable(
            "another shared-memory writer is already connected"
        ))
    ));
}

#[test]
fn incompatible_second_writer_cannot_resize_the_live_region() {
    let memory = TestMemory::new();
    let mut first = SharedFrameSink::new(&memory.name, 1024);
    let mut incompatible = SharedFrameSink::new(&memory.name, 2048);
    first.connect().unwrap();

    assert!(matches!(
        incompatible.connect(),
        Err(CameraManError::VirtualCameraUnavailable(
            "existing shared-memory object has an incompatible header"
        ))
    ));

    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    let frame = Frame::solid_bgra(2, 2, [5, 6, 7, 255]).unwrap();
    first.send(&frame).unwrap();
    assert_eq!(reader.read_latest().unwrap().unwrap().frame, frame);
}

#[test]
fn reader_observes_a_graceful_writer_disconnect() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    sink.connect().unwrap();
    let reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    assert!(reader.writer_is_alive());

    sink.disconnect();

    assert!(!reader.writer_is_alive());
    assert!(SharedFrameReader::try_open(&memory.name).unwrap().is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn reader_rejects_a_reused_pid_with_a_different_start_token() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    sink.connect().unwrap();
    let reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    let header = sink.mapping.as_ref().unwrap().header();
    let original = header.writer_process_start_token.load(Ordering::Acquire);
    let different = original.checked_add(1).unwrap_or(original - 1);

    assert_ne!(original, 0);
    assert!(reader.writer_is_alive());
    header
        .writer_process_start_token
        .store(different, Ordering::Release);
    assert!(!reader.writer_is_alive());
    header
        .writer_process_start_token
        .store(original, Ordering::Release);
    assert!(reader.writer_is_alive());
}

#[test]
fn writer_reclaims_slots_owned_by_dead_readers() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 1024);
    sink.connect().unwrap();
    let mapping = sink.mapping.as_ref().unwrap();
    for slot in &mapping.header().slots {
        slot.state
            .store(reading_slot_state(u32::MAX), Ordering::Release);
    }

    let frame = Frame::solid_bgra(2, 2, [8, 9, 10, 255]).unwrap();
    sink.send(&frame).unwrap();

    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();
    assert_eq!(reader.read_latest().unwrap().unwrap().frame, frame);
}

#[test]
fn concurrent_reader_never_observes_a_torn_frame() {
    let memory = TestMemory::new();
    let mut sink = SharedFrameSink::new(&memory.name, 64 * 64 * 4);
    sink.connect().unwrap();
    let mut reader = SharedFrameReader::try_open(&memory.name).unwrap().unwrap();

    // Dropping the sink clears writer ownership and unlinks the name, after
    // which every read reports no writer. The writer must therefore outlive the
    // reader's acknowledgement, or a descheduled reader observes an ownerless
    // region instead of the final frame.
    let (reader_done_tx, reader_done_rx) = std::sync::mpsc::channel::<()>();

    let writer = std::thread::spawn(move || {
        for value in 1..=50_u8 {
            let frame = Frame::solid_bgra(64, 64, [value, value, value, 255]).unwrap();
            sink.send(&frame).unwrap();
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        // Parks instead of spinning, and returns immediately on a disconnect if
        // the reader unwinds on the tearing assertion.
        let _ = reader_done_rx.recv_timeout(CONCURRENT_READ_DEADLINE);
        drop(sink);
    });

    let deadline = std::time::Instant::now() + CONCURRENT_READ_DEADLINE;
    let mut saw_last = false;
    // A writer that unwound publishes nothing further, so stop instead of
    // spinning out the hang guard and burying its panic behind this one.
    while !saw_last && !writer.is_finished() && std::time::Instant::now() < deadline {
        if let Some(transported) = reader.read_latest().unwrap() {
            let first = transported.frame.data()[0];
            assert!(
                transported
                    .frame
                    .data()
                    .chunks_exact(4)
                    .all(|pixel| pixel == [first, first, first, 255])
            );
            saw_last = first == 50;
        } else {
            std::hint::spin_loop();
        }
    }
    let _ = reader_done_tx.send(());
    if let Err(panic) = writer.join() {
        // Re-raise with the writer's own message instead of an opaque `Any`.
        std::panic::resume_unwind(panic);
    }
    assert!(saw_last, "reader did not observe the final published frame");
}
