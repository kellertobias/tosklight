//! Authenticated user-scoped Preload enter, GO, pending-clear, and release actions.

use super::{
    playback::PlaybackRuntimeProjection,
    preload_playback_queue::ProgrammingPreloadPlaybackQueueProjection,
    preload_values::ProgrammingPreloadValuesProjection,
    programming::ProgrammingCaptureModeProjection,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingPreloadLifecycleAction {
    Enter {},
    Go {
        show_id: Uuid,
        #[ts(type = "number")]
        expected_show_revision: u64,
        #[ts(type = "number")]
        expected_playback_event_sequence: u64,
    },
    ClearPending {},
    Release {},
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadLifecycleRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_capture_mode_revision: u64,
    #[ts(type = "number")]
    pub expected_values_revision: u64,
    #[ts(type = "number")]
    pub expected_queue_revision: u64,
    #[ts(type = "number")]
    pub expected_selection_revision: u64,
    pub action: ProgrammingPreloadLifecycleAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadRuntimeOutcome {
    pub projection: PlaybackRuntimeProjection,
    #[ts(type = "number")]
    pub event_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadCommitOutcome {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(type = "number")]
    pub playback_event_sequence_before: u64,
    #[ts(type = "number")]
    pub playback_event_sequence_after: u64,
    pub committed_at: String,
    #[ts(type = "number")]
    pub programmer_fade_millis: u64,
    #[ts(type = "number")]
    pub executed_playback_actions: u64,
    /// Ordered actions consumed by this one atomic commit. Runtime changes remain separately
    /// deduplicated by authoritative Playback identity below.
    pub executed: Vec<super::preload_playback_queue::ProgrammingPreloadPlaybackQueueItem>,
    pub runtime_changes: Vec<ProgrammingPreloadRuntimeOutcome>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingPreloadLifecycleState {
    Changed,
    NoChange,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadLifecycleOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub replayed: bool,
    pub status: ProgrammingPreloadLifecycleState,
    /// True only while retained active Preload fixture or Group values exist. Armed capture is
    /// represented independently by `capture_mode.blind`.
    pub active: bool,
    pub capture_mode: ProgrammingCaptureModeProjection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub capture_mode_event_sequence: Option<u64>,
    #[ts(type = "number")]
    pub values_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub values_projection: Option<ProgrammingPreloadValuesProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub values_event_sequence: Option<u64>,
    #[ts(type = "number")]
    pub queue_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub queue_projection: Option<ProgrammingPreloadPlaybackQueueProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub queue_event_sequence: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub interaction_event_sequence: Option<u64>,
    #[ts(type = "number")]
    pub selection_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub commit: Option<ProgrammingPreloadCommitOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingPreloadLifecycleErrorKind {
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
pub struct ProgrammingPreloadLifecycleErrorResponse {
    pub kind: ProgrammingPreloadLifecycleErrorKind,
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
    fn no_change_cannot_carry_broad_or_pending_projections_by_default() {
        let outcome = ProgrammingPreloadLifecycleOutcome {
            request_id: "clear-empty".into(),
            correlation_id: Uuid::from_u128(1),
            replayed: false,
            status: ProgrammingPreloadLifecycleState::NoChange,
            active: false,
            capture_mode: ProgrammingCaptureModeProjection {
                user_id: Uuid::from_u128(2),
                revision: 0,
                blind: false,
                preview: false,
                preload_capture_programmer: true,
            },
            capture_mode_event_sequence: None,
            values_revision: 0,
            values_projection: None,
            values_event_sequence: None,
            queue_revision: 0,
            queue_projection: None,
            queue_event_sequence: None,
            interaction_event_sequence: None,
            selection_revision: 0,
            commit: None,
            warning: None,
        };
        let json = serde_json::to_value(outcome).unwrap();
        assert_eq!(json["status"], "no_change");
        assert!(json.get("values_projection").is_none());
        assert!(json.get("queue_projection").is_none());
        assert!(json.get("programmer").is_none());
    }

    #[test]
    fn request_and_tagged_action_reject_unknown_fields() {
        let request = serde_json::json!({
            "request_id":"strict-preload",
            "expected_capture_mode_revision":0,
            "expected_values_revision":0,
            "expected_queue_revision":0,
            "expected_selection_revision":0,
            "action":{"type":"enter"},
            "programmer":{"forged":true},
        });
        assert!(serde_json::from_value::<ProgrammingPreloadLifecycleRequest>(request).is_err());

        let action = serde_json::json!({"type":"release","future":true});
        assert!(serde_json::from_value::<ProgrammingPreloadLifecycleAction>(action).is_err());
    }
}
