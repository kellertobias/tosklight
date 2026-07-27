//! Dedicated, versioned Stage visualization stream contracts.

use super::preload_values::ProgrammingPreloadAttributeValue;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

pub const VISUALIZATION_PROTOCOL_VERSION: u16 = 1;
pub const VISUALIZATION_MAX_RATE_HZ: u8 = 10;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum VisualizationLane {
    Normal,
    Preload,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VisualizationClientMessage {
    Subscribe {
        lanes: Vec<VisualizationLane>,
        max_rate_hz: u8,
    },
    Unsubscribe {
        lanes: Vec<VisualizationLane>,
    },
    Resynchronize {
        lane: VisualizationLane,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationValue {
    pub fixture_id: Uuid,
    pub attribute: String,
    pub value: ProgrammingPreloadAttributeValue,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationLaneSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub generated_at: String,
    pub grand_master: f32,
    pub blackout: bool,
    pub preload: bool,
    pub values: Vec<VisualizationValue>,
    pub profile_output_values: Vec<VisualizationValue>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VisualizationServerMessage {
    Hello {
        protocol_version: u16,
        max_rate_hz: u8,
        lanes: Vec<VisualizationLane>,
    },
    Snapshot {
        lane: VisualizationLane,
        #[ts(type = "number")]
        sequence: u64,
        #[ts(type = "number")]
        source_frame: u64,
        source_timestamp: String,
        published_at: String,
        snapshot: VisualizationLaneSnapshot,
    },
    Heartbeat {
        #[ts(type = "number")]
        sequence: u64,
        published_at: String,
    },
    StructuralInvalidation {
        #[ts(type = "number")]
        revision: u64,
    },
    Error {
        code: String,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_tolerates_unknown_fields() {
        let message: VisualizationClientMessage = serde_json::from_value(serde_json::json!({
            "type": "subscribe",
            "lanes": ["normal"],
            "max_rate_hz": 10,
            "future": true
        }))
        .expect("unknown request fields remain forward compatible");
        assert_eq!(
            message,
            VisualizationClientMessage::Subscribe {
                lanes: vec![VisualizationLane::Normal],
                max_rate_hz: 10,
            }
        );
    }
}
