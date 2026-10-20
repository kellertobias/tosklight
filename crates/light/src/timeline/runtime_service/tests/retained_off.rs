use super::*;
use chrono::{TimeZone, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, ManualClock, SharedClock};
use light_playback::{
    CueChange, PlaybackAddress, PlaybackControlSource, PlaybackDefinition, PlaybackPage,
    PlaybackSoloRegion,
};

fn address(position: u16) -> PlaybackAddress {
    PlaybackAddress::new(1, position).unwrap()
}

fn act(engine: &Engine, position: u16, action: light_engine::PlaybackAction) {
    engine
        .execute_playback(EnginePlaybackCommand::Playback {
            address: address(position),
            action,
            activation_origin: None,
            control_source: PlaybackControlSource {
                surface_id: "timecode-test".into(),
                control_id: 1,
            },
        })
        .unwrap();
}

fn dimmer() -> (light_fixture::PatchedFixture, FixtureId) {
    let fixture_id = FixtureId::new();
    let mut profile = light_fixture::FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Timecode dimmer".into();
    let mode = &mut profile.modes[0];
    mode.heads[0].master_shared = false;
    let head = mode.heads[0].id;
    mode.splits[0].footprint = 1;
    mode.channels =
        vec![serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "head_id":head, "split":1,
        "fixture_attribute":"intensity", "attribute":"intensity", "resolution":"u8",
        "default_raw":0, "highlight_raw":255, "functions":[{
            "id":Uuid::new_v4(), "name":"Dimmer", "attribute":"intensity", "dmx_from":0,
            "dmx_to":255, "priority":0,
            "behavior":{"type":"continuous", "physical_min":0.0,"physical_max":1.0,"unit":null}
        }]
    })).unwrap()];
    let mode_id = mode.id;
    let fixture = serde_json::from_value(serde_json::json!({
        "fixture_id":Uuid::new_v4(), "definition":profile.resolved_definition(mode_id).unwrap(),
        "universe":1, "address":1, "logical_heads":[{"profile_head_id":head,"head_index":0,"fixture_id":fixture_id}],
        "group_masters_enabled":true
    })).unwrap();
    (fixture, fixture_id)
}

