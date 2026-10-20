use light_core::CueListId;
use light_engine::{
    CueListPlaybackAction, Engine, EnginePlaybackCommand, EngineSnapshot, PlaybackAction,
};
use light_playback::*;
use light_programmer::ProgrammerRegistry;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
#[test]
fn direct_cuelist_start_does_not_activate_assigned_solo_members() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let incoming = cue_list(vec![Cue::new(1_u16.into())]);
    let peer1 = cue_list(vec![Cue::new(1_u16.into())]);
    let peer2 = cue_list(vec![Cue::new(1_u16.into())]);
    let id = incoming.id;
    let addresses =
        [(1, 1), (1, 2), (2, 1), (2, 2)].map(|(p, n)| PlaybackAddress::new(p, n).unwrap());
    let d = |id| PlaybackDefinition::new_cue_list("Test", id);
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: Arc::new(vec![incoming, peer1.clone(), peer2.clone()]),
            playback_pages: Arc::new(vec![
                PlaybackPage {
                    number: 1,
                    name: "One".into(),
                    assignments: HashMap::from([(1, d(id)), (2, d(peer1.id))]),
                },
                PlaybackPage {
                    number: 2,
                    name: "Two".into(),
                    assignments: HashMap::from([(1, d(id)), (2, d(peer2.id))]),
                },
            ]),
            playback_solo_regions: Arc::new(vec![
                PlaybackSoloRegion {
                    color: None,
                    id: Uuid::new_v4(),
                    name: "First".into(),
                    members: addresses[..2].to_vec(),
                },
                PlaybackSoloRegion {
                    color: None,
                    id: Uuid::new_v4(),
                    name: "Second".into(),
                    members: addresses[2..].to_vec(),
                },
            ]),
            ..Default::default()
        })
        .unwrap();
    for address in [addresses[1], addresses[3]] {
        engine
            .execute_playback(EnginePlaybackCommand::Playback {
                address,
                action: PlaybackAction::On,
                control_source: PlaybackControlSource {
                    surface_id: "main".into(),
                    control_id: 1,
                },
                activation_origin: None,
            })
            .unwrap();
    }
    engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id,
            action: CueListPlaybackAction::Go,
        })
        .unwrap();
    let active = engine.active_playbacks();
    assert!(active.iter().any(|r| r.cue_list_id == id));
    assert!(
        active.iter().any(|r| r.cue_list_id == peer1.id)
            && active.iter().any(|r| r.cue_list_id == peer2.id),
        "direct Cue control must leave both Playback Solo peers running"
    );
}
#[test]
fn external_timecode_solo_bridge_uses_exact_member_address() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let peer = cue_list(vec![Cue::new(1_u16.into())]);
    let peer_id = peer.id;
    let timecode = TimecodeId(Uuid::new_v4());
    let external:PlaybackDefinition=serde_json::from_value(serde_json::json!({"name":"External Timecode","target":{"type":"timecode","timecode_id":timecode},"has_fader":false})).unwrap();
    let a = PlaybackAddress::new(1, 1).unwrap();
    let alias = PlaybackAddress::new(2, 1).unwrap();
    let b = PlaybackAddress::new(2, 2).unwrap();
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: Arc::new(vec![peer]),
            playback_pages: Arc::new(vec![
                PlaybackPage {
                    number: 1,
                    name: "One".into(),
                    assignments: HashMap::from([(1, external.clone())]),
                },
                PlaybackPage {
                    number: 2,
                    name: "Two".into(),
                    assignments: HashMap::from([
                        (1, external),
                        (2, PlaybackDefinition::new_cue_list("Peer", peer_id)),
                    ]),
                },
            ]),
            playback_solo_regions: Arc::new(vec![PlaybackSoloRegion {
                color: None,
                id: Uuid::new_v4(),
                name: "Across aliases".into(),
                members: vec![alias, b],
            }]),
            ..Default::default()
        })
        .unwrap();
    engine
        .execute_playback(EnginePlaybackCommand::Playback {
            address: b,
            action: PlaybackAction::On,
            activation_origin: None,
            control_source: PlaybackControlSource {
                surface_id: "main".into(),
                control_id: 1,
            },
        })
        .unwrap();
    engine
        .execute_playback(EnginePlaybackCommand::EnforceSoloForTarget { address: a })
        .unwrap();
    assert!(
        engine.permanent_playback_enabled(b),
        "an outside-region alias has no Solo membership"
    );
    engine
        .execute_playback(EnginePlaybackCommand::EnforceSoloForTarget { address: alias })
        .unwrap();
    assert!(!engine.permanent_playback_enabled(b));
}
#[test]
fn prepared_solo_batch_reports_exact_region_releases_and_shared_peer_feedback() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let incoming = cue_list(vec![Cue::new(1_u16.into())]);
    let peer = cue_list(vec![Cue::new(1_u16.into())]);
    let address = PlaybackAddress::new(1, 1).unwrap();
    let alias = PlaybackAddress::new(2, 1).unwrap();
    let peer_address = PlaybackAddress::new(2, 2).unwrap();
    let peer_alias = PlaybackAddress::new(3, 1).unwrap();
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: Arc::new(vec![incoming.clone(), peer.clone()]),
            playback_pages: Arc::new(vec![
                PlaybackPage {
                    number: 1,
                    name: "One".into(),
                    assignments: HashMap::from([(
                        1,
                        PlaybackDefinition::new_cue_list("Incoming", incoming.id),
                    )]),
                },
                PlaybackPage {
                    number: 2,
                    name: "Two".into(),
                    assignments: HashMap::from([
                        (
                            1,
                            PlaybackDefinition::new_cue_list("Incoming alias", incoming.id),
                        ),
                        (2, PlaybackDefinition::new_cue_list("Peer", peer.id)),
                    ]),
                },
                PlaybackPage {
                    number: 3,
                    name: "Three".into(),
                    assignments: HashMap::from([(
                        1,
                        PlaybackDefinition::new_cue_list("Peer alias", peer.id),
                    )]),
                },
            ]),
            playback_solo_regions: Arc::new(vec![
                PlaybackSoloRegion {
                    color: None,
                    id: Uuid::new_v4(),
                    name: "One".into(),
                    members: vec![alias, peer_address],
                },
                PlaybackSoloRegion {
                    color: None,
                    id: Uuid::new_v4(),
                    name: "Overlapping".into(),
                    members: vec![alias, peer_address],
                },
            ]),
            ..Default::default()
        })
        .unwrap();
    let source = PlaybackControlSource {
        surface_id: "main".into(),
        control_id: 1,
    };
    engine
        .execute_playback(EnginePlaybackCommand::Playback {
            address: peer_address,
            action: PlaybackAction::On,
            activation_origin: None,
            control_source: source.clone(),
        })
        .unwrap();
    let prepared = engine
        .prepare_playback_batch(
            &[light_engine::PlaybackBatchCommand {
                address: alias,
                action: light_engine::PlaybackBatchAction::On,
                activation_origin: None,
                control_source: source,
            }],
            chrono::Utc::now(),
            0,
        )
        .unwrap();
    assert_eq!(
        prepared.outcomes()[0].released_playbacks,
        vec![peer_address],
        "released peers must be deduplicated across regions containing the addressed member"
    );
    let changed = prepared.changed_playback_addresses().collect::<Vec<_>>();
    for alias in [address, alias, peer_address, peer_alias] {
        assert!(
            changed.contains(&alias),
            "shared alias effect missing: {alias}"
        );
    }
    assert!(
        engine.permanent_playback_enabled(peer_address),
        "preparation must remain isolated"
    );
}
fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
        id: CueListId::new(),
        name: "Render transition test".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues,
    }
}
