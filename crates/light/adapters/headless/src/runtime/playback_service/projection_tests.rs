use super::*;
use light_playback::{
    ActiveDynamicPlayback, Cue, CueList, CueListMode, DynamicPlaybackAssignment,
    DynamicPlaybackFaderMode, DynamicPlaybackResumePolicy, FlashReleaseMode, IntensityPriorityMode,
    PlaybackButtonAction, PlaybackDefinition, PlaybackEngine, PlaybackFaderMode, RestartMode,
    WrapMode,
};

#[test]
fn direct_cuelist_action_projection_is_not_replaced_by_assigned_playbacks() {
    let cue_list = cue_list();
    let cue_list_id = cue_list.id;
    let first = definition(1, cue_list_id);
    let second = definition(2, cue_list_id);
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go(cue_list_id).unwrap();
    let direct_runtime = engine.active().pop().unwrap();
    engine.register_definition(first.clone()).unwrap();
    engine.register_definition(second.clone()).unwrap();
    engine.goto_playback(1, 2.0).unwrap();
    engine.goto_playback(2, 3.0).unwrap();
    engine.restore_active([direct_runtime]);
    let runtime = engine.runtime_status();
    let requested = PlaybackRuntimeIdentity::CueList(cue_list_id);
    let scope = test_scope();

    let direct = cue_list_projection(
        scope,
        requested.clone(),
        None,
        cue_list_id,
        direct_cue_list_runtime(&runtime, cue_list_id),
    );
    assert_eq!(direct.playback_number, None);
    assert_eq!(direct.current_cue().map(|cue| cue.number), Some(1.0));

    let snapshot = EngineSnapshot {
        playbacks: vec![first, second].into(),
        ..EngineSnapshot::default()
    };
    let mut repair = Vec::new();
    project_cue_list(
        scope,
        &snapshot,
        &runtime,
        requested,
        cue_list_id,
        &mut repair,
    );
    assert_eq!(repair.len(), 3);
    assert_eq!(
        repair
            .iter()
            .map(|projection| (
                projection.playback_number,
                projection.current_cue().map(|cue| cue.number)
            ))
            .collect::<Vec<_>>(),
        vec![
            (Some(1), Some(2.0)),
            (Some(2), Some(3.0)),
            (None, Some(1.0))
        ]
    );
}

