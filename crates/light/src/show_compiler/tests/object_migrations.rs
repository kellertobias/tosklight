use super::super::{compile_show_candidate, stage_candidate_migrations};
use super::support::{document_with_objects, snapshot_without_revision, stored_body};
use light_core::CueListId;
use light_output::{DeliveryMode, OutputRoute, Protocol};
use light_playback::{Cue, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode};
use serde_json::json;
use uuid::Uuid;

#[test]
fn stage_layout_migration_generates_automatic_2d_and_preserves_legacy_manual_entries() {
    let (left, right) = (Uuid::new_v4(), Uuid::new_v4());
    let automatic = json!({
        "version": 2,
        "positions3d": {
            left.to_string(): {"x": -5.0, "y": 0.0, "z": 0.0},
            right.to_string(): {"x": 5.0, "y": 0.0, "z": 10.0},
        },
        "future_layout": "kept"
    });
    let manual_fixture = Uuid::new_v4();
    let manual = json!({
        "version": 2,
        "positions": {
            manual_fixture.to_string(): {
                "x": 23.0,
                "y": 37.0,
                "rotation": 12.0,
                "future_position": "kept"
            }
        },
        "positions3d": {
            manual_fixture.to_string(): {"x": 99.0, "y": 88.0, "z": 77.0}
        }
    });
    let originals = vec![
        ("stage_layout", "automatic", automatic.clone()),
        ("stage_layout", "manual", manual.clone()),
    ];
    let (store, document) = document_with_objects(&originals);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    assert_eq!(stored_body(&store, "stage_layout", "automatic"), automatic);
    assert_eq!(stored_body(&store, "stage_layout", "manual"), manual);

    let candidate = document.candidate(&transaction).unwrap();
    let generated = candidate
        .object("stage_layout", "automatic")
        .unwrap()
        .body();
    assert_eq!(
        generated["positions2dConfig"],
        json!({"provenance": "automatic", "projection": "front_to_back"})
    );
    assert_eq!(generated["positions"][left.to_string()]["x"], 5.0);
    assert_eq!(generated["positions"][left.to_string()]["y"], 95.0);
    assert_eq!(generated["positions"][right.to_string()]["x"], 95.0);
    assert_eq!(generated["positions"][right.to_string()]["y"], 5.0);
    assert_eq!(generated["future_layout"], "kept");

    let preserved = candidate.object("stage_layout", "manual").unwrap().body();
    assert_eq!(
        preserved["positions2dConfig"],
        json!({"provenance": "manual", "projection": "front_to_back"})
    );
    assert_eq!(
        preserved["positions"][manual_fixture.to_string()]["x"],
        23.0
    );
    assert_eq!(
        preserved["positions"][manual_fixture.to_string()]["future_position"],
        "kept"
    );

    let migrated = candidate
        .objects()
        .map(|object| {
            (
                object.key().kind().to_owned(),
                object.key().id().to_owned(),
                object.body().clone(),
            )
        })
        .collect::<Vec<_>>();
    let migrated_refs = migrated
        .iter()
        .map(|(kind, id, body)| (kind.as_str(), id.as_str(), body.clone()))
        .collect::<Vec<_>>();
    let (_, migrated_document) = document_with_objects(&migrated_refs);
    let mut second_pass = migrated_document.transaction();
    stage_candidate_migrations(&migrated_document, &mut second_pass).unwrap();
    assert!(second_pass.is_empty(), "Stage migration must be idempotent");
}

