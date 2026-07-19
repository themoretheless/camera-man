use serde::{Deserialize, Serialize};

use crate::frame::PixelFormat;

/// Identity, timing and format fields that must advance as one source history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameObservation {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub generation: u64,
    pub sequence: u64,
    pub monotonic_timestamp_nanos: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameDiscontinuity {
    #[default]
    None,
    WriterRestarted,
    FramesDropped,
    FormatChanged,
    TimingReset,
    BecameStale,
    Resumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityDropReason {
    SourceUnavailable,
    DecodeFailed,
    TransportReadFailed,
    Stale,
    ConsumerBackpressure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameIntegritySnapshot {
    pub source_id: String,
    pub last_observation: Option<FrameObservation>,
    pub last_discontinuity: FrameDiscontinuity,
    pub last_drop_reason: Option<IntegrityDropReason>,
    pub inferred_dropped_frames: u64,
    pub writer_restarts: u64,
    pub stale: bool,
}

/// Runtime-owned integrity history for exactly one source.
#[derive(Debug, Clone)]
pub struct FrameIntegrityState {
    source_id: String,
    last_observation: Option<FrameObservation>,
    last_discontinuity: FrameDiscontinuity,
    last_drop_reason: Option<IntegrityDropReason>,
    inferred_dropped_frames: u64,
    writer_restarts: u64,
    stale: bool,
}

impl FrameIntegrityState {
    pub fn new(source_id: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            last_observation: None,
            last_discontinuity: FrameDiscontinuity::None,
            last_drop_reason: None,
            inferred_dropped_frames: 0,
            writer_restarts: 0,
            stale: false,
        }
    }

    pub fn observe(&mut self, current: FrameObservation) -> FrameDiscontinuity {
        let discontinuity =
            classify_frame_integrity(self.last_observation.as_ref(), &current, self.stale);
        if discontinuity == FrameDiscontinuity::WriterRestarted {
            self.writer_restarts = self.writer_restarts.saturating_add(1);
        }
        if discontinuity == FrameDiscontinuity::FramesDropped
            && let Some(previous) = self.last_observation
        {
            let expected = previous.sequence.wrapping_add(1);
            self.inferred_dropped_frames = self
                .inferred_dropped_frames
                .saturating_add(current.sequence.wrapping_sub(expected).max(1));
        }
        self.last_observation = Some(current);
        self.last_discontinuity = discontinuity;
        self.last_drop_reason = None;
        self.stale = false;
        discontinuity
    }

    pub fn mark_drop(&mut self, reason: IntegrityDropReason, count: u64) {
        self.last_drop_reason = Some(reason);
        self.inferred_dropped_frames = self.inferred_dropped_frames.saturating_add(count);
    }

    pub fn mark_stale(&mut self) -> FrameDiscontinuity {
        self.mark_drop(IntegrityDropReason::Stale, 1);
        if self.stale {
            return FrameDiscontinuity::None;
        }
        self.stale = true;
        self.last_discontinuity = FrameDiscontinuity::BecameStale;
        FrameDiscontinuity::BecameStale
    }

    pub fn snapshot(&self) -> FrameIntegritySnapshot {
        FrameIntegritySnapshot {
            source_id: self.source_id.clone(),
            last_observation: self.last_observation,
            last_discontinuity: self.last_discontinuity,
            last_drop_reason: self.last_drop_reason,
            inferred_dropped_frames: self.inferred_dropped_frames,
            writer_restarts: self.writer_restarts,
            stale: self.stale,
        }
    }
}

pub fn classify_frame_integrity(
    previous: Option<&FrameObservation>,
    current: &FrameObservation,
    was_stale: bool,
) -> FrameDiscontinuity {
    let Some(previous) = previous else {
        return if was_stale {
            FrameDiscontinuity::Resumed
        } else {
            FrameDiscontinuity::None
        };
    };
    if previous.generation != current.generation {
        return FrameDiscontinuity::WriterRestarted;
    }
    if previous.width != current.width
        || previous.height != current.height
        || previous.pixel_format != current.pixel_format
    {
        return FrameDiscontinuity::FormatChanged;
    }
    if current.monotonic_timestamp_nanos < previous.monotonic_timestamp_nanos {
        return FrameDiscontinuity::TimingReset;
    }
    if current.sequence != previous.sequence.wrapping_add(1) {
        return FrameDiscontinuity::FramesDropped;
    }
    if was_stale {
        return FrameDiscontinuity::Resumed;
    }
    FrameDiscontinuity::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(generation: u64, sequence: u64, timestamp: u64) -> FrameObservation {
        FrameObservation {
            width: 1_920,
            height: 1_080,
            pixel_format: PixelFormat::Bgra8,
            generation,
            sequence,
            monotonic_timestamp_nanos: timestamp,
        }
    }

    #[test]
    fn one_state_links_restart_sequence_timing_stale_and_drop_reason() {
        let mut state = FrameIntegrityState::new("camera-1");
        assert_eq!(
            state.observe(observation(1, 4, 100)),
            FrameDiscontinuity::None
        );
        assert_eq!(
            state.observe(observation(1, 7, 200)),
            FrameDiscontinuity::FramesDropped
        );
        assert_eq!(state.mark_stale(), FrameDiscontinuity::BecameStale);
        assert_eq!(
            state.observe(observation(2, 0, 300)),
            FrameDiscontinuity::WriterRestarted
        );

        let snapshot = state.snapshot();
        assert_eq!(snapshot.source_id, "camera-1");
        assert_eq!(snapshot.writer_restarts, 1);
        assert_eq!(snapshot.inferred_dropped_frames, 3);
        assert_eq!(snapshot.last_observation.unwrap().generation, 2);
    }

    #[test]
    fn same_generation_timestamp_regression_is_not_a_restart() {
        let previous = observation(7, 10, 200);
        let current = observation(7, 11, 100);
        assert_eq!(
            classify_frame_integrity(Some(&previous), &current, false),
            FrameDiscontinuity::TimingReset
        );
    }
}
