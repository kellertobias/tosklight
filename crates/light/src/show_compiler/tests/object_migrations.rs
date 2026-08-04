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
    let group = candidate.object("group", "7").unwrap().body();
    assert_eq!(group["id"], "7");
    assert!(group.get("grid").is_none());
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
fn group_sources_backfill_losslessly_once_and_canonical_source_wins() {
    let first = "00000000-0000-0000-0000-000000000101";
    let second = "00000000-0000-0000-0000-000000000102";
    let explicit = json!({
        "name": "Explicit",
        "fixtures": [second, first],
        "future_group": {"kept": true}
    });
    let empty = json!({"name": "Empty", "fixtures": []});
    let derived = json!({
        "name": "Derived",
        "fixtures": [first],
        "derived_from": {
            "source_group_id": "base",
            "rule": {"type": "every_nth", "n": 3, "offset": 1, "future_rule": true},
            "future_derived": [3, 2, 1]
        }
    });
    let frozen = json!({
        "name": "Frozen",
        "fixtures": [first, second],
        "frozen_from": {
            "source_group_id": "base",
            "source_revision": 9,
            "captured_at": "2026-08-02T00:00:00Z",
            "future_frozen": {"kept": true}
        }
    });
    let gridded = json!({
        "name": "Gridded",
        "fixtures": [second],
        "grid": {
            "method": "rows_first",
            "columns": 2,
            "future_grid": {"kept": true}
        }
    });
    let canonical = json!({
        "name": "Canonical",
        "fixtures": [first],
        "derived_from": {
            "source_group_id": "legacy-base",
            "rule": {"type": "all"}
        },
        "source": {
            "type": "explicit",
            "fixture_ids": [],
            "future_source": {"kept": true}
        }
    });
    let originals = vec![
        ("group", "1", explicit),
        ("group", "2", empty),
        ("group", "3", derived),
        ("group", "4", frozen),
        ("group", "5", gridded),
        ("group", "6", canonical.clone()),
    ];
    let (store, document) = document_with_objects(&originals);
    let mut transaction = document.transaction();

    stage_candidate_migrations(&document, &mut transaction).unwrap();

    let candidate = document.candidate(&transaction).unwrap();
    assert_eq!(
        candidate.object("group", "1").unwrap().body()["source"],
        json!({"type": "explicit", "fixture_ids": [second, first]})
    );
    assert_eq!(
        candidate.object("group", "1").unwrap().body()["future_group"],
        json!({"kept": true})
    );
    assert_eq!(
        candidate.object("group", "2").unwrap().body()["source"],
        json!({"type": "explicit", "fixture_ids": []})
    );
    let migrated_derived = candidate.object("group", "3").unwrap().body();
    assert_eq!(
        migrated_derived["source"],
        json!({
            "type": "references",
            "references": [{
                "group_id": "base",
                "rule": {"type": "every_nth", "n": 3, "offset": 1, "future_rule": true}
            }]
        })
    );
    assert_eq!(
        migrated_derived["derived_from"]["future_derived"],
        json!([3, 2, 1])
    );
    let migrated_frozen = candidate.object("group", "4").unwrap().body();
    assert_eq!(
        migrated_frozen["source"],
        json!({"type": "explicit", "fixture_ids": [first, second]})
    );
    assert_eq!(
        migrated_frozen["frozen_from"]["future_frozen"],
        json!({"kept": true})
    );
    let migrated_gridded = candidate.object("group", "5").unwrap().body();
    assert_eq!(
        migrated_gridded["source"],
        json!({"type": "explicit", "fixture_ids": [second]})
    );
    assert_eq!(migrated_gridded["grid"], originals[4].2["grid"]);
    let migrated_canonical = candidate.object("group", "6").unwrap().body();
    assert_eq!(migrated_canonical["source"], canonical["source"]);
    assert_eq!(migrated_canonical["fixtures"], canonical["fixtures"]);
    assert_eq!(
        migrated_canonical["derived_from"],
        canonical["derived_from"]
    );

    let first_commit = store.apply_portable_transaction(transaction).unwrap();
    assert_eq!(first_commit.written_objects().len(), originals.len());
    let migrated_document = store.portable_document().unwrap();
    let show_revision = migrated_document.revision();
    let object_revisions = (1..=6)
        .map(|id| {
            migrated_document
                .object("group", &id.to_string())
                .unwrap()
                .revision()
        })
        .collect::<Vec<_>>();
    let mut second_pass = migrated_document.transaction();
    stage_candidate_migrations(&migrated_document, &mut second_pass).unwrap();
    assert!(
        second_pass.is_empty(),
        "Group source migration must be idempotent"
    );
    let second_commit = store.apply_portable_transaction(second_pass).unwrap();
    assert_eq!(second_commit.revision(), show_revision);
    assert!(second_commit.written_objects().is_empty());
    let stable_document = store.portable_document().unwrap();
    assert_eq!(stable_document.revision(), show_revision);
    assert_eq!(
        (1..=6)
            .map(|id| {
                stable_document
                    .object("group", &id.to_string())
                    .unwrap()
                    .revision()
            })
            .collect::<Vec<_>>(),
        object_revisions
    );
}

