//! Report actual accepted peer changes; Solo arbitration remains in the central authorities.
use super::super::{
    AppState,
    playback_solo::{SoloCoordinator, target},
};
use light_playback::PlaybackAddress;

/// The caller holds the ordered Playback operation across observation, action, and result.
pub(super) fn enabled_peers(state: &AppState, incoming: PlaybackAddress) -> Vec<PlaybackAddress> {
    let coordinator = SoloCoordinator {
        engine: state.output.engine(),
        timecodes: &state.timecodes,
    };
    let snapshot = state.output.snapshot();
    let target_at = |address: PlaybackAddress| {
        snapshot
            .playback_pages
            .iter()
            .find(|page| page.number == address.page())
            .and_then(|page| page.assignments.get(&address.position()))
            .and_then(|definition| target(&definition.target))
    };
    let incoming_target = target_at(incoming);
    coordinator
        .related_addresses(incoming)
        .into_iter()
        .filter(|address| target_at(*address).is_some_and(|peer| Some(peer) != incoming_target))
        .filter(|address| coordinator.enabled(*address))
        .collect()
}

pub(super) fn released_peers(
    state: &AppState,
    enabled_peers: Vec<PlaybackAddress>,
) -> Vec<PlaybackAddress> {
    let coordinator = SoloCoordinator {
        engine: state.output.engine(),
        timecodes: &state.timecodes,
    };
    enabled_peers
        .into_iter()
        .filter(|address| !coordinator.enabled(*address))
        .collect()
}
