//! Lock-free, fixed-slot frame transport shared by the app and extension.
//!
//! This facade keeps the public API small while protocol layout, mmap
//! ownership, publishing, and borrowing remain independently testable.

mod mapped_backend;
mod protocol;
mod reader;
#[doc(hidden)]
pub mod slot_state_machine;
mod validation;
mod writer;

pub use mapped_backend::{
    SharedFrameEndpoint, default_shared_frame_endpoint, default_shared_memory_name,
};
pub use reader::{BorrowedTransportFrame, SharedFrameReader};
pub use validation::{
    SHARED_PROTOCOL_MAGIC, SHARED_PROTOCOL_SLOT_COUNT, SHARED_PROTOCOL_VERSION,
    SharedHeaderMetadata, SharedProtocolValidationError, SharedSlotMetadata,
    validate_shared_header_metadata, validate_shared_slot_metadata,
};
pub use writer::{ConsumerProgress, ProducerProgress, SharedFrameSink};

#[doc(hidden)]
pub use protocol::{checked_mapped_len, checked_slot_offset};

#[cfg(test)]
mod tests;
