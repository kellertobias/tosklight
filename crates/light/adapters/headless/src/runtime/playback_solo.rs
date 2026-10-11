//! Solo coordination for transports whose authority lives outside the Playback engine.
use super::{ApiError, AppState};
use light_core::CueListId;
use light_engine::{EnginePlaybackCommand, EnginePlaybackOutcome, EngineSnapshot};
use light_playback::{
    PlaybackAddress, PlaybackTarget, TimecodeId, TimecodeTransportAction, TimecodeTransportState,
};
use std::collections::BTreeSet;

mod components;
pub(in crate::runtime) use components::SoloCoordinator;

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

/// Membership belongs to the exact Playback which was activated.
pub(in crate::runtime) fn solo_members(
    snapshot: &EngineSnapshot,
    incoming: PlaybackAddress,
) -> Vec<PlaybackAddress> {
    snapshot
        .playback_solo_regions
        .iter()
        .filter(|region| region.members.contains(&incoming))
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
    incoming: PlaybackAddress,
) -> Vec<TimecodeId> {
    let incoming_target = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == incoming.page())
        .and_then(|page| page.assignments.get(&incoming.position()))
        .and_then(|definition| target(&definition.target));
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
        if incoming_target != Some(SoloTarget::Timecode(id)) && !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}
fn publish_addresses(state: &AppState, addresses: Vec<PlaybackAddress>) -> Result<(), ApiError> {
    use light_application::{
        ActionContext, ActionSource, EventDraft, EventSource, PlaybackRuntimeChange,
        PlaybackRuntimeIdentity,
    };
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
    addresses: Vec<PlaybackAddress>,
    before: &[light_application::PlaybackRuntimeProjection],
    result: Result<T, ApiError>,
) -> Result<T, ApiError> {
    match result {
        Err(error) => {
            publish_failure_feedback(state, &addresses, before);
            Err(error)
        }
        Ok(value) => {
            publish_addresses(state, addresses)?;
            Ok(value)
        }
    }
}
fn publish_failure_feedback(
    state: &AppState,
    addresses: &[PlaybackAddress],
    before: &[light_application::PlaybackRuntimeProjection],
) {
    if let Err(feedback) = publish_failed_changes(state, addresses, before) {
        tracing::warn!(error = %feedback.message, "Solo partial failure feedback failed");
    }
}
fn capture_address_projections(
    state: &AppState,
    addresses: &[PlaybackAddress],
) -> Result<Vec<light_application::PlaybackRuntimeProjection>, ApiError> {
    let context = light_application::ActionContext::system(
        uuid::Uuid::nil(),
        light_application::ActionSource::System,
    );
    let identities = addresses
        .iter()
        .copied()
        .map(light_application::PlaybackRuntimeIdentity::Playback)
        .collect::<Vec<_>>();
    super::playback_service::read_runtime_projections(state, &context, &identities)
        .map_err(|error| ApiError::internal(format!("Solo feedback failed: {}", error.message)))
}
fn publish_failed_changes(
    state: &AppState,
    addresses: &[PlaybackAddress],
    before: &[light_application::PlaybackRuntimeProjection],
) -> Result<(), ApiError> {
    for projection in capture_address_projections(state, addresses)? {
        if before.iter().any(|previous| previous == &projection) {
            continue;
        }
        state
            .events
            .publish(light_application::EventDraft::playback_runtime_changed(
                None,
                light_application::PlaybackRuntimeChange {
                    projection,
                    transition: None,
                },
                light_application::EventSource::Runtime,
                None,
            ));
    }
    Ok(())
}

