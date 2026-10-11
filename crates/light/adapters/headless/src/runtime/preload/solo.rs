//! External Solo peers retain the same policy as immediate Playback actions.
use super::*;
use crate::runtime::playback_solo::SoloCoordinator;
use light_playback::TimecodeId;

pub(super) fn prepare_timecode_releases(
    state: &AppState,
    prepared: &light_engine::PreparedPlaybackBatch,
) -> Result<Vec<TimecodeId>, String> {
    let coordinator = SoloCoordinator {
        engine: state.output.engine(),
        timecodes: &state.timecodes,
    };
    let mut peers = Vec::new();
    for outcome in prepared
        .outcomes()
        .iter()
        .filter(|outcome| outcome.permanent_started)
    {
        for peer in coordinator
            .prepare_peer_releases(outcome.address)
            .map_err(|error| error.message)?
        {
            if !peers.contains(&peer) {
                peers.push(peer);
            }
        }
    }
    Ok(peers)
}

/// Run only after every fallible batch preparation and the accepted Live installation.
/// Audio may still reject Stop; preserve installed state and expose the actual failure.
pub(super) fn stop_timecode_releases(state: &AppState, peers: &[TimecodeId]) -> Vec<String> {
    let coordinator = SoloCoordinator {
        engine: state.output.engine(),
        timecodes: &state.timecodes,
    };
    let warnings = peers
        .iter()
        .filter_map(|peer| {
            coordinator.stop_peer_releases(&[*peer]).err().map(|error| {
                format!(
                    "Preload GO was committed, but Solo Timecode peer {} could not stop: {}",
                    peer.0, error.message
                )
            })
        })
        .collect();
    if !peers.is_empty() {
        // Reconcile after releasing the prepared Engine guard, never inside its callback.
        // The transport service publishes its existing authoritative Stop feedback.
        state.timecodes.reconcile_cue_lists(state.output.engine());
    }
    warnings
}
