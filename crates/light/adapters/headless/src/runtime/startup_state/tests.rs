use super::{migrate_frozen_group_selection, migrate_retired_programmer_attributes};

/// Restart recovery of a durable programmer persisted before the DEGRP rework: the removed
/// `frozen_group` selection expression (top level and inside undo/redo snapshots) must map to
/// the dereferenced `sources` form instead of dropping the whole programmer.
#[test]
fn legacy_frozen_group_programmer_snapshots_restore_as_dereferenced_sources() {
    let fixture_a = uuid::Uuid::new_v4();
    let fixture_b = uuid::Uuid::new_v4();
    let mut value = serde_json::json!({
        "id": uuid::Uuid::new_v4(),
        "session_id": uuid::Uuid::new_v4(),
        "user_id": uuid::Uuid::new_v4(),
        "priority": 0,
        "selected": [fixture_a, fixture_b],
        "selection_expression": {"type": "frozen_group", "group_id": "7", "source_revision": 4},
        "values": [],
        "connected": true,
        "last_activity": "2026-07-20T09:30:00Z",
        "undo": [{
            "selected": [fixture_a],
            "selection_expression": {"type": "frozen_group", "group_id": "7", "source_revision": 3},
        }],
    });
    migrate_frozen_group_selection(&mut value);
    let programmer: light_programmer::ProgrammerState =
        serde_json::from_value(value).expect("legacy frozen_group programmer deserializes");
    let expected = |fixtures: &[uuid::Uuid]| light_programmer::SelectionExpression::Sources {
        items: fixtures
            .iter()
            .map(|id| light_programmer::SelectionReference::Fixture {
                fixture_id: light_core::FixtureId(*id),
            })
            .collect(),
    };
    assert_eq!(
        programmer.selection_expression,
        Some(expected(&[fixture_a, fixture_b]))
    );
    assert_eq!(
        programmer.undo[0].selection_expression,
        Some(expected(&[fixture_a]))
    );
    assert_eq!(
        programmer.selected,
        vec![
            light_core::FixtureId(fixture_a),
            light_core::FixtureId(fixture_b)
        ]
    );
}

#[test]
fn retired_strobe_programmer_values_migrate_across_normal_preload_dynamic_and_history_state() {
    let fixture = uuid::Uuid::new_v4();
    let mut value = serde_json::json!({
        "values": [{"fixture_id": fixture, "attribute": "strobe", "value": 0.4}],
        "dynamic_values": [{
            "fixture_id": fixture,
            "attribute": "strobe",
            "value": {"type": "dynamic_on", "dynamic": {
                "embedded_fallback": {"definition": {
                    "target_binding": {"type": "targetless"},
                    "lanes": [{
                        "id": uuid::Uuid::new_v4(),
                        "attribute": "strobe",
                        "keyframes": {"points": [{"source": {
                            "type": "preset", "attribute": "strobe"
                        }}]}
                    }],
                    "phase": {},
                    "speed": {}
                }}
            }}
        }],
        "preload_pending": [{"fixture_id": fixture, "attribute": "strobe"}],
        "group_values": {"front": {"strobe": {"value": 0.5}}},
        "preload_group_active": {"front": {"strobe": {"value": 0.6}}},
        "undo": [{
            "values": [{"fixture_id": fixture, "attribute": "strobe"}],
            "group_values": {"front": {"strobe": {"value": 0.3}}}
        }],
        "future_programmer": {"kept": true}
    });

    migrate_retired_programmer_attributes(&mut value).unwrap();

    assert_eq!(value["values"][0]["attribute"], "shutter");
    assert_eq!(value["dynamic_values"][0]["attribute"], "shutter");
    assert_eq!(
        value["dynamic_values"][0]["value"]["dynamic"]["embedded_fallback"]["definition"]["lanes"]
            [0]["attribute"],
        "shutter"
    );
    assert_eq!(
        value["dynamic_values"][0]["value"]["dynamic"]["embedded_fallback"]["definition"]["lanes"]
            [0]["keyframes"]["points"][0]["source"]["attribute"],
        "shutter"
    );
    assert_eq!(value["preload_pending"][0]["attribute"], "shutter");
    assert_eq!(value["group_values"]["front"]["shutter"]["value"], 0.5);
    assert_eq!(
        value["preload_group_active"]["front"]["shutter"]["value"],
        0.6
    );
    assert_eq!(value["undo"][0]["values"][0]["attribute"], "shutter");
    assert_eq!(
        value["undo"][0]["group_values"]["front"]["shutter"]["value"],
        0.3
    );
    assert_eq!(
        value["future_programmer"],
        serde_json::json!({"kept": true})
    );

    let once = value.clone();
    migrate_retired_programmer_attributes(&mut value).unwrap();
    assert_eq!(value, once, "Programmer migration must be idempotent");
}