/// Caller already holds the shared ordered Playback operation.
pub(in crate::runtime) fn execute_engine(
    state: &AppState,
    command: EnginePlaybackCommand,
) -> Result<EnginePlaybackOutcome, ApiError> {
    let coordinator = SoloCoordinator {
        engine: state.output.engine(),
        timecodes: &state.timecodes,
    };
    let address = match &command {
        EnginePlaybackCommand::Playback { address, .. } => Some(*address),
        _ => None,
    };
    let addresses = address
        .map(|address| coordinator.related_addresses(address))
        .unwrap_or_default();
    let before = capture_address_projections(state, &addresses)?;
    match coordinator.execute(command) {
        Ok(outcome) => Ok(outcome.output),
        Err(error) => {
            // Rejection before mutation emits nothing. An external Stop can fail after
            // accepted engine changes; publish only those actual changed authorities.
            publish_failure_feedback(state, &addresses, &before);
            Err(error)
        }
    }
}

/// Caller already holds the shared ordered Playback operation.
pub(in crate::runtime) fn timecode_action(
    state: &AppState,
    id: TimecodeId,
    action: TimecodeTransportAction,
) -> Result<light_application::timeline::TimecodeRuntimeOutcome, ApiError> {
    let coordinator = SoloCoordinator {
        engine: state.output.engine(),
        timecodes: &state.timecodes,
    };
    let addresses = aliases(&state.output.snapshot(), SoloTarget::Timecode(id));
    let before = capture_address_projections(state, &addresses)?;
    let result = coordinator
        .transport(id, action)
        .map(|outcome| outcome.output);
    finish(state, addresses, &before, result)
}

