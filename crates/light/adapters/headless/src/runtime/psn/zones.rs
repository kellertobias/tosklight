//! Who is standing in which box, and when that is worth acting on.
//!
//! A zone is occupied when a tracker the zone watches is inside it. That much is a point-in-box
//! test. What makes this a state machine is the edge: a marker walking along the boundary of a
//! zone enters and leaves it many times a second, and a macro that ran every time would be worse
//! than no zone at all. So a change of state has to hold for the zone's dwell time before it
//! counts, and only then does the enter or leave macro run.
//!
//! Time is passed in rather than read, so a marker loitering on a boundary can be tested without
//! anybody waiting a quarter of a second for the answer.

use std::collections::HashMap;
use uuid::Uuid;

use super::config::PsnConfiguration;

/// What the desk should do about a zone, having watched it long enough to be sure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum ZoneTransition {
    Entered,
    Left,
}

/// One zone as the tracker sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::runtime) struct ZoneState {
    /// What the desk has committed to and acted on.
    pub occupied: bool,
    /// What the last look said, when it disagreed with `occupied`.
    pending: Option<(bool, u64)>,
}

/// Advance every configured zone by one look at the trackers.
///
/// `positions` is every tracker the receiver currently has a position for, in show metres, keyed
/// by tracker id. Returns the zones whose state actually changed, so the caller runs a macro only
/// on a change it can explain to the operator.
///
/// Zones the operator has deleted are dropped from `states`, so a zone that comes back starts
/// unoccupied rather than remembering a show ago.
pub(in crate::runtime) fn advance(
    configuration: &PsnConfiguration,
    positions: &HashMap<u16, [f32; 3]>,
    states: &mut HashMap<Uuid, ZoneState>,
    now_millis: u64,
) -> Vec<(Uuid, ZoneTransition)> {
    let configured: Vec<Uuid> = configuration.zones.iter().map(|zone| zone.id).collect();
    states.retain(|id, _| configured.contains(id));
    if !configuration.enabled {
        // Switching PSN off leaves every zone as it was rather than firing every leave macro at
        // once: an operator turning the source off has not asked for the show to change.
        return Vec::new();
    }
    let mut transitions = Vec::new();
    for zone in &configuration.zones {
        let occupied_now = positions
            .iter()
            .any(|(tracker_id, position)| zone.watches(*tracker_id) && zone.contains(*position));
        let state = states.entry(zone.id).or_default();
        if occupied_now == state.occupied {
            state.pending = None;
            continue;
        }
        // Either this look continues what the last one saw, or the marker changed its mind and
        // the clock starts again from here.
        let since = match state.pending {
            Some((pending, since)) if pending == occupied_now => since,
            _ => now_millis,
        };
        if now_millis.saturating_sub(since) >= zone.dwell_millis {
            state.occupied = occupied_now;
            state.pending = None;
            transitions.push((
                zone.id,
                if occupied_now {
                    ZoneTransition::Entered
                } else {
                    ZoneTransition::Left
                },
            ));
        } else {
            state.pending = Some((occupied_now, since));
        }
    }
    transitions
}

#[cfg(test)]
#[path = "zones_tests.rs"]
mod tests;