#[test]
fn defaults_are_raw_preserving_side_effect_free_and_compile_equivalent() {
    let cue_list_id = CueListId::new();
    let cue_list = CueList {
        id: cue_list_id,
        name: "Legacy Chaser".into(),
        priority: 0,
        mode: CueListMode::Chaser,
        looped: true,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Tracking),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 250,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues: vec![Cue::new(1.0)],
    };
    let mut legacy_cue = serde_json::to_value(cue_list).unwrap();
    legacy_cue["cues"][0].as_object_mut().unwrap().remove("id");
    legacy_cue["cues"][0]["future_cue"] = json!({"kept": [3, 1, 2]});
    legacy_cue["future_list"] = json!({"kept": true});
    let group = json!({
        "name": "Front",
        "fixtures": [],
        "future_group": {"kept": true}
    });
    let preset = json!({
        "name": "Dim",
        "family": "Intensity",
        "values": {"00000000-0000-0000-0000-000000000123": {
            "intensity": {"kind": "normalized", "value": 0.5, "future_value": {"kept": true}}
        }},
        "future_preset": {"kept": true}
    });
    let playback = json!({
        "number": 1,
        "name": "Speed",
        "target": {"type": "speed_group", "group": "A", "future_target": {"kept": true}},
        "fader": "speed",
        "future_playback": {"kept": true}
    });
    let route = OutputRoute {
        protocol: Protocol::ArtNet,
        logical_universe: 1,
        destination_universe: 1,
        delivery_mode: Some(DeliveryMode::Broadcast),
        destination: None,
        enabled: true,
        minimum_slots: 512,
    };
    let mut legacy_route = serde_json::to_value(route).unwrap();
    for field in ["delivery_mode", "destination", "minimum_slots"] {
        legacy_route.as_object_mut().unwrap().remove(field);
    }
    legacy_route["future_route"] = json!({"kept": true});

    let originals = vec![
        ("cue_list", "legacy", legacy_cue),
        ("group", "7", group),
        ("playback", "1", playback),
        ("preset", "1.5", preset),
        ("route", "one", legacy_route),
    ];
    let (store, document) = document_with_objects(&originals);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();

    for (kind, id, body) in &originals {
        assert_eq!(stored_body(&store, kind, id), *body);
    }
    let candidate = document.candidate(&transaction).unwrap();
    let cue = candidate.object("cue_list", "legacy").unwrap().body();
    assert_eq!(cue["chaser_xfade_percent"], 25);
    assert!(cue.get("chaser_xfade_millis").is_none());
    assert!(Uuid::parse_str(cue["cues"][0]["id"].as_str().unwrap()).is_ok());
    assert_eq!(cue["cues"][0]["future_cue"], json!({"kept": [3, 1, 2]}));
    assert_eq!(candidate.object("group", "7").unwrap().body()["id"], "7");
    assert_eq!(
        candidate.object("group", "7").unwrap().body()["grid"],
        json!({
            "method": "stage2d",
            "axis_origin": {"x": 0.0, "y": 0.0, "z": 0.0}
        })
    );
    let preset = candidate.object("preset", "1.5").unwrap().body();
    assert_eq!(preset["number"], 5);
    assert_eq!(
        preset["values"]["00000000-0000-0000-0000-000000000123"]["intensity"]["future_value"],
        json!({"kept": true})
    );
    let playback = candidate.object("playback", "1").unwrap().body();
    assert_eq!(playback["fader"], "learned_percentage");
    assert_eq!(playback["buttons"], json!(["double", "half", "learn"]));
    assert_eq!(playback["target"]["future_target"], json!({"kept": true}));
    let route = candidate.object("route", "one").unwrap().body();
    assert_eq!(route["delivery_mode"], "broadcast");
    assert!(route["destination"].is_null());
    assert_eq!(route["minimum_slots"], 512);
    assert_eq!(route["future_route"], json!({"kept": true}));

    let migrated = candidate
        .objects()
        .map(|object| {
            (
                object.key().kind().to_owned(),
                object.key().id().to_owned(),
                object.body().clone(),
            )
        })
        .collect::<Vec<_>>();
    let legacy_snapshot = compile_show_candidate(candidate).unwrap();
    let migrated_refs = migrated
        .iter()
        .map(|(kind, id, body)| (kind.as_str(), id.as_str(), body.clone()))
        .collect::<Vec<_>>();
    let (_, current_document) = document_with_objects(&migrated_refs);
    let mut current_transaction = current_document.transaction();
    stage_candidate_migrations(&current_document, &mut current_transaction).unwrap();
    assert!(current_transaction.is_empty());
    let current_snapshot =
        compile_show_candidate(current_document.candidate(&current_transaction).unwrap()).unwrap();
    assert_eq!(
        snapshot_without_revision(legacy_snapshot),
        snapshot_without_revision(current_snapshot)
    );
}

