//! Dedicated, versioned Stage visualization stream contracts.

use super::preload_values::ProgrammingPreloadAttributeValue;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

pub const VISUALIZATION_PROTOCOL_VERSION: u16 = 1;
pub const VISUALIZATION_MAX_RATE_HZ: u8 = 10;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationScope {
    /// The authoritative Show that produced this frame. `None` represents the
    /// no-active-Show runtime state and prevents a previous Show frame from
    /// being mistaken for the replacement Show at the same revision.
    pub show_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
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
        /// Opt into application-level flow control. The server then retains at
        /// most one in-flight batch and one newest pending source until the
        /// client acknowledges the highest processed sequence.
        #[serde(default)]
        acknowledgements: bool,
        /// Include the diagnostic Dynamic source stack. Stage and channel
        /// consumers need only resolved values; Fixture Sheet claims this
        /// heavier projection explicitly.
        #[serde(default)]
        include_dynamic_stack: bool,
        /// Permit deltas to omit an unchanged Dynamic stack. Missing means
        /// retain the previously installed stack; an explicit empty array
        /// still clears it. Older clients leave this disabled and continue to
        /// receive the complete array on every delta.
        #[serde(default)]
        sparse_dynamic_stack: bool,
        /// Permit the server to encode all lane messages for one publication as
        /// a single JSON array/WebSocket frame. Older clients leave this
        /// disabled and continue to receive one JSON object per frame.
        #[serde(default)]
        batched_messages: bool,
    },
    Unsubscribe {
        lanes: Vec<VisualizationLane>,
    },
    Resynchronize {
        lane: VisualizationLane,
    },
    Acknowledge {
        #[ts(type = "number")]
        sequence: u64,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationValue {
    pub fixture_id: Uuid,
    pub attribute: String,
    pub value: ProgrammingPreloadAttributeValue,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationValueKey {
    pub fixture_id: Uuid,
    pub attribute: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum VisualizationStackEntryType {
    OrdinaryStatic,
    Dynamic,
    FixAt,
    DynamicOff,
    Static,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationDynamicStackEntry {
    pub fixture_id: Uuid,
    pub attribute: String,
    pub entry_type: VisualizationStackEntryType,
    pub priority: i16,
    #[ts(type = "number")]
    pub changed_at_millis: u64,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_number: Option<u16>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_instance_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_mix: Option<f32>,
    pub paused: bool,
    pub hidden: bool,
    pub pending: bool,
    pub winning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<ProgrammingPreloadAttributeValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_value: Option<ProgrammingPreloadAttributeValue>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationLaneSnapshot {
    pub scope: VisualizationScope,
    #[ts(type = "number")]
    pub revision: u64,
    pub generated_at: String,
    pub grand_master: f32,
    pub blackout: bool,
    pub preload: bool,
    pub values: Vec<VisualizationValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic_stack: Vec<VisualizationDynamicStackEntry>,
    pub profile_output_values: Vec<VisualizationValue>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationLaneDelta {
    pub scope: VisualizationScope,
    #[ts(type = "number")]
    pub revision: u64,
    pub generated_at: String,
    pub grand_master: f32,
    pub blackout: bool,
    pub preload: bool,
    pub values: Vec<VisualizationValue>,
    pub removed_values: Vec<VisualizationValueKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_stack: Option<Vec<VisualizationDynamicStackEntry>>,
    pub profile_output_values: Vec<VisualizationValue>,
    pub removed_profile_output_values: Vec<VisualizationValueKey>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VisualizationServerMessage {
    Hello {
        protocol_version: u16,
        max_rate_hz: u8,
        lanes: Vec<VisualizationLane>,
        scope: VisualizationScope,
    },
    Snapshot {
        lane: VisualizationLane,
        scope: VisualizationScope,
        #[ts(type = "number")]
        sequence: u64,
        #[ts(type = "number")]
        source_frame: u64,
        source_timestamp: String,
        published_at: String,
        snapshot: VisualizationLaneSnapshot,
    },
    Delta {
        lane: VisualizationLane,
        scope: VisualizationScope,
        #[ts(type = "number")]
        sequence: u64,
        #[ts(type = "number")]
        source_frame: u64,
        source_timestamp: String,
        published_at: String,
        delta: VisualizationLaneDelta,
    },
    Heartbeat {
        scope: VisualizationScope,
        #[ts(type = "number")]
        sequence: u64,
        published_at: String,
    },
    StructuralInvalidation {
        scope: VisualizationScope,
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
                acknowledgements: false,
                include_dynamic_stack: false,
                sparse_dynamic_stack: false,
                batched_messages: false,
            }
        );
    }

    #[test]
    fn sparse_dynamic_stack_distinguishes_unchanged_from_clear() {
        let mut delta = VisualizationLaneDelta {
            scope: VisualizationScope { show_id: None },
            revision: 1,
            generated_at: "2026-07-29T00:00:00Z".into(),
            grand_master: 1.0,
            blackout: false,
            preload: false,
            values: Vec::new(),
            removed_values: Vec::new(),
            dynamic_stack: None,
            profile_output_values: Vec::new(),
            removed_profile_output_values: Vec::new(),
        };
        let unchanged = serde_json::to_value(&delta).expect("delta serializes");
        assert!(unchanged.get("dynamic_stack").is_none());
        delta.dynamic_stack = Some(Vec::new());
        let cleared = serde_json::to_value(&delta).expect("clear delta serializes");
        assert_eq!(cleared["dynamic_stack"], serde_json::json!([]));
    }

    #[test]
    fn structural_invalidation_carries_show_identity_at_same_revision() {
        let first = VisualizationServerMessage::StructuralInvalidation {
            scope: VisualizationScope {
                show_id: Some(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()),
            },
            revision: 7,
        };
        let replacement = VisualizationServerMessage::StructuralInvalidation {
            scope: VisualizationScope {
                show_id: Some(Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap()),
            },
            revision: 7,
        };

        let first = serde_json::to_value(first).unwrap();
        let replacement = serde_json::to_value(replacement).unwrap();

        assert_eq!(first["revision"], replacement["revision"]);
        assert_ne!(first["scope"]["show_id"], replacement["scope"]["show_id"]);
    }
}
