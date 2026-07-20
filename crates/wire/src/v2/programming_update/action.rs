use super::{
    ProgrammingUpdateCueSource, ProgrammingUpdateMode, ProgrammingUpdateObjectKind,
    ProgrammingUpdateTarget, ProgrammingUpdateTargetIdentity, mode_matches_family,
};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use serde_json::Value;
use std::sync::Arc;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingUpdateAction {
    ConfirmPreview {
        target: ProgrammingUpdateTarget,
        mode: ProgrammingUpdateMode,
        #[ts(type = "number")]
        expected_object_revision: u64,
        expected_programmer_revision: String,
    },
    ApplyDirect {
        target: ProgrammingUpdateTarget,
        mode: ProgrammingUpdateMode,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawProgrammingUpdateAction {
    ConfirmPreview {
        target: ProgrammingUpdateTarget,
        mode: ProgrammingUpdateMode,
        expected_object_revision: u64,
        expected_programmer_revision: String,
    },
    ApplyDirect {
        target: ProgrammingUpdateTarget,
        mode: ProgrammingUpdateMode,
    },
}

impl<'de> Deserialize<'de> for ProgrammingUpdateAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawProgrammingUpdateAction::deserialize(deserializer)?;
        let (target, mode) = match &raw {
            RawProgrammingUpdateAction::ConfirmPreview { target, mode, .. }
            | RawProgrammingUpdateAction::ApplyDirect { target, mode } => (target, *mode),
        };
        if !mode_matches_family(mode, target.family()) {
            return Err(D::Error::custom("Update mode does not match target family"));
        }
        Ok(match raw {
            RawProgrammingUpdateAction::ConfirmPreview {
                target,
                mode,
                expected_object_revision,
                expected_programmer_revision,
            } => Self::ConfirmPreview {
                target,
                mode,
                expected_object_revision,
                expected_programmer_revision,
            },
            RawProgrammingUpdateAction::ApplyDirect { target, mode } => {
                Self::ApplyDirect { target, mode }
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub action: ProgrammingUpdateAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateProjection {
    pub kind: ProgrammingUpdateObjectKind,
    #[schemars(length(min = 1, max = 256))]
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[ts(type = "unknown")]
    pub body: Arc<Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateSummary {
    pub target: ProgrammingUpdateTargetIdentity,
    #[ts(type = "number")]
    pub revision_before: u64,
    #[ts(type = "number")]
    pub revision_after: u64,
    #[ts(type = "number")]
    pub eligible_count: u64,
    #[ts(type = "number")]
    pub changed_count: u64,
    #[ts(type = "number")]
    pub added_count: u64,
    #[ts(type = "number")]
    pub ignored_count: u64,
    pub changed_cues: Vec<ProgrammingUpdateCueSource>,
    pub programmer_values_retained: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingUpdateActionOutcome {
    Changed {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        show_id: Uuid,
        #[ts(type = "number")]
        show_revision: u64,
        projection: ProgrammingUpdateProjection,
        #[ts(type = "number")]
        event_sequence: u64,
        summary: ProgrammingUpdateSummary,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingUpdateErrorKind {
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
pub struct ProgrammingUpdateErrorResponse {
    pub kind: ProgrammingUpdateErrorKind,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_object_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_show_revision: Option<u64>,
    pub retryable: bool,
}
