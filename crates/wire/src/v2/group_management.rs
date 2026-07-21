//! Typed management of one stored Group: properties, undo, frozen refresh, and derived detach.
//!
//! Scope is deliberately server-authored. A client declares only the operation, the exact Group
//! storage ID it observed, and the revisions it expects; desk, user, session, and Show identity
//! come from the authenticated session and the request path.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupPropertiesUpdate {
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
    #[schemars(length(max = 64))]
    #[ts(optional = nullable)]
    pub color: Option<String>,
    #[schemars(length(max = 64))]
    #[ts(optional = nullable)]
    pub icon: Option<String>,
}

/// Exact source authority a client observed. A mismatch fails before anything mutates.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupSourceExpectation {
    #[schemars(length(min = 1, max = 256))]
    pub source_group_id: String,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub expected_source_revision: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupManagementOperation {
    UpdateProperties {
        properties: GroupPropertiesUpdate,
    },
    /// Declared as an empty struct variant so `deny_unknown_fields` still applies; serde does not
    /// enforce it for an internally tagged unit variant.
    Undo {},
    RefreshFrozen {
        #[ts(optional = nullable)]
        expected_source: Option<GroupSourceExpectation>,
    },
    DetachDerived {
        #[ts(optional = nullable)]
        expected_source: Option<GroupSourceExpectation>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupManagementRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[schemars(length(min = 1, max = 256))]
    pub group_id: String,
    pub operation: GroupManagementOperation,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub expected_object_revision: u64,
}

/// Authoritative lossless Group projection. These operations never delete their target.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupManagementObjectProjection {
    #[schemars(length(min = 1, max = 256))]
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[ts(type = "unknown")]
    pub body: Arc<Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupManagementOutcome {
    Changed {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        show_id: Uuid,
        #[ts(type = "number")]
        show_revision: u64,
        group: GroupManagementObjectProjection,
        #[ts(type = "number")]
        show_event_sequence: u64,
        #[ts(optional = nullable)]
        persistence_warning: Option<String>,
    },
    NoChange {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        show_id: Uuid,
        #[ts(type = "number")]
        show_revision: u64,
        group: GroupManagementObjectProjection,
        #[ts(optional = nullable)]
        persistence_warning: Option<String>,
    },
}

impl GroupManagementOutcome {
    pub const fn group_revision(&self) -> u64 {
        match self {
            Self::Changed { group, .. } | Self::NoChange { group, .. } => group.object_revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupManagementErrorKind {
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
pub struct GroupManagementErrorResponse {
    pub kind: GroupManagementErrorKind,
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

    #[test]
    fn request_is_strict_and_keeps_scope_server_authored() {
        let request = serde_json::json!({
            "request_id":"manage-1",
            "group_id":"house",
            "operation":{
                "type":"update_properties",
                "properties":{"name":"Front wash","color":"#ff0000","icon":"◆"}
            },
            "expected_object_revision":4
        });
        assert!(serde_json::from_value::<GroupManagementRequest>(request.clone()).is_ok());
        for forged in [
            "desk_id",
            "show_id",
            "user_id",
            "session_id",
            "expected_show_revision",
        ] {
            let mut forged_request = request.clone();
            forged_request[forged] = serde_json::json!("forged");
            assert!(serde_json::from_value::<GroupManagementRequest>(forged_request).is_err());
        }
    }

    #[test]
    fn tagged_operations_and_outcomes_reject_unknown_fields() {
        let operation = serde_json::json!({"type":"undo","group_id":"house"});
        assert!(serde_json::from_value::<GroupManagementOperation>(operation).is_err());
        let outcome = serde_json::json!({"status":"changed","unexpected":true});
        assert!(serde_json::from_value::<GroupManagementOutcome>(outcome).is_err());
    }

    #[test]
    fn a_client_cannot_supply_frozen_membership_or_captured_metadata() {
        let operation = serde_json::json!({
            "type":"refresh_frozen",
            "expected_source":{"source_group_id":"source","expected_source_revision":2},
            "fixtures":["forged"]
        });
        assert!(serde_json::from_value::<GroupManagementOperation>(operation).is_err());
        let expectation = serde_json::json!({
            "source_group_id":"source",
            "expected_source_revision":2,
            "captured_at":"2020-01-01T00:00:00Z"
        });
        assert!(serde_json::from_value::<GroupSourceExpectation>(expectation).is_err());
    }
}