#[test]
fn group_master_migration_reconciles_assignments_strips_groups_and_is_idempotent() {
    let group = |name: &str, master: f64| {
        json!({
            "name": name,
            "fixtures": [],
            "master": master,
            "playback_fader": 77,
            "future_group": {"kept": true}
        })
    };
    let playback = |number: u16, group_id: &str, initial_master: Option<f32>| {
        let mut target = json!({"type": "group", "group_id": group_id});
        if let Some(initial_master) = initial_master {
            target["initial_master"] = json!(initial_master);
        }
        json!({
            "number": number,
            "name": format!("Playback {number}"),
            "target": target,
            "future_playback": {"kept": number}
        })
    };
    let objects = [
        ("group", "front", group("Front", 0.5)),
        ("group", "back", group("Back", 0.625)),
        (
            "group",
            "orphan",
            json!({
                "name": "Orphan",
                "fixtures": [],
                "master": "invalid but unassigned",
                "playback_fader": 99,
                "future_group": {"kept": true}
            }),
        ),
        ("playback", "9", playback(9, "front", Some(0.75))),
        ("playback", "2", playback(2, "front", Some(0.25))),
        ("playback", "3", playback(3, "back", None)),
        (
            "playback_page",
            "1",
            json!({
                "number": 1,
                "name": "Virtual",
                "slots": {},
                "virtual_playbacks": {
                    "1001": playback(1001, "front", Some(0.875)),
                    "1002": playback(1002, "back", None)
                },
                "future_page": {"kept": true}
            }),
        ),
    ];
    let object_refs = objects
        .iter()
        .map(|(kind, id, body)| (*kind, *id, body.clone()))
        .collect::<Vec<_>>();
    let (store, document) = document_with_objects(&object_refs);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();

    for group_id in ["front", "back", "orphan"] {
        let body = candidate.object("group", group_id).unwrap().body();
        assert!(body.get("master").is_none());
        assert!(body.get("playback_fader").is_none());
        assert_eq!(body["future_group"], json!({"kept": true}));
    }
    for number in ["2", "9"] {
        let body = candidate.object("playback", number).unwrap().body();
        assert_eq!(body["target"]["initial_master"], 0.25);
        assert_eq!(body["future_playback"]["kept"], body["number"]);
    }
    assert_eq!(
        candidate.object("playback", "3").unwrap().body()["target"]["initial_master"],
        0.625
    );
    let page = candidate.object("playback_page", "1").unwrap().body();
    assert_eq!(
        page["virtual_playbacks"]["1001"]["target"]["initial_master"],
        0.25
    );
    assert_eq!(
        page["virtual_playbacks"]["1002"]["target"]["initial_master"],
        0.625
    );
    assert_eq!(page["future_page"], json!({"kept": true}));

    store.apply_portable_transaction(transaction).unwrap();
    let migrated = store.portable_document().unwrap();
    let revision = migrated.revision();
    let mut second_pass = migrated.transaction();
    stage_candidate_migrations(&migrated, &mut second_pass).unwrap();
    assert!(
        second_pass.is_empty(),
        "Group Master migration must be idempotent"
    );
    assert_eq!(
        store
            .apply_portable_transaction(second_pass)
            .unwrap()
            .revision(),
        revision
    );
}

