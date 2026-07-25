//! Action-time recording of normal or Preload Programmer values into one Cue transaction.

use super::playback::PlaybackRuntimeProjection;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CueRecordTarget {
    Pool {
        #[schemars(range(min = 1, max = 1000))]
        playback_number: u16,
    },
    SelectedPlayback,
    PageSlot {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1, max = 127))]
        slot: u8,
    },
    CueList {
        cue_list_id: Uuid,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueRecordOperation {
    Overwrite,
    Merge,
    Subtract,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueRecordTiming {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueRecordCapturePolicy {
    CurrentCapture,
    PendingOrActivePreload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueRecordActivationPolicy {
    Hold,
    GoToIfNormal,
}

/// Requests an action-time capture; recordable Programmer values never cross the client boundary.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueRecordRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub target: CueRecordTarget,
    pub operation: CueRecordOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub cue_number: Option<f64>,
    pub timing: CueRecordTiming,
    pub cue_only: bool,
    #[schemars(length(max = 256))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub name: Option<String>,
    pub capture_policy: CueRecordCapturePolicy,
    pub activation_policy: CueRecordActivationPolicy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueRecordCapturedSource {
    Normal,
    PendingPreload,
    ActivePreload,
}

/// One exact losslessly merged portable show object.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RecordedCueObjectProjection {
    pub id: String,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "unknown")]
    pub body: Arc<Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueRecordProjections {
    pub cue_list: RecordedCueObjectProjection,
    pub playback: Option<RecordedCueObjectProjection>,
    pub page: Option<RecordedCueObjectProjection>,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RecordedCueProjection {
    pub id: Uuid,
    pub number: f64,
    pub deleted: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueRecordRuntimeOutcome {
    pub projection: PlaybackRuntimeProjection,
    #[ts(type = "number")]
    pub event_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CueRecordOutcome {
    Changed {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        captured_source: CueRecordCapturedSource,
        #[ts(type = "number")]
        show_revision: u64,
        recorded_cue: RecordedCueProjection,
        projections: CueRecordProjections,
        #[ts(type = "number")]
        show_event_sequence: u64,
        runtime: Option<Box<CueRecordRuntimeOutcome>>,
    },
    NoChange {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        captured_source: CueRecordCapturedSource,
        #[ts(type = "number")]
        show_revision: u64,
        recorded_cue: RecordedCueProjection,
        projections: CueRecordProjections,
    },
}

impl CueRecordOutcome {
    pub const fn show_revision(&self) -> u64 {
        match self {
            Self::Changed { show_revision, .. } | Self::NoChange { show_revision, .. } => {
                *show_revision
            }
        }
    }

    pub fn request_id(&self) -> &str {
        match self {
            Self::Changed { request_id, .. } | Self::NoChange { request_id, .. } => request_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueRecordErrorKind {
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
pub struct CueRecordErrorResponse {
    pub kind: CueRecordErrorKind,
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
    fn request_rejects_client_authored_programmer_or_identity_state() {
        for forged in [
            "values",
            "programmer",
            "selection",
            "user_id",
            "session_id",
            "desk_id",
            "show_id",
        ] {
            let mut input = request();
            input[forged] = serde_json::json!({"forged": true});
            assert!(serde_json::from_value::<CueRecordRequest>(input).is_err());
        }
    }

    #[test]
    fn target_variants_preserve_exact_addressing() {
        let targets = [
            serde_json::json!({"kind":"pool","playback_number":27}),
            serde_json::json!({"kind":"selected_playback"}),
            serde_json::json!({"kind":"page_slot","page":2,"slot":7}),
            serde_json::json!({"kind":"cue_list","cue_list_id":Uuid::from_u128(4)}),
        ];
        for target in targets {
            assert!(serde_json::from_value::<CueRecordTarget>(target).is_ok());
        }
    }

    #[test]
    fn nested_request_shapes_reject_unknown_fields() {
        let mut target = request();
        target["target"]["playback"] = 27.into();
        assert!(serde_json::from_value::<CueRecordRequest>(target).is_err());
        let mut timing = request();
        timing["timing"]["fade"] = 3.into();
        assert!(serde_json::from_value::<CueRecordRequest>(timing).is_err());
    }

    #[test]
    fn changed_outcome_keeps_authoritative_extensions() {
        let outcome = changed_outcome();
        let encoded = serde_json::to_value(outcome).unwrap();
        assert_eq!(encoded["status"], "changed");
        assert_eq!(encoded["show_event_sequence"], 9);
        assert_eq!(encoded["projections"]["cue_list"]["body"]["future"], true);
    }

    #[test]
    fn no_change_rejects_show_or_runtime_event_fields() {
        let mut input = serde_json::to_value(changed_outcome()).unwrap();
        input["status"] = "no_change".into();
        assert!(serde_json::from_value::<CueRecordOutcome>(input).is_err());
    }

    fn request() -> Value {
        serde_json::json!({
            "request_id":"cue-record-1",
            "target":{"kind":"pool","playback_number":27},
            "operation":"overwrite",
            "cue_number":2.5,
            "timing":{"fade_millis":1000},
            "cue_only":false,
            "name":"Look",
            "capture_policy":"current_capture",
            "activation_policy":"hold"
        })
    }

    fn changed_outcome() -> CueRecordOutcome {
        CueRecordOutcome::Changed {
            request_id: "cue-record-1".into(),
            correlation_id: Uuid::from_u128(2),
            replayed: false,
            captured_source: CueRecordCapturedSource::Normal,
            show_revision: 8,
            recorded_cue: RecordedCueProjection {
                id: Uuid::from_u128(3),
                number: 2.5,
                deleted: false,
            },
            projections: CueRecordProjections {
                cue_list: RecordedCueObjectProjection {
                    id: Uuid::from_u128(4).to_string(),
                    revision: 3,
                    body: Arc::new(serde_json::json!({"future":true})),
                },
                playback: None,
                page: None,
            },
            show_event_sequence: 9,
            runtime: None,
        }
    }
}