#[test]
fn dynamics_compile_preset_sources_into_per_target_sampler_fallbacks() {
    let target = light_core::FixtureId::new();
    let dynamic = light_dynamics::DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 7,
        revision: 1,
        name: "Preset wave".into(),
        color: None,
        icon: None,
        overall_speed_multiplier: light_dynamics::Rational::ONE,
        target_binding: light_dynamics::DynamicTargetBinding::FrozenTargets {
            targets: vec![target],
        },
        lanes: vec![light_dynamics::DynamicLane {
            id: Uuid::new_v4(),
            attribute: light_core::AttributeKey::intensity(),
            mode: light_dynamics::DynamicLaneMode::Keyframes,
            keyframes: light_dynamics::KeyframeConfiguration {
                points: vec![
                    light_dynamics::DynamicKeyframe {
                        position: 0.0,
                        source: light_dynamics::ScalarSource::Preset {
                            preset_id: "1.1".into(),
                            attribute: light_core::AttributeKey::intensity(),
                            last_valid_by_target: Vec::new(),
                        },
                        interpolation: light_dynamics::ScalarInterpolation::Linear,
                    },
                    light_dynamics::DynamicKeyframe {
                        position: 0.5,
                        source: light_dynamics::ScalarSource::Value { value: 1.0 },
                        interpolation: light_dynamics::ScalarInterpolation::Linear,
                    },
                ],
                size: 1.0,
            },
            max_min: light_dynamics::MaxMinConfiguration {
                minimum: light_dynamics::ScalarSource::Value { value: 0.0 },
                maximum: light_dynamics::ScalarSource::Value { value: 1.0 },
                function: light_dynamics::PeriodicFunction::Sinus,
                size: 1.0,
                pwm: light_dynamics::PwmShape::default(),
            },
            middle_amplitude: light_dynamics::MiddleAmplitudeConfiguration {
                middle: light_dynamics::ScalarSource::Current,
                amplitude: 0.5,
                function: light_dynamics::PeriodicFunction::Sinus,
                size: 1.0,
                pwm: light_dynamics::PwmShape::default(),
            },
            speed_multiplier: light_dynamics::Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
        }],
        random_groups: Vec::new(),
        phase_spread_mode: light_dynamics::DynamicPhaseSpreadMode::Uniform,
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
    let objects = vec![
        ("dynamic", "7", serde_json::to_value(dynamic).unwrap()),
        (
            "dynamic",
            "broken",
            json!({
                "id": Uuid::new_v4(),
                "pool_number": 8,
                "revision": 1,
                "name": "Needs repair",
                "lanes": "not-an-array"
            }),
        ),
        (
            "preset",
            "1.1",
            json!({
                "name": "Dim",
                "family": "Intensity",
                "number": 1,
                "values": {
                    target.0.to_string(): {
                        "intensity": {"kind": "normalized", "value": 0.75}
                    }
                }
            }),
        ),
    ];
    let refs = objects
        .iter()
        .map(|(kind, id, body)| (*kind, *id, body.clone()))
        .collect::<Vec<_>>();
    let (_, document) = document_with_objects(&refs);
    let snapshot =
        compile_show_candidate(document.candidate(&document.transaction()).unwrap()).unwrap();
    assert_eq!(
        snapshot.dynamics.len(),
        1,
        "a malformed Dynamic remains repairable show content but is not runtime-installed"
    );
    let light_dynamics::ScalarSource::Preset {
        last_valid_by_target,
        ..
    } = &snapshot.dynamics[0].lanes[0].keyframes.points[0].source
    else {
        panic!("expected Preset source")
    };
    assert_eq!(
        last_valid_by_target,
        &[light_dynamics::TargetScalarFallback {
            target,
            value: 0.75,
        }]
    );
}