#[test]
fn retired_strobe_programmer_conflict_preserves_the_original_json() {
    let fixture = uuid::Uuid::new_v4();
    let original = serde_json::json!({
        "values": [
            {"fixture_id": fixture, "attribute": "strobe"},
            {"fixture_id": fixture, "attribute": "shutter"}
        ]
    });
    let mut value = original.clone();

    let error = migrate_retired_programmer_attributes(&mut value).unwrap_err();

    assert!(error.contains("attribute migration conflict at values/1"));
    assert_eq!(value, original);
}

#[test]
fn legacy_cmy_programmer_values_migrate_inverse_across_normal_preload_and_dynamic_state() {
    let fixture = uuid::Uuid::new_v4();
    let mut value = serde_json::json!({
        "values": [{
            "fixture_id": fixture,
            "attribute": "color.cyan",
            "value": {"kind":"normalized","value":0.2}
        }, {
            "fixture_id": fixture,
            "attribute": "color.cold_white",
            "value": {"kind":"normalized","value":0.35,"future_value":"kept"}
        }, {
            "fixture_id": fixture,
            "attribute": "frost.1",
            "value": {"kind":"normalized","value":0.55}
        }],
        "dynamic_values": [{
            "fixture_id": fixture,
            "attribute": "color.magenta",
            "value": {"type":"fix_at","value":0.3,"timing":{}}
        }],
        "preload_pending": [{
            "fixture_id": fixture,
            "attribute": "color.yellow",
            "value": {"kind":"spread","value":[0.0,0.25,1.0]}
        }, {
            "fixture_id": fixture,
            "attribute": "color.warm_white",
            "value": {"kind":"normalized","value":0.65}
        }],
        "group_values": {"front": {"color.cyan": {
            "value":{"kind":"normalized","value":0.4},
            "changed_at":"2026-08-04T00:00:00Z"
        }}},
        "preload_group_active": {"front": {"color.magenta": {
            "kind":"normalized","value":0.1
        }}},
        "future_programmer": {"kept":true}
    });

    migrate_retired_programmer_attributes(&mut value).unwrap();

    assert_eq!(value["values"][0]["attribute"], "color.red");
    assert_migrated_number(&value["values"][0]["value"]["value"], 0.8);
    assert_eq!(value["values"][1]["attribute"], "color.white");
    assert_migrated_number(&value["values"][1]["value"]["value"], 0.35);
    assert_eq!(value["values"][1]["value"]["future_value"], "kept");
    assert_eq!(value["values"][2]["attribute"], "softness");
    assert_migrated_number(&value["values"][2]["value"]["value"], 0.55);
    assert_eq!(value["dynamic_values"][0]["attribute"], "color.green");
    assert_migrated_number(&value["dynamic_values"][0]["value"]["value"], 0.7);
    assert_eq!(value["preload_pending"][0]["attribute"], "color.blue");
    let spread = value["preload_pending"][0]["value"]["value"]
        .as_array()
        .unwrap();
    for (actual, expected) in spread.iter().zip([1.0, 0.75, 0.0]) {
        assert_migrated_number(actual, expected);
    }
    assert_eq!(value["preload_pending"][1]["attribute"], "color.amber");
    assert_migrated_number(&value["preload_pending"][1]["value"]["value"], 0.65);
    assert_migrated_number(
        &value["group_values"]["front"]["color.red"]["value"]["value"],
        0.6,
    );
    assert_migrated_number(
        &value["preload_group_active"]["front"]["color.green"]["value"],
        0.9,
    );
    assert_eq!(value["future_programmer"], serde_json::json!({"kept":true}));
}

