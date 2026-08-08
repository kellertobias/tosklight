//! Tempo sources.
//!
//! Each output has exactly one explicit tempo source. There is no implicit priority race between
//! a desk Speed Group and the per-layer Playback BPM channel: selecting one disables the other
//! for that output.

use serde::{Deserialize, Serialize};

/// Identity of a Light desk Speed Group. Light is the authority; Media consumes the group's
/// published BPM, phase, observation time, and freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpeedGroupId(u32);

impl SpeedGroupId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for SpeedGroupId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Where an output's synchronized playback modes get their clock.
///
/// This is an output-scoped application setting, never another layer DMX channel: a desk selects
/// the group once for the output rather than spending a slot per layer on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum TempoSource {
    /// Follow a Light desk Speed Group. When the group goes stale the output holds the last
    /// clock and warns; it never silently falls back to the channel.
    SpeedGroup { group_id: SpeedGroupId },
    /// Follow each layer's own Playback BPM channel. Zero on the wire means off.
    #[default]
    PlaybackBpmChannel,
}

impl TempoSource {
    /// The Speed Group selector is only meaningful — and only shown in settings — in Speed Group
    /// mode.
    pub const fn speed_group(self) -> Option<SpeedGroupId> {
        match self {
            Self::SpeedGroup { group_id } => Some(group_id),
            Self::PlaybackBpmChannel => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_channel_source_exposes_no_speed_group() {
        assert_eq!(TempoSource::default(), TempoSource::PlaybackBpmChannel);
        assert_eq!(TempoSource::PlaybackBpmChannel.speed_group(), None);
    }

    #[test]
    fn a_selected_group_is_readable() {
        let source = TempoSource::SpeedGroup {
            group_id: SpeedGroupId::new(3),
        };
        assert_eq!(source.speed_group().map(SpeedGroupId::value), Some(3));
    }
}
