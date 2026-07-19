use std::path::PathBuf;
use std::sync::atomic::Ordering;

use crate::diagnostics::{PipelineStage, stage_span};
use crate::error::CameraManError;
use crate::frame::{FrameView, PixelFormat};
use crate::frame_transport::TransportFrame;
use crate::media_contract::{Colorimetry, FrameContract};
use crate::performance::{CopyStage, copy_ledger};

use super::mapped_backend::{MappedRegion, SharedFrameEndpoint, process_matches_start_token};
use super::protocol::{SLOT_COUNT, SharedSlot};
use super::slot_state_machine::{release_read, try_claim_for_read};
use super::validation::{SharedSlotMetadata, validate_shared_slot_metadata};

pub struct SharedFrameReader {
    pub(super) mapping: MappedRegion,
    pub(super) slot_capacity: usize,
    last_sequence: Option<u64>,
    writer_generation: u64,
}

/// Zero-copy view of one published mmap slot. The slot remains in `READING`
/// state for this value's lifetime and is released automatically on drop.
pub struct BorrowedTransportFrame<'a> {
    pub frame: FrameView<'a>,
    pub sequence: u64,
    pub generation: u64,
    pub monotonic_timestamp_nanos: u64,
    pub timestamp_nanos: u128,
    pub fps: u32,
    _guard: ReadSlotGuard<'a>,
}

impl BorrowedTransportFrame<'_> {
    pub fn into_owned(self) -> Result<TransportFrame, CameraManError> {
        let bytes = self.frame.data().len();
        copy_ledger().record_allocation(CopyStage::SharedMemoryMaterialize, bytes);
        copy_ledger().record_copy(CopyStage::SharedMemoryMaterialize, bytes);
        Ok(TransportFrame {
            frame: self.frame.to_owned_tightly_packed()?,
            sequence: self.sequence,
            generation: self.generation,
            monotonic_timestamp_nanos: self.monotonic_timestamp_nanos,
            timestamp_nanos: self.timestamp_nanos,
            fps: self.fps,
        })
    }
}

impl SharedFrameReader {
    pub fn try_open(name: &str) -> Result<Option<Self>, CameraManError> {
        Self::try_open_endpoint(&SharedFrameEndpoint::Posix(name.to_owned()))
    }

    pub fn try_open_file(path: impl Into<PathBuf>) -> Result<Option<Self>, CameraManError> {
        Self::try_open_endpoint(&SharedFrameEndpoint::File(path.into()))
    }

    pub fn try_open_endpoint(
        endpoint: &SharedFrameEndpoint,
    ) -> Result<Option<Self>, CameraManError> {
        let Some(mapping) = MappedRegion::try_open_reader(endpoint)? else {
            return Ok(None);
        };
        let slot_capacity = usize::try_from(mapping.header().slot_capacity.load(Ordering::Acquire))
            .map_err(|_| CameraManError::BufferTooLarge { bytes: u64::MAX })?;
        let writer_generation = mapping.header().writer_generation.load(Ordering::Acquire);
        Ok(Some(Self {
            mapping,
            slot_capacity,
            last_sequence: None,
            writer_generation,
        }))
    }

    pub fn read_latest(&mut self) -> Result<Option<TransportFrame>, CameraManError> {
        self.read_latest_borrowed()?
            .map(BorrowedTransportFrame::into_owned)
            .transpose()
    }