#[test]
fn legacy_position_movement_programmer_values_migrate_and_conflicts_stay_atomic() {
    let fixture = uuid::Uuid::new_v4();
    let mut value = serde_json::json!({
        "values": [{
            "fixture_id": fixture,
            "attribute": "fixture.mspeed",
            "value": {"kind":"normalized","value":0.25}
        }],
        "preload_pending": [{
            "fixture_id": fixture,
            "attribute": "fixture.pan_tilt_speed_time",
            "value": {"kind":"normalized","value":0.75}
        }]
    });
    migrate_retired_programmer_attributes(&mut value).unwrap();
    assert_eq!(value["values"][0]["attribute"], "position.movement");
    assert_eq!(
        value["preload_pending"][0]["attribute"],
        "position.movement"
    );

    let original = serde_json::json!({
        "values": [
            {"fixture_id": fixture, "attribute": "pan.time"},
            {"fixture_id": fixture, "attribute": "tilt.time"}
        ]
    });
    let mut conflict = original.clone();
    let error = migrate_retired_programmer_attributes(&mut conflict).unwrap_err();
    assert!(error.contains("attribute migration conflict"));
    assert!(error.contains("position.movement"));
    assert_eq!(conflict, original);
}

#[test]
fn legacy_media_programmer_values_migrate_and_conflicts_stay_atomic() {
    let fixture = uuid::Uuid::new_v4();
    let mut value = serde_json::json!({
        "values": [{
            "fixture_id": fixture,
            "attribute": "media.opacity",
            "value": {"kind":"normalized","value":0.25}
        }],
        "preload_pending": [{
            "fixture_id": fixture,
            "attribute": "media.rotation",
            "value": {"kind":"normalized","value":0.75}
        }, {
            "fixture_id": fixture,
            "attribute": "media.tint",
            "value": {"kind":"color_xyz","value":{"x":0.2,"y":0.3,"z":0.4}}
        }]
    });
    migrate_retired_programmer_attributes(&mut value).unwrap();
    assert_eq!(value["values"][0]["attribute"], "intensity");
    assert_eq!(
        value["preload_pending"][0]["attribute"],
        "position.rotation"
    );
    assert_eq!(value["preload_pending"][1]["attribute"], "color");

    for (original, target) in [
        (
            serde_json::json!({
                "values": [
                    {"fixture_id": fixture, "attribute": "media.opacity"},
                    {"fixture_id": fixture, "attribute": "intensity"}
                ]
            }),
            "intensity",
        ),
        (
            serde_json::json!({
                "values": [
                    {"fixture_id": fixture, "attribute": "media.tint"},
                    {"fixture_id": fixture, "attribute": "color"}
                ]
            }),
            "color",
        ),
    ] {
        let mut conflict = original.clone();
        let error = migrate_retired_programmer_attributes(&mut conflict).unwrap_err();
        assert!(error.contains("attribute migration conflict"));
        assert!(error.contains(target));
        assert_eq!(conflict, original);
    }
}

#[test]
fn legacy_endless_axis_programmer_values_migrate_and_conflicts_stay_atomic() {
    let fixture = uuid::Uuid::new_v4();
    let mut value = serde_json::json!({
        "values": [
            {"fixture_id": fixture, "attribute": "pan.continuous"},
            {"fixture_id": fixture, "attribute": "tilt.continuous"}
        ]
    });
    migrate_retired_programmer_attributes(&mut value).unwrap();
    assert_eq!(value["values"][0]["attribute"], "pan");
    assert_eq!(value["values"][1]["attribute"], "tilt");

    for (source, target) in [("pan.continuous", "pan"), ("tilt.continuous", "tilt")] {
        let original = serde_json::json!({
            "values": [
                {"fixture_id": fixture, "attribute": source},
                {"fixture_id": fixture, "attribute": target}
            ]
        });
        let mut conflict = original.clone();
        let error = migrate_retired_programmer_attributes(&mut conflict).unwrap_err();
        assert!(error.contains("attribute migration conflict"));
        assert!(error.contains(target));
        assert_eq!(conflict, original);
    }
}

fn assert_migrated_number(value: &serde_json::Value, expected: f64) {
    let actual = value.as_f64().expect("expected JSON number");
    assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
}
