use light_core::{CueListId, FixtureId};
use light_engine::{Engine, EnginePlaybackCommand, EngineSnapshot, PlaybackAction};
use light_playback::*;
use light_programmer::ProgrammerRegistry;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
fn address(page: u8, position: u16) -> PlaybackAddress {
    PlaybackAddress::new(page, position).unwrap()
}
fn origin(tick: i64) -> PlaybackActivationOrigin {
    PlaybackActivationOrigin {
        at: chrono::DateTime::from_timestamp_millis(32_503_680_000_000 + tick).unwrap(),
        desk_id: Some(Uuid::new_v4()),
        surface: PlaybackActivationSurface::Physical,
        exclusion_scope: PlaybackExclusionScope::Show,
    }
}
fn start(engine: &Engine, address: PlaybackAddress, tick: i64) {
    engine
        .execute_playback(EnginePlaybackCommand::Playback {
            address,
            action: PlaybackAction::On,
            activation_origin: Some(origin(tick)),
            control_source: PlaybackControlSource {
                surface_id: "main".into(),
                control_id: 1,
            },
        })
        .unwrap();
}
fn region(members: Vec<PlaybackAddress>) -> PlaybackSoloRegion {
    PlaybackSoloRegion {
        color: None,
        id: Uuid::new_v4(),
        name: "Restore".into(),
        members,
    }
}
fn restore(engine: &Engine, cues: Vec<ActivePlayback>, dynamics: Vec<ActiveDynamicPlayback>) {
    engine
        .execute_playback(EnginePlaybackCommand::RestorePlaybackRuntime {
            playbacks: cues,
            dynamic_playbacks: dynamics,
        })
        .unwrap();
}
#[test]
fn restored_outside_region_owner_does_not_gain_membership_through_alias() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let first = cue_list(vec![Cue::new(1_u16.into())]);
    let second = cue_list(vec![Cue::new(1_u16.into())]);
    let d = PlaybackDefinition::new_cue_list("Outside", first.id);
    let mut snapshot = EngineSnapshot {
        cue_lists: vec![first, second.clone()].into(),
        playback_pages: vec![
            PlaybackPage {
                number: 1,
                name: "One".into(),
                assignments: HashMap::from([
                    (1, d.clone()),
                    (2, PlaybackDefinition::new_cue_list("Peer", second.id)),
                ]),
            },
            PlaybackPage {
                number: 2,
                name: "Two".into(),
                assignments: HashMap::from([(1, d)]),
            },
        ]
        .into(),
        ..Default::default()
    };
    engine.replace_snapshot(snapshot.clone()).unwrap();
    start(&engine, address(1, 2), 0);
    start(&engine, address(2, 1), 1);
    let saved = engine.playback_runtime();
    snapshot.playback_solo_regions = vec![region(vec![address(1, 1), address(1, 2)])].into();
    engine.replace_snapshot(snapshot).unwrap();
    restore(&engine, saved, vec![]);
    assert!(
        engine.permanent_playback_enabled(address(1, 2)),
        "outside alias must not evict the region member on restore"
    );
    assert!(engine.permanent_playback_enabled(address(2, 1)));
}
#[test]
fn same_clock_dynamic_activation_ordinal_survives_checkpoint_and_future_activation() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let cue = cue_list(vec![Cue::new(1_u16.into())]);
    let mut snapshot = EngineSnapshot {
        cue_lists: vec![cue.clone()].into(),
        playback_pages: vec![PlaybackPage {
            number: 1,
            name: "One".into(),
            assignments: HashMap::from([
                (
                    1,
                    dynamic_playback_definition(17, DynamicPlaybackFaderMode::Master, false),
                ),
                (2, PlaybackDefinition::new_cue_list("Cue", cue.id)),
            ]),
        }]
        .into(),
        ..Default::default()
    };
    engine.replace_snapshot(snapshot.clone()).unwrap();
    start(&engine, address(1, 2), 0);
    start(&engine, address(1, 1), 0);
    let cues = engine.playback_runtime();
    let dynamic = engine.active_dynamic_playbacks();
    let provenance = dynamic[0]
        .activation
        .as_ref()
        .expect("accepted Dynamic activation has provenance");
    assert!(provenance.ordinal > cues[0].activation.as_ref().unwrap().ordinal);
    let mut row = serde_json::to_value(&dynamic[0]).unwrap();
    assert!(
        row.get("activation").is_none(),
        "internal provenance is not public status"
    );
    row["activation"] = serde_json::to_value(provenance).unwrap();
    let saved: ActiveDynamicPlayback = serde_json::from_value(row).unwrap();
    snapshot.playback_solo_regions = vec![region(vec![address(1, 1), address(1, 2)])].into();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine.replace_snapshot(snapshot).unwrap();
    restore(&engine, cues, vec![saved]);
    assert!(engine.permanent_playback_enabled(address(1, 1)));
    assert!(!engine.permanent_playback_enabled(address(1, 2)));
    start(&engine, address(1, 2), 0);
    assert!(engine.permanent_playback_enabled(address(1, 2)));
    assert!(
        !engine.permanent_playback_enabled(address(1, 1)),
        "restored ordinal must reserve later activations"
    );
}
#[test]
fn combined_restore_decides_overlapping_losers_from_whole_saved_checkpoint() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let a = cue_list(vec![Cue::new(1_u16.into())]);
    let b = cue_list(vec![Cue::new(1_u16.into())]);
    let mut snapshot = EngineSnapshot {
        cue_lists: vec![a.clone(), b.clone()].into(),
        playback_pages: vec![PlaybackPage {
            number: 1,
            name: "One".into(),
            assignments: HashMap::from([
                (1, PlaybackDefinition::new_cue_list("Newest", a.id)),
                (2, PlaybackDefinition::new_cue_list("Middle", b.id)),
                (
                    3,
                    dynamic_playback_definition(18, DynamicPlaybackFaderMode::Master, false),
                ),
            ]),
        }]
        .into(),
        ..Default::default()
    };
    engine.replace_snapshot(snapshot.clone()).unwrap();
    start(&engine, address(1, 3), 0);
    start(&engine, address(1, 2), 1);
    start(&engine, address(1, 1), 2);
    let cues = engine.playback_runtime();
    let dynamics = engine.active_dynamic_playbacks();
    snapshot.playback_solo_regions = vec![
        region(vec![address(1, 1), address(1, 2)]),
        region(vec![address(1, 2), address(1, 3)]),
    ]
    .into();
    engine.replace_snapshot(snapshot).unwrap();
    restore(&engine, cues, dynamics);
    assert!(engine.permanent_playback_enabled(address(1, 1)));
    assert!(!engine.permanent_playback_enabled(address(1, 2)));
    assert!(
        !engine.permanent_playback_enabled(address(1, 3)),
        "a saved loser cannot survive because another region already released its winner"
    );
    let cues = engine.playback_runtime();
    let dynamics = engine.active_dynamic_playbacks();
    restore(&engine, cues, dynamics);
    assert!(
        !engine.permanent_playback_enabled(address(1, 3)),
        "restoration is idempotent"
    );
}
fn dynamic_playback_definition(
    number: u16,
    fader_mode: DynamicPlaybackFaderMode,
    targetless: bool,
) -> PlaybackDefinition {
    let dynamic_id = Uuid::new_v4();
    let definition = light_dynamics::DynamicDefinition {
        id: dynamic_id,
        pool_number: number,
        revision: 1,
        name: format!("Dynamic {number}"),
        color: None,
        icon: None,
        overall_speed_multiplier: light_dynamics::Rational::ONE,
        target_binding: if targetless {
            light_dynamics::DynamicTargetBinding::Targetless
        } else {
            light_dynamics::DynamicTargetBinding::FrozenTargets {
                targets: vec![FixtureId::new()],
            }
        },
        lanes: Vec::new(),
        random_groups: Vec::new(),
        phase_spread_mode: light_dynamics::DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: light_dynamics::DynamicSpatialMappingOverride::default(),
        phase: light_dynamics::PhaseDistribution {
            ordering: light_dynamics::PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: Vec::new(),
        },
        speed: light_dynamics::DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        run_mode: light_dynamics::DynamicRunMode::Loop,
        default_activation: light_dynamics::ActivationPolicy::StartNow,
        activation_boundary: light_dynamics::ActivationBoundary::Beat,
    };
    let target = PlaybackTarget::Dynamic {
        assignment: DynamicPlaybackAssignment {
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(dynamic_id),
                last_known_pool_number: number,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(definition),
                },
            },
            revision: 1,
            target_scope: targetless.then(|| DynamicPlaybackTargetScope::FrozenTargets {
                targets: vec![FixtureId::new()],
            }),
            fader_mode,
            priority: 0,
            activation_override: None,
            resume_policy: DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: true,
            auto_off_flash_release: true,
            auto_off_full_control: true,
        },
    };
    PlaybackDefinition {
        name: format!("Dynamic Playback {number}"),
        buttons: PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        footprint: PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target,
    }
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
