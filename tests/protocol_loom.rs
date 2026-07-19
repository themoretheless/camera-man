use std::sync::atomic::Ordering;

use camera_man::shared_memory_transport::slot_state_machine::{
    AtomicSlotState, SLOT_FREE, SLOT_READING, SLOT_READY, publish_ready, reading_slot_state,
    release_read, reset_for_new_writer, slot_state, try_claim_for_read, try_claim_for_write,
};
use loom::sync::Arc;
use loom::sync::atomic::{AtomicU64, AtomicUsize};
use loom::thread;

struct LoomSlotState(AtomicU64);

impl AtomicSlotState for LoomSlotState {
    fn load_state(&self, ordering: Ordering) -> u64 {
        self.0.load(ordering)
    }

    fn compare_exchange_state(
        &self,
        current: u64,
        new: u64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u64, u64> {
        self.0.compare_exchange(current, new, success, failure)
    }

    fn store_state(&self, value: u64, ordering: Ordering) {
        self.0.store(value, ordering);
    }
}

#[test]
fn reader_and_writer_never_overlap_the_payload() {
    loom::model(|| {
        let state = Arc::new(LoomSlotState(AtomicU64::new(SLOT_READY)));
        let first = Arc::new(AtomicUsize::new(1));
        let second = Arc::new(AtomicUsize::new(1));

        let writer_state = Arc::clone(&state);
        let writer_first = Arc::clone(&first);
        let writer_second = Arc::clone(&second);
        let writer = thread::spawn(move || {
            if try_claim_for_write(writer_state.as_ref(), |_| true) {
                writer_first.store(2, Ordering::Relaxed);
                thread::yield_now();
                writer_second.store(2, Ordering::Relaxed);
                publish_ready(writer_state.as_ref());
            }
        });

        let reader_state = Arc::clone(&state);
        let reader_first = Arc::clone(&first);
        let reader_second = Arc::clone(&second);
        let reader = thread::spawn(move || {
            if try_claim_for_read(reader_state.as_ref(), 7) {
                let observed_first = reader_first.load(Ordering::Relaxed);
                thread::yield_now();
                let observed_second = reader_second.load(Ordering::Relaxed);
                assert_eq!(observed_first, observed_second);
                release_read(reader_state.as_ref());
            }
        });

        writer.join().unwrap();
        reader.join().unwrap();
    });
}

#[test]
fn only_one_writer_can_claim_a_ready_slot() {
    loom::model(|| {
        let state = Arc::new(LoomSlotState(AtomicU64::new(SLOT_READY)));
        let winners = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();
        for _ in 0..2 {
            let state = Arc::clone(&state);
            let winners = Arc::clone(&winners);
            workers.push(thread::spawn(move || {
                if try_claim_for_write(state.as_ref(), |_| true) {
                    winners.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(winners.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn restart_preserves_live_reader_but_reclaims_dead_reader_history() {
    loom::model(|| {
        let live = LoomSlotState(AtomicU64::new(reading_slot_state(7)));
        reset_for_new_writer(&live, |pid| pid == 7);
        assert_eq!(slot_state(live.load_state(Ordering::Acquire)), SLOT_READING);

        let dead = LoomSlotState(AtomicU64::new(reading_slot_state(7)));
        reset_for_new_writer(&dead, |_| false);
        assert_eq!(dead.load_state(Ordering::Acquire), SLOT_FREE);
    });
}
