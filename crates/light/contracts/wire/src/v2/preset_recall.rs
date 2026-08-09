//! Action-time Preset recall into one server-selected normal or pending-Preload values batch.

use super::{
    preload_values::ProgrammingPreloadValuesProjection, preset_recording::PresetRecordingAddress,
    programming::ProgrammingValuesProjection,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PresetRecallRequest {
    pub address: PresetRecordingAddress,
    #[ts(type = "number")]
    pub expected_preset_revision: u64,
    #[ts(type = "number")]
    pub expected_show_revision: u64,
    #[ts(type = "number")]
    pub expected_programmer_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub expected_preload_values_revision: Option<u64>,
    #[ts(type = "number")]
    pub expected_capture_mode_revision: u64,
    #[ts(type = "number")]
    pub expected_selection_revision: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RecalledPresetProjection {
    pub id: String,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "unknown")]
    pub body: Value,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PresetRecallOutcome {
    pub correlation_id: Uuid,
    pub disposition: PresetRecallDisposition,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(type = "number")]
    pub programmer_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub target: Option<PresetRecallTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub preload_values_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub preload_projection: Option<ProgrammingPreloadValuesProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub preload_event_sequence: Option<u64>,
    #[ts(type = "number")]
    pub capture_mode_revision: u64,
    #[ts(type = "number")]
    pub selection_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub interaction_event_sequence: Option<u64>,
    #[ts(type = "number")]
    pub applied_fixtures: u64,
    #[ts(type = "number")]
    pub selected_targets: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub active_context: Option<String>,
    pub preset: RecalledPresetProjection,
    #[serde(flatten)]
    pub outcome: PresetRecallActionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PresetRecallTarget {
    Programmer,
    Preload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PresetRecallDisposition {
    Recalled,
    TargetsSelected,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PresetRecallActionState {
    Changed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        projection: Option<ProgrammingValuesProjection>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(as = "Option<f64>", optional = nullable)]
        event_sequence: Option<u64>,
    },
    NoChange,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PresetRecallErrorKind {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Unavailable,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PresetRecallErrorResponse {
    pub kind: PresetRecallErrorKind,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_related_revision: Option<u64>,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ignores_future_client_fields_without_authoring_values() {
        let value = serde_json::json!({
            "address": {"family":"color", "number":1},
            "expected_preset_revision": 2,
            "expected_show_revision": 3,
            "expected_programmer_revision": 4,
            "expected_preload_values_revision": 7,
            "expected_capture_mode_revision": 5,
            "expected_selection_revision": 6,
            "values": []
        });
        let request = serde_json::from_value::<PresetRecallRequest>(value).unwrap();
        assert_eq!(request.expected_programmer_revision, 4);
        assert_eq!(request.expected_preload_values_revision, Some(7));
    }

    #[test]
    fn no_change_omits_values_projection_and_event() {
        let state = PresetRecallActionState::NoChange;
        let json = serde_json::to_value(state).unwrap();
        assert_eq!(json["status"], "no_change");
        assert!(json.get("projection").is_none());
        assert!(json.get("event_sequence").is_none());
    }
}
