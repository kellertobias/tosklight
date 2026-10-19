use super::*;
use serde_json::json;

#[test]
fn normal_snapshot_filter_keeps_only_requested_fixture_values() {
    let included = Uuid::from_u128(1);
    let excluded = Uuid::from_u128(2);
    let mut snapshot = json!({
        "values": [
            {"fixture_id": included, "attribute": "intensity", "value": 0.5},
            {"fixture_id": excluded, "attribute": "intensity", "value": 1.0}
        ],
        "profile_output_values": [
            {"fixture_id": included, "attribute": "intensity", "value": 0.4},
            {"fixture_id": excluded, "attribute": "intensity", "value": 0.9}
        ],
        "dynamic_stack": [
            {"fixture_id": included, "attribute": "intensity"},
            {"fixture_id": excluded, "attribute": "intensity"}
        ],
        "revision": 7
    });
    retain_visualization_fixtures(&mut snapshot, &HashSet::from([included]));
    for field in ["values", "profile_output_values", "dynamic_stack"] {
        assert_eq!(snapshot[field].as_array().unwrap().len(), 1);
        assert_eq!(snapshot[field][0]["fixture_id"], included.to_string());
    }
    assert_eq!(snapshot["revision"], 7);
}

#[test]
fn fixture_sheet_projection_keeps_stable_bases_and_individual_dynamic_identities() {
    let included = light_core::FixtureId(Uuid::from_u128(1));
    let excluded = light_core::FixtureId(Uuid::from_u128(2));
    let first_dynamic = Uuid::from_u128(10);
    let second_dynamic = Uuid::from_u128(11);
    let mut snapshot = json!({
        "values": [
            {"fixture_id": included.0, "attribute": "intensity", "value": 0.95}
        ],
        "profile_output_values": [
            {"fixture_id": included.0, "attribute": "intensity", "value": 0.9}
        ],
        "dynamic_stack": [
            {
                "fixture_id": included.0,
                "attribute": "intensity",
                "entry_type": "ordinary_static",
                "name": "Static base"
            },
            {
                "fixture_id": included.0,
                "attribute": "shaper_1_a",
                "entry_type": "dynamic",
                "dynamic_id": first_dynamic,
                "pool_number": 7,
                "name": "Fan",
                "paused": false,
                "pending": false,
                "hidden": false,
                "winning": true,
                "value": 0.42,
                "resolved_value": 0.73
            },
            {
                "fixture_id": included.0,
                "attribute": "shaper_1_a",
                "entry_type": "dynamic",
                "dynamic_id": second_dynamic,
                "pool_number": 8,
                "name": "Pulse",
                "paused": true,
                "pending": false,
                "hidden": true,
                "winning": false,
                "value": 0.12,
                "resolved_value": 0.73
            },
            {
                "fixture_id": excluded.0,
                "attribute": "media_file",
                "entry_type": "dynamic",
                "dynamic_id": Uuid::from_u128(12),
                "pool_number": 9,
                "name": "Excluded"
            }
        ]
    });
    let ordinary = light_engine::ResolvedValues::from_iter([
        (
            (included, light_core::AttributeKey("intensity".into())),
            light_core::AttributeValue::Normalized(0.25),
        ),
        (
            (included, light_core::AttributeKey("media.file".into())),
            light_core::AttributeValue::Discrete("4".into()),
        ),
        (
            (excluded, light_core::AttributeKey("intensity".into())),
            light_core::AttributeValue::Normalized(0.5),
        ),
    ]);

    project_fixture_sheet_snapshot(&mut snapshot, &ordinary, Some(&HashSet::from([included.0])));

    let values = snapshot["values"].as_array().unwrap();
    assert_eq!(values.len(), 2);
    assert!(values.iter().any(|value| {
        value["attribute"] == "intensity"
            && value["value"] == serde_json::json!({"kind": "normalized", "value": 0.25})
    }));
    assert!(values.iter().any(|value| {
        value["attribute"] == "media.file"
            && value["value"] == serde_json::json!({"kind": "discrete", "value": "4"})
    }));
    assert_eq!(snapshot["profile_output_values"], json!([]));
    let dynamic_stack = snapshot["dynamic_stack"].as_array().unwrap();
    assert_eq!(dynamic_stack.len(), 2);
    assert_eq!(dynamic_stack[0]["dynamic_id"], first_dynamic.to_string());
    assert_eq!(dynamic_stack[1]["dynamic_id"], second_dynamic.to_string());
    assert_eq!(dynamic_stack[0]["attribute"], "shaper_1_a");
    assert_eq!(dynamic_stack[1]["attribute"], "shaper_1_a");
    assert!(dynamic_stack.iter().all(|entry| {
        entry.get("value").is_none()
            && entry.get("resolved_value").is_none()
            && entry.get("summary_count").is_none()
    }));
}

#[test]
fn explicit_fixture_sheet_wire_elision_matches_the_complete_final_projection() {
    let included = light_core::FixtureId(Uuid::from_u128(1));
    let excluded = light_core::FixtureId(Uuid::from_u128(2));
    let ordinary = light_engine::ResolvedValues::from_iter([
        (
            (included, light_core::AttributeKey::intensity()),
            light_core::AttributeValue::Normalized(0.25),
        ),
        (
            (excluded, light_core::AttributeKey::intensity()),
            light_core::AttributeValue::Normalized(0.5),
        ),
    ]);
    let sampled = light_engine::ResolvedValues::from_iter([
        (
            (included, light_core::AttributeKey::intensity()),
            light_core::AttributeValue::Normalized(0.9),
        ),
        (
            (excluded, light_core::AttributeKey("media.file".into())),
            light_core::AttributeValue::Discrete("4".into()),
        ),
    ]);
    let common = json!({"scope":{"show_id":null},"revision":7,"preload":true,"dynamic_stack":[
        {"fixture_id":included.0,"attribute":"intensity","entry_type":"dynamic","dynamic_id":Uuid::from_u128(9),"winning":true,"value":0.9,"resolved_value":0.9},
        {"fixture_id":excluded.0,"attribute":"intensity","entry_type":"ordinary_static","value":0.5}
    ]});
    let mut full = common.clone();
    let mut elided = common;
    full["values"] = VisualizationValueContent::Full.wire_values(&sampled).into();
    full["profile_output_values"] = VisualizationValueContent::Full.wire_values(&sampled).into();
    elided["values"] = VisualizationValueContent::FixtureSheet
        .wire_values(&sampled)
        .into();
    elided["profile_output_values"] = VisualizationValueContent::FixtureSheet
        .wire_values(&sampled)
        .into();
    assert_eq!(elided["values"], json!([]));
    assert_ne!(full["values"], elided["values"]);
    assert_eq!(
        VisualizationValueContent::Full.wire_values(&sampled).len(),
        2,
        "ordinary content retains its full values regardless of stack-summary flags"
    );
    let ids = HashSet::from([included.0]);
    project_fixture_sheet_snapshot(&mut full, &ordinary, Some(&ids));
    project_fixture_sheet_snapshot(&mut elided, &ordinary, Some(&ids));
    assert_eq!(full, elided);
    assert_eq!(
        full["values"][0]["value"],
        json!({"kind":"normalized","value":0.25})
    );
    assert_eq!(full["profile_output_values"], json!([]));
    assert_eq!(full["dynamic_stack"].as_array().unwrap().len(), 1);
    assert_eq!(
        full["dynamic_stack"][0]["dynamic_id"],
        Uuid::from_u128(9).to_string()
    );
}
