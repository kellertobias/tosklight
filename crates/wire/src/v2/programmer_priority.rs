//! Lightweight user-scoped Programmer priority mutation and event contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::events::EventSnapshotCursor;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammerPriorityActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub priority: i16,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammerPriorityProjection {
    pub user_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub priority: i16,
    pub changed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammerPriorityChange {
    Upsert {
        projection: ProgrammerPriorityProjection,
    },
    Remove {
        user_id: Uuid,
        #[ts(type = "number")]
        revision: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammerPrioritySnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammerPriorityProjection,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerPriorityActionOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub projection: ProgrammerPriorityProjection,
    #[serde(flatten)]
    pub outcome: ProgrammerPriorityActionState,
    pub replayed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProgrammerPriorityActionState {
    Changed {
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammerPriorityErrorKind {
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
pub struct ProgrammerPriorityErrorResponse {
    pub kind: ProgrammerPriorityErrorKind,
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
    fn no_change_contains_only_lightweight_priority_authority() {
        let outcome = ProgrammerPriorityActionOutcome {
            request_id: "priority-1".into(),
            correlation_id: Uuid::from_u128(2),
            projection: ProgrammerPriorityProjection {
                user_id: Uuid::from_u128(3),
                revision: 4,
                priority: 90,
                changed_at: "2026-07-21T10:00:00Z".into(),
            },
            outcome: ProgrammerPriorityActionState::NoChange,
            replayed: false,
            warning: None,
        };
        let json = serde_json::to_value(outcome).unwrap();
        assert_eq!(json["status"], "no_change");
        assert!(json.get("fixture_values").is_none());
        assert!(json.get("group_values").is_none());
        assert!(json.get("event_sequence").is_none());
    }
}