#[test]
fn dynamic_projection_reports_hidden_controller_identity_speed_and_coverage() {
    let fixture = light_core::FixtureId::new();
    let definition = dynamic_definition(fixture);
    let playback_controller = dynamic_playback_controller_id(7);
    let mut runtime = light_dynamics::DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance_id = runtime
        .start(light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: playback_controller,
                source: light_dynamics::DynamicControllerSource::Playback { playback_number: 7 },
                priority: 0,
                activated_at_millis: 10,
                size: 0.75,
                speed_multiplier: 2.0,
                phase_offset_degrees: 0.0,
                paused: false,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: vec![fixture],
            },
            stage_positions: std::collections::HashMap::new(),
            now_millis: 10,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    let winning_controller = uuid::Uuid::new_v4();
    runtime
        .start(light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: winning_controller,
                source: light_dynamics::DynamicControllerSource::Programmer {
                    programmer_id: uuid::Uuid::new_v4(),
                },
                priority: 1,
                activated_at_millis: 11,
                size: 1.0,
                speed_multiplier: 1.0,
                phase_offset_degrees: 0.0,
                paused: false,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: vec![fixture],
            },
            stage_positions: std::collections::HashMap::new(),
            now_millis: 11,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    let assignment = DynamicPlaybackAssignment {
        dynamic: light_dynamics::DynamicReference {
            dynamic_id: Some(definition.id),
            last_known_pool_number: definition.pool_number,
            embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                definition: Box::new(definition.clone()),
            },
        },
        revision: 1,
        target_scope: None,
        fader_mode: DynamicPlaybackFaderMode::SizeAndMaster,
        priority: 0,
        activation_override: None,
        resume_policy: DynamicPlaybackResumePolicy::FollowDynamic,
        local_speed_multiplier: light_dynamics::Rational {
            numerator: 2,
            denominator: 1,
        },
        learned_duration_millis: None,
        crossfade_non_intensity: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        auto_off_full_control: true,
    };
    let active = ActiveDynamicPlayback {
        playback_number: 7,
        playback_identity: None,
        enabled: true,
        paused: false,
        flash: false,
        flash_restore_off: false,
        activated_at: chrono::Utc::now(),
        fader_value: 1.0,
        size: 0.75,
        master: 0.8,
        local_speed_multiplier: assignment.local_speed_multiplier,
        learned_duration_millis: None,
        last_learn_tap_millis: None,
        learn_intervals_millis: Vec::new(),
    };
    let snapshot = EngineSnapshot {
        dynamics: vec![definition].into(),
        fixtures: vec![projection_fixture(fixture, true, &["intensity"])].into(),
        ..EngineSnapshot::default()
    };

    let projection = dynamic_runtime_projection_from_snapshot(
        &snapshot,
        &assignment,
        active,
        &runtime.snapshot(),
        10,
    );

    assert_eq!(projection.state, DynamicPlaybackRuntimeState::Hidden);
    assert_eq!(projection.instance_id, Some(instance_id));
    assert_eq!(projection.controller_id, playback_controller);
    assert_eq!(projection.winning_controller_id, Some(winning_controller));
    assert_eq!(
        projection.controller_status,
        DynamicPlaybackControllerStatus::Losing
    );
    assert_eq!(projection.target_count, 1);
    assert_eq!(projection.compatible_target_count, 1);
    assert_eq!(projection.missing_target_count, 0);
    assert_eq!(projection.unpatched_target_count, 0);
    assert_eq!(projection.lane_count, 1);
    assert_eq!(projection.supported_address_count, 1);
    assert_eq!(projection.skipped_address_count, 0);
    assert!((projection.effective_speed_multiplier - 3.0).abs() < f32::EPSILON);
    assert_eq!(projection.effective_duration_millis, Some(400));
    assert_eq!(projection.warning, None);
}

#[test]
fn dynamic_coverage_reports_supported_skipped_missing_and_unpatched_addresses() {
    let supported = light_core::FixtureId::new();
    let unsupported = light_core::FixtureId::new();
    let missing = light_core::FixtureId::new();
    let mut definition = dynamic_definition(supported);
    definition.lanes.push(light_dynamics::DynamicLane {
        id: uuid::Uuid::new_v4(),
        attribute: light_core::AttributeKey("pan".into()),
        ..definition.lanes[0].clone()
    });
    let snapshot = EngineSnapshot {
        fixtures: vec![
            projection_fixture(supported, true, &["intensity", "pan"]),
            projection_fixture(unsupported, false, &["intensity"]),
        ]
        .into(),
        ..EngineSnapshot::default()
    };

    let coverage =
        dynamic_target_lane_coverage(&snapshot, &definition, &[supported, unsupported, missing]);

    assert_eq!(
        coverage,
        DynamicTargetLaneCoverage {
            compatible_target_count: 2,
            missing_target_count: 1,
            unpatched_target_count: 1,
            supported_address_count: 3,
            skipped_address_count: 3,
            total_address_count: 6,
        }
    );
}

#[test]
fn dynamic_coverage_resolves_persisted_logical_head_targets() {
    let fixture_id = light_core::FixtureId::new();
    let head_id = light_core::FixtureId::new();
    let mut fixture = projection_fixture(fixture_id, true, &["intensity"]);
    fixture.definition.heads[0].index = 7;
    fixture.logical_heads.push(light_fixture::PatchedHead {
        profile_head_id: None,
        head_index: 7,
        fixture_id: head_id,
    });
    let definition = dynamic_definition(head_id);
    let snapshot = EngineSnapshot {
        fixtures: vec![fixture].into(),
        ..EngineSnapshot::default()
    };

    let coverage = dynamic_target_lane_coverage(&snapshot, &definition, &[head_id]);

    assert_eq!(
        coverage,
        DynamicTargetLaneCoverage {
            compatible_target_count: 1,
            missing_target_count: 0,
            unpatched_target_count: 0,
            supported_address_count: 1,
            skipped_address_count: 0,
            total_address_count: 1,
        }
    );
}

