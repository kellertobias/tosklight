//! User-scoped, recordable Programmer value projections and repair snapshots.

use super::events::EventSnapshotCursor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingColorXyz {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProgrammingAttributeValue {
    Normalized(f32),
    Spread(Vec<f32>),
    Discrete(String),
    ColorXyz(ProgrammingColorXyz),
    RawDmx(u8),
    RawDmxExact(u32),
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingFixtureValue {
    pub fixture_id: Uuid,
    pub attribute: String,
    pub value: ProgrammingAttributeValue,
    #[ts(type = "number")]
    pub programmer_order: u64,
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingGroupValue {
    pub group_id: String,
    pub attribute: String,
    pub value: ProgrammingAttributeValue,
    #[ts(type = "number")]
    pub programmer_order: u64,
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub delay_millis: Option<u64>,
}

/// Full retained projection of one user's normal, recordable Programmer values.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesProjection {
    pub user_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub fixture_values: Vec<ProgrammingFixtureValue>,
    pub group_values: Vec<ProgrammingGroupValue>,
}

/// Authoritative capture routing for one user's Programmer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingCaptureModeProjection {
    pub user_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub blind: bool,
    pub preview: bool,
    pub preload_capture_programmer: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingCaptureModeChange {
    pub projection: ProgrammingCaptureModeProjection,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingCaptureModeSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingCaptureModeProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesChange {
    pub projection: ProgrammingValuesProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingValuesProjection,
}

/// One authenticated, idempotent mutation of normal, recordable Programmer values.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingValuesActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    #[ts(type = "number")]
    pub expected_capture_mode_revision: u64,
    pub action: ProgrammingValuesAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingValuesAction {
    SetFixture {
        fixture_id: Uuid,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    ReleaseFixture {
        fixture_id: Uuid,
        attribute: String,
    },
    SetGroup {
        group_id: String,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: String,
    },
    Batch {
        #[schemars(length(max = 10_000))]
        mutations: Vec<ProgrammingValueMutation>,
    },
    Clear,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingValueMutation {
    SetFixture {
        fixture_id: Uuid,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    ReleaseFixture {
        fixture_id: Uuid,
        attribute: String,
    },
    SetGroup {
        group_id: String,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: String,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingValueTiming {
    #[serde(default)]
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub delay_millis: Option<u64>,
}

/// Typed result for one Programmer-values action. No-change results deliberately omit the full
/// projection so interaction-only actions do not force projection materialization or transport.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesActionOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub capture_mode_revision: u64,
    #[serde(flatten)]
    pub outcome: ProgrammingValuesActionState,
    pub replayed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProgrammingValuesActionState {
    Changed {
        projection: ProgrammingValuesProjection,
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesErrorResponse {
    pub kind: ProgrammingValuesErrorKind,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_capture_mode_revision: Option<u64>,
    pub retryable: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingValuesErrorKind {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Unavailable,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_keeps_addresses_order_and_timing() {
        let value = ProgrammingFixtureValue {
            fixture_id: Uuid::from_u128(1),
            attribute: "intensity".into(),
            value: ProgrammingAttributeValue::Normalized(0.5),
            programmer_order: 7,
            fade: true,
            fade_millis: Some(1_000),
            delay_millis: Some(250),
        };
        let json = serde_json::to_value(value).unwrap();
        assert_eq!(json["fixture_id"], Uuid::from_u128(1).to_string());
        assert_eq!(json["programmer_order"], 7);
        assert_eq!(json["fade"], true);
        assert_eq!(json["fade_millis"], 1_000);
        assert_eq!(json["delay_millis"], 250);
        assert_eq!(json["value"]["kind"], "normalized");
    }

    #[test]
    fn actions_reject_fields_outside_the_recordable_values_contract() {
        let value = serde_json::json!({
            "request_id": "request-1",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {
                "type": "set_fixture",
                "fixture_id": Uuid::from_u128(1),
                "attribute": "intensity",
                "value": {"kind": "normalized", "value": 0.5},
                "mode": "preload"
            }
        });
        assert!(serde_json::from_value::<ProgrammingValuesActionRequest>(value).is_err());
    }

    #[test]
    fn action_requires_the_capture_mode_revision() {
        let value = serde_json::json!({
            "request_id": "request-1",
            "expected_revision": 0,
            "action": {"type": "clear"}
        });
        assert!(serde_json::from_value::<ProgrammingValuesActionRequest>(value).is_err());
    }

    #[test]
    fn capture_mode_projection_rejects_unknown_fields() {
        let value = serde_json::json!({
            "user_id": Uuid::from_u128(1),
            "revision": 2,
            "blind": false,
            "preview": false,
            "preload_capture_programmer": true,
            "selection": []
        });
        assert!(serde_json::from_value::<ProgrammingCaptureModeProjection>(value).is_err());
    }

    #[test]
    fn no_change_outcome_does_not_serialize_a_projection_or_event_sequence() {
        let outcome = ProgrammingValuesActionOutcome {
            request_id: "request-2".into(),
            correlation_id: Uuid::from_u128(2),
            revision: 7,
            capture_mode_revision: 3,
            outcome: ProgrammingValuesActionState::NoChange,
            replayed: false,
            warning: None,
        };
        let json = serde_json::to_value(outcome).unwrap();
        assert_eq!(json["status"], "no_change");
        assert!(json.get("projection").is_none());
        assert!(json.get("event_sequence").is_none());
    }
}
