//! Revisioned installation-global Grand Master and blackout mutation contract.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::events::OutputRuntimeProjection;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OutputRuntimeActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub expected_show_id: Uuid,
    #[ts(type = "number")]
    pub expected_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub grand_master: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub blackout: Option<bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputRuntimeDurability {
    Durable,
    PersistencePending,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum OutputRuntimeActionState {
    Changed {
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange {},
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OutputRuntimeActionOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub projection: OutputRuntimeProjection,
    #[serde(flatten)]
    pub outcome: OutputRuntimeActionState,
    pub replayed: bool,
    pub durability: OutputRuntimeDurability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputRuntimeErrorKind {
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
pub struct OutputRuntimeErrorResponse {
    pub kind: OutputRuntimeErrorKind,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_and_nested_outcome_reject_unknown_fields() {
        assert!(
            serde_json::from_value::<OutputRuntimeActionRequest>(serde_json::json!({
                "request_id":"output-1",
                "expected_show_id":Uuid::from_u128(1),
                "expected_revision":2,
                "grand_master":0.5,
                "unexpected":true
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<OutputRuntimeActionOutcome>(serde_json::json!({
                "request_id":"output-1",
                "correlation_id":Uuid::from_u128(2),
                "projection":{
                    "scope":{"show_id":Uuid::from_u128(1)},
                    "identity":"global_master",
                    "revision":3,
                    "grand_master":0.5,
                    "blackout":false,
                    "unexpected":true
                },
                "status":"no_change",
                "replayed":false,
                "durability":"durable"
            }))
            .is_err()
        );
    }

    #[test]
    fn no_change_omits_event_sequence_and_warning() {
        let outcome = OutputRuntimeActionOutcome {
            request_id: "output-1".into(),
            correlation_id: Uuid::from_u128(2),
            projection: OutputRuntimeProjection {
                scope: super::super::events::OutputRuntimeScope {
                    show_id: Uuid::from_u128(1),
                },
                identity: super::super::events::OutputRuntimeIdentity::GlobalMaster,
                revision: 3,
                grand_master: 0.5,
                blackout: false,
            },
            outcome: OutputRuntimeActionState::NoChange {},
            replayed: false,
            durability: OutputRuntimeDurability::Durable,
            warning: None,
        };
        let json = serde_json::to_value(outcome).unwrap();
        assert_eq!(json["status"], "no_change");
        assert!(json.get("event_sequence").is_none());
        assert!(json.get("warning").is_none());
    }
}