fn test_scope() -> PlaybackShowScope {
    PlaybackShowScope {
        show_id: uuid::Uuid::from_u128(1),
        show_revision: 3,
    }
}

fn cue_list() -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Shared".into(),
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
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)],
    }
}

fn definition(number: u16, cue_list_id: CueListId) -> PlaybackDefinition {
    PlaybackDefinition {
        number,
        name: format!("Playback {number}"),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

fn dynamic_definition(fixture: light_core::FixtureId) -> light_dynamics::DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": uuid::Uuid::new_v4(),
        "pool_number": 7,
        "revision": 1,
        "name": "Projection wave",
        "color": null,
        "icon": null,
        "target_binding": {"type": "frozen_targets", "targets": [fixture]},
        "lanes": [{
            "id": uuid::Uuid::new_v4(),
            "attribute": "intensity",
            "mode": "keyframes",
            "keyframes": {
                "points": [
                    {"position": 0.0, "source": {"type": "value", "value": 0.25}, "interpolation": "linear"},
                    {"position": 0.5, "source": {"type": "value", "value": 0.75}, "interpolation": "linear"}
                ],
                "size": 1.0
            },
            "max_min": {
                "minimum": {"type": "value", "value": 0.25},
                "maximum": {"type": "value", "value": 0.75},
                "function": "sinus",
                "size": 1.0,
                "pwm": {
                    "attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                    "attack_interpolation": "linear", "decay_interpolation": "linear"
                }
            },
            "middle_amplitude": {
                "middle": {"type": "current"},
                "amplitude": 0.25,
                "function": "sinus",
                "size": 1.0,
                "pwm": {
                    "attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                    "attack_interpolation": "linear", "decay_interpolation": "linear"
                }
            },
            "speed_multiplier": {"numerator": 1, "denominator": 1},
            "width": 1.0,
            "random_group_id": null
        }],
        "random_groups": [],
        "phase": {
            "ordering": {"type": "selection"},
            "offset_degrees": 0.0,
            "span_degrees": 360.0,
            "block_size": 1,
            "repeats": 1,
            "wings": false,
            "anchors_degrees": []
        },
        "speed": {"type": "fixed", "duration_millis": 1200},
        "overall_speed_multiplier": {"numerator": 3, "denominator": 2},
        "default_activation": "start_now"
    }))
    .unwrap()
}

fn projection_fixture(
    fixture_id: light_core::FixtureId,
    patched: bool,
    attributes: &[&str],
) -> light_fixture::PatchedFixture {
    serde_json::from_value(serde_json::json!({
        "fixture_id": fixture_id,
        "definition": {
            "schema_version": 1,
            "id": uuid::Uuid::new_v4(),
            "revision": 1,
            "manufacturer": "Test",
            "name": "Projection fixture",
            "model": "Projection fixture",
            "mode": "Test",
            "footprint": attributes.len().max(1),
            "heads": [{
                "index": 0,
                "name": "Main",
                "parameters": attributes.iter().enumerate().map(|(index, attribute)| serde_json::json!({
                    "attribute": attribute,
                    "components": [{"offset": index, "byte_order": "msb_first"}],
                    "default": 0.0,
                    "virtual_dimmer": false
                })).collect::<Vec<_>>()
            }],
            "color_calibration": null,
            "hazardous": false,
            "safe_values": {}
        },
        "universe": patched.then_some(1),
        "address": patched.then_some(1)
    }))
    .unwrap()
}
