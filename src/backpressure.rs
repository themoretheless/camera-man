use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlowConsumerPolicy {
    KeepLatest,
    DropOldest,
    Disconnect,
    BlockUntilDeadline,
    DegradeQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelBackpressureContract {
    pub channel: &'static str,
    pub policy: SlowConsumerPolicy,
    pub capacity: usize,
}

impl ChannelBackpressureContract {
    pub const fn validate(self) -> bool {
        self.capacity > 0
    }
}

pub const RENDER_REQUESTS: ChannelBackpressureContract = ChannelBackpressureContract {
    channel: "ui_to_render_worker",
    policy: SlowConsumerPolicy::KeepLatest,
    capacity: 1,
};

pub const SHARED_FRAME_SLOTS: ChannelBackpressureContract = ChannelBackpressureContract {
    channel: "app_to_cmio_extension",
    policy: SlowConsumerPolicy::KeepLatest,
    capacity: 3,
};

pub const CAMERA_DISCOVERY_RESULTS: ChannelBackpressureContract = ChannelBackpressureContract {
    channel: "camera_discovery_to_ui",
    policy: SlowConsumerPolicy::Disconnect,
    capacity: 1,
};

/// Realized as the capture worker's single `latest` frame slot, not as a
/// channel: one slot, newest write wins, readers never block the worker.
pub const CAPTURE_RESULTS: ChannelBackpressureContract = ChannelBackpressureContract {
    channel: "camera_capture_to_ui",
    policy: SlowConsumerPolicy::KeepLatest,
    capacity: 1,
};

pub const IO_RESULTS: ChannelBackpressureContract = ChannelBackpressureContract {
    channel: "file_io_to_ui",
    policy: SlowConsumerPolicy::Disconnect,
    capacity: 1,
};

pub const LOCAL_CHANNELS: [ChannelBackpressureContract; 5] = [
    RENDER_REQUESTS,
    SHARED_FRAME_SLOTS,
    CAMERA_DISCOVERY_RESULTS,
    CAPTURE_RESULTS,
    IO_RESULTS,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_runtime_channel_has_a_bounded_explicit_policy() {
        assert!(
            LOCAL_CHANNELS
                .into_iter()
                .all(ChannelBackpressureContract::validate)
        );
        assert_eq!(RENDER_REQUESTS.policy, SlowConsumerPolicy::KeepLatest);
        assert_eq!(SHARED_FRAME_SLOTS.capacity, 3);
    }
}
