//! Action-time recording of the current selection into one active-show Group.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupRecordOperation {
    Overwrite,
    Merge,
    Subtract,
    Delete,
}

/// Requests action-time capture; selection and Programmer state never cross the client boundary.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupRecordRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[schemars(length(min = 1, max = 256))]
    pub group_id: String,
    pub operation: GroupRecordOperation,
    #[ts(type = "number")]
    pub expected_object_revision: u64,
}

/// Exact authoritative Group object or its revisioned deletion tombstone.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordedGroupProjection {
    Stored {
        id: String,
        #[ts(type = "number")]
        revision: u64,
        #[ts(type = "unknown")]
        body: Value,
    },
    Deleted {
        id: String,
        #[ts(type = "number")]
        revision: u64,
    },
}

impl RecordedGroupProjection {
    pub fn id(&self) -> &str {
        match self {
            Self::Stored { id, .. } | Self::Deleted { id, .. } => id,
        }
    }

    pub const fn revision(&self) -> u64 {
        match self {
            Self::Stored { revision, .. } | Self::Deleted { revision, .. } => *revision,
        }
    }
}

/// A no-change result can only retain an existing stored Group, never a deletion tombstone.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordedStoredGroupProjection {
    Stored {
        id: String,
        #[ts(type = "number")]
        revision: u64,
        #[ts(type = "unknown")]
        body: Value,
    },
}

impl RecordedStoredGroupProjection {
    pub const fn revision(&self) -> u64 {
        match self {
            Self::Stored { revision, .. } => *revision,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupRecordOutcome {
    Changed {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        #[ts(type = "number")]
        show_revision: u64,
        group: RecordedGroupProjection,
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        #[ts(type = "number")]
        show_revision: u64,
        group: RecordedStoredGroupProjection,
    },
}

impl GroupRecordOutcome {
    pub const fn group_revision(&self) -> u64 {
        match self {
            Self::Changed { group, .. } => group.revision(),
            Self::NoChange { group, .. } => group.revision(),
        }
    }

    pub const fn changed_group(&self) -> Option<&RecordedGroupProjection> {
        match self {
            Self::Changed { group, .. } => Some(group),
            Self::NoChange { .. } => None,
        }
    }

    pub fn request_id(&self) -> &str {
        match self {
            Self::Changed { request_id, .. } | Self::NoChange { request_id, .. } => request_id,
        }
    }

    pub const fn replayed(&self) -> bool {
        match self {
            Self::Changed { replayed, .. } | Self::NoChange { replayed, .. } => *replayed,
        }
    }

    pub const fn event_sequence(&self) -> Option<u64> {
        match self {
            Self::Changed { event_sequence, .. } => Some(*event_sequence),
            Self::NoChange { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupRecordErrorKind {
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
pub struct GroupRecordErrorResponse {
    pub kind: GroupRecordErrorKind,
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
    fn request_rejects_client_authored_selection_and_unknown_fields() {
        for forged in ["fixtures", "selection", "programmer", "programming"] {
            let mut input = request();
            input[forged] = serde_json::json!(["fixture-1"]);
            assert!(serde_json::from_value::<GroupRecordRequest>(input).is_err());
        }
    }

    #[test]
    fn request_preserves_an_opaque_group_id() {
        let mut input = request();
        input["group_id"] = serde_json::json!("Front Wash A / É");
        let decoded: GroupRecordRequest = serde_json::from_value(input).unwrap();
        assert_eq!(decoded.group_id, "Front Wash A / É");
    }

    #[test]
    fn stored_projection_keeps_authoritative_extensions() {
        let outcome = GroupRecordOutcome::Changed {
            request_id: "record-1".into(),
            correlation_id: Uuid::from_u128(2),
            replayed: false,
            show_revision: 8,
            group: RecordedGroupProjection::Stored {
                id: "front".into(),
                revision: 3,
                body: serde_json::json!({"id":"front","future":{"kept":true}}),
            },
            event_sequence: 9,
        };
        let encoded = serde_json::to_value(outcome).unwrap();
        assert_eq!(encoded["status"], "changed");
        assert_eq!(encoded["group"]["state"], "stored");
        assert_eq!(encoded["group"]["body"]["future"]["kept"], true);
    }

    #[test]
    fn stored_no_change_omits_event_sequence() {
        let input = serde_json::json!({
            "request_id": "record-1",
            "correlation_id": Uuid::from_u128(2),
            "replayed": true,
            "show_revision": 8,
            "group": {"state":"stored","id":"front","revision":3,"body":{}},
            "status": "no_change"
        });
        let decoded: GroupRecordOutcome = serde_json::from_value(input).unwrap();
        assert_eq!(decoded.group_revision(), 3);
        assert_eq!(decoded.event_sequence(), None);
    }

    #[test]
    fn no_change_rejects_a_deleted_tombstone() {
        let input = serde_json::json!({
            "request_id": "record-1",
            "correlation_id": Uuid::from_u128(2),
            "replayed": false,
            "show_revision": 8,
            "group": {"state":"deleted","id":"front","revision":3},
            "status": "no_change"
        });
        assert!(serde_json::from_value::<GroupRecordOutcome>(input).is_err());
    }

    #[test]
    fn outcome_rejects_inconsistent_status_and_event_sequence() {
        let base = serde_json::json!({
            "request_id": "record-1",
            "correlation_id": Uuid::from_u128(2),
            "replayed": false,
            "show_revision": 8,
            "group": {"state":"stored","id":"front","revision":3,"body":{}}
        });
        let mut changed = base.clone();
        changed["status"] = "changed".into();
        assert!(serde_json::from_value::<GroupRecordOutcome>(changed).is_err());
        let mut no_change = base;
        no_change["status"] = "no_change".into();
        no_change["event_sequence"] = 9.into();
        assert!(serde_json::from_value::<GroupRecordOutcome>(no_change).is_err());
    }

    #[test]
    fn projection_rejects_body_on_a_deleted_tombstone() {
        let input = serde_json::json!({
            "state":"deleted","id":"front","revision":3,"body":{}
        });
        assert!(serde_json::from_value::<RecordedGroupProjection>(input).is_err());
    }

    fn request() -> serde_json::Value {
        serde_json::json!({
            "request_id": "record-1",
            "group_id": "front",
            "operation": "overwrite",
            "expected_object_revision": 0
        })
    }
}