#[test]
fn dynamics_persist_preset_fallbacks_losslessly_before_the_preset_is_deleted() {
    let target = light_core::FixtureId::new();
    let dynamic = light_dynamics::DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 7,
        revision: 1,
        name: "Portable preset wave".into(),
        color: None,
        icon: None,
        overall_speed_multiplier: light_dynamics::Rational::ONE,
        target_binding: light_dynamics::DynamicTargetBinding::FrozenTargets {
            targets: vec![target],
        },
        lanes: vec![light_dynamics::DynamicLane {
            id: Uuid::new_v4(),
            attribute: light_core::AttributeKey::intensity(),
            mode: light_dynamics::DynamicLaneMode::Keyframes,
            keyframes: light_dynamics::KeyframeConfiguration {
                points: vec![
                    light_dynamics::DynamicKeyframe {
                        position: 0.0,
                        source: light_dynamics::ScalarSource::Preset {
                            preset_id: "1.1".into(),
                            attribute: light_core::AttributeKey::intensity(),
                            last_valid_by_target: Vec::new(),
                        },
                        interpolation: light_dynamics::ScalarInterpolation::Linear,
                    },
                    light_dynamics::DynamicKeyframe {
                        position: 0.5,
                        source: light_dynamics::ScalarSource::Value { value: 1.0 },
                        interpolation: light_dynamics::ScalarInterpolation::Linear,
                    },
                ],
                size: 1.0,
            },
            max_min: light_dynamics::MaxMinConfiguration {
                minimum: light_dynamics::ScalarSource::Value { value: 0.0 },
                maximum: light_dynamics::ScalarSource::Value { value: 1.0 },
                function: light_dynamics::PeriodicFunction::Sinus,
                size: 1.0,
                pwm: light_dynamics::PwmShape::default(),
            },
            middle_amplitude: light_dynamics::MiddleAmplitudeConfiguration {
                middle: light_dynamics::ScalarSource::Current,
                amplitude: 0.5,
                function: light_dynamics::PeriodicFunction::Sinus,
                size: 1.0,
                pwm: light_dynamics::PwmShape::default(),
            },
            speed_multiplier: light_dynamics::Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
        }],
        random_groups: Vec::new(),
        phase_spread_mode: light_dynamics::DynamicPhaseSpreadMode::Uniform,
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
    let mut dynamic_body = serde_json::to_value(dynamic).unwrap();
    dynamic_body["future_dynamic"] = json!({"kept": true});
    dynamic_body["lanes"][0]["keyframes"]["points"][0]["source"]["future_source"] =
        json!({"kept": true});
    let preset = json!({
        "name": "Dim",
        "family": "Intensity",
        "number": 1,
        "values": {
            target.0.to_string(): {
                "intensity": {"kind": "normalized", "value": 0.75}
            }
        }
    });
    let objects = vec![("dynamic", "7", dynamic_body), ("preset", "1.1", preset)];
    let (store, document) = document_with_objects(&objects);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();
    let migrated_dynamic = candidate.object("dynamic", "7").unwrap().body().clone();

    assert_eq!(migrated_dynamic["future_dynamic"], json!({"kept": true}));
    assert_eq!(
        migrated_dynamic["lanes"][0]["keyframes"]["points"][0]["source"]["future_source"],
        json!({"kept": true})
    );
    assert_eq!(
        migrated_dynamic["lanes"][0]["keyframes"]["points"][0]["source"]["last_valid_by_target"][0]
            ["value"],
        0.75
    );
    assert_eq!(
        stored_body(&store, "dynamic", "7")["lanes"][0]["keyframes"]["points"][0]["source"]["last_valid_by_target"],
        json!([]),
        "staging the migration must remain side-effect free"
    );

    let fallback_only = vec![("dynamic", "7", migrated_dynamic.clone())];
    let decoded: light_dynamics::DynamicDefinition =
        serde_json::from_value(migrated_dynamic.clone()).unwrap();
    light_dynamics::validate_definition(&decoded).unwrap();
    let (_, fallback_document) = document_with_objects(&fallback_only);
    let snapshot = compile_show_candidate(
        fallback_document
            .candidate(&fallback_document.transaction())
            .unwrap(),
    )
    .unwrap();
    let light_dynamics::ScalarSource::Preset {
        last_valid_by_target,
        ..
    } = &snapshot.dynamics[0].lanes[0].keyframes.points[0].source
    else {
        panic!("expected Preset source")
    };
    assert_eq!(
        last_valid_by_target,
        &[light_dynamics::TargetScalarFallback {
            target,
            value: 0.75,
        }]
    );

    let migrated_refs = [
        ("dynamic", "7", migrated_dynamic),
        ("preset", "1.1", objects[1].2.clone()),
    ];
    let (_, migrated_document) = document_with_objects(&migrated_refs);
    let mut idempotent = migrated_document.transaction();
    stage_candidate_migrations(&migrated_document, &mut idempotent).unwrap();
    assert!(idempotent.is_empty());
}
