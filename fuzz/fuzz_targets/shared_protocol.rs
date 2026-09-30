#![no_main]

use camera_man::{
    SharedHeaderMetadata, SharedSlotMetadata, checked_mapped_len, validate_shared_header_metadata,
    validate_shared_slot_metadata,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let Some(bytes) = input.get(..80) else {
        return;
    };
    let u32_at = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let u64_at = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());

    let slot_capacity = u64_at(16);
    let page_size = usize::try_from(u32_at(24)).unwrap_or(0);
    let mapping_len = usize::try_from(u64_at(28)).unwrap_or(usize::MAX);
    let header = SharedHeaderMetadata {
        magic: u32_at(0),
        version: u32_at(4),
        slot_count: u32_at(8),
        active_slot: u32_at(12),
        slot_capacity,
        mapping_len,
        page_size,
    };
    let _ = validate_shared_header_metadata(header);

    let slot = SharedSlotMetadata {
        state: u64_at(36),
        width: u32_at(44),
        height: u32_at(48),
        pixel_format: u32_at(52),
        fps: u32_at(56),
        color_contract: u32_at(60),
        generation: u64_at(64),
        data_len: u64_at(72),
    };
    let _ = validate_shared_slot_metadata(slot, slot_capacity);

    if let Ok(capacity) = usize::try_from(slot_capacity) {
        let _ = checked_mapped_len(capacity, page_size);
    }
});
