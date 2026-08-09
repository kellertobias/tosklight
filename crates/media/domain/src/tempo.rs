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

/// A Speed Group observation, as the Light desk publishes it.
///
/// Light is the authority; Media consumes. The snapshot carries enough to reconstruct the clock
/// locally — identity, tempo, where in the bar it was, and when that was observed — so a
/// consumer can interpolate between updates instead of stepping on every packet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedGroupSnapshot {
    pub group_id: SpeedGroupId,
    pub bpm: f64,
    /// Position within the bar at [`Self::observed_at`], in beats.
    pub phase_beats: f64,
    pub observed_at: crate::command::Timestamp,
}

/// How long a Speed Group observation stays live.
///
/// Past this the group is stale: the output holds the last clock and warns. It never silently
/// falls back to the channel, because a show that quietly changed tempo source mid-cue is worse
/// than one that visibly lost its master clock.
pub const SPEED_GROUP_FRESHNESS: std::time::Duration = std::time::Duration::from_millis(1_500);

impl SpeedGroupSnapshot {
    /// Whether this observation is still live at `now`.
    pub fn is_fresh(&self, now: crate::command::Timestamp) -> bool {
        now.since(self.observed_at) < SPEED_GROUP_FRESHNESS
    }
}

/// The tempo an output is following, and whether it is trustworthy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ResolvedTempo {
    /// No tempo at all: the channel is at zero, or no Speed Group has ever been seen.
    None,
    /// A live tempo.
    Live { bpm: f64 },
    /// The last tempo a Speed Group published, held past its freshness. The output keeps running
    /// at this rate and warns.
    Stale { bpm: f64 },
}

impl ResolvedTempo {
    pub const fn bpm(self) -> Option<f64> {
        match self {
            Self::None => None,
            Self::Live { bpm } | Self::Stale { bpm } => Some(bpm),
        }
    }

