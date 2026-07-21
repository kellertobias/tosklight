//! Revisioned retained/manual Speed Group authority for Groups A-E.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::events::EventSnapshotCursor;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
pub enum SpeedGroupId {
    A,
    B,
    C,
    D,
    E,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeedGroupProjection {
    pub group: SpeedGroupId,
    pub manual_bpm: f64,
    pub paused: bool,
    pub speed_master_scale: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub synchronized_with: Option<SpeedGroupId>,
    #[ts(type = "number")]
    pub phase_origin_millis: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeedGroupAuthorityProjection {
    pub authority_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub groups: Vec<SpeedGroupProjection>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeedGroupSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: SpeedGroupAuthorityProjection,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeedGroupAction {
    SetBpm {
        group: SpeedGroupId,
        bpm: f64,
    },
    AdjustBpm {
        group: SpeedGroupId,
        delta_bpm: f64,
    },
    Synchronize {
        source: SpeedGroupId,
        target: SpeedGroupId,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeedGroupActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub expected_authority_id: Uuid,
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub action: SpeedGroupAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SpeedGroupDurability {
    Durable,
    PersistencePending,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeedGroupActionState {
    Changed {
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange {},
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeedGroupActionOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub authority_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub applied_at_millis: u64,
    pub groups: Vec<SpeedGroupProjection>,
    #[serde(flatten)]
    pub outcome: SpeedGroupActionState,
    pub replayed: bool,
    pub durability: SpeedGroupDurability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeedGroupChange {
    pub authority_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub applied_at_millis: u64,
    pub groups: Vec<SpeedGroupProjection>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SpeedGroupErrorKind {
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
pub struct SpeedGroupErrorResponse {
    pub kind: SpeedGroupErrorKind,
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
    fn request_and_nested_action_reject_unknown_fields() {
        assert!(
            serde_json::from_value::<SpeedGroupActionRequest>(serde_json::json!({
                "request_id":"speed-1",
                "expected_authority_id":Uuid::from_u128(1),
                "expected_revision":2,
                "action":{"type":"set_bpm","group":"A","bpm":120.5,"extra":true}
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<SpeedGroupActionRequest>(serde_json::json!({
                "request_id":"speed-1",
                "expected_authority_id":Uuid::from_u128(1),
                "expected_revision":2,
                "action":{"type":"set_bpm","group":"A","bpm":120.5},
                "extra":true
            }))
            .is_err()
        );
    }

    #[test]
    fn no_change_omits_event_sequence_and_warning() {
        let outcome = SpeedGroupActionOutcome {
            request_id: "speed-1".into(),
            correlation_id: Uuid::from_u128(2),
            authority_id: Uuid::from_u128(1),
            revision: 3,
            applied_at_millis: 100,
            groups: vec![projection()],
            outcome: SpeedGroupActionState::NoChange {},
            replayed: false,
            durability: SpeedGroupDurability::Durable,
            warning: None,
        };
        let json = serde_json::to_value(outcome).unwrap();
        assert_eq!(json["status"], "no_change");
        assert!(json.get("event_sequence").is_none());
        assert!(json.get("warning").is_none());
    }

    fn projection() -> SpeedGroupProjection {
        SpeedGroupProjection {
            group: SpeedGroupId::A,
            manual_bpm: 120.0,
            paused: false,
            speed_master_scale: 1.0,
            synchronized_with: None,
            phase_origin_millis: 0,
        }
    }
}