#[test]
fn targetless_dynamic_assignments_migrate_to_distinct_target_bound_fallbacks() {
    let original = targetless_dynamic(7);
    let original_id = original.id;
    let first_target = light_core::FixtureId::new();
    let second_target = light_core::FixtureId::new();
    let mut first = dynamic_playback(
        1,
        &original,
        light_playback::DynamicPlaybackTargetScope::FrozenTargets {
            targets: vec![first_target],
        },
    );
    let second = dynamic_playback(
        2,
        &original,
        light_playback::DynamicPlaybackTargetScope::LiveGroup {
            group_id: "front".into(),
        },
    );
    first.name = "First legacy scope".into();
    let mut first_body = serde_json::to_value(first).unwrap();
    first_body["future_playback"] = json!({"kept": true});
    let mut virtual_playback = dynamic_playback(
        1_001,
        &original,
        light_playback::DynamicPlaybackTargetScope::FrozenTargets {
            targets: vec![second_target],
        },
    );
    virtual_playback.has_fader = false;
    virtual_playback.button_count = 1;
    virtual_playback.buttons[1] = light_playback::PlaybackButtonAction::None;
    virtual_playback.buttons[2] = light_playback::PlaybackButtonAction::None;
    let objects = vec![
        ("dynamic", "7", serde_json::to_value(&original).unwrap()),
        ("playback", "1", first_body),
        ("playback", "2", serde_json::to_value(second).unwrap()),
        (
            "playback_page",
            "1",
            json!({
                "number": 1,
                "name": "Virtual",
                "slots": {},
                "virtual_playbacks": {"1001": virtual_playback},
                "future_page": {"kept": true}
            }),
        ),
    ];
    let (store, document) = document_with_objects(&objects);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();

    let first = candidate.object("playback", "1").unwrap().body();
    let second = candidate.object("playback", "2").unwrap().body();
    let virtual_playback =
        &candidate.object("playback_page", "1").unwrap().body()["virtual_playbacks"]["1001"];
    for playback in [first, second, virtual_playback] {
        assert!(playback["target"]["assignment"]["dynamic"]["dynamic_id"].is_null());
        assert!(playback["target"]["assignment"]["target_scope"].is_null());
    }
    assert_eq!(
        migrated_dynamic_definition(first)["target_binding"],
        json!({"type":"frozen_targets","targets":[first_target]})
    );
    assert_eq!(
        migrated_dynamic_definition(second)["target_binding"],
        json!({"type":"live_group","group_id":"front"})
    );
    assert_eq!(
        migrated_dynamic_definition(virtual_playback)["target_binding"],
        json!({"type":"frozen_targets","targets":[second_target]})
    );
    let migrated_ids = [first, second, virtual_playback]
        .map(|playback| {
            migrated_dynamic_definition(playback)["id"]
                .as_str()
                .unwrap()
        })
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(migrated_ids.len(), 3);
    let original_id = original_id.to_string();
    assert!(!migrated_ids.contains(original_id.as_str()));
    assert_eq!(first["future_playback"], json!({"kept": true}));
    assert_eq!(
        candidate.object("playback_page", "1").unwrap().body()["future_page"],
        json!({"kept": true})
    );
    assert_eq!(
        candidate.object("dynamic", "7").unwrap().body()["target_binding"],
        json!({"type":"targetless"})
    );
    assert_eq!(
        stored_body(&store, "playback", "1")["target"]["assignment"]["dynamic"]["dynamic_id"],
        original_id.to_string()
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
    assert!(second_pass.is_empty());
}

#[test]
fn targetless_dynamic_assignment_without_legacy_scope_fails_visibly() {
    let original = targetless_dynamic(7);
    let mut playback = serde_json::to_value(dynamic_playback(
        1,
        &original,
        light_playback::DynamicPlaybackTargetScope::FrozenTargets {
            targets: vec![light_core::FixtureId::new()],
        },
    ))
    .unwrap();
    playback["target"]["assignment"]["target_scope"] = serde_json::Value::Null;
    let objects = vec![
        ("dynamic", "7", serde_json::to_value(original).unwrap()),
        ("playback", "1", playback),
    ];
    let (_, document) = document_with_objects(&objects);
    let mut transaction = document.transaction();

    let error = stage_candidate_migrations(&document, &mut transaction).unwrap_err();

    assert_eq!(
        error.message,
        "invalid playback 1: legacy targetless Dynamic Playback assignment has no stored target scope"
    );
    assert!(transaction.is_empty());
}

#[test]
fn legacy_dynamic_phase_ordering_migrates_losslessly_across_pool_cue_and_playback_fallbacks() {
    let target = light_core::FixtureId::new();
    let mut dynamic = targetless_dynamic(7);
    dynamic.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    dynamic.phase.ordering = light_dynamics::PhaseOrdering::RadialIn {
        center_x: 1.25,
        center_z: -2.5,
    };
    let mut dynamic_body = serde_json::to_value(&dynamic).unwrap();
    dynamic_body["future_dynamic"] = json!({"kept": true});

    let reference = light_dynamics::DynamicReference {
        dynamic_id: Some(dynamic.id),
        last_known_pool_number: dynamic.pool_number,
        embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
            definition: std::sync::Arc::new(dynamic.clone()),
        },
    };
    let mut cue = Cue::new(1.0);
    cue.dynamic_changes.push(light_playback::CueDynamicChange {
        fixture_id: target,
        attribute: light_core::AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link: Uuid::new_v4(),
            dynamic: reference,
            lane_id: Uuid::new_v4(),
            overrides: light_dynamics::DynamicInstanceOverrides {
                size: 1.0,
                speed_multiplier: light_dynamics::Rational::ONE,
                phase_offset_degrees: 0.0,
            },
            timing: light_dynamics::DynamicValueTiming::default(),
        },
        automatic_restore: false,
    });
    let cue_list = CueList {
        id: CueListId::new(),
        name: "Legacy Dynamic fallback".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Tracking),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues: vec![cue],
    };
    let playback = dynamic_playback(
        1,
        &dynamic,
        light_playback::DynamicPlaybackTargetScope::FrozenTargets {
            targets: vec![target],
        },
    );
    let objects = vec![
        ("dynamic", "7", dynamic_body),
        ("cue_list", "1", serde_json::to_value(cue_list).unwrap()),
        ("playback", "1", serde_json::to_value(playback).unwrap()),
    ];
    let (store, document) = document_with_objects(&objects);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();

    let pool = candidate.object("dynamic", "7").unwrap().body();
    let cue_fallback = &candidate.object("cue_list", "1").unwrap().body()["cues"][0]["dynamic_changes"]
        [0]["value"]["dynamic"]["embedded_fallback"]["definition"];
    let playback_fallback =
        migrated_dynamic_definition(candidate.object("playback", "1").unwrap().body());
    for definition in [pool, cue_fallback, playback_fallback] {
        assert_eq!(definition["phase"]["ordering"]["type"], "radial_in");
        assert_eq!(
            definition["spatial_mapping"]["shape"]["value"]["type"],
            "radial"
        );
        assert_eq!(
            definition["spatial_mapping"]["shape"]["value"]["direction"],
            "inward"
        );
    }
    assert_eq!(pool["future_dynamic"], json!({"kept": true}));
    assert!(
        stored_body(&store, "dynamic", "7")
            .get("spatial_mapping")
            .is_none(),
        "migration must remain staged until the candidate commit"
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
    assert!(
        second_pass.is_empty(),
        "Dynamic migration must be idempotent"
    );
}

fn migrated_dynamic_definition(playback: &serde_json::Value) -> &serde_json::Value {
    &playback["target"]["assignment"]["dynamic"]["embedded_fallback"]["definition"]
}

fn targetless_dynamic(pool_number: u16) -> light_dynamics::DynamicDefinition {
    light_dynamics::DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number,
        revision: 1,
        name: format!("Targetless {pool_number}"),
        color: None,
        icon: None,
        target_binding: light_dynamics::DynamicTargetBinding::Targetless,
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
        overall_speed_multiplier: light_dynamics::Rational::ONE,
        run_mode: light_dynamics::DynamicRunMode::Loop,
        default_activation: light_dynamics::ActivationPolicy::StartNow,
        activation_boundary: light_dynamics::ActivationBoundary::Beat,
    }
}

