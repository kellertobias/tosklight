use super::*;
use serde_json::{Value, json};

const CUE_LIST_ID: Uuid = Uuid::from_u128(1);
const CUE_ID: Uuid = Uuid::from_u128(2);
const SHOW_ID: Uuid = Uuid::from_u128(3);
const CORRELATION_ID: Uuid = Uuid::from_u128(4);
const FIXTURE_ID: Uuid = Uuid::from_u128(5);

#[test]
fn preview_request_has_strict_semantic_target_and_mode_tags() {
    let request = ProgrammingUpdatePreviewRequest {
        request_id: "preview-1".into(),
        target: cue_request_target(),
        mode: ProgrammingUpdateMode::Cue(ProgrammingUpdateCueMode::ExistingOnly),
    };

    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "request_id":"preview-1",
            "target":{
                "type":"cue",
                "cue_list_id":CUE_LIST_ID,
                "playback_number":7,
                "cue_id":CUE_ID,
                "cue_number":2.5,
                "validate_active_context":true
            },
            "mode":{"target_type":"cue","mode":"existing_only"}
        })
    );
}

#[test]
fn preview_and_action_requests_reject_unknown_or_client_owned_fields() {
    for path in ["show_id", "desk_id", "user_id", "programmer", "selection"] {
        let mut input = preview_request_json();
        input[path] = json!("forged");
        assert!(serde_json::from_value::<ProgrammingUpdatePreviewRequest>(input).is_err());
    }

    let mut target = preview_request_json();
    target["target"]["object_id"] = json!("legacy-storage-key");
    assert!(serde_json::from_value::<ProgrammingUpdatePreviewRequest>(target).is_err());

    let mut mode = preview_request_json();
    mode["mode"]["future"] = json!(true);
    assert!(serde_json::from_value::<ProgrammingUpdatePreviewRequest>(mode).is_err());

    let mut action = confirm_action_json();
    action["action"]["expected_show_revision"] = json!(8);
    assert!(serde_json::from_value::<ProgrammingUpdateActionRequest>(action).is_err());
}

#[test]
fn mode_family_mismatches_are_rejected_on_every_client_authored_path() {
    let mut cue_preview = preview_request_json();
    cue_preview["mode"] = json!({"target_type":"existing_content","mode":"add_new"});
    assert!(serde_json::from_value::<ProgrammingUpdatePreviewRequest>(cue_preview).is_err());

    let preset_with_cue_mode = json!({
        "request_id":"preset-preview",
        "target":{"type":"preset","object_id":"2.7"},
        "mode":{"target_type":"cue","mode":"add_new"}
    });
    assert!(
        serde_json::from_value::<ProgrammingUpdatePreviewRequest>(preset_with_cue_mode).is_err()
    );

    for action_type in ["confirm_preview", "apply_direct"] {
        let mut input = if action_type == "confirm_preview" {
            confirm_action_json()
        } else {
            direct_action_json()
        };
        input["action"]["mode"] =
            json!({"target_type":"existing_content","mode":"update_existing"});
        assert!(serde_json::from_value::<ProgrammingUpdateActionRequest>(input).is_err());
    }

    let mut response = serde_json::to_value(preview_response()).unwrap();
    response["preview"]["mode"] =
        json!({"target_type":"existing_content","mode":"update_existing"});
    assert!(serde_json::from_value::<ProgrammingUpdatePreviewResponse>(response).is_err());
}

#[test]
fn direct_and_confirm_actions_have_distinct_precondition_shapes() {
    let confirm: ProgrammingUpdateActionRequest =
        serde_json::from_value(confirm_action_json()).unwrap();
    assert!(matches!(
        confirm.action,
        ProgrammingUpdateAction::ConfirmPreview {
            expected_object_revision: 6,
            ..
        }
    ));

    let direct: ProgrammingUpdateActionRequest =
        serde_json::from_value(direct_action_json()).unwrap();
    assert!(matches!(
        direct.action,
        ProgrammingUpdateAction::ApplyDirect { .. }
    ));

    let mut direct_with_preview_revision = direct_action_json();
    direct_with_preview_revision["action"]["expected_object_revision"] = json!(6);
    assert!(
        serde_json::from_value::<ProgrammingUpdateActionRequest>(direct_with_preview_revision)
            .is_err()
    );
}