fn retained_off_engine() -> (Engine, Arc<ManualClock>, CueList, FixtureId, CueListId) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 10, 11, 0, 0, 0).unwrap(),
    ));
    let shared: SharedClock = clock.clone();
    let engine = Engine::new(ProgrammerRegistry::with_clock(shared));
    let (fixture, fixture_id) = dimmer();
    let mut list = execution_cue_list();
    list.cues[0].fade_millis = 1_000;
    list.cues[0].delay_millis = 200;
    list.cues[0].changes = vec![CueChange::set(
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.6),
    )];
    list.cues[1].fade_millis = 0;
    list.cues[1].out_fade_millis = Some(10_000);
    list.cues[1].changes = vec![CueChange::set(
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.9),
    )];
    let mut peer = execution_cue_list();
    peer.id = CueListId::new();
    let peer_id = peer.id;
    let mut target = PlaybackDefinition::new_cue_list("Target", list.id);
    target.go_activates = false;
    target.auto_off = false;
    let mut peer_definition = PlaybackDefinition::new_cue_list("Peer", peer.id);
    peer_definition.auto_off = false;
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![list.clone(), peer].into(),
            playback_pages: vec![PlaybackPage {
                number: 1,
                name: "Timecode test".into(),
                assignments: [(1, target), (2, peer_definition)].into(),
            }]
            .into(),
            playback_solo_regions: vec![PlaybackSoloRegion {
                color: None,
                id: Uuid::new_v4(),
                name: "Peers".into(),
                members: vec![address(1), address(2)],
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    act(
        &engine,
        1,
        light_engine::PlaybackAction::GoTo(CueNumber::from(2_u8)),
    );
    act(&engine, 2, light_engine::PlaybackAction::On);
    assert!(
        !engine
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .enabled
    );
    assert!(
        engine
            .playback_runtime_status_for_cue_list(peer_id)
            .unwrap()
            .playback
            .enabled
    );
    (engine, clock, list, fixture_id, peer_id)
}

fn clip_definition(list: &CueList, start: TimecodeClipStart) -> light_playback::TimecodeDefinition {
    let mut definition = execution_definition(list);
    let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content else {
        unreachable!()
    };
    clips[0].start_behavior = start;
    clips[0].end_cue_id = list.cues[0].id;
    clips[0].end_frame = TimecodeFrame(100);
    definition
}

fn intensity(engine: &Engine, fixture_id: FixtureId) -> f32 {
    match engine
        .resolved_values()
        .get(&(fixture_id, AttributeKey::intensity()))
    {
        Some(AttributeValue::Normalized(value)) => *value,
        None => 0.0,
        other => panic!("unexpected intensity: {other:?}"),
    }
}

#[test]
fn retained_off_cue_start_reactivates_exact_cue_without_solo_or_old_out_fade() {
    let (engine, clock, list, fixture, peer) = retained_off_engine();
    let (service, _, _) = service();
    let definition = clip_definition(&list, TimecodeClipStart::Cue);
    service.install(definition.clone(), None).unwrap();
    service
        .handle(definition.id, TimecodeTransportAction::Go)
        .unwrap();
    service.reconcile_cue_lists(&engine);
    let started = engine
        .playback_runtime_status_for_cue_list(list.id)
        .unwrap()
        .playback;
    assert!(
        started.enabled,
        "new Cue Start must reactivate retained Off runtime"
    );
    assert_eq!(started.current_cue_id, Some(list.cues[0].id));
    assert!(!started.discrete_cue_actions_suppressed);
    assert!(
        started.previous_index.is_none(),
        "Off Cue's Out timing cannot become a new start source"
    );
    assert_eq!(intensity(&engine, fixture), 0.0);
    clock.advance_millis(700);
    assert!((intensity(&engine, fixture) - 0.3).abs() < 0.001);
    service.reconcile_cue_lists(&engine);
    assert_eq!(
        engine
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .transition_ordinal,
        started.transition_ordinal
    );
    assert!(
        engine
            .playback_runtime_status_for_cue_list(peer)
            .unwrap()
            .playback
            .enabled,
        "direct Timecode Cue start bypasses Solo"
    );
}

#[test]
fn retained_off_state_start_and_explicit_seek_reconstruct_output_without_discrete_replay() {
    let (engine, _, list, fixture, peer) = retained_off_engine();
    let (service, _, _) = service();
    let definition = clip_definition(&list, TimecodeClipStart::State);
    service.install(definition.clone(), None).unwrap();
    service
        .handle(definition.id, TimecodeTransportAction::Go)
        .unwrap();
    service
        .handle(
            definition.id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(10),
            },
        )
        .unwrap();
    service.reconcile_cue_lists(&engine);
    let reconstructed = engine
        .playback_runtime_status_for_cue_list(list.id)
        .unwrap()
        .playback;
    assert!(
        reconstructed.enabled,
        "State Start must reactivate retained Off runtime"
    );
    assert_eq!(reconstructed.current_cue_id, Some(list.cues[0].id));
    assert!(reconstructed.discrete_cue_actions_suppressed);
    assert!((intensity(&engine, fixture) - 0.12).abs() < 0.001);
    act(&engine, 1, light_engine::PlaybackAction::Off);
    service.reconcile_cue_lists(&engine);
    assert!(
        !engine
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .enabled,
        "same-Cue tick must not defeat explicit manual Off"
    );
    act(&engine, 1, light_engine::PlaybackAction::Go);
    assert!(
        !engine
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .enabled,
        "ordinary GO retains go_activates=false"
    );
    service
        .handle(
            definition.id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(15),
            },
        )
        .unwrap();
    service.reconcile_cue_lists(&engine);
    let sought = engine
        .playback_runtime_status_for_cue_list(list.id)
        .unwrap()
        .playback;
    assert!(sought.enabled);
    assert_eq!(sought.current_cue_id, Some(list.cues[0].id));
    assert!(sought.discrete_cue_actions_suppressed);
    assert!((intensity(&engine, fixture) - 0.24).abs() < 0.001);
    assert!(
        engine
            .playback_runtime_status_for_cue_list(peer)
            .unwrap()
            .playback
            .enabled
    );
}
