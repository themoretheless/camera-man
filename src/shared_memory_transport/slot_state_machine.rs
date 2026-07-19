//! Atomic ownership model for one latest-frame slot.
//!
//! Linearization points are deliberately small and testable: a successful
//! `FREE|READY -> WRITING` CAS owns bytes for the producer; the release store
//! in `publish_ready` makes metadata/pixels visible; a successful
//! `READY -> READING(pid)` CAS owns immutable bytes for one reader; and the
//! release store in `release_read` acknowledges that the lease ended. Writer
//! generation and process-start token form a separate history so PID reuse
//! cannot inherit ownership from a dead process.

use std::sync::atomic::{AtomicU64, Ordering};

pub const SLOT_FREE: u64 = 0;
pub const SLOT_READY: u64 = 1;
pub const SLOT_READING: u64 = 2;
pub const SLOT_WRITING: u64 = 3;
const SLOT_STATE_MASK: u64 = 0xff;

/// Minimal atomic surface shared by production atomics and Loom models.
#[doc(hidden)]
pub trait AtomicSlotState {
    fn load_state(&self, ordering: Ordering) -> u64;
    fn compare_exchange_state(
        &self,
        current: u64,
        new: u64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u64, u64>;
    fn store_state(&self, value: u64, ordering: Ordering);
}

impl AtomicSlotState for AtomicU64 {
    fn load_state(&self, ordering: Ordering) -> u64 {
        self.load(ordering)
    }

    fn compare_exchange_state(
        &self,
        current: u64,
        new: u64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u64, u64> {
        self.compare_exchange(current, new, success, failure)
    }

    fn store_state(&self, value: u64, ordering: Ordering) {
        self.store(value, ordering);
    }
}

pub const fn reading_slot_state(pid: u32) -> u64 {
    (pid as u64) << 32 | SLOT_READING
}

pub const fn slot_state(state: u64) -> u64 {
    state & SLOT_STATE_MASK
}

pub const fn slot_owner(state: u64) -> u32 {
    (state >> 32) as u32
}

pub const fn is_known_state(state: u64) -> bool {
    matches!(
        slot_state(state),
        SLOT_FREE | SLOT_READY | SLOT_READING | SLOT_WRITING
    )
}

pub fn try_claim_for_write<A, F>(state: &A, reader_is_alive: F) -> bool
where
    A: AtomicSlotState,
    F: FnOnce(u32) -> bool,
{
    let mut current = state.load_state(Ordering::Acquire);
    if slot_state(current) == SLOT_READING && !reader_is_alive(slot_owner(current)) {
        if state
            .compare_exchange_state(current, SLOT_FREE, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        current = SLOT_FREE;
    }
    if !matches!(slot_state(current), SLOT_FREE | SLOT_READY) {
        return false;
    }
    state
        .compare_exchange_state(current, SLOT_WRITING, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

/// Linearizes publication with one release store after every payload byte and
/// relaxed metadata field has been written.
pub fn publish_ready<A: AtomicSlotState>(state: &A) {
    state.store_state(SLOT_READY, Ordering::Release);
}

/// Linearizes the read lease at the successful ownership CAS.
pub fn try_claim_for_read<A: AtomicSlotState>(state: &A, pid: u32) -> bool {
    state
        .compare_exchange_state(
            SLOT_READY,
            reading_slot_state(pid),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
}

/// Linearizes acknowledgement at the release store back to `READY`.
pub fn release_read<A: AtomicSlotState>(state: &A) {
    state.store_state(SLOT_READY, Ordering::Release);
}

pub fn reset_for_new_writer<A, F>(state: &A, reader_is_alive: F)
where
    A: AtomicSlotState,
    F: Fn(u32) -> bool,
{
    loop {
        let current = state.load_state(Ordering::Acquire);
        match slot_state(current) {
            SLOT_FREE => return,
            SLOT_READING if reader_is_alive(slot_owner(current)) => return,
            SLOT_READY | SLOT_READING | SLOT_WRITING => {
                if state
                    .compare_exchange_state(current, SLOT_FREE, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return;
                }
            }
            _ => {
                state.store_state(SLOT_FREE, Ordering::Release);
                return;
            }
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn encoded_reader_state_round_trips() {
        let pid = kani::any::<u32>();
        let encoded = reading_slot_state(pid);
        assert_eq!(slot_state(encoded), SLOT_READING);
        assert_eq!(slot_owner(encoded), pid);
    }
}
