//! Solo coordination for transports whose authority lives outside the Playback engine.
use super::{ApiError, AppState};
use light_core::CueListId;
use light_engine::{EnginePlaybackCommand, EnginePlaybackOutcome, EngineSnapshot};
use light_playback::{
    PlaybackAddress, PlaybackTarget, TimecodeId, TimecodeTransportAction, TimecodeTransportState,
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SoloTarget {
    CueList(CueListId),
    Dynamic(uuid::Uuid),
    Timecode(TimecodeId),
}
pub(in crate::runtime) fn target(definition: &PlaybackTarget) -> Option<SoloTarget> {
    match definition {
        PlaybackTarget::CueList { cue_list_id } => Some(SoloTarget::CueList(*cue_list_id)),
        PlaybackTarget::Dynamic { assignment } => Some(SoloTarget::Dynamic(assignment.target_id())),
        PlaybackTarget::Timecode { timecode_id } => Some(SoloTarget::Timecode(*timecode_id)),
        _ => None,
    }
}

/// All aliases of a shared target participate, including aliases on different pages.
pub(in crate::runtime) fn solo_members(
    snapshot: &EngineSnapshot,
    incoming: SoloTarget,
) -> Vec<PlaybackAddress> {
    snapshot
        .playback_solo_regions
        .iter()
        .filter(|region| {
            region.members.iter().any(|address| {
                snapshot
                    .playback_pages
                    .iter()
                    .find(|page| page.number == address.page())
                    .and_then(|page| page.assignments.get(&address.position()))
                    .is_some_and(|definition| target(&definition.target) == Some(incoming))
            })
        })
        .flat_map(|region| region.members.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn aliases(snapshot: &EngineSnapshot, incoming: SoloTarget) -> Vec<PlaybackAddress> {
    snapshot
        .playback_pages
        .iter()
        .flat_map(|page| {
            page.assignments
                .iter()
                .filter_map(move |(position, definition)| {
                    (target(&definition.target) == Some(incoming))
                        .then(|| PlaybackAddress::new(page.number, *position).ok())
                        .flatten()
                })
        })
        .collect()
}
pub(in crate::runtime) fn timecode_peer_ids(
    snapshot: &EngineSnapshot,
    incoming: SoloTarget,
) -> Vec<TimecodeId> {
    let mut ids = Vec::new();
    for address in solo_members(snapshot, incoming) {
        let definition = snapshot
            .playback_pages
            .iter()
            .find(|page| page.number == address.page())
            .and_then(|page| page.assignments.get(&address.position()));
        let Some(SoloTarget::Timecode(id)) =
            definition.and_then(|definition| target(&definition.target))
        else {
            continue;
        };
        if incoming != SoloTarget::Timecode(id) && !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}
fn timecode_peers(state: &AppState, incoming: SoloTarget) -> Result<Vec<TimecodeId>, ApiError> {
    timecode_peer_ids(&state.output.snapshot(), incoming)
        .into_iter()
        .filter_map(|id| match state.timecodes.snapshot(id) {
            Ok(runtime) if runtime.transport != TimecodeTransportState::Stopped => Some(Ok(id)),
            Ok(_) => None,
            Err(error) => Some(Err(ApiError::internal(format!(
                "Solo Timecode peer unavailable: {}",
                error.message
            )))),
        })
        .collect()
}

fn stop_peers(state: &AppState, peers: Vec<TimecodeId>) -> Result<(), ApiError> {
    for id in peers {
        state
            .timecodes
            .handle(id, TimecodeTransportAction::Stop)
            .map_err(|error| {
                ApiError::internal(format!(
                    "Solo could not stop Timecode {}: {}",
                    id.0, error.message
                ))
            })?;
    }
    Ok(())
}
fn publish_settled(state: &AppState, incoming: SoloTarget) -> Result<(), ApiError> {
    use light_application::{
        ActionContext, ActionSource, EventDraft, EventSource, PlaybackRuntimeChange,
        PlaybackRuntimeIdentity,
    };
    let snapshot = state.output.snapshot();
    let mut addresses = solo_members(&snapshot, incoming);
    addresses.extend(aliases(&snapshot, incoming));
    addresses.sort_unstable();
    addresses.dedup();
    let identities = addresses
        .into_iter()
        .map(PlaybackRuntimeIdentity::Playback)
        .collect::<Vec<_>>();
    if identities.is_empty() {
        return Ok(());
    }
    let context = ActionContext::system(uuid::Uuid::nil(), ActionSource::System);
    let projections =
        super::playback_service::read_runtime_projections(state, &context, &identities).map_err(
            |error| ApiError::internal(format!("Solo feedback failed: {}", error.message)),
        )?;
    for projection in projections {
        state.events.publish(EventDraft::playback_runtime_changed(
            None,
            PlaybackRuntimeChange {
                projection,
                transition: None,
            },
            EventSource::Runtime,
            None,
        ));
    }
    Ok(())
}
fn finish<T>(
    state: &AppState,
    incoming: SoloTarget,
    result: Result<T, ApiError>,
) -> Result<T, ApiError> {
    // An external device can reject Stop after another target accepted its start. Publish actual
    // authorities even on failure; returning an error must never advertise coherent Solo success.
    let feedback = publish_settled(state, incoming);
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            feedback?;
            Ok(value)
        }
    }
}
fn enabled(state: &AppState, incoming: SoloTarget) -> bool {
    match incoming {
        SoloTarget::CueList(id) => state
            .output
            .playback_runtime()
            .iter()
            .any(|runtime| runtime.cue_list_id == id && runtime.enabled),
        _ => aliases(&state.output.snapshot(), incoming)
            .iter()
            .any(|address| state.output.engine().permanent_playback_enabled(*address)),
    }
}
/// Caller already holds the shared ordered Playback operation.
pub(in crate::runtime) fn execute_engine(
    state: &AppState,
    command: EnginePlaybackCommand,
) -> Result<EnginePlaybackOutcome, ApiError> {
    let incoming = match &command {
        EnginePlaybackCommand::CueList { id, .. } => Some(SoloTarget::CueList(*id)),
        EnginePlaybackCommand::Playback { address, .. } => state
            .output
            .snapshot()
            .playback_pages
            .iter()
            .find(|page| page.number == address.page())
            .and_then(|page| page.assignments.get(&address.position()))
            .and_then(|definition| target(&definition.target)),
        _ => None,
    };
    let before = incoming.is_some_and(|incoming| enabled(state, incoming));
    let may_activate = match &command {
        EnginePlaybackCommand::CueList { action, .. } => matches!(
            action,
            light_engine::CueListPlaybackAction::Go
                | light_engine::CueListPlaybackAction::GoAt(_)
                | light_engine::CueListPlaybackAction::Back
                | light_engine::CueListPlaybackAction::Jump(_)
                | light_engine::CueListPlaybackAction::ExecuteCueId(_)
                | light_engine::CueListPlaybackAction::ReconstructCueId { .. }
        ),
        EnginePlaybackCommand::Playback { action, .. } => !matches!(
            action,
            light_engine::PlaybackAction::Off
                | light_engine::PlaybackAction::Release
                | light_engine::PlaybackAction::SetFlash(_)
                | light_engine::PlaybackAction::Pause
                | light_engine::PlaybackAction::TogglePause
        ),
        _ => false,
    };
    let peers = if !before && may_activate {
        incoming
            .map(|incoming| timecode_peers(state, incoming))
            .transpose()?
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let outcome = state
        .output
        .execute_playback(command)
        .map_err(ApiError::bad_request)?;
    if !before && incoming.is_some_and(|incoming| enabled(state, incoming)) {
        let incoming = incoming.expect("checked incoming target");
        if let Err(error) = stop_peers(state, peers) {
            return finish(state, incoming, Err(error));
        }
        // PlaybackService publishes successful related projections as one action transaction.
        return Ok(outcome);
    }
    Ok(outcome)
}
/// Caller already holds the shared ordered Playback operation.
pub(in crate::runtime) fn timecode_action(
    state: &AppState,
    id: TimecodeId,
    action: TimecodeTransportAction,
) -> Result<light_application::timeline::TimecodeRuntimeOutcome, ApiError> {
    let before = state
        .timecodes
        .snapshot(id)
        .map_err(|error| ApiError::bad_request(error.message))?;
    let incoming = SoloTarget::Timecode(id);
    let peers = if before.transport == TimecodeTransportState::Stopped
        && matches!(action, TimecodeTransportAction::Go)
    {
        timecode_peers(state, incoming)?
    } else {
        Vec::new()
    };
    let outcome = state
        .timecodes
        .handle(id, action)
        .map_err(|error| ApiError::bad_request(error.message))?;
    if before.transport == TimecodeTransportState::Stopped
        && outcome.snapshot.transport != TimecodeTransportState::Stopped
    {
        let result = (|| {
            stop_peers(state, peers)?;
            if let Some(address) = aliases(&state.output.snapshot(), incoming).first().copied() {
                state
                    .output
                    .execute_playback(EnginePlaybackCommand::EnforceSoloForTarget { address })
                    .map_err(ApiError::internal)?;
            }
            Ok(outcome)
        })();
        return finish(state, incoming, result);
    }
    finish(state, incoming, Ok(outcome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_playback::*;
    use std::{collections::HashMap, sync::Arc};
    fn install(state: &AppState, id: TimecodeId, number: u32) {
        state
            .timecodes
            .install(
                TimecodeDefinition {
                    id,
                    number,
                    name: format!("Timecode {number}"),
                    duration: Some(TimecodeFrame(440)),
                    transport_offset: TimecodeFrame::ZERO,
                    auto_start: false,
                    audio: None,
                    markers: Vec::new(),
                    lanes: Vec::new(),
                },
                None,
            )
            .unwrap();
    }
    fn assigned(id: TimecodeId) -> PlaybackDefinition {
        serde_json::from_value(
            serde_json::json!({"name":"Timecode","target":{"type":"timecode","timecode_id":id}}),
        )
        .unwrap()
    }
    fn address(page: u8, position: u16) -> PlaybackAddress {
        PlaybackAddress::new(page, position).unwrap()
    }
    fn snapshot(incoming: TimecodeId, first: TimecodeId, second: TimecodeId) -> EngineSnapshot {
        EngineSnapshot {
            playback_pages: Arc::new(vec![
                PlaybackPage {
                    number: 1,
                    name: "One".into(),
                    assignments: HashMap::from([(1, assigned(incoming)), (2, assigned(first))]),
                },
                PlaybackPage {
                    number: 2,
                    name: "Two".into(),
                    assignments: HashMap::from([(1, assigned(incoming)), (2, assigned(second))]),
                },
            ]),
            playback_solo_regions: Arc::new(vec![
                PlaybackSoloRegion {
                    id: uuid::Uuid::new_v4(),
                    name: "One".into(),
                    members: vec![address(1, 1), address(1, 2)],
                },
                PlaybackSoloRegion {
                    id: uuid::Uuid::new_v4(),
                    name: "Two".into(),
                    members: vec![address(2, 1), address(2, 2)],
                },
            ]),
            ..Default::default()
        }
    }
    #[tokio::test]
    async fn accepted_timecode_start_stops_actual_peers_across_all_alias_regions() {
        let (state, path) = super::super::tests::test_state();
        let ids = [1, 2, 3].map(|id| TimecodeId(uuid::Uuid::from_u128(id)));
        for (index, id) in ids.iter().enumerate() {
            install(&state, *id, index as u32 + 1);
        }
        state
            .output
            .replace_snapshot(snapshot(ids[0], ids[1], ids[2]))
            .unwrap();
        for id in &ids[1..] {
            state
                .timecodes
                .handle(*id, TimecodeTransportAction::Go)
                .unwrap();
        }
        super::super::timecode_v2::apply_transport_action(
            &state,
            ids[0],
            TimecodeTransportAction::Go,
        )
        .unwrap();
        assert_eq!(
            state.timecodes.snapshot(ids[0]).unwrap().transport,
            TimecodeTransportState::Playing
        );
        for id in &ids[1..] {
            assert_eq!(
                state.timecodes.snapshot(*id).unwrap().transport,
                TimecodeTransportState::Stopped
            );
        }
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn invalid_solo_peer_rejects_start_before_mutating_incoming_transport() {
        let (state, path) = super::super::tests::test_state();
        let ids = [11, 12, 13].map(|id| TimecodeId(uuid::Uuid::from_u128(id)));
        install(&state, ids[0], 1);
        install(&state, ids[1], 2);
        state
            .output
            .replace_snapshot(snapshot(ids[0], ids[1], ids[2]))
            .unwrap();
        state
            .timecodes
            .handle(ids[1], TimecodeTransportAction::Go)
            .unwrap();
        let error = super::super::timecode_v2::apply_transport_action(
            &state,
            ids[0],
            TimecodeTransportAction::Go,
        )
        .unwrap_err();
        assert!(error.message.contains("Solo Timecode peer unavailable"));
        assert_eq!(
            state.timecodes.snapshot(ids[0]).unwrap().transport,
            TimecodeTransportState::Stopped
        );
        assert_eq!(
            state.timecodes.snapshot(ids[1]).unwrap().transport,
            TimecodeTransportState::Playing
        );
        // Off remains available even when a different Solo member has disappeared.
        super::super::timecode_v2::apply_transport_action(
            &state,
            ids[0],
            TimecodeTransportAction::Stop,
        )
        .unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }
}
