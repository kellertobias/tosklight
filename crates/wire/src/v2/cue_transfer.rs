//! Typed completion of one retained Cue copy or move choice.

use super::command_line::{CommandLineResponse, CueTransferOperation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueTransferMode {
    Plain,
    Status,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueTransferRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub choice_id: Uuid,
    pub mode: CueTransferMode,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub expected_command_line_revision: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueTransferObjectProjection {
    pub cue_list_id: Uuid,
    #[schemars(length(min = 1, max = 256))]
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[ts(type = "unknown")]
    pub body: Arc<Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueTransferSummary {
    pub operation: CueTransferOperation,
    pub mode: CueTransferMode,
    pub source_cue_id: Uuid,
    pub source_cue_number: f64,
    pub destination_cue_id: Uuid,
    pub destination_cue_number: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CueTransferOutcome {
    Changed {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        show_id: Uuid,
        choice_id: Uuid,
        summary: CueTransferSummary,
        #[ts(type = "number")]
        show_revision: u64,
        projections: Vec<CueTransferObjectProjection>,
        #[ts(type = "number")]
        show_event_sequence: u64,
        command_line: CommandLineResponse,
        #[ts(as = "Option<f64>", optional = nullable)]
        interaction_event_sequence: Option<u64>,
        #[ts(optional = nullable)]
        persistence_warning: Option<String>,
    },
}

impl CueTransferOutcome {
    pub const fn show_revision(&self) -> u64 {
        match self {
            Self::Changed { show_revision, .. } => *show_revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueTransferErrorKind {
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
pub struct CueTransferErrorResponse {
    pub kind: CueTransferErrorKind,
    pub error: String,
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_related_revision: Option<u64>,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::command_line::CommandTarget;

    #[test]
    fn request_rejects_unknown_or_client_authored_scope() {
        for forged in ["show_id", "desk_id", "user_id", "session_id", "command"] {
            let mut value = request();
            value[forged] = serde_json::json!("forged");
            assert!(serde_json::from_value::<CueTransferRequest>(value).is_err());
        }
    }

    #[test]
    fn outcome_is_strictly_changed_and_keeps_authoritative_extensions() {
        let encoded = serde_json::to_value(outcome()).unwrap();
        assert_eq!(encoded["status"], "changed");
        assert_eq!(encoded["projections"][0]["body"]["future"], true);

        let mut no_change = encoded.clone();
        no_change["status"] = "no_change".into();
        assert!(serde_json::from_value::<CueTransferOutcome>(no_change).is_err());

        let mut extra = encoded;
        extra["selection"] = serde_json::json!([]);
        assert!(serde_json::from_value::<CueTransferOutcome>(extra).is_err());
    }

    #[test]
    fn error_response_rejects_unknown_fields() {
        let value = serde_json::json!({
            "kind":"conflict",
            "error":"stale",
            "current_revision":8,
            "current_related_revision":null,
            "retryable":false,
            "details":{}
        });
        assert!(serde_json::from_value::<CueTransferErrorResponse>(value).is_err());
    }

    fn request() -> Value {
        serde_json::json!({
            "request_id":"cue-transfer-1",
            "choice_id":Uuid::from_u128(1),
            "mode":"status",
            "expected_command_line_revision":7
        })
    }

    fn outcome() -> CueTransferOutcome {
        CueTransferOutcome::Changed {
            request_id: "cue-transfer-1".into(),
            correlation_id: Uuid::from_u128(2),
            replayed: false,
            show_id: Uuid::from_u128(6),
            choice_id: Uuid::from_u128(1),
            summary: CueTransferSummary {
                operation: CueTransferOperation::Copy,
                mode: CueTransferMode::Status,
                source_cue_id: Uuid::from_u128(3),
                source_cue_number: 1.0,
                destination_cue_id: Uuid::from_u128(4),
                destination_cue_number: 2.0,
            },
            show_revision: 8,
            projections: vec![CueTransferObjectProjection {
                cue_list_id: Uuid::from_u128(5),
                object_id: Uuid::from_u128(5).to_string(),
                object_revision: 3,
                body: Arc::new(serde_json::json!({"future":true})),
            }],
            show_event_sequence: 9,
            command_line: CommandLineResponse {
                text: String::new(),
                target: CommandTarget::Fixture,
                pristine: true,
                revision: 8,
                pending_choice: None,
            },
            interaction_event_sequence: Some(10),
            persistence_warning: None,
        }
    }
}
