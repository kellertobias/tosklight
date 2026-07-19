use super::normalize_body;
use crate::active_show::{
    ActiveShowObjectKind, ActiveShowObjectMutation, ActiveShowObjectMutationKind,
};
use serde_json::{Value, json};

const FIXTURE_ID: &str = "00000000-0000-0000-0000-000000000123";

#[test]
fn group_update_preserves_extensions_inside_derived_and_frozen_sources() {
    let existing = json!({
        "id": "1",
        "name": "Before",
        "color": null,
        "icon": null,
        "fixtures": [],
        "derived_from": {
            "source_group_id": "source",
            "rule": {"type": "all", "future_rule": "kept"},
            "future_derived": {"kept": true}
        },
        "frozen_from": {
            "source_group_id": "source",
            "source_revision": 7,
            "captured_at": "2026-07-19T00:00:00Z",
            "future_frozen": [1, 2, 3]
        },
        "programming": {},
        "master": 1.0,
        "playback_fader": null
    });
    let mut request = canonical_group(&existing);
    request["name"] = json!("After");

    let normalized = normalize(&existing, ActiveShowObjectKind::Group, "1", request);

    assert_eq!(
        normalized["derived_from"]["future_derived"],
        json!({"kept": true})
    );
    assert_eq!(normalized["derived_from"]["rule"]["future_rule"], "kept");
    assert_eq!(normalized["frozen_from"]["future_frozen"], json!([1, 2, 3]));
    assert_eq!(normalized["name"], "After");
}

#[test]
fn preset_update_preserves_unknown_attribute_value_fields() {
    let existing = preset(json!({
        "intensity": {"kind": "normalized", "value": 0.25, "future_curve": "smooth"}
    }));
    let mut request = canonical_preset(&existing);
    request["values"][FIXTURE_ID]["intensity"]["value"] = json!(0.75);

    let normalized = normalize(&existing, ActiveShowObjectKind::Preset, "1.1", request);

    assert_eq!(
        normalized["values"][FIXTURE_ID]["intensity"]["future_curve"],
        "smooth"
    );
    assert_eq!(normalized["values"][FIXTURE_ID]["intensity"]["value"], 0.75);
}

#[test]
fn preset_update_accepts_client_supplied_nested_extensions() {
    let request = preset(json!({
        "intensity": {
            "kind": "normalized",
            "value": 0.5,
            "future_client_metadata": {"source": "newer-desk"}
        }
    }));

    let mutation = mutation(ActiveShowObjectKind::Preset, "1.1", request.clone());
    let normalized = normalize_body(None, &mutation, &request).unwrap();

    assert_eq!(
        normalized["values"][FIXTURE_ID]["intensity"]["future_client_metadata"],
        json!({"source": "newer-desk"})
    );
}

#[test]
fn preset_update_does_not_resurrect_a_removed_known_map_entry() {
    let existing = preset(json!({
        "intensity": {"kind": "normalized", "value": 0.25, "future": "remove-with-parent"},
        "dimmer": {"kind": "normalized", "value": 0.4, "future": "keep"}
    }));
    let request = preset(json!({
        "dimmer": {"kind": "normalized", "value": 0.8}
    }));

    let normalized = normalize(&existing, ActiveShowObjectKind::Preset, "1.1", request);

    assert!(normalized["values"][FIXTURE_ID].get("intensity").is_none());
    assert_eq!(normalized["values"][FIXTURE_ID]["dimmer"]["future"], "keep");
}

fn normalize(existing: &Value, kind: ActiveShowObjectKind, id: &str, request: Value) -> Value {
    let mutation = mutation(kind, id, request.clone());
    normalize_body(Some(existing), &mutation, &request).unwrap()
}

fn mutation(kind: ActiveShowObjectKind, id: &str, body: Value) -> ActiveShowObjectMutation {
    ActiveShowObjectMutation {
        kind,
        object_id: id.into(),
        expected_object_revision: 1,
        mutation: ActiveShowObjectMutationKind::Put { body },
    }
}

fn canonical_group(raw: &Value) -> Value {
    serde_json::to_value(
        serde_json::from_value::<light_programmer::GroupDefinition>(raw.clone()).unwrap(),
    )
    .unwrap()
}

fn canonical_preset(raw: &Value) -> Value {
    serde_json::to_value(serde_json::from_value::<light_programmer::Preset>(raw.clone()).unwrap())
        .unwrap()
}

fn preset(attributes: Value) -> Value {
    json!({
        "name": "Look",
        "family": "Intensity",
        "number": 1,
        "values": {(FIXTURE_ID): attributes},
        "group_values": {}
    })
}
