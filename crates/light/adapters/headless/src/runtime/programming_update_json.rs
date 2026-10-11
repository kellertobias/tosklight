//! Input field paths for Update's tagged DTOs; values and validation stay untouched.
use crate::tolerant_json::JsonFieldInspector;
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) struct PreviewFields;
pub(super) struct TargetsFields;
pub(super) struct ActionFields;

impl JsonFieldInspector for PreviewFields {
    fn unknown_fields(raw: &Value) -> BTreeSet<String> {
        let mut paths = BTreeSet::new();
        extras(raw, "", &["request_id", "target", "mode"], &mut paths);
        target(&raw["target"], "target", &mut paths);
        mode(&raw["mode"], "mode", &mut paths);
        paths
    }
}
impl JsonFieldInspector for TargetsFields {
    fn unknown_fields(raw: &Value) -> BTreeSet<String> {
        let mut paths = BTreeSet::new();
        extras(raw, "", &["request_id", "filter"], &mut paths);
        paths
    }
}
impl JsonFieldInspector for ActionFields {
    fn unknown_fields(raw: &Value) -> BTreeSet<String> {
        let mut paths = BTreeSet::new();
        extras(raw, "", &["request_id", "action"], &mut paths);
        let action = &raw["action"];
        let fields: &[&str] = if action["type"] == "confirm_preview" {
            &[
                "type",
                "target",
                "mode",
                "expected_object_revision",
                "expected_programmer_revision",
            ]
        } else {
            &["type", "target", "mode"]
        };
        extras(action, "action", fields, &mut paths);
        target(&action["target"], "action.target", &mut paths);
        mode(&action["mode"], "action.mode", &mut paths);
        paths
    }
}
fn target(raw: &Value, path: &str, paths: &mut BTreeSet<String>) {
    let fields: &[&str] = if raw["type"] == "cue" {
        &[
            "type",
            "cue_list_id",
            "playback_address",
            "cue_id",
            "cue_number",
            "validate_active_context",
        ]
    } else {
        &["type", "object_id"]
    };
    extras(raw, path, fields, paths);
    if raw["type"] == "cue" {
        extras(
            &raw["playback_address"],
            &format!("{path}.playback_address"),
            &["page", "position"],
            paths,
        );
    }
}
fn mode(raw: &Value, path: &str, paths: &mut BTreeSet<String>) {
    extras(raw, path, &["target_type", "mode"], paths);
}
fn extras(raw: &Value, path: &str, fields: &[&str], paths: &mut BTreeSet<String>) {
    let Some(object) = raw.as_object() else {
        return;
    };
    for key in object.keys().filter(|key| !fields.contains(&key.as_str())) {
        paths.insert(if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_address_and_inactive_action_fields_are_logged_as_paths_only() {
        let input = serde_json::json!({"request_id":"proof", "action":{
            "type":"apply_direct", "expected_object_revision":"do-not-log-this",
            "target":{"type":"cue", "cue_list_id":"known", "object_id":"do-not-log-this",
                "playback_address":{"page":2,"position":101,"future_address":"do-not-log-this"},
                "cue_id":null,"cue_number":null,"validate_active_context":true},
            "mode":{"target_type":"cue","mode":"add_new","future_mode":"do-not-log-this"}
        }});
        assert_eq!(
            ActionFields::unknown_fields(&input),
            BTreeSet::from([
                "action.expected_object_revision".to_owned(),
                "action.target.object_id".to_owned(),
                "action.target.playback_address.future_address".to_owned(),
                "action.mode.future_mode".to_owned(),
            ])
        );
    }
}
