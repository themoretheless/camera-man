use std::path::PathBuf;
use std::sync::atomic::Ordering;

use crate::config::VIRTUAL_CAMERA_DEFAULT_FPS;
use crate::diagnostics::{DropReason, PipelineStage, record_drop, record_output_frame, stage_span};
use crate::error::CameraManError;
use crate::frame::{Frame, PixelFormat};
use crate::media_time::{monotonic_time_nanos, wall_time_nanos};
use crate::performance::{CopyStage, copy_ledger};
use crate::virtual_camera::VirtualCameraSink;

use super::mapped_backend::process_is_alive;
use super::mapped_backend::{MappedRegion, SharedFrameEndpoint, default_shared_frame_endpoint};
use super::protocol::{DEFAULT_SLOT_CAPACITY, PIXEL_FORMAT_BGRA8, SLOT_COUNT};
use super::slot_state_machine::{publish_ready, try_claim_for_write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProducerProgress {
    pub generation: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumerProgress {
    pub pid: u32,
    pub generation: u64,
    pub sequence: u64,
    pub heartbeat_monotonic_nanos: u64,
}

pub struct SharedFrameSink {
    endpoint: SharedFrameEndpoint,
    slot_capacity: usize,
    pub(super) mapping: Option<MappedRegion>,
    target_fps: u32,
    next_slot: usize,
}

impl Default for SharedFrameSink {
    fn default() -> Self {
        Self::from_endpoint(default_shared_frame_endpoint(), DEFAULT_SLOT_CAPACITY)
    }
}

impl SharedFrameSink {
    pub fn new(name: impl Into<String>, slot_capacity: usize) -> Self {
        Self::from_endpoint(SharedFrameEndpoint::Posix(name.into()), slot_capacity)
    }

    pub fn new_file(path: impl Into<PathBuf>, slot_capacity: usize) -> Self {
        Self::from_endpoint(SharedFrameEndpoint::File(path.into()), slot_capacity)
    }

    pub fn from_endpoint(endpoint: SharedFrameEndpoint, slot_capacity: usize) -> Self {
        debug_assert_eq!(crate::backpressure::SHARED_FRAME_SLOTS.capacity, SLOT_COUNT);
        Self {
            endpoint,
            slot_capacity,
            mapping: None,
            target_fps: VIRTUAL_CAMERA_DEFAULT_FPS,
            next_slot: 0,
        }
    }

    pub fn endpoint(&self) -> &SharedFrameEndpoint {
        &self.endpoint
    }

    pub fn description(&self) -> String {
        self.endpoint.description()
    }

    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps.max(1);
    }

    pub const fn target_fps(&self) -> u32 {
        self.target_fps
    }

    pub fn producer_progress(&self) -> Option<ProducerProgress> {
        let header = self.mapping.as_ref()?.header();
        let generation = header.writer_generation.load(Ordering::Acquire);
        let sequence = header
            .next_sequence
            .load(Ordering::Acquire)
            .checked_sub(1)?;
        (generation != 0).then_some(ProducerProgress {
            generation,
            sequence,
        })
    }

    pub fn consumer_progress(&self) -> Option<ConsumerProgress> {
        let header = self.mapping.as_ref()?.header();
        let pid = header.consumer_pid.load(Ordering::Acquire);
        if pid == 0 {
            return None;
        }
        let progress = ConsumerProgress {
            pid,
            generation: header.consumer_generation.load(Ordering::Relaxed),
            sequence: header.consumer_sequence.load(Ordering::Relaxed),
            heartbeat_monotonic_nanos: header
                .consumer_heartbeat_monotonic_nanos
                .load(Ordering::Relaxed),
        };
        (header.consumer_pid.load(Ordering::Acquire) == pid).then_some(progress)
    }

    fn claim_slot(&mut self) -> Option<usize> {
        let mapping = self.mapping.as_ref()?;
        for offset in 0..SLOT_COUNT {
            let index = (self.next_slot + offset) % SLOT_COUNT;
            let state = &mapping.header().slots[index].state;
            if try_claim_for_write(state, process_is_alive) {
                self.next_slot = (index + 1) % SLOT_COUNT;
                return Some(index);
            }
        }
        None
    }
}

impl VirtualCameraSink for SharedFrameSink {
    fn connect(&mut self) -> Result<(), CameraManError> {
        if self.mapping.is_none() {
            self.mapping = Some(MappedRegion::create_writer(
                &self.endpoint,
                self.slot_capacity,
            )?);
        }
        Ok(())
    }

    fn send(&mut self, frame: &Frame) -> Result<(), CameraManError> {
        if self.mapping.is_none() {
            return Err(CameraManError::VirtualCameraUnavailable(
                "shared-memory sink is not connected",
            ));
        }
        if frame.pixel_format() != PixelFormat::Bgra8 {
            return Err(CameraManError::UnsupportedPixelFormat);
        }
        if !frame
            .contract()
            .is_canonical_bgra(frame.width(), frame.height())
        {
            return Err(CameraManError::InvalidMediaContract(
                "transport accepts only compositor-normalized frames",
            ));
        }
        if frame.data().len() > self.slot_capacity {
            return Err(CameraManError::BufferTooLarge {
                bytes: frame.data().len() as u64,
            });
        }
        let Some(slot_index) = self.claim_slot() else {
            record_drop(DropReason::AllSlotsBusy, 1, None);
            return Err(CameraManError::VirtualCameraUnavailable(
                "all shared-memory frame slots are busy",
            ));
        };
        let mapping = self
            .mapping
            .as_ref()
            .ok_or(CameraManError::VirtualCameraUnavailable(
                "shared-memory sink is not connected",
            ))?;
        let header = mapping.header();
        let slot = &header.slots[slot_index];
        let sequence = header.next_sequence.fetch_add(1, Ordering::Relaxed);
        let _publish_span = stage_span(
            PipelineStage::Publish,
            sequence,
            frame.width(),
            frame.height(),
        );
        let generation = header.writer_generation.load(Ordering::Acquire);
        let monotonic_timestamp_nanos = monotonic_time_nanos();
        let wall_timestamp_nanos = wall_time_nanos().min(u128::from(u64::MAX)) as u64;

        slot.width.store(frame.width(), Ordering::Relaxed);
        slot.height.store(frame.height(), Ordering::Relaxed);
        slot.pixel_format
            .store(PIXEL_FORMAT_BGRA8, Ordering::Relaxed);
        slot.fps.store(self.target_fps, Ordering::Relaxed);
        slot.color_contract
            .store(frame.contract().colorimetry.wire_code(), Ordering::Relaxed);
        slot.sequence.store(sequence, Ordering::Relaxed);
        slot.generation.store(generation, Ordering::Relaxed);
        slot.monotonic_timestamp_nanos
            .store(monotonic_timestamp_nanos, Ordering::Relaxed);
        slot.wall_timestamp_nanos
            .store(wall_timestamp_nanos, Ordering::Relaxed);
        slot.data_len
            .store(frame.data().len() as u64, Ordering::Relaxed);
        // SAFETY: `connect` sized every slot to `slot_capacity`, the frame
        // length was checked against that capacity, and WRITING ownership
        // excludes readers until `publish_ready` performs the release.
        unsafe {
            std::ptr::copy_nonoverlapping(
                frame.data().as_ptr(),
                mapping.slot_data_ptr(slot_index, self.slot_capacity),
                frame.data().len(),
            );
        }
        copy_ledger().record_copy(CopyStage::SharedMemoryPublish, frame.data().len());

        publish_ready(&slot.state);
        // The active-slot release is the latest-frame selection point. A
        // reader that acquires this index can then acquire the READY lease and
        // observe the payload published immediately above.
        header
            .active_slot
            .store(slot_index as u32, Ordering::Release);
        header
            .writer_heartbeat_monotonic_nanos
            .store(monotonic_timestamp_nanos, Ordering::Release);
        record_output_frame();
        Ok(())
    }

    fn disconnect(&mut self) {
        let Some(mapping) = self.mapping.take() else {
            return;
        };
        drop(mapping);
        self.endpoint.cleanup();
    }
}

impl Drop for SharedFrameSink {
    fn drop(&mut self) {
        self.disconnect();
    }
}
