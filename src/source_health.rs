use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessHealth {
    NotRunning,
    Waiting,
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropHealth {
    Unknown,
    Clear,
    Recent { count: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JitterHealth {
    Unknown,
    WithinBudget,
    OverBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconnectHealth {
    NotRequired,
    Connected,
    Reconnecting,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatHealth {
    Unknown,
    Negotiated,
    Mismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerAckHealth {
    NotRequired,
    Waiting,
    Current,
    Lagging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceHealthSummary {
    Ready,
    Waiting,
    Retry,
    Missing,
    Off,
}

/// Typed health dimensions. The UI may render a compact summary, but it must
/// retain every component for diagnostics and accessible descriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHealthVector {
    pub freshness: FreshnessHealth,
    pub drops: DropHealth,
    pub jitter: JitterHealth,
    pub reconnect: ReconnectHealth,
    pub format: FormatHealth,
    pub consumer_ack: ConsumerAckHealth,
}

impl SourceHealthVector {
    pub const fn synthetic(running: bool) -> Self {
        Self {
            freshness: if running {
                FreshnessHealth::Fresh
            } else {
                FreshnessHealth::NotRunning
            },
            drops: DropHealth::Clear,
            jitter: JitterHealth::WithinBudget,
            reconnect: ReconnectHealth::NotRequired,
            format: FormatHealth::Negotiated,
            consumer_ack: ConsumerAckHealth::NotRequired,
        }
    }

    pub const fn off() -> Self {
        Self {
            freshness: FreshnessHealth::NotRunning,
            drops: DropHealth::Unknown,
            jitter: JitterHealth::Unknown,
            reconnect: ReconnectHealth::NotRequired,
            format: FormatHealth::Unknown,
            consumer_ack: ConsumerAckHealth::NotRequired,
        }
    }

    pub const fn waiting() -> Self {
        Self {
            freshness: FreshnessHealth::Waiting,
            drops: DropHealth::Unknown,
            jitter: JitterHealth::Unknown,
            reconnect: ReconnectHealth::Connected,
            format: FormatHealth::Unknown,
            consumer_ack: ConsumerAckHealth::NotRequired,
        }
    }

    pub const fn connected() -> Self {
        Self {
            freshness: FreshnessHealth::Fresh,
            drops: DropHealth::Clear,
            jitter: JitterHealth::WithinBudget,
            reconnect: ReconnectHealth::Connected,
            format: FormatHealth::Negotiated,
            consumer_ack: ConsumerAckHealth::NotRequired,
        }
    }

    pub const fn retrying(failures: u64) -> Self {
        Self {
            freshness: FreshnessHealth::Stale,
            drops: DropHealth::Recent { count: failures },
            jitter: JitterHealth::Unknown,
            reconnect: ReconnectHealth::Reconnecting,
            format: FormatHealth::Unknown,
            consumer_ack: ConsumerAckHealth::NotRequired,
        }
    }

    pub const fn missing() -> Self {
        Self {
            freshness: FreshnessHealth::Stale,
            drops: DropHealth::Unknown,
            jitter: JitterHealth::Unknown,
            reconnect: ReconnectHealth::Missing,
            format: FormatHealth::Unknown,
            consumer_ack: ConsumerAckHealth::NotRequired,
        }
    }

    pub const fn summary(self) -> SourceHealthSummary {
        if matches!(self.reconnect, ReconnectHealth::Missing) {
            return SourceHealthSummary::Missing;
        }
        if matches!(self.reconnect, ReconnectHealth::Reconnecting)
            || matches!(self.format, FormatHealth::Mismatch)
            || matches!(self.freshness, FreshnessHealth::Stale)
        {
            return SourceHealthSummary::Retry;
        }
        if matches!(self.freshness, FreshnessHealth::NotRunning) {
            return SourceHealthSummary::Off;
        }
        if matches!(self.freshness, FreshnessHealth::Waiting)
            || matches!(self.drops, DropHealth::Recent { .. })
            || matches!(self.jitter, JitterHealth::OverBudget)
            || matches!(
                self.consumer_ack,
                ConsumerAckHealth::Waiting | ConsumerAckHealth::Lagging
            )
        {
            return SourceHealthSummary::Waiting;
        }
        SourceHealthSummary::Ready
    }

    pub fn description(self) -> String {
        format!(
            "freshness={:?}, drops={:?}, jitter={:?}, reconnect={:?}, format={:?}, consumer_ack={:?}",
            self.freshness, self.drops, self.jitter, self.reconnect, self.format, self.consumer_ack
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_never_discards_the_typed_health_vector() {
        let health = SourceHealthVector {
            freshness: FreshnessHealth::Fresh,
            drops: DropHealth::Recent { count: 2 },
            jitter: JitterHealth::OverBudget,
            reconnect: ReconnectHealth::Connected,
            format: FormatHealth::Negotiated,
            consumer_ack: ConsumerAckHealth::Lagging,
        };
        assert_eq!(health.summary(), SourceHealthSummary::Waiting);
        let description = health.description();
        assert!(description.contains("drops=Recent { count: 2 }"));
        assert!(description.contains("consumer_ack=Lagging"));
    }

    #[test]
    fn recent_drops_or_excess_jitter_degrade_an_otherwise_ready_source() {
        let mut health = SourceHealthVector::connected();
        health.drops = DropHealth::Recent { count: 1 };
        assert_eq!(health.summary(), SourceHealthSummary::Waiting);

        health.drops = DropHealth::Clear;
        health.jitter = JitterHealth::OverBudget;
        assert_eq!(health.summary(), SourceHealthSummary::Waiting);
    }
}