    pub fn read_latest_borrowed(
        &mut self,
    ) -> Result<Option<BorrowedTransportFrame<'_>>, CameraManError> {
        for _ in 0..SLOT_COUNT {
            let header = self.mapping.header();
            let writer_generation = header.writer_generation.load(Ordering::Acquire);
            if writer_generation == 0 {
                return Ok(None);
            }
            if writer_generation != self.writer_generation {
                self.writer_generation = writer_generation;
                self.last_sequence = None;
            }
            let active = header.active_slot.load(Ordering::Acquire);
            let Ok(slot_index) = usize::try_from(active) else {
                return Ok(None);
            };
            if slot_index >= SLOT_COUNT {
                return Ok(None);
            }
            let slot = &self.mapping.header().slots[slot_index];
            if !try_claim_for_read(&slot.state, std::process::id()) {
                std::hint::spin_loop();
                continue;
            }
            let guard = ReadSlotGuard { slot };

            let sequence = slot.sequence.load(Ordering::Relaxed);
            let generation = slot.generation.load(Ordering::Relaxed);
            if generation != writer_generation {
                std::hint::spin_loop();
                continue;
            }
            if self.last_sequence == Some(sequence) {
                return Ok(None);
            }
            let width = slot.width.load(Ordering::Relaxed);
            let height = slot.height.load(Ordering::Relaxed);
            let _consume_span = stage_span(PipelineStage::Consume, sequence, width, height);
            let pixel_format = slot.pixel_format.load(Ordering::Relaxed);
            let fps = slot.fps.load(Ordering::Relaxed).max(1);
            let color_contract = slot.color_contract.load(Ordering::Relaxed);
            let monotonic_timestamp_nanos = slot.monotonic_timestamp_nanos.load(Ordering::Relaxed);
            let timestamp_nanos = slot.wall_timestamp_nanos.load(Ordering::Relaxed);
            let data_len_u64 = slot.data_len.load(Ordering::Relaxed);
            validate_shared_slot_metadata(
                SharedSlotMetadata {
                    state: slot.state.load(Ordering::Acquire),
                    width,
                    height,
                    pixel_format,
                    fps,
                    color_contract,
                    generation,
                    data_len: data_len_u64,
                },
                self.slot_capacity as u64,
            )
            .map_err(|_| {
                CameraManError::InvalidMediaContract("shared-memory slot metadata is inconsistent")
            })?;
            let data_len =
                usize::try_from(data_len_u64).map_err(|_| CameraManError::BufferTooLarge {
                    bytes: data_len_u64,
                })?;
            // SAFETY: validated metadata bounds `data_len` by slot capacity;
            // `guard` keeps this slot in READING state for the slice lifetime,
            // so the writer cannot mutate or reclaim its mapped bytes.
            let data = unsafe {
                std::slice::from_raw_parts(
                    self.mapping.slot_data_ptr(slot_index, self.slot_capacity),
                    data_len,
                )
            };
            let row_stride = usize::try_from(width)
                .ok()
                .and_then(|width| width.checked_mul(PixelFormat::Bgra8.bytes_per_pixel()))
                .ok_or(CameraManError::BufferTooLarge {
                    bytes: u64::from(width) * 4,
                })?;
            let colorimetry = Colorimetry::from_wire_code(color_contract).ok_or(
                CameraManError::InvalidMediaContract("unknown shared-memory color contract"),
            )?;
            let mut contract = FrameContract::canonical_bgra(width, height);
            contract.colorimetry = colorimetry;
            let frame = FrameView::new_checked_with_contract(
                width,
                height,
                PixelFormat::Bgra8,
                contract,
                row_stride,
                data,
            )?;
            self.last_sequence = Some(sequence);
            // Publishing `consumer_pid` last is the acknowledgement
            // linearization point: generation/sequence/heartbeat become one
            // visible consumer-progress snapshot under its release store.
            header.consumer_pid.store(0, Ordering::Release);
            header
                .consumer_generation
                .store(generation, Ordering::Relaxed);
            header.consumer_sequence.store(sequence, Ordering::Relaxed);
            header
                .consumer_heartbeat_monotonic_nanos
                .store(crate::media_time::monotonic_time_nanos(), Ordering::Relaxed);
            header
                .consumer_pid
                .store(std::process::id(), Ordering::Release);
            return Ok(Some(BorrowedTransportFrame {
                frame,
                sequence,
                generation,
                monotonic_timestamp_nanos,
                timestamp_nanos: u128::from(timestamp_nanos),
                fps,
                _guard: guard,
            }));
        }
        Ok(None)
    }

    pub fn writer_is_alive(&self) -> bool {
        let header = self.mapping.header();
        let pid = header.writer_pid.load(Ordering::Acquire);
        let generation = header.writer_generation.load(Ordering::Acquire);
        let process_start_token = header.writer_process_start_token.load(Ordering::Acquire);
        pid != 0 && generation != 0 && process_matches_start_token(pid, process_start_token)
    }
}

struct ReadSlotGuard<'a> {
    slot: &'a SharedSlot,
}

impl Drop for ReadSlotGuard<'_> {
    fn drop(&mut self) {
        release_read(&self.slot.state);
    }
}
