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

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesChange {
    pub projection: ProgrammingValuesProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingValuesProjection,
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
}
