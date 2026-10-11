use super::*;
use serde_json::json;
#[test]
fn sparse_assignment_uses_one_address_and_one_page_guard() {
    let schema = serde_json::to_value(schemars::schema_for!(PlaybackTopologyAction)).unwrap();
    let variants = schema["oneOf"].as_array().unwrap();
    let assignment = variants
        .iter()
        .find(|v| v["properties"]["type"]["const"] == "assign_playback")
        .unwrap();
    assert!(assignment["properties"].get("address").is_some());
    assert!(
        assignment["properties"]
            .get("expected_page_revision")
            .is_some()
    );
    assert!(
        assignment["properties"]
            .get("expected_playback_revision")
            .is_none()
    );
}
#[test]
fn old_split_topology_actions_are_rejected() {
    for action in [
        "configure_slot",
        "configure_virtual",
        "map_existing_playback",
        "clear_virtual",
    ] {
        assert!(serde_json::from_value::<PlaybackTopologyAction>(json!({"type":action})).is_err());
    }
}
#[test]
fn clear_uses_address_and_tolerates_future_fields() {
    let action:PlaybackTopologyAction=serde_json::from_value(json!({"type":"clear_playback","address":{"page":99,"position":1000},"expected_page_revision":4,"expected_page_object_id":"page99","future":true})).unwrap();
    assert!(matches!(
        action,
        PlaybackTopologyAction::ClearPlayback {
            address: super::super::playback::PlaybackAddressValue {
                page: 99,
                position: 1000
            },
            ..
        }
    ));
}