    pub const fn is_stale(self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

/// Resolves what tempo a layer should follow.
///
/// The output's tempo source decides which input is even consulted: selecting the channel
/// disables Speed Group consumption entirely, and selecting a group makes it authoritative. There
/// is no priority race between them.
pub fn resolve_tempo(
    source: TempoSource,
    snapshot: Option<SpeedGroupSnapshot>,
    channel_bpm: Option<u8>,
    now: crate::command::Timestamp,
) -> ResolvedTempo {
    match source {
        TempoSource::PlaybackBpmChannel => match channel_bpm {
            Some(bpm) => ResolvedTempo::Live {
                bpm: f64::from(bpm),
            },
            None => ResolvedTempo::None,
        },
        TempoSource::SpeedGroup { group_id } => match snapshot {
            // A snapshot for a different group is not this output's clock.
            Some(snapshot) if snapshot.group_id == group_id => {
                if snapshot.is_fresh(now) {
                    ResolvedTempo::Live { bpm: snapshot.bpm }
                } else {
                    ResolvedTempo::Stale { bpm: snapshot.bpm }
                }
            }
            _ => ResolvedTempo::None,
        },
    }
}

/// The effective playback rate, as a positive magnitude.
///
/// Unsynchronized modes ignore both the intrinsic BPM and the tempo source entirely:
///
/// ```text
/// effectiveRate = speedMultiplier
/// ```
///
/// Synchronized playback with an intrinsic BPM retimes the asset to the target:
///
/// ```text
/// effectiveRate = targetBpm / intrinsicBpm × speedMultiplier
/// ```
///
/// Synchronized playback *without* an intrinsic BPM treats the incoming target as the reference,
/// so the ratio is exactly one and the multiplier is the only speed change. This is not an error
/// and must never be reported as a missing intrinsic BPM.
pub fn effective_rate(
    synchronized: bool,
    intrinsic_bpm: Option<f64>,
    tempo: ResolvedTempo,
    speed_multiplier: f64,
) -> f64 {
    let multiplier = speed_multiplier.max(0.0);
    if !synchronized {
        return multiplier;
    }
    match (tempo.bpm(), intrinsic_bpm) {
        // A zero or negative intrinsic tempo is not a ratio; treat the asset as unauthored rather
        // than dividing by it.
        (Some(target), Some(intrinsic)) if intrinsic > 0.0 => target / intrinsic * multiplier,
        _ => multiplier,
    }
}

#[cfg(test)]
mod tempo_resolution_tests {
    use super::*;
    use crate::command::Timestamp;

    fn snapshot(group: u32, bpm: f64, observed_millis: u64) -> SpeedGroupSnapshot {
        SpeedGroupSnapshot {
            group_id: SpeedGroupId::new(group),
            bpm,
            phase_beats: 0.0,
            observed_at: Timestamp::from_millis(observed_millis),
        }
    }

    fn at(millis: u64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    #[test]
    fn the_channel_source_reads_the_channel_and_ignores_any_group() {
        let source = TempoSource::PlaybackBpmChannel;
        assert_eq!(
            resolve_tempo(source, Some(snapshot(1, 128.0, 0)), Some(90), at(0)),
            ResolvedTempo::Live { bpm: 90.0 },
            "selecting the channel disables Speed Group consumption"
        );
        assert_eq!(
            resolve_tempo(source, Some(snapshot(1, 128.0, 0)), None, at(0)),
            ResolvedTempo::None,
            "a zero channel byte is off, not a fallback to the group"
        );
    }

    #[test]
    fn a_group_source_follows_only_its_own_group() {
        let source = TempoSource::SpeedGroup {
            group_id: SpeedGroupId::new(2),
        };
        assert_eq!(
            resolve_tempo(source, Some(snapshot(2, 128.0, 0)), Some(90), at(0)),
            ResolvedTempo::Live { bpm: 128.0 },
            "the channel is not consulted in group mode"
        );
        assert_eq!(
            resolve_tempo(source, Some(snapshot(7, 128.0, 0)), Some(90), at(0)),
            ResolvedTempo::None,
            "another group's clock is not this output's"
        );
        assert_eq!(
            resolve_tempo(source, None, Some(90), at(0)),
            ResolvedTempo::None
        );
    }

    #[test]
    fn a_stale_group_holds_its_last_clock_rather_than_falling_back() {
        let source = TempoSource::SpeedGroup {
            group_id: SpeedGroupId::new(1),
        };
        let observed = snapshot(1, 128.0, 1_000);
        let expiry = 1_000 + SPEED_GROUP_FRESHNESS.as_millis() as u64;

        assert_eq!(
            resolve_tempo(source, Some(observed), Some(90), at(expiry - 1)),
            ResolvedTempo::Live { bpm: 128.0 }
        );
        let held = resolve_tempo(source, Some(observed), Some(90), at(expiry));
        assert_eq!(held, ResolvedTempo::Stale { bpm: 128.0 });
        assert!(
            held.is_stale(),
            "the output warns rather than silently retiming"
        );
        assert_eq!(
            held.bpm(),
            Some(128.0),
            "and keeps running at the last known tempo"
        );
    }

    #[test]
    fn unsynchronized_playback_ignores_tempo_entirely() {
        for tempo in [
            ResolvedTempo::None,
            ResolvedTempo::Live { bpm: 200.0 },
            ResolvedTempo::Stale { bpm: 60.0 },
        ] {
            assert_eq!(effective_rate(false, Some(120.0), tempo, 1.0), 1.0);
            assert_eq!(effective_rate(false, None, tempo, 2.0), 2.0);
        }
    }

    #[test]
    fn synchronized_playback_retimes_an_authored_asset_to_the_target() {
        let tempo = ResolvedTempo::Live { bpm: 140.0 };
        // A 70 BPM loop played against a 140 BPM master runs at double speed.
        assert_eq!(effective_rate(true, Some(70.0), tempo, 1.0), 2.0);
        // And the multiplier still applies on top.
        assert_eq!(effective_rate(true, Some(70.0), tempo, 0.5), 1.0);
    }

    #[test]
    fn synchronized_playback_without_an_intrinsic_bpm_is_not_an_error() {
        let tempo = ResolvedTempo::Live { bpm: 140.0 };
        assert_eq!(
            effective_rate(true, None, tempo, 1.0),
            1.0,
            "the target becomes the reference, so the ratio is exactly one"
        );
        assert_eq!(
            effective_rate(true, None, tempo, 4.0),
            4.0,
            "and the multiplier is the only change"
        );
    }

    #[test]
    fn a_synchronized_layer_with_no_tempo_at_all_still_plays() {
        assert_eq!(
            effective_rate(true, Some(120.0), ResolvedTempo::None, 1.0),
            1.0
        );
    }

    #[test]
    fn a_nonsense_intrinsic_tempo_is_not_divided_by() {
        let tempo = ResolvedTempo::Live { bpm: 120.0 };
        for intrinsic in [0.0, -60.0] {
            let rate = effective_rate(true, Some(intrinsic), tempo, 1.0);
            assert!(rate.is_finite(), "intrinsic {intrinsic} produced {rate}");
            assert_eq!(rate, 1.0);
        }
    }

    #[test]
    fn a_negative_multiplier_never_reverses_playback_behind_the_modes_back() {
        assert_eq!(effective_rate(false, None, ResolvedTempo::None, -2.0), 0.0);
    }
}
