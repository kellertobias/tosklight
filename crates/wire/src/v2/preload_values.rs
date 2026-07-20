//! User-scoped pending Preload Programmer values and mutation contracts.

use super::events::EventSnapshotCursor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadColorXyz {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProgrammingPreloadAttributeValue {
    Normalized(f32),
    Spread(Vec<f32>),
    Discrete(String),
    ColorXyz(ProgrammingPreloadColorXyz),
    RawDmx(u8),
    RawDmxExact(u32),
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadFixtureValue {
    pub fixture_id: Uuid,
    pub attribute: String,
    pub value: ProgrammingPreloadAttributeValue,
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
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadGroupValue {
    pub group_id: String,
    pub attribute: String,
    pub value: ProgrammingPreloadAttributeValue,
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
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadValuesProjection {
    pub user_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub fixture_values: Vec<ProgrammingPreloadFixtureValue>,
    pub group_values: Vec<ProgrammingPreloadGroupValue>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadValuesChange {
    pub projection: ProgrammingPreloadValuesProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadValuesSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingPreloadValuesProjection,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadValueTiming {
    #[serde(default)]
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingPreloadValueMutation {
    SetFixture {
        fixture_id: Uuid,
        attribute: String,
        value: ProgrammingPreloadAttributeValue,
        #[serde(default)]
        timing: ProgrammingPreloadValueTiming,
    },
    ReleaseFixture {
        fixture_id: Uuid,
        attribute: String,
    },
    SetGroup {
        group_id: String,
        attribute: String,
        value: ProgrammingPreloadAttributeValue,
        #[serde(default)]
        timing: ProgrammingPreloadValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: String,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingPreloadValuesAction {
    SetFixture {
        fixture_id: Uuid,
        attribute: String,
        value: ProgrammingPreloadAttributeValue,
        #[serde(default)]
        timing: ProgrammingPreloadValueTiming,
    },
    ReleaseFixture {
        fixture_id: Uuid,
        attribute: String,
    },
    SetGroup {
        group_id: String,
        attribute: String,
        value: ProgrammingPreloadAttributeValue,
        #[serde(default)]
        timing: ProgrammingPreloadValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: String,
    },
    Batch {
        #[schemars(length(max = 10_000))]
        mutations: Vec<ProgrammingPreloadValueMutation>,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadValuesActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    #[ts(type = "number")]
    pub expected_capture_mode_revision: u64,
    pub action: ProgrammingPreloadValuesAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingPreloadValuesActionState {
    Changed {
        projection: ProgrammingPreloadValuesProjection,
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingPreloadValuesActionOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub capture_mode_revision: u64,
    #[serde(flatten)]
    pub outcome: ProgrammingPreloadValuesActionState,
    pub replayed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingPreloadValuesErrorKind {
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
pub struct ProgrammingPreloadValuesErrorResponse {
    pub kind: ProgrammingPreloadValuesErrorKind,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_capture_mode_revision: Option<u64>,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_is_a_distinct_pending_preload_contract() {
        let value = serde_json::json!({
            "request_id": "preload-1",
            "expected_revision": 2,
            "expected_capture_mode_revision": 3,
            "action": {
                "type": "set_fixture",
                "fixture_id": Uuid::from_u128(1),
                "attribute": "intensity",
                "value": {"kind": "normalized", "value": 0.5},
                "mode": "normal"
            }
        });
        assert!(serde_json::from_value::<ProgrammingPreloadValuesActionRequest>(value).is_err());
    }

    #[test]
    fn no_change_omits_projection_and_event_sequence() {
        let outcome = ProgrammingPreloadValuesActionOutcome {
            request_id: "preload-2".into(),
            correlation_id: Uuid::from_u128(2),
            revision: 7,
            capture_mode_revision: 3,
            outcome: ProgrammingPreloadValuesActionState::NoChange,
            replayed: false,
            warning: None,
        };
        let json = serde_json::to_value(outcome).unwrap();
        assert_eq!(json["status"], "no_change");
        assert!(json.get("projection").is_none());
        assert!(json.get("event_sequence").is_none());
    }
}
