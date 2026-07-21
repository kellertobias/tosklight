use super::*;
use serde_json::json;

#[test]
fn all_semantic_actions_have_strict_readable_discriminants() {
    let actions = [
        json!({
            "request_id":"save",
            "action":{"type":"save_cue_list","cue_list_id":Uuid::nil(),
                "expected_revision":3,"expected_object_id":null,
                "body":{"id":Uuid::nil(),"future":true}}
        }),
        json!({
            "request_id":"configure",
            "action":{"type":"configure_slot","page":2,"slot":4,
                "expected_page_revision":5,"expected_page_object_id":null,
                "expected_playback_revision":6,"expected_playback_object_id":null,
                "playback":playback_json()}
        }),
        json!({
            "request_id":"clear",
            "action":{"type":"clear_mapped_playback","page":2,"slot":4,
                "expected_page_revision":5,"expected_page_object_id":null,
                "expected_playback_revision":6,"expected_playback_object_id":null}
        }),
        map_existing_request(),
    ];
    for action in actions {
        serde_json::from_value::<PlaybackTopologyActionRequest>(action).unwrap();
    }
}

#[test]
fn identity_and_nested_unknown_fields_are_rejected() {
    for forged in [
        "show_id",
        "user_id",
        "desk_id",
        "session_id",
        "correlation_id",
    ] {
        let mut request = configure_request();
        request[forged] = json!("forged");
        assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(request).is_err());
    }
    let mut action = configure_request();
    action["action"]["object_kind"] = json!("playback");
    assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(action).is_err());
    let mut playback = configure_request();
    playback["action"]["playback"]["future"] = json!(true);
    assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(playback).is_err());
    let mut target = configure_request();
    target["action"]["playback"]["target"]["future"] = json!(true);
    assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(target).is_err());

    for field in ["expected_page_object_id", "expected_playback_object_id"] {
        let mut missing = configure_request();
        missing["action"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(missing).is_err());
    }
    let missing_save_identity = json!({
        "request_id":"save",
        "action":{"type":"save_cue_list","cue_list_id":Uuid::nil(),
            "expected_revision":3,"body":{"id":Uuid::nil()}}
    });
    assert!(
        serde_json::from_value::<PlaybackTopologyActionRequest>(missing_save_identity).is_err()
    );
    let mut map = map_existing_request();
    map["action"]["target"] = json!({"type":"cue_list"});
    assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(map).is_err());
    for field in ["expected_page_object_id", "expected_playback_object_id"] {
        let mut missing = map_existing_request();
        missing["action"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(missing).is_err());
    }
}

#[test]
fn changed_and_no_change_outcomes_enforce_event_and_projection_shapes() {
    let present = json!({"state":"present","kind":"playback","object_id":"7",
        "object_revision":4,"body":{"number":7}});
    let changed = json!({"request_id":"r","correlation_id":Uuid::nil(),"show_revision":9,
        "resolution":{"kind":"page_slot","page":1,"slot":2,"playback_number":7},
        "status":"changed","objects":[present.clone()],"event_sequence":10,"replayed":false});
    serde_json::from_value::<PlaybackTopologyActionOutcome>(changed).unwrap();
    let no_change = json!({"request_id":"r","correlation_id":Uuid::nil(),"show_revision":9,
        "resolution":{"kind":"page_slot","page":1,"slot":2,"playback_number":null},
        "status":"no_change","objects":[present],"replayed":true});
    serde_json::from_value::<PlaybackTopologyActionOutcome>(no_change).unwrap();
    let invalid = json!({"request_id":"r","correlation_id":Uuid::nil(),"show_revision":9,
        "resolution":{"kind":"cue_list","cue_list_id":Uuid::nil()},
        "status":"no_change","objects":[],"event_sequence":10,"replayed":false});
    assert!(serde_json::from_value::<PlaybackTopologyActionOutcome>(invalid).is_err());
    let unknown = json!({"request_id":"r","correlation_id":Uuid::nil(),"show_revision":9,
        "resolution":{"kind":"cue_list","cue_list_id":Uuid::nil()},
        "status":"no_change","objects":[],"future":true,"replayed":false});
    assert!(serde_json::from_value::<PlaybackTopologyActionOutcome>(unknown).is_err());
}

#[test]
fn deleted_projection_cannot_smuggle_a_body() {
    let invalid = json!({"state":"deleted","kind":"playback","object_id":"2",
        "object_revision":4,"body":{"number":2}});
    assert!(serde_json::from_value::<PlaybackTopologyObjectProjection>(invalid).is_err());
}

fn configure_request() -> Value {
    json!({"request_id":"configure","action":{"type":"configure_slot","page":2,"slot":4,
        "expected_page_revision":5,"expected_page_object_id":"legacy-page-two",
        "expected_playback_revision":6,"expected_playback_object_id":"legacy-six",
        "playback":playback_json()}})
}

fn map_existing_request() -> Value {
    json!({"request_id":"map-existing","action":{"type":"map_existing_playback",
        "page":2,"slot":4,"playback_number":12,
        "expected_page_revision":5,"expected_page_object_id":"legacy-page-two",
        "expected_playback_revision":6,"expected_playback_object_id":"legacy-twelve"}})
}

fn playback_json() -> Value {
    json!({"number":7,"name":"Main","target":{"type":"grand_master"},
        "buttons":["blackout","pause_dynamics","flash"],"button_count":3,
        "fader":"master","has_fader":true,"go_activates":true,"auto_off":true,
        "xfade_millis":0,"color":"#20c997","flash_release":"release_all",
        "protect_from_swap":false})
}
