use std::fmt;

use crate::frame::{PixelFormat, checked_frame_byte_len};
use crate::media_contract::Colorimetry;

use super::protocol::{
    INVALID_SLOT, MAGIC, PIXEL_FORMAT_BGRA8, SLOT_COUNT, VERSION, checked_mapped_len,
};
use super::slot_state_machine::{SLOT_READING, SLOT_READY, is_known_state, slot_owner, slot_state};

pub const SHARED_PROTOCOL_MAGIC: u32 = MAGIC;
pub const SHARED_PROTOCOL_VERSION: u32 = VERSION;
pub const SHARED_PROTOCOL_SLOT_COUNT: u32 = SLOT_COUNT as u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedHeaderMetadata {
    pub magic: u32,
    pub version: u32,
    pub slot_count: u32,
    pub active_slot: u32,
    pub slot_capacity: u64,
    pub mapping_len: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedSlotMetadata {
    pub state: u64,
    pub width: u32,
    pub height: u32,
    pub pixel_format: u32,
    pub fps: u32,
    pub color_contract: u32,
    pub generation: u64,
    pub data_len: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedProtocolValidationError {
    Magic,
    Version,
    SlotCount,
    ActiveSlot,
    SlotCapacity,
    MappingLength,
    SlotState,
    ReaderOwner,
    FrameMetadata,
}

impl fmt::Display for SharedProtocolValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Magic => "invalid shared-memory magic",
            Self::Version => "unsupported shared-memory protocol version",
            Self::SlotCount => "invalid shared-memory slot count",
            Self::ActiveSlot => "active shared-memory slot is out of range",
            Self::SlotCapacity => "shared-memory slot capacity does not fit this process",
            Self::MappingLength => "shared-memory mapping length is inconsistent",
            Self::SlotState => "unknown shared-memory slot state",
            Self::ReaderOwner => "reading slot has no owner",
            Self::FrameMetadata => "published frame metadata is inconsistent",
        })
    }
}

impl std::error::Error for SharedProtocolValidationError {}

pub fn validate_shared_header_metadata(
    metadata: SharedHeaderMetadata,
) -> Result<(), SharedProtocolValidationError> {
    if metadata.magic != MAGIC {
        return Err(SharedProtocolValidationError::Magic);
    }
    if metadata.version != VERSION {
        return Err(SharedProtocolValidationError::Version);
    }
    if metadata.slot_count != SLOT_COUNT as u32 {
        return Err(SharedProtocolValidationError::SlotCount);
    }
    if metadata.active_slot != INVALID_SLOT && metadata.active_slot >= SLOT_COUNT as u32 {
        return Err(SharedProtocolValidationError::ActiveSlot);
    }
    let slot_capacity = usize::try_from(metadata.slot_capacity)
        .map_err(|_| SharedProtocolValidationError::SlotCapacity)?;
    let expected = checked_mapped_len(slot_capacity, metadata.page_size)
        .ok_or(SharedProtocolValidationError::MappingLength)?;
    if expected != metadata.mapping_len {
        return Err(SharedProtocolValidationError::MappingLength);
    }
    Ok(())
}

pub fn validate_shared_slot_metadata(
    metadata: SharedSlotMetadata,
    slot_capacity: u64,
) -> Result<(), SharedProtocolValidationError> {
    if !is_known_state(metadata.state) {
        return Err(SharedProtocolValidationError::SlotState);
    }
    if slot_state(metadata.state) == SLOT_READING && slot_owner(metadata.state) == 0 {
        return Err(SharedProtocolValidationError::ReaderOwner);
    }
    if !matches!(slot_state(metadata.state), SLOT_READY | SLOT_READING) {
        return Ok(());
    }
    let expected = checked_frame_byte_len(
        metadata.width,
        metadata.height,
        PixelFormat::Bgra8.bytes_per_pixel(),
    )
    .ok_or(SharedProtocolValidationError::FrameMetadata)?;
    if metadata.width == 0
        || metadata.height == 0
        || metadata.pixel_format != PIXEL_FORMAT_BGRA8
        || metadata.fps == 0
        || Colorimetry::from_wire_code(metadata.color_contract)
            != Some(Colorimetry::BT709_FULL_OPAQUE)
        || metadata.generation == 0
        || metadata.data_len != expected
        || metadata.data_len > slot_capacity
    {
        return Err(SharedProtocolValidationError::FrameMetadata);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_memory_transport::slot_state_machine::{SLOT_FREE, reading_slot_state};

    #[test]
    fn accepts_a_consistent_header_and_rejects_corruption() {
        let capacity = 1920 * 1080 * 4;
        let valid = SharedHeaderMetadata {
            magic: MAGIC,
            version: VERSION,
            slot_count: SLOT_COUNT as u32,
            active_slot: INVALID_SLOT,
            slot_capacity: capacity as u64,
            mapping_len: checked_mapped_len(capacity, 4096).unwrap(),
            page_size: 4096,
        };
        assert_eq!(validate_shared_header_metadata(valid), Ok(()));
        assert_eq!(
            validate_shared_header_metadata(SharedHeaderMetadata {
                version: VERSION + 1,
                ..valid
            }),
            Err(SharedProtocolValidationError::Version)
        );
    }

    #[test]
    fn validates_only_published_slot_payloads() {
        let capacity = 16;
        let valid = SharedSlotMetadata {
            state: reading_slot_state(42),
            width: 2,
            height: 2,
            pixel_format: PIXEL_FORMAT_BGRA8,
            fps: 30,
            color_contract: Colorimetry::BT709_FULL_OPAQUE.wire_code(),
            generation: 1,
            data_len: 16,
        };
        assert_eq!(validate_shared_slot_metadata(valid, capacity), Ok(()));
        assert_eq!(
            validate_shared_slot_metadata(
                SharedSlotMetadata {
                    color_contract: 0,
                    ..valid
                },
                capacity,
            ),
            Err(SharedProtocolValidationError::FrameMetadata)
        );
        assert_eq!(
            validate_shared_slot_metadata(
                SharedSlotMetadata {
                    data_len: 15,
                    ..valid
                },
                capacity,
            ),
            Err(SharedProtocolValidationError::FrameMetadata)
        );
        assert_eq!(
            validate_shared_slot_metadata(
                SharedSlotMetadata {
                    state: SLOT_FREE,
                    data_len: u64::MAX,
                    ..valid
                },
                capacity,
            ),
            Ok(())
        );
    }
}