#[test]
fn preview_keeps_semantic_cue_identity_separate_from_storage_identity() {
    let response = preview_response();
    let encoded = serde_json::to_value(&response).unwrap();

    assert_eq!(
        encoded["preview"]["target"]["object_id"],
        CUE_LIST_ID.to_string()
    );
    assert_eq!(encoded["object"]["kind"], "cue_list");
    assert_eq!(encoded["object"]["object_id"], "legacy-cuelist-record");
    assert_eq!(encoded["object"]["object_revision"], 6);
    assert_eq!(
        serde_json::from_value::<ProgrammingUpdatePreviewResponse>(encoded).unwrap(),
        response
    );
}

#[test]
fn targets_response_is_one_coherent_show_scoped_projection() {
    let preview = preview();
    let add_new = ProgrammingUpdatePreview {
        mode: ProgrammingUpdateMode::Cue(ProgrammingUpdateCueMode::AddNew),
        ..preview.clone()
    };
    let response = ProgrammingUpdateTargetsResponse {
        request_id: "targets-1".into(),
        correlation_id: CORRELATION_ID,
        show_id: SHOW_ID,
        show_revision: 11,
        targets: vec![ProgrammingUpdateTargetEntry {
            request_target: cue_request_target(),
            object: object_identity(),
            programmer_revision: "sha256:programmer".into(),
            active_or_referenced: true,
            existing_preview: preview,
            add_new_preview: add_new,
        }],
    };
    let encoded = serde_json::to_value(&response).unwrap();

    assert_eq!(encoded["show_revision"], 11);
    assert_eq!(
        encoded["targets"][0]["object"]["object_id"],
        "legacy-cuelist-record"
    );
    assert_eq!(
        encoded["targets"][0]["add_new_preview"]["mode"]["mode"],
        "add_new"
    );
    assert_eq!(
        serde_json::from_value::<ProgrammingUpdateTargetsResponse>(encoded).unwrap(),
        response
    );
}

#[test]
fn successful_action_outcome_is_changed_only_and_authoritative() {
    let outcome = changed_outcome();
    let encoded = serde_json::to_value(&outcome).unwrap();

    assert_eq!(encoded["status"], "changed");
    assert_eq!(encoded["event_sequence"], 27);
    assert_eq!(encoded["projection"]["object_id"], "legacy-cuelist-record");
    assert_eq!(encoded["projection"]["body"]["future"]["kept"], true);

    let mut no_change = encoded.clone();
    no_change["status"] = json!("no_change");
    assert!(serde_json::from_value::<ProgrammingUpdateActionOutcome>(no_change).is_err());

    let mut missing_event = encoded.clone();
    missing_event
        .as_object_mut()
        .unwrap()
        .remove("event_sequence");
    assert!(serde_json::from_value::<ProgrammingUpdateActionOutcome>(missing_event).is_err());

    let mut changed_flag = encoded;
    changed_flag["changed"] = json!(false);
    assert!(serde_json::from_value::<ProgrammingUpdateActionOutcome>(changed_flag).is_err());
}

#[test]
fn settings_exclude_legacy_other_target_modes() {
    let settings = ProgrammingUpdateSettings {
        cue_mode: ProgrammingUpdateCueMode::AddToCurrentCue,
        preset_mode: ProgrammingUpdateExistingContentMode::UpdateExisting,
        group_mode: ProgrammingUpdateExistingContentMode::AddNew,
        show_update_modal_on_touch: true,
    };
    let projection = ProgrammingUpdateSettingsProjection {
        desk_id: Uuid::from_u128(9),
        settings,
    };
    let encoded = serde_json::to_value(projection).unwrap();
    assert!(encoded["settings"].get("other_target_modes").is_none());

    let mut legacy = encoded["settings"].clone();
    legacy["other_target_modes"] = json!({"future":"add_new"});
    assert!(serde_json::from_value::<ProgrammingUpdateSettings>(legacy).is_err());
}

