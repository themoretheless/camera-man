use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MonotonicTimestampNanos(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WallTimestampNanos(pub u128);

/// Presentation time in a media-specific timescale. Unlike monotonic time it
/// may be rebased by a stream, and unlike wall time it has no calendar meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaTimestamp {
    pub value: i64,
    pub timescale: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTimestamps {
    pub media: Option<MediaTimestamp>,
    pub monotonic: MonotonicTimestampNanos,
    pub wall: WallTimestampNanos,
}

impl CaptureTimestamps {
    pub fn now(clock: &impl Clock) -> Self {
        Self {
            media: None,
            monotonic: MonotonicTimestampNanos(clock.monotonic_nanos()),
            wall: WallTimestampNanos(clock.wall_time_nanos()),
        }
    }
}

/// Time source used by transport and scheduling code.
///
/// Monotonic time is comparable between processes on the same boot. Wall time
/// is retained only for logs and persisted metadata; it must never drive frame
/// pacing or stale-frame decisions.
pub trait Clock: Send + Sync {
    fn monotonic_nanos(&self) -> u64;
    fn wall_time_nanos(&self) -> u128;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn monotonic_nanos(&self) -> u64 {
        monotonic_time_nanos()
    }

    fn wall_time_nanos(&self) -> u128 {
        wall_time_nanos()
    }
}

/// Returns nanoseconds from the operating system's boot-relative monotonic
/// clock. `clock_gettime` gives all local processes the same epoch, unlike a
/// process-local `Instant` origin.
#[cfg(unix)]
pub fn monotonic_time_nanos() -> u64 {
    let mut timestamp = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: `timestamp` points to writable storage for one `timespec` and
    // remains alive for the duration of the call.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, timestamp.as_mut_ptr()) };
    if status == 0 {
        // SAFETY: a zero return from `clock_gettime` guarantees that the
        // complete `timespec` output was initialized.
        let timestamp = unsafe { timestamp.assume_init() };
        let seconds = u64::try_from(timestamp.tv_sec).unwrap_or_default();
        let nanos = u64::try_from(timestamp.tv_nsec).unwrap_or_default();
        return seconds.saturating_mul(1_000_000_000).saturating_add(nanos);
    }
    process_monotonic_time_nanos()
}

#[cfg(not(unix))]
pub fn monotonic_time_nanos() -> u64 {
    process_monotonic_time_nanos()
}

pub fn wall_time_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

/// Creates a non-zero per-writer generation identifier without introducing a
/// random-number dependency into the transport hot path. It is an identity
/// nonce, not a cryptographic secret.
pub fn new_generation_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);

    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let wall = wall_time_nanos() as u64;
    let pid = u64::from(std::process::id());
    let seed = monotonic_time_nanos()
        ^ wall.rotate_left(17)
        ^ pid.rotate_left(32)
        ^ counter.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    splitmix64(seed).max(1)
}

fn process_monotonic_time_nanos() -> u64 {
    static ORIGIN: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(Instant::now)
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_clock_does_not_move_backwards() {
        let first = monotonic_time_nanos();
        let second = monotonic_time_nanos();
        assert!(second >= first);
    }

    #[test]
    fn generation_ids_are_non_zero_and_change() {
        let first = new_generation_id();
        let second = new_generation_id();
        assert_ne!(first, 0);
        assert_ne!(first, second);
    }

    #[test]
    fn capture_timestamps_keep_three_clock_domains_distinct() {
        struct FixedClock;
        impl Clock for FixedClock {
            fn monotonic_nanos(&self) -> u64 {
                10
            }

            fn wall_time_nanos(&self) -> u128 {
                20
            }
        }

        let timestamps = CaptureTimestamps::now(&FixedClock);
        assert_eq!(timestamps.monotonic, MonotonicTimestampNanos(10));
        assert_eq!(timestamps.wall, WallTimestampNanos(20));
        assert_eq!(timestamps.media, None);
    }
}