fn dynamic_playback(
    number: u16,
    definition: &light_dynamics::DynamicDefinition,
    target_scope: light_playback::DynamicPlaybackTargetScope,
) -> light_playback::PlaybackDefinition {
    let target = light_playback::PlaybackTarget::Dynamic {
        assignment: light_playback::DynamicPlaybackAssignment {
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(definition.id),
                last_known_pool_number: definition.pool_number,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: std::sync::Arc::new(definition.clone()),
                },
            },
            revision: 1,
            target_scope: Some(target_scope),
            fader_mode: light_playback::DynamicPlaybackFaderMode::SizeAndMaster,
            priority: 0,
            activation_override: None,
            resume_policy: light_playback::DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: true,
            auto_off_flash_release: true,
            auto_off_full_control: true,
        },
    };
    light_playback::PlaybackDefinition {
        number,
        name: format!("Dynamic Playback {number}"),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        target,
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
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
                invert_waveform: false,
            },
            speed_multiplier: light_dynamics::Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
        }],
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
                invert_waveform: false,
            },
            speed_multiplier: light_dynamics::Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
        }],
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

#[test]
fn retired_strobe_identity_migrates_losslessly_across_show_value_owners() {
    let fixture = light_core::FixtureId::new();
    let strobe = light_core::AttributeKey("strobe".into());
    let mut cue = Cue::new(1.0);
    cue.changes.push(light_playback::CueChange::set(
        fixture,
        strobe.clone(),
        light_core::AttributeValue::Normalized(0.6),
    ));
    let cue_list = CueList {
        id: CueListId::new(),
        name: "Legacy Strobe".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Tracking),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues: vec![cue],
    };
    let mut dynamic = targetless_dynamic(7);
    dynamic.lanes.push(strobe_lane(strobe.clone()));
    let mut dynamic_body = serde_json::to_value(dynamic).unwrap();
    dynamic_body["future_dynamic"] = json!({"kept": true});
    let objects = vec![
        (
            "group",
            "front",
            json!({
                "name": "Front",
                "programming": {"strobe": {"kind":"normalized","value":0.2}},
                "future_group": {"kept": true}
            }),
        ),
        (
            "preset",
            "1.1",
            json!({
                "name": "Legacy Strobe",
                "family": "Intensity",
                "number": 1,
                "values": {fixture.0.to_string(): {
                    "strobe": {"kind":"normalized","value":0.4}
                }},
                "group_values": {"front": {
                    "strobe": {"kind":"normalized","value":0.5}
                }},
                "future_preset": {"kept": true}
            }),
        ),
        ("cue_list", "main", serde_json::to_value(cue_list).unwrap()),
        ("dynamic", "7", dynamic_body),
    ];
    let (store, document) = document_with_objects(&objects);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();

    let group = candidate.object("group", "front").unwrap().body();
    assert_eq!(group["programming"]["shutter"]["value"], 0.2);
    assert!(group["programming"].get("strobe").is_none());
    assert_eq!(group["future_group"], json!({"kept": true}));
    let preset = candidate.object("preset", "1.1").unwrap().body();
    assert_eq!(
        preset["values"][fixture.0.to_string()]["shutter"]["value"],
        0.4
    );
    assert_eq!(preset["group_values"]["front"]["shutter"]["value"], 0.5);
    assert_eq!(preset["future_preset"], json!({"kept": true}));
    assert_eq!(
        candidate.object("cue_list", "main").unwrap().body()["cues"][0]["changes"][0]["attribute"],
        "shutter"
    );
    let dynamic = candidate.object("dynamic", "7").unwrap().body();
    assert_eq!(dynamic["lanes"][0]["attribute"], "shutter");
    assert_eq!(
        dynamic["lanes"][0]["keyframes"]["points"][0]["source"]["attribute"],
        "shutter"
    );
    assert_eq!(dynamic["future_dynamic"], json!({"kept": true}));
    assert_eq!(
        stored_body(&store, "group", "front")["programming"]["strobe"]["value"],
        0.2
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
    assert!(
        second_pass.is_empty(),
        "second pass changed {:?}",
        second_pass
            .changed_object_keys()
            .map(|key| (key.kind(), key.id()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn retired_strobe_identity_reports_an_actionable_address_conflict() {
    let fixture = light_core::FixtureId::new();
    let preset = json!({
        "name": "Conflict",
        "family": "Intensity",
        "number": 1,
        "values": {fixture.0.to_string(): {
            "strobe": {"kind":"normalized","value":0.4},
            "shutter": {"kind":"normalized","value":0.5}
        }},
        "group_values": {}
    });
    let (_, document) = document_with_objects(&[("preset", "1.1", preset)]);
    let mut transaction = document.transaction();

    let error = stage_candidate_migrations(&document, &mut transaction).unwrap_err();

    assert!(error.message.contains("attribute migration conflict"));
    assert!(error.message.contains("/values/"));
    assert!(error.message.contains("shutter"));
    assert!(
        transaction.is_empty(),
        "failed migrations remain side-effect free"
    );
}

#[test]
fn legacy_position_movement_values_migrate_and_conflicting_axes_stop_atomically() {
    let fixture = light_core::FixtureId::new();
    let preset = json!({
        "name": "Movement",
        "family": "Position",
        "number": 1,
        "values": {fixture.0.to_string(): {
            "fixture.mspeed": {"kind":"normalized","value":0.4}
        }},
        "group_values": {}
    });
    let (_, document) = document_with_objects(&[("preset", "3.1", preset)]);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();
    let values =
        &candidate.object("preset", "3.1").unwrap().body()["values"][fixture.0.to_string()];
    assert_eq!(values["position.movement"]["value"], 0.4);
    assert!(values.get("fixture.mspeed").is_none());

    let conflict = json!({
        "name": "Conflict",
        "family": "Position",
        "number": 2,
        "values": {fixture.0.to_string(): {
            "pan.time": {"kind":"normalized","value":0.4},
            "tilt.time": {"kind":"normalized","value":0.5}
        }},
        "group_values": {}
    });
    let (_, document) = document_with_objects(&[("preset", "3.2", conflict)]);
    let mut transaction = document.transaction();
    let error = stage_candidate_migrations(&document, &mut transaction).unwrap_err();
    assert!(error.message.contains("attribute migration conflict"));
    assert!(error.message.contains("position.movement"));
    assert!(transaction.is_empty());
}

#[test]
fn legacy_media_values_migrate_and_source_or_target_collisions_stop_atomically() {
    let fixture = light_core::FixtureId::new();
    let preset = json!({
        "name": "Media",
        "family": "Mixed",
        "number": 1,
        "values": {fixture.0.to_string(): {
            "media.opacity": {"kind":"normalized","value":0.4},
            "media.rotation": {"kind":"normalized","value":0.6},
            "media.tint": {"kind":"color_xyz","value":{"x":0.2,"y":0.3,"z":0.4}}
        }},
        "group_values": {}
    });
    let (_, document) = document_with_objects(&[("preset", "0.1", preset)]);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();
    let values =
        &candidate.object("preset", "0.1").unwrap().body()["values"][fixture.0.to_string()];
    assert_eq!(values["intensity"]["value"], 0.4);
    assert_eq!(values["position.rotation"]["value"], 0.6);
    assert_eq!(values["color"]["value"]["x"], 0.2);
    assert!(values.get("media.opacity").is_none());
    assert!(values.get("media.rotation").is_none());
    assert!(values.get("media.tint").is_none());

    for (conflicting_values, target) in [
        (
            json!({
                "media.opacity": {"kind":"normalized","value":0.4},
                "intensity": {"kind":"normalized","value":0.5}
            }),
            "intensity",
        ),
        (
            json!({
                "media.tint": {"kind":"color_xyz","value":{"x":0.2,"y":0.3,"z":0.4}},
                "color": {"kind":"color_xyz","value":{"x":0.4,"y":0.3,"z":0.2}}
            }),
            "color",
        ),
    ] {
        let preset = json!({
            "name": "Conflict",
            "family": "Mixed",
            "number": 2,
            "values": {fixture.0.to_string(): conflicting_values},
            "group_values": {}
        });
        let (_, document) = document_with_objects(&[("preset", "0.2", preset)]);
        let mut transaction = document.transaction();
        let error = stage_candidate_migrations(&document, &mut transaction).unwrap_err();
        assert!(error.message.contains("attribute migration conflict"));
        assert!(error.message.contains(target));
        assert!(transaction.is_empty());
    }
}

#[test]
fn legacy_emitter_and_softness_controls_retire_with_values_and_preserve_unknown_configuration_data()
{
    let mut configuration = light_core::AttributeConfiguration::recommended();
    for (source, encoder, label) in [
        (
            "color.cyan",
            light_core::EncoderPlacement::new(light_core::EncoderGroup::Color, 4, 1),
            "Cyan",
        ),
        (
            "color.magenta",
            light_core::EncoderPlacement::new(light_core::EncoderGroup::Color, 4, 2),
            "Magenta",
        ),
        (
            "color.yellow",
            light_core::EncoderPlacement::new(light_core::EncoderGroup::Color, 4, 3),
            "Yellow",
        ),
    ] {
        configuration
            .placements
            .push(light_core::AttributePlacement {
                attribute: light_core::AttributeKey(source.into()),
                encoder,
                push_turn_of: None,
            });
        configuration
            .activation_groups
            .push(light_core::AttributeActivationGroup {
                id: source.into(),
                label: label.into(),
                members: vec![light_core::AttributeKey(source.into())],
            });
    }
    configuration
        .placements
        .push(light_core::AttributePlacement {
            attribute: light_core::AttributeKey("color.tint".into()),
            encoder: light_core::EncoderPlacement::new(light_core::EncoderGroup::Color, 3, 1),
            push_turn_of: None,
        });
    configuration
        .activation_groups
        .iter_mut()
        .find(|group| group.id == "color_mix")
        .unwrap()
        .members
        .push(light_core::AttributeKey("color.tint".into()));
    for (source, encoder) in [
        (
            "color.cold_white",
            light_core::EncoderPlacement::new(light_core::EncoderGroup::Color, 2, 1),
        ),
        (
            "color.warm_white",
            light_core::EncoderPlacement::new(light_core::EncoderGroup::Color, 2, 2),
        ),
    ] {
        configuration
            .placements
            .push(light_core::AttributePlacement {
                attribute: light_core::AttributeKey(source.into()),
                encoder,
                push_turn_of: None,
            });
        configuration
            .activation_groups
            .iter_mut()
            .find(|group| group.id == "color_mix")
            .unwrap()
            .members
            .push(light_core::AttributeKey(source.into()));
    }
    for (source, encoder, label) in [
        (
            "frost.1",
            light_core::EncoderPlacement::new(light_core::EncoderGroup::Focus, 1, 3),
            "Frost 1",
        ),
        (
            "beam.edge",
            light_core::EncoderPlacement::new(light_core::EncoderGroup::Focus, 1, 5),
            "Beam Edge",
        ),
    ] {
        configuration
            .placements
            .push(light_core::AttributePlacement {
                attribute: light_core::AttributeKey(source.into()),
                encoder,
                push_turn_of: None,
            });
        configuration
            .activation_groups
            .push(light_core::AttributeActivationGroup {
                id: source.into(),
                label: label.into(),
                members: vec![light_core::AttributeKey(source.into())],
            });
    }
    let mut body = serde_json::to_value(configuration).unwrap();
    body["future_configuration"] = json!({"kept": true});
    let cct_group = json!({
        "name": "Legacy CCT",
        "programming": {
            "color.cold_white": {"kind":"normalized","value":0.25,"future_value":"kept"},
            "color.warm_white": {"kind":"normalized","value":0.75}
        }
    });
    let softness_group = json!({
        "name": "Legacy Frost",
        "programming": {
            "frost.1": {"kind":"normalized","value":0.45,"future_value":"kept"}
        }
    });
    let (_, document) = document_with_objects(&[
        ("attribute_configuration", "default", body),
        ("group", "cct", cct_group),
        ("group", "softness", softness_group),
    ]);
    let mut transaction = document.transaction();

    stage_candidate_migrations(&document, &mut transaction).unwrap();

    let candidate = document.candidate(&transaction).unwrap();
    let migrated = candidate
        .object("attribute_configuration", "default")
        .unwrap()
        .body();
    assert_eq!(migrated["future_configuration"], json!({"kept": true}));
    for source in [
        "color.cyan",
        "color.magenta",
        "color.yellow",
        "color.cold_white",
        "color.warm_white",
        "frost.1",
        "beam.edge",
        "color.tint",
    ] {
        assert!(
            migrated["placements"]
                .as_array()
                .unwrap()
                .iter()
                .all(|placement| placement["attribute"] != source)
        );
        assert!(
            migrated["activation_groups"]
                .as_array()
                .unwrap()
                .iter()
                .all(|group| group["members"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|member| member != source))
        );
    }
    let decoded: light_core::AttributeConfiguration =
        serde_json::from_value(migrated.clone()).unwrap();
    decoded.validate().unwrap();
    let group = candidate.object("group", "cct").unwrap().body();
    assert_eq!(group["programming"]["color.white"]["value"], 0.25);
    assert_eq!(group["programming"]["color.white"]["future_value"], "kept");
    assert_eq!(group["programming"]["color.amber"]["value"], 0.75);
    let softness = candidate.object("group", "softness").unwrap().body();
    assert_eq!(softness["programming"]["softness"]["value"], 0.45);
    assert_eq!(softness["programming"]["softness"]["future_value"], "kept");

    let migrated_objects = candidate
        .objects()
        .map(|object| {
            (
                object.key().kind().to_owned(),
                object.key().id().to_owned(),
                object.body().clone(),
            )
        })
        .collect::<Vec<_>>();
    let migrated_refs = migrated_objects
        .iter()
        .map(|(kind, id, body)| (kind.as_str(), id.as_str(), body.clone()))
        .collect::<Vec<_>>();
    let (_, migrated_document) = document_with_objects(&migrated_refs);
    let mut second_pass = migrated_document.transaction();
    stage_candidate_migrations(&migrated_document, &mut second_pass).unwrap();
    assert!(second_pass.is_empty());
}

#[test]
fn legacy_cmy_static_values_migrate_inverse_to_rgb_without_losing_unknown_data() {
    let fixture = light_core::FixtureId::new();
    let cyan = light_core::AttributeKey("color.cyan".into());
    let mut cue = Cue::new(1.0);
    cue.changes.push(light_playback::CueChange::set(
        fixture,
        cyan.clone(),
        light_core::AttributeValue::Normalized(0.2),
    ));
    cue.group_changes.push(light_playback::GroupCueChange {
        group_id: "front".into(),
        attribute: cyan.clone(),
        value: Some(light_core::AttributeValue::Spread(vec![0.0, 0.25, 1.0])),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    cue.dynamic_changes.push(light_playback::CueDynamicChange {
        fixture_id: fixture,
        attribute: cyan,
        value: light_dynamics::DynamicSemanticValue::FixAt {
            value: 0.3,
            timing: light_dynamics::DynamicValueTiming::default(),
        },
        automatic_restore: false,
    });
    let cue_list = CueList {
        id: CueListId::new(),
        name: "Legacy CMY".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Tracking),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues: vec![cue],
    };
    let mut cue_body = serde_json::to_value(cue_list).unwrap();
    cue_body["cues"][0]["future_cue"] = json!({"kept": true});
    let objects = vec![
        (
            "group",
            "front",
            json!({
                "name": "Front",
                "programming": {"color.cyan": {
                    "kind":"normalized", "value":0.1, "future_value":{"kept":true}
                }}
            }),
        ),
        (
            "preset",
            "2.1",
            json!({
                "name": "Legacy Cyan",
                "family": "Color",
                "number": 1,
                "values": {fixture.0.to_string(): {
                    "color.cyan": {"kind":"normalized","value":0.4}
                }},
                "group_values": {"front": {
                    "color.cyan": {"kind":"spread","value":[0.0,0.5,1.0]}
                }}
            }),
        ),
        ("cue_list", "main", cue_body),
    ];
    let (_, document) = document_with_objects(&objects);
    let mut transaction = document.transaction();

    stage_candidate_migrations(&document, &mut transaction).unwrap();

    let candidate = document.candidate(&transaction).unwrap();
    let group = candidate.object("group", "front").unwrap().body();
    assert_json_number(&group["programming"]["color.red"]["value"], 0.9);
    assert_eq!(
        group["programming"]["color.red"]["future_value"],
        json!({"kept": true})
    );
    let preset = candidate.object("preset", "2.1").unwrap().body();
    assert_json_number(
        &preset["values"][fixture.0.to_string()]["color.red"]["value"],
        0.6,
    );
    assert_json_numbers(
        &preset["group_values"]["front"]["color.red"]["value"],
        &[1.0, 0.5, 0.0],
    );
    let cue = &candidate.object("cue_list", "main").unwrap().body()["cues"][0];
    assert_eq!(cue["changes"][0]["attribute"], "color.red");
    assert_json_number(&cue["changes"][0]["value"]["value"], 0.8);
    assert_eq!(cue["group_changes"][0]["attribute"], "color.red");
    assert_json_numbers(
        &cue["group_changes"][0]["value"]["value"],
        &[1.0, 0.75, 0.0],
    );
    assert_eq!(cue["dynamic_changes"][0]["attribute"], "color.red");
    assert_json_number(&cue["dynamic_changes"][0]["value"]["value"], 0.7);
    assert_eq!(cue["future_cue"], json!({"kept": true}));
}

#[test]
fn legacy_cmy_and_rgb_collision_stops_without_staging_partial_migration() {
    let fixture = light_core::FixtureId::new();
    let preset = json!({
        "name": "Conflict",
        "family": "Color",
        "number": 1,
        "values": {fixture.0.to_string(): {
            "color.cyan": {"kind":"normalized","value":0.4},
            "color.red": {"kind":"normalized","value":0.5}
        }},
        "group_values": {}
    });
    let (_, document) = document_with_objects(&[("preset", "2.1", preset)]);
    let mut transaction = document.transaction();

    let error = stage_candidate_migrations(&document, &mut transaction).unwrap_err();

    assert!(error.message.contains("attribute migration conflict"));
    assert!(error.message.contains("color.red"));
    assert!(transaction.is_empty());
}

#[test]
fn legacy_cmy_dynamic_waveforms_migrate_exactly_across_every_lane_mode() {
    let target = light_core::FixtureId::new();
    let random_group_id = Uuid::new_v4();
    let mut legacy = targetless_dynamic(19);
    legacy.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    legacy.lanes = vec![
        cmy_lane(
            "color.cyan",
            light_dynamics::DynamicLaneMode::Keyframes,
            None,
        ),
        cmy_lane(
            "color.magenta",
            light_dynamics::DynamicLaneMode::MaxMin,
            None,
        ),
        cmy_lane(
            "color.yellow",
            light_dynamics::DynamicLaneMode::MiddleAmplitude,
            None,
        ),
        cmy_lane(
            "color.cyan",
            light_dynamics::DynamicLaneMode::Random,
            Some(random_group_id),
        ),
    ];
    legacy.random_groups = vec![light_dynamics::DynamicRandomGroup {
        id: random_group_id,
        seed: 7,
        low: light_dynamics::ScalarSource::Value { value: 0.15 },
        high: light_dynamics::ScalarSource::Value { value: 0.75 },
        decision_interval_millis: 100,
        start_probability: 1.0,
        mean_duration_millis: 100,
        duration_spread_millis: 0,
        attack_ratio: 0.0,
        decay_ratio: 0.0,
    }];
    light_dynamics::validate_definition(&legacy).unwrap();
    let mut body = serde_json::to_value(&legacy).unwrap();
    body["future_dynamic"] = json!({"kept": true});
    let playback = serde_json::to_value(dynamic_playback(
        3,
        &legacy,
        light_playback::DynamicPlaybackTargetScope::FrozenTargets {
            targets: vec![target],
        },
    ))
    .unwrap();
    let (_, document) =
        document_with_objects(&[("dynamic", "19", body), ("playback", "3", playback)]);
    let mut transaction = document.transaction();

    stage_candidate_migrations(&document, &mut transaction).unwrap();

    let candidate = document.candidate(&transaction).unwrap();
    let body = candidate.object("dynamic", "19").unwrap().body();
    assert_eq!(body["future_dynamic"], json!({"kept": true}));
    let migrated: light_dynamics::DynamicDefinition = serde_json::from_value(body.clone()).unwrap();
    light_dynamics::validate_definition(&migrated).unwrap();
    assert_eq!(
        migrated
            .lanes
            .iter()
            .map(|lane| lane.attribute.0.as_str())
            .collect::<Vec<_>>(),
        ["color.red", "color.green", "color.blue", "color.red"]
    );
    assert!(migrated.lanes[2].middle_amplitude.invert_waveform);
    assert_ne!(
        migrated.lanes[3].random_group_id,
        Some(random_group_id),
        "inverse random data gets a canonical clone instead of mutating a shared legacy group"
    );
    let fallback = migrated_dynamic_definition(candidate.object("playback", "3").unwrap().body());
    assert_eq!(fallback["lanes"][0]["attribute"], "color.red");
    assert_eq!(
        fallback["lanes"][2]["middle_amplitude"]["invert_waveform"],
        true
    );

    let sources = EmptyDynamicSources;
    for elapsed_millis in [0, 125, 500, 875] {
        for (legacy_lane, migrated_lane) in legacy.lanes.iter().zip(&migrated.lanes) {
            let context = light_dynamics::DynamicEvaluationContext {
                instance_id: Uuid::nil(),
                target,
                elapsed_millis,
                cycle_duration_millis: 1_000,
                phase_degrees: 0.0,
                output_interval_millis: 25,
                random_envelope: Some(0.35),
                sources: &sources,
            };
            let before = light_dynamics::DynamicEvaluator::new(&legacy)
                .sample_lane(legacy_lane, context)
                .unwrap();
            let after = light_dynamics::DynamicEvaluator::new(&migrated)
                .sample_lane(migrated_lane, context)
                .unwrap();
            assert!(
                (after - (1.0 - before)).abs() < 1.0e-5,
                "{:?} at {elapsed_millis}: {before} did not migrate inversely to {after}",
                legacy_lane.mode
            );
        }
    }

    let migrated_objects = candidate
        .objects()
        .map(|object| {
            (
                object.key().kind().to_owned(),
                object.key().id().to_owned(),
                object.body().clone(),
            )
        })
        .collect::<Vec<_>>();
    let migrated_refs = migrated_objects
        .iter()
        .map(|(kind, id, body)| (kind.as_str(), id.as_str(), body.clone()))
        .collect::<Vec<_>>();
    let (_, migrated_document) = document_with_objects(&migrated_refs);
    let mut second_pass = migrated_document.transaction();
    stage_candidate_migrations(&migrated_document, &mut second_pass).unwrap();
    assert!(second_pass.is_empty());
}

struct EmptyDynamicSources;

impl light_dynamics::ScalarSourceResolver for EmptyDynamicSources {
    fn current(
        &self,
        _target: light_core::FixtureId,
        _attribute: &light_core::AttributeKey,
    ) -> Option<f32> {
        None
    }

    fn preset(
        &self,
        _preset_id: &str,
        _target: light_core::FixtureId,
        _attribute: &light_core::AttributeKey,
    ) -> Option<f32> {
        None
    }
}

fn cmy_lane(
    attribute: &str,
    mode: light_dynamics::DynamicLaneMode,
    random_group_id: Option<Uuid>,
) -> light_dynamics::DynamicLane {
    light_dynamics::DynamicLane {
        id: Uuid::new_v4(),
        attribute: light_core::AttributeKey(attribute.into()),
        mode,
        keyframes: light_dynamics::KeyframeConfiguration {
            points: vec![
                light_dynamics::DynamicKeyframe {
                    position: 0.0,
                    source: light_dynamics::ScalarSource::Value { value: 0.2 },
                    interpolation: light_dynamics::ScalarInterpolation::EaseInOut,
                },
                light_dynamics::DynamicKeyframe {
                    position: 0.5,
                    source: light_dynamics::ScalarSource::Value { value: 0.8 },
                    interpolation: light_dynamics::ScalarInterpolation::Linear,
                },
            ],
            size: 0.8,
        },
        max_min: light_dynamics::MaxMinConfiguration {
            minimum: light_dynamics::ScalarSource::Value { value: 0.1 },
            maximum: light_dynamics::ScalarSource::Value { value: 0.9 },
            function: light_dynamics::PeriodicFunction::Cosinus,
            size: 0.7,
            pwm: light_dynamics::PwmShape::default(),
        },
        middle_amplitude: light_dynamics::MiddleAmplitudeConfiguration {
            middle: light_dynamics::ScalarSource::Value { value: 0.45 },
            amplitude: 0.3,
            function: light_dynamics::PeriodicFunction::Pwm,
            size: 0.6,
            pwm: light_dynamics::PwmShape {
                attack: 0.1,
                on: 0.35,
                decay: 0.2,
                off: 0.35,
                attack_interpolation: light_dynamics::ScalarInterpolation::EaseIn,
                decay_interpolation: light_dynamics::ScalarInterpolation::EaseOut,
            },
            invert_waveform: false,
        },
        speed_multiplier: light_dynamics::Rational::ONE,
        width: 1.0,
        phase: None,
        random_group_id,
    }
}

fn strobe_lane(attribute: light_core::AttributeKey) -> light_dynamics::DynamicLane {
    light_dynamics::DynamicLane {
        id: Uuid::new_v4(),
        attribute: attribute.clone(),
        mode: light_dynamics::DynamicLaneMode::Keyframes,
        keyframes: light_dynamics::KeyframeConfiguration {
            points: vec![light_dynamics::DynamicKeyframe {
                position: 0.0,
                source: light_dynamics::ScalarSource::Preset {
                    preset_id: "1.1".into(),
                    attribute,
                    last_valid_by_target: Vec::new(),
                },
                interpolation: light_dynamics::ScalarInterpolation::Linear,
            }],
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
            invert_waveform: false,
        },
        speed_multiplier: light_dynamics::Rational::ONE,
        width: 1.0,
        phase: None,
        random_group_id: None,
    }
}

fn assert_json_number(value: &serde_json::Value, expected: f64) {
    let actual = value.as_f64().expect("expected JSON number");
    assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
}

fn assert_json_numbers(value: &serde_json::Value, expected: &[f64]) {
    let actual = value.as_array().expect("expected JSON number array");
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_json_number(actual, *expected);
    }
}