fn preview_request_json() -> Value {
    json!({
        "request_id":"preview-1",
        "target":{
            "type":"cue",
            "cue_list_id":CUE_LIST_ID,
            "playback_number":7,
            "cue_id":CUE_ID,
            "cue_number":2.5,
            "validate_active_context":true
        },
        "mode":{"target_type":"cue","mode":"existing_only"}
    })
}

fn confirm_action_json() -> Value {
    json!({
        "request_id":"update-1",
        "action":{
            "type":"confirm_preview",
            "target":preview_request_json()["target"].clone(),
            "mode":preview_request_json()["mode"].clone(),
            "expected_object_revision":6,
            "expected_programmer_revision":"sha256:programmer"
        }
    })
}

fn direct_action_json() -> Value {
    json!({
        "request_id":"update-direct-1",
        "action":{
            "type":"apply_direct",
            "target":preview_request_json()["target"].clone(),
            "mode":preview_request_json()["mode"].clone()
        }
    })
}

fn cue_request_target() -> ProgrammingUpdateTarget {
    ProgrammingUpdateTarget::Cue {
        cue_list_id: CUE_LIST_ID,
        playback_number: Some(7),
        cue_id: Some(CUE_ID),
        cue_number: Some(2.5),
        validate_active_context: true,
    }
}

fn target_identity() -> ProgrammingUpdateTargetIdentity {
    ProgrammingUpdateTargetIdentity {
        family: ProgrammingUpdateTargetFamily::Cue,
        object_id: CUE_LIST_ID.to_string(),
        name: "Main Cuelist".into(),
        playback_number: Some(7),
        cue: Some(ProgrammingUpdateCueIdentity {
            id: CUE_ID,
            number: 2.5,
        }),
    }
}

fn object_identity() -> ProgrammingUpdateObjectIdentity {
    ProgrammingUpdateObjectIdentity {
        kind: ProgrammingUpdateObjectKind::CueList,
        object_id: "legacy-cuelist-record".into(),
        object_revision: 6,
    }
}

fn preview() -> ProgrammingUpdatePreview {
    ProgrammingUpdatePreview {
        target: target_identity(),
        mode: ProgrammingUpdateMode::Cue(ProgrammingUpdateCueMode::ExistingOnly),
        items: vec![ProgrammingUpdatePreviewItem {
            address: ProgrammingUpdateAddress::FixtureAttribute {
                fixture_id: FIXTURE_ID,
                attribute: "intensity".into(),
            },
            outcome: ProgrammingUpdateItemOutcome::ChangeAtSource {
                source: cue_source(),
            },
        }],
    }
}

fn preview_response() -> ProgrammingUpdatePreviewResponse {
    ProgrammingUpdatePreviewResponse {
        request_id: "preview-1".into(),
        correlation_id: CORRELATION_ID,
        show_id: SHOW_ID,
        show_revision: 11,
        object: object_identity(),
        programmer_revision: "sha256:programmer".into(),
        preview: preview(),
    }
}

fn cue_source() -> ProgrammingUpdateCueSource {
    ProgrammingUpdateCueSource {
        cue_id: CUE_ID,
        cue_number: 2.5,
        cue_index: 1,
    }
}

fn changed_outcome() -> ProgrammingUpdateActionOutcome {
    ProgrammingUpdateActionOutcome::Changed {
        request_id: "update-1".into(),
        correlation_id: CORRELATION_ID,
        replayed: false,
        show_id: SHOW_ID,
        show_revision: 12,
        projection: ProgrammingUpdateProjection {
            kind: ProgrammingUpdateObjectKind::CueList,
            object_id: "legacy-cuelist-record".into(),
            object_revision: 7,
            body: json!({"id":CUE_LIST_ID,"future":{"kept":true}}).into(),
        },
        event_sequence: 27,
        summary: ProgrammingUpdateSummary {
            target: target_identity(),
            revision_before: 6,
            revision_after: 7,
            eligible_count: 1,
            changed_count: 1,
            added_count: 0,
            ignored_count: 0,
            changed_cues: vec![cue_source()],
            programmer_values_retained: true,
        },
    }
}
