//! Typed deletion of one whole Cue from an authoritative Cuelist projection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CueDeletionAddress {
    Pool { playback_number: u16 },
    CurrentPage { expected_page: u8, slot: u8 },
    PageSlot { page: u8, slot: u8 },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueDeletionAuthority {
    pub playback_number: u16,
    pub cue_list_id: Uuid,
    #[schemars(length(min = 1, max = 256))]
    pub object_id: String,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub object_revision: u64,
    pub cue_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueDeletionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub address: CueDeletionAddress,
    pub cue_number: f64,
    pub authority: CueDeletionAuthority,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueDeletionObjectProjection {
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
pub struct DeletedCueProjection {
    pub id: Uuid,
    pub number: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CueDeletionOutcome {
    Changed {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        show_id: Uuid,
        #[ts(type = "number")]
        show_revision: u64,
        cue_list: CueDeletionObjectProjection,
        deleted_cue: DeletedCueProjection,
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
        cue_list: CueDeletionObjectProjection,
        deleted_cue: DeletedCueProjection,
        #[ts(optional = nullable)]
        persistence_warning: Option<String>,
    },
}

impl CueDeletionOutcome {
    pub const fn show_revision(&self) -> u64 {
        match self {
            Self::Changed { show_revision, .. } | Self::NoChange { show_revision, .. } => {
                *show_revision
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueDeletionErrorKind {
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
pub struct CueDeletionErrorResponse {
    pub kind: CueDeletionErrorKind,
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
            "request_id":"delete-1",
            "address":{"type":"current_page","expected_page":2,"slot":3},
            "cue_number":2.5,
            "authority":{
                "playback_number":7,
                "cue_list_id":Uuid::from_u128(1),
                "object_id":"stored-list",
                "object_revision":4,
                "cue_id":Uuid::from_u128(2)
            }
        });
        assert!(serde_json::from_value::<CueDeletionRequest>(request.clone()).is_ok());
        for forged in ["desk_id", "show_id", "user_id", "expected_show_revision"] {
            let mut forged_request = request.clone();
            forged_request[forged] = serde_json::json!("forged");
            assert!(serde_json::from_value::<CueDeletionRequest>(forged_request).is_err());
        }
    }

    #[test]
    fn tagged_addresses_and_outcomes_reject_unknown_fields() {
        let address = serde_json::json!({"type":"pool","playback_number":1,"page":2});
        assert!(serde_json::from_value::<CueDeletionAddress>(address).is_err());
        let outcome = serde_json::json!({"status":"changed","unexpected":true});
        assert!(serde_json::from_value::<CueDeletionOutcome>(outcome).is_err());
    }
}