/// Assigned transport dispatch retains its exact Playback membership.
pub(in crate::runtime) fn timecode_playback_action(
    state: &AppState,
    address: PlaybackAddress,
    action: TimecodeTransportAction,
) -> Result<light_application::timeline::TimecodeRuntimeOutcome, ApiError> {
    let coordinator = SoloCoordinator {
        engine: state.output.engine(),
        timecodes: &state.timecodes,
    };
    let addresses = coordinator.related_addresses(address);
    let before = capture_address_projections(state, &addresses)?;
    match coordinator.transport_playback(address, action) {
        Ok(outcome) => Ok(outcome.output),
        Err(error) => {
            publish_failure_feedback(state, &addresses, &before);
            Err(error)
        }
    }
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
            serde_json::json!({"name":"Timecode","target":{"type":"timecode","timecode_id":id},"has_fader":false}),
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
    async fn assigned_timecode_start_stops_actual_peers_only_in_its_exact_region() {
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
        timecode_playback_action(&state, address(1, 1), TimecodeTransportAction::Go).unwrap();
        assert_eq!(
            state.timecodes.snapshot(ids[0]).unwrap().transport,
            TimecodeTransportState::Playing
        );
        assert_eq!(
            state.timecodes.snapshot(ids[1]).unwrap().transport,
            TimecodeTransportState::Stopped
        );
        assert_eq!(
            state.timecodes.snapshot(ids[2]).unwrap().transport,
            TimecodeTransportState::Playing
        );
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn invalid_solo_peer_rejects_start_before_mutating_incoming_transport() {
        let (state, path) = super::super::tests::test_state();
        let ids = [11, 12, 13].map(|id| TimecodeId(uuid::Uuid::from_u128(id)));
        install(&state, ids[0], 1);
        install(&state, ids[2], 3);
        state
            .output
            .replace_snapshot(snapshot(ids[0], ids[1], ids[2]))
            .unwrap();
        state
            .timecodes
            .handle(ids[2], TimecodeTransportAction::Go)
            .unwrap();
        let error = timecode_playback_action(&state, address(1, 1), TimecodeTransportAction::Go)
            .unwrap_err();
        assert!(error.message.contains("Solo Timecode peer unavailable"));
        assert_eq!(
            state.timecodes.snapshot(ids[0]).unwrap().transport,
            TimecodeTransportState::Stopped
        );
        assert_eq!(
            state.timecodes.snapshot(ids[2]).unwrap().transport,
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
    #[tokio::test]
    async fn direct_transport_and_outside_alias_do_not_arbitrate_member_playbacks() {
        let (state, path) = super::super::tests::test_state();
        let ids = [21, 22, 23].map(|id| TimecodeId(uuid::Uuid::from_u128(id)));
        for (index, id) in ids.iter().enumerate() {
            install(&state, *id, index as u32 + 1);
        }
        let mut show = snapshot(ids[0], ids[1], ids[2]);
        Arc::make_mut(&mut show.playback_pages).push(PlaybackPage {
            number: 3,
            name: "Outside".into(),
            assignments: [(1, assigned(ids[0]))].into(),
        });
        state.output.replace_snapshot(show).unwrap();
        for id in &ids[1..] {
            state
                .timecodes
                .handle(*id, TimecodeTransportAction::Go)
                .unwrap();
        }
        let coordinator = SoloCoordinator {
            engine: state.output.engine(),
            timecodes: &state.timecodes,
        };
        assert!(
            coordinator
                .transport(ids[0], TimecodeTransportAction::Go)
                .unwrap()
                .stopped_timecodes
                .is_empty()
        );
        for id in &ids[1..] {
            assert_eq!(
                state.timecodes.snapshot(*id).unwrap().transport,
                TimecodeTransportState::Playing
            );
        }
        coordinator
            .transport(ids[0], TimecodeTransportAction::Stop)
            .unwrap();
        assert!(
            coordinator
                .transport_playback(address(3, 1), TimecodeTransportAction::Go)
                .unwrap()
                .stopped_timecodes
                .is_empty()
        );
        for id in &ids[1..] {
            assert_eq!(
                state.timecodes.snapshot(*id).unwrap().transport,
                TimecodeTransportState::Playing
            );
        }
        std::fs::remove_dir_all(path).unwrap();
    }

    #[tokio::test]
    async fn peer_disappearing_after_validation_returns_failure_and_keeps_actual_changes() {
        let (state, path) = super::super::tests::test_state();
        let ids = [31, 32, 33].map(|id| TimecodeId(uuid::Uuid::from_u128(id)));
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
        let coordinator = SoloCoordinator {
            engine: state.output.engine(),
            timecodes: &state.timecodes,
        };
        let mut peers = coordinator.prepare_peer_releases(address(1, 1)).unwrap();
        peers.extend(coordinator.prepare_peer_releases(address(2, 1)).unwrap());
        state.timecodes.uninstall(ids[2]).unwrap();
        let error = coordinator.stop_peer_releases(&peers).unwrap_err();
        assert!(error.message.contains("Solo could not stop Timecode"));
        assert_eq!(
            state.timecodes.snapshot(ids[1]).unwrap().transport,
            TimecodeTransportState::Stopped
        );
        assert!(state.timecodes.snapshot(ids[2]).is_err());
        assert_eq!(
            state.timecodes.snapshot(ids[0]).unwrap().transport,
            TimecodeTransportState::Stopped
        );
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn direct_cue_start_keeps_assigned_timecode_peer_but_playback_activation_stops_it() {
        let (state, path) = super::super::tests::test_state();
        let timecode = TimecodeId(uuid::Uuid::from_u128(41));
        install(&state, timecode, 1);
        let cue_list = CueListId::new();
        let list = serde_json::from_value(serde_json::json!({
            "id":cue_list,"name":"Direct Cue","priority":0,"mode":"sequence","looped":false,
            "cues":[Cue::new(CueNumber::from(1_u8))]
        }))
        .unwrap();
        let cue_definition = serde_json::from_value(serde_json::json!({
            "name":"Member Cue","auto_off":false,"target":{"type":"cue_list","cue_list_id":cue_list}
        }))
        .unwrap();
        state
            .output
            .replace_snapshot(EngineSnapshot {
                cue_lists: vec![list].into(),
                playback_pages: vec![PlaybackPage {
                    number: 1,
                    name: "Main".into(),
                    assignments: [(1, cue_definition), (2, assigned(timecode))].into(),
                }]
                .into(),
                playback_solo_regions: vec![PlaybackSoloRegion {
                    id: uuid::Uuid::new_v4(),
                    name: "Solo".into(),
                    members: vec![address(1, 1), address(1, 2)],
                }]
                .into(),
                ..Default::default()
            })
            .unwrap();
        state
            .timecodes
            .handle(timecode, TimecodeTransportAction::Go)
            .unwrap();
        let coordinator = SoloCoordinator {
            engine: state.output.engine(),
            timecodes: &state.timecodes,
        };
        let direct = EnginePlaybackCommand::CueList {
            id: cue_list,
            action: light_engine::CueListPlaybackAction::Go,
        };
        assert_eq!(coordinator.command_target(&direct), None);
        assert!(
            coordinator
                .execute(direct)
                .unwrap()
                .stopped_timecodes
                .is_empty()
        );
        assert_eq!(
            state.timecodes.snapshot(timecode).unwrap().transport,
            TimecodeTransportState::Playing
        );
        coordinator
            .execute(EnginePlaybackCommand::CueList {
                id: cue_list,
                action: light_engine::CueListPlaybackAction::Release,
            })
            .unwrap();
        let result = coordinator
            .execute(EnginePlaybackCommand::Playback {
                address: address(1, 1),
                action: light_engine::PlaybackAction::On,
                activation_origin: None,
                control_source: PlaybackControlSource {
                    surface_id: "main".into(),
                    control_id: 1,
                },
            })
            .unwrap();
        assert_eq!(result.stopped_timecodes, vec![timecode]);
        assert_eq!(
            state.timecodes.snapshot(timecode).unwrap().transport,
            TimecodeTransportState::Stopped
        );
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn flash_preserves_running_timecode_peer_until_actual_permanent_start() {
        let (state, path) = super::super::tests::test_state();
        let timecode = TimecodeId(uuid::Uuid::new_v4());
        install(&state, timecode, 1);
        let fixture_id = light_core::FixtureId::new();
        let mut profile = light_fixture::FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "Solo dimmer".into();
        let mode = &mut profile.modes[0];
        mode.heads[0].master_shared = false;
        let head = mode.heads[0].id;
        mode.splits[0].footprint = 1;
        mode.channels = vec![serde_json::from_value(serde_json::json!({
            "id":uuid::Uuid::new_v4(), "head_id":head, "split":1,
            "fixture_attribute":"intensity", "attribute":"intensity", "resolution":"u8",
            "default_raw":0, "highlight_raw":255, "functions":[{
                "id":uuid::Uuid::new_v4(), "name":"Dimmer", "attribute":"intensity",
                "dmx_from":0, "dmx_to":255, "priority":0,
                "behavior":{"type":"continuous", "physical_min":0.0,"physical_max":1.0,"unit":null}
            }]
        })).unwrap()];
        let mode_id = mode.id;
        let fixture = serde_json::from_value(serde_json::json!({
            "fixture_id":uuid::Uuid::new_v4(),
            "definition":profile.resolved_definition(mode_id).unwrap(), "universe":1, "address":1,
            "logical_heads":[{"profile_head_id":head,"head_index":0,"fixture_id":fixture_id}],
            "group_masters_enabled":true
        }))
        .unwrap();
        let cue_list = CueListId::new();
        let mut cue = Cue::new(CueNumber::from(1_u8));
        cue.fade_millis = 0;
        cue.changes = vec![CueChange::set(
            fixture_id,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(0.6),
        )];
        let list = serde_json::from_value(serde_json::json!({
            "id":cue_list,"name":"Flash member","priority":0,"mode":"sequence",
            "looped":false,"cues":[cue]
        }))
        .unwrap();
        let mut member = PlaybackDefinition::new_cue_list("Flash member", cue_list);
        member.auto_off = false;
        member.flash_release = FlashReleaseMode::ReleaseAll;
        state
            .output
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture].into(),
                cue_lists: vec![list].into(),
                playback_pages: vec![PlaybackPage {
                    number: 1,
                    name: "Main".into(),
                    assignments: [(1, member), (2, assigned(timecode))].into(),
                }]
                .into(),
                playback_solo_regions: vec![PlaybackSoloRegion {
                    id: uuid::Uuid::new_v4(),
                    name: "Solo".into(),
                    members: vec![address(1, 1), address(1, 2)],
                }]
                .into(),
                ..Default::default()
            })
            .unwrap();
        timecode_playback_action(&state, address(1, 2), TimecodeTransportAction::Go).unwrap();
        let coordinator = SoloCoordinator {
            engine: state.output.engine(),
            timecodes: &state.timecodes,
        };
        let act = |action| {
            coordinator
                .execute(EnginePlaybackCommand::Playback {
                    address: address(1, 1),
                    action,
                    activation_origin: None,
                    control_source: PlaybackControlSource {
                        surface_id: "flash-test".into(),
                        control_id: 1,
                    },
                })
                .unwrap()
        };
        let output = || state.output.render(Default::default()).unwrap().universes[&1][0];
        assert_eq!(output(), 0);
        for pressed in [true, false] {
            let result = act(light_engine::PlaybackAction::SetFlash(pressed));
            assert!(result.stopped_timecodes.is_empty());
            assert_eq!(
                state.timecodes.snapshot(timecode).unwrap().transport,
                TimecodeTransportState::Playing
            );
            assert!(
                !state
                    .output
                    .engine()
                    .permanent_playback_enabled(address(1, 1))
            );
            assert_eq!(output(), if pressed { 153 } else { 0 });
        }
        let result = act(light_engine::PlaybackAction::On);
        assert_eq!(result.stopped_timecodes, vec![timecode]);
        assert_eq!(
            state.timecodes.snapshot(timecode).unwrap().transport,
            TimecodeTransportState::Stopped
        );
        assert!(
            state
                .output
                .engine()
                .permanent_playback_enabled(address(1, 1))
        );
        assert_eq!(output(), 153);
        std::fs::remove_dir_all(path).unwrap();
    }
    fn install_cue_aliases(state: &AppState) {
        let id = CueListId::new();
        let list = serde_json::from_value(serde_json::json!({
            "id":id,"name":"Feedback Cue","priority":0,"mode":"sequence","looped":false,
            "cues":[Cue::new(CueNumber::from(1_u8))]
        }))
        .unwrap();
        let definition = PlaybackDefinition::new_cue_list("Alias", id);
        state
            .output
            .replace_snapshot(EngineSnapshot {
                cue_lists: vec![list].into(),
                playback_pages: vec![PlaybackPage {
                    number: 1,
                    name: "One".into(),
                    assignments: HashMap::from([(1, definition.clone()), (2, definition)]),
                }]
                .into(),
                ..Default::default()
            })
            .unwrap();
    }
    fn cue_command(action: light_engine::PlaybackAction) -> EnginePlaybackCommand {
        EnginePlaybackCommand::Playback {
            address: address(1, 1),
            action,
            activation_origin: None,
            control_source: PlaybackControlSource {
                surface_id: "main".into(),
                control_id: 1,
            },
        }
    }
    #[tokio::test]
    async fn rejected_cue_does_not_publish_unchanged_alias_feedback() {
        let (state, path) = super::super::tests::test_state();
        install_cue_aliases(&state);
        let cursor = state.events.latest_sequence();
        let error = execute_engine(
            &state,
            cue_command(light_engine::PlaybackAction::GoTo(CueNumber::from(99_u8))),
        )
        .unwrap_err();
        assert!(error.message.contains("cue does not exist"));
        assert_eq!(state.events.latest_sequence(), cursor);
        assert!(
            !state
                .output
                .engine()
                .permanent_playback_enabled(address(1, 1))
        );
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn accepted_cue_then_partial_external_stop_failure_publishes_changed_aliases() {
        let (state, path) = super::super::tests::test_state();
        install_cue_aliases(&state);
        let ids = [51, 52].map(|id| TimecodeId(uuid::Uuid::from_u128(id)));
        for (index, id) in ids.iter().enumerate() {
            install(&state, *id, index as u32 + 1);
            state
                .timecodes
                .handle(*id, TimecodeTransportAction::Go)
                .unwrap();
        }
        let mut snapshot = (*state.output.snapshot()).clone();
        Arc::make_mut(&mut snapshot.playback_pages)[0]
            .assignments
            .extend([(3, assigned(ids[0])), (4, assigned(ids[1]))]);
        snapshot.playback_solo_regions = vec![PlaybackSoloRegion {
            id: uuid::Uuid::new_v4(),
            name: "Partial".into(),
            members: vec![address(1, 1), address(1, 3), address(1, 4)],
        }]
        .into();
        state.output.replace_snapshot(snapshot).unwrap();
        let coordinator = SoloCoordinator {
            engine: state.output.engine(),
            timecodes: &state.timecodes,
        };
        let addresses = coordinator.related_addresses(address(1, 1));
        let before = capture_address_projections(&state, &addresses).unwrap();
        let peers = coordinator.prepare_peer_releases(address(1, 1)).unwrap();
        state
            .output
            .engine()
            .execute_playback(cue_command(light_engine::PlaybackAction::On))
            .unwrap();
        state.timecodes.uninstall(ids[1]).unwrap();
        let error = coordinator.stop_peer_releases(&peers).unwrap_err();
        assert!(error.message.contains("Solo could not stop Timecode"));
        assert_eq!(
            state.timecodes.snapshot(ids[0]).unwrap().transport,
            TimecodeTransportState::Stopped
        );
        assert!(
            state
                .output
                .engine()
                .permanent_playback_enabled(address(1, 1))
        );
        let cursor = state.events.latest_sequence();
        publish_failed_changes(&state, &addresses, &before).unwrap();
        let light_application::EventReplay::Events(events) = state
            .events
            .replay(cursor, &light_application::EventFilter::default())
        else {
            panic!("feedback replay unavailable")
        };
        assert_eq!(events.len(), 2);
        let identities = events
            .iter()
            .map(|event| match &event.payload {
                light_application::ApplicationEvent::Playback(
                    light_application::PlaybackEvent::RuntimeChanged(change),
                ) => change.projection.identity.clone(),
                _ => panic!("expected actual Playback feedback"),
            })
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(
            identities,
            std::collections::HashSet::from([
                light_application::PlaybackRuntimeIdentity::Playback(address(1, 1)),
                light_application::PlaybackRuntimeIdentity::Playback(address(1, 2))
            ])
        );
        std::fs::remove_dir_all(path).unwrap();
    }

    #[tokio::test]
    async fn unavailable_direct_and_assigned_transports_do_not_publish_unchanged_feedback() {
        let (state, path) = super::super::tests::test_state();
        let ids = [61, 62, 63].map(|id| TimecodeId(uuid::Uuid::from_u128(id)));
        state
            .output
            .replace_snapshot(snapshot(ids[0], ids[1], ids[2]))
            .unwrap();
        let cursor = state.events.latest_sequence();
        let direct = timecode_action(&state, ids[0], TimecodeTransportAction::Go).unwrap_err();
        assert!(direct.message.contains("not installed"));
        assert_eq!(state.events.latest_sequence(), cursor);
        let assigned = timecode_playback_action(&state, address(1, 1), TimecodeTransportAction::Go)
            .unwrap_err();
        assert!(assigned.message.contains("not installed"));
        assert_eq!(state.events.latest_sequence(), cursor);
        std::fs::remove_dir_all(path).unwrap();
    }
}
