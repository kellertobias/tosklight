//! Action-time recording of normal Programmer values into one active-show Preset.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PresetRecordingFamily {
    Mixed,
    Intensity,
    Color,
    Position,
    Beam,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PresetRecordingAddress {
    pub family: PresetRecordingFamily,
    #[schemars(range(min = 1))]
    pub number: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PresetRecordingMode {
    Merge,
    Overwrite,
}

/// Requests action-time capture; recordable Programmer values never cross the client boundary.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PresetRecordRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub address: PresetRecordingAddress,
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
    pub mode: PresetRecordingMode,
    #[ts(type = "number")]
    pub expected_object_revision: u64,
}

/// Exact persisted Preset object, including unknown compatible extensions.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RecordedPresetProjection {
    pub id: String,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "unknown")]
    pub body: Value,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresetRecordOutcome {
    Changed {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        #[ts(type = "number")]
        show_revision: u64,
        preset: RecordedPresetProjection,
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        #[ts(type = "number")]
        show_revision: u64,
        preset: RecordedPresetProjection,
    },
}

impl PresetRecordOutcome {
    pub const fn preset(&self) -> &RecordedPresetProjection {
        match self {
            Self::Changed { preset, .. } | Self::NoChange { preset, .. } => preset,
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
pub enum PresetRecordErrorKind {
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
pub struct PresetRecordErrorResponse {
    pub kind: PresetRecordErrorKind,
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
    fn request_rejects_client_authored_values_and_unknown_fields() {
        let input = serde_json::json!({
            "request_id": "record-1",
            "address": {"family": "color", "number": 7},
            "name": "Color 7",
            "mode": "overwrite",
            "expected_object_revision": 0,
            "values": {"fixture": "forged"}
        });
        assert!(serde_json::from_value::<PresetRecordRequest>(input).is_err());
    }

    #[test]
    fn changed_outcome_keeps_authoritative_extensions() {
        let outcome = PresetRecordOutcome::Changed {
            request_id: "record-1".into(),
            correlation_id: Uuid::from_u128(2),
            replayed: false,
            show_revision: 8,
            preset: RecordedPresetProjection {
                id: "07".into(),
                revision: 3,
                body: serde_json::json!({"family":"Color","future":{"kept":true}}),
            },
            event_sequence: 9,
        };
        let encoded = serde_json::to_value(outcome).unwrap();
        assert_eq!(encoded["status"], "changed");
        assert_eq!(encoded["event_sequence"], 9);
        assert_eq!(encoded["preset"]["body"]["future"]["kept"], true);
    }

    #[test]
    fn no_change_outcome_omits_event_sequence() {
        let input = serde_json::json!({
            "request_id": "record-1",
            "correlation_id": Uuid::from_u128(2),
            "replayed": true,
            "show_revision": 8,
            "preset": {"id":"2.7","revision":3,"body":{}},
            "status": "no_change"
        });
        let decoded: PresetRecordOutcome = serde_json::from_value(input).unwrap();
        assert!(matches!(decoded, PresetRecordOutcome::NoChange { .. }));
        assert_eq!(decoded.event_sequence(), None);
    }

    #[test]
    fn outcome_rejects_inconsistent_status_and_event_sequence() {
        let base = serde_json::json!({
            "request_id": "record-1",
            "correlation_id": Uuid::from_u128(2),
            "replayed": false,
            "show_revision": 8,
            "preset": {"id":"2.7","revision":3,"body":{}}
        });
        let mut changed = base.clone();
        changed["status"] = "changed".into();
        assert!(serde_json::from_value::<PresetRecordOutcome>(changed).is_err());
        let mut no_change = base;
        no_change["status"] = "no_change".into();
        no_change["event_sequence"] = 9.into();
        assert!(serde_json::from_value::<PresetRecordOutcome>(no_change).is_err());
    }

    #[test]
    fn outcome_rejects_unknown_top_level_fields() {
        let input = serde_json::json!({
            "request_id": "record-1",
            "correlation_id": Uuid::from_u128(2),
            "replayed": false,
            "show_revision": 8,
            "preset": {"id":"2.7","revision":3,"body":{}},
            "status": "no_change",
            "unexpected": true
        });
        assert!(serde_json::from_value::<PresetRecordOutcome>(input).is_err());
    }
}
