use std::mem::size_of;
use std::sync::atomic::{AtomicU32, AtomicU64};

use crate::config::{VIRTUAL_CAMERA_DEFAULT_FPS, VIRTUAL_CAMERA_HEIGHT, VIRTUAL_CAMERA_WIDTH};
use crate::error::CameraManError;
use crate::wire::WirePixelFormat;

use super::slot_state_machine::SLOT_FREE;

pub(super) const MAGIC: u32 = 0x434D_414E;
pub(super) const VERSION: u32 = 5;
pub(super) const SLOT_COUNT: usize = 3;
pub(super) const INVALID_SLOT: u32 = u32::MAX;
pub(super) const PIXEL_FORMAT_BGRA8: u32 = WirePixelFormat::Bgra8.code();
pub(super) const DEFAULT_SLOT_CAPACITY: usize =
    VIRTUAL_CAMERA_WIDTH as usize * VIRTUAL_CAMERA_HEIGHT as usize * 4;
const CACHE_LINE: usize = 64;

#[repr(C, align(64))]
pub(super) struct SharedSlot {
    pub(super) state: AtomicU64,
    pub(super) width: AtomicU32,
    pub(super) height: AtomicU32,
    pub(super) pixel_format: AtomicU32,
    pub(super) fps: AtomicU32,
    pub(super) color_contract: AtomicU32,
    pub(super) sequence: AtomicU64,
    pub(super) generation: AtomicU64,
    pub(super) monotonic_timestamp_nanos: AtomicU64,
    pub(super) wall_timestamp_nanos: AtomicU64,
    pub(super) data_len: AtomicU64,
}

impl SharedSlot {
    fn new() -> Self {
        Self {
            state: AtomicU64::new(SLOT_FREE),
            width: AtomicU32::new(0),
            height: AtomicU32::new(0),
            pixel_format: AtomicU32::new(PIXEL_FORMAT_BGRA8),
            fps: AtomicU32::new(VIRTUAL_CAMERA_DEFAULT_FPS),
            color_contract: AtomicU32::new(
                crate::media_contract::Colorimetry::BT709_FULL_OPAQUE.wire_code(),
            ),
            sequence: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            monotonic_timestamp_nanos: AtomicU64::new(0),
            wall_timestamp_nanos: AtomicU64::new(0),
            data_len: AtomicU64::new(0),
        }
    }
}

#[repr(C, align(64))]
pub(super) struct SharedHeader {
    pub(super) magic: AtomicU32,
    pub(super) version: AtomicU32,
    pub(super) slot_count: AtomicU32,
    pub(super) active_slot: AtomicU32,
    pub(super) writer_pid: AtomicU32,
    pub(super) consumer_pid: AtomicU32,
    pub(super) writer_generation: AtomicU64,
    pub(super) writer_process_start_token: AtomicU64,
    pub(super) writer_heartbeat_monotonic_nanos: AtomicU64,
    pub(super) consumer_generation: AtomicU64,
    pub(super) consumer_sequence: AtomicU64,
    pub(super) consumer_heartbeat_monotonic_nanos: AtomicU64,
    pub(super) slot_capacity: AtomicU64,
    pub(super) next_sequence: AtomicU64,
    pub(super) slots: [SharedSlot; SLOT_COUNT],
}

impl SharedHeader {
    pub(super) fn new(slot_capacity: usize) -> Self {
        Self {
            // The writer publishes magic only after every other field is ready.
            magic: AtomicU32::new(0),
            version: AtomicU32::new(VERSION),
            slot_count: AtomicU32::new(SLOT_COUNT as u32),
            active_slot: AtomicU32::new(INVALID_SLOT),
            writer_pid: AtomicU32::new(0),
            consumer_pid: AtomicU32::new(0),
            writer_generation: AtomicU64::new(0),
            writer_process_start_token: AtomicU64::new(0),
            writer_heartbeat_monotonic_nanos: AtomicU64::new(0),
            consumer_generation: AtomicU64::new(0),
            consumer_sequence: AtomicU64::new(0),
            consumer_heartbeat_monotonic_nanos: AtomicU64::new(0),
            slot_capacity: AtomicU64::new(slot_capacity as u64),
            next_sequence: AtomicU64::new(0),
            slots: std::array::from_fn(|_| SharedSlot::new()),
        }
    }
}

pub(super) const fn aligned_header_len() -> usize {
    size_of::<SharedHeader>().div_ceil(CACHE_LINE) * CACHE_LINE
}

pub(super) fn mapped_len(slot_capacity: usize) -> Result<usize, CameraManError> {
    checked_mapped_len(slot_capacity, page_size()).ok_or(CameraManError::BufferTooLarge {
        bytes: slot_capacity as u64,
    })
}

#[doc(hidden)]
pub fn checked_mapped_len(slot_capacity: usize, page_size: usize) -> Option<usize> {
    if page_size == 0 {
        return None;
    }
    let raw_len = slot_capacity
        .checked_mul(SLOT_COUNT)?
        .checked_add(aligned_header_len())?;
    raw_len
        .checked_add(page_size - 1)
        .map(|len| len / page_size * page_size)
}

#[doc(hidden)]
pub fn checked_slot_offset(slot: usize, slot_capacity: usize) -> Option<usize> {
    (slot < SLOT_COUNT)
        .then_some(slot)
        .and_then(|slot| slot.checked_mul(slot_capacity))
        .and_then(|bytes| aligned_header_len().checked_add(bytes))
}

pub(super) fn page_size() -> usize {
    // SAFETY: the selector is a constant and `sysconf` neither receives nor
    // retains any Rust pointer.
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    usize::try_from(size)
        .ok()
        .filter(|size| *size > 0)
        .unwrap_or(4096)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn successful_mapping_contains_every_slot_start() {
        let slot_capacity = usize::try_from(kani::any::<u32>()).unwrap();
        let mapped = checked_mapped_len(slot_capacity, 4_096).unwrap();
        let offset0 = checked_slot_offset(0, slot_capacity).unwrap();
        let offset1 = checked_slot_offset(1, slot_capacity).unwrap();
        let offset2 = checked_slot_offset(2, slot_capacity).unwrap();
        assert!(slot_capacity <= mapped - offset0);
        assert!(slot_capacity <= mapped - offset1);
        assert!(slot_capacity <= mapped - offset2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_mapping_contains_all_three_slots() {
        for capacity in [0, 1, 4, 4_096, DEFAULT_SLOT_CAPACITY] {
            let mapped = checked_mapped_len(capacity, 4_096).unwrap();
            for slot in 0..SLOT_COUNT {
                let offset = checked_slot_offset(slot, capacity).unwrap();
                assert!(offset <= mapped);
                assert!(capacity <= mapped - offset);
            }
        }
    }

    #[test]
    fn checked_mapping_rejects_invalid_page_or_overflow() {
        assert_eq!(checked_mapped_len(1, 0), None);
        assert_eq!(checked_mapped_len(usize::MAX, 4_096), None);
        assert_eq!(checked_slot_offset(SLOT_COUNT, 1), None);
    }

    #[test]
    fn protocol_layout_is_cache_line_aligned() {
        assert_eq!(std::mem::align_of::<SharedHeader>(), CACHE_LINE);
        assert_eq!(aligned_header_len() % CACHE_LINE, 0);
        assert!(aligned_header_len() >= std::mem::size_of::<SharedHeader>());
    }
}
