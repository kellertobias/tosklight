//! Strict v2 transport contracts for Programmer Update preview and mutation workflows.

mod action;
mod preview;
mod settings;

pub use action::*;
pub use preview::*;
pub use settings::*;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingUpdateCueMode {
    ExistingOnly,
    ExistingInCurrentCue,
    AddToCurrentCue,
    AddNew,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingUpdateExistingContentMode {
    UpdateExisting,
    AddNew,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(
    tag = "target_type",
    content = "mode",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProgrammingUpdateMode {
    Cue(ProgrammingUpdateCueMode),
    ExistingContent(ProgrammingUpdateExistingContentMode),
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingUpdateTarget {
    Cue {
        cue_list_id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        playback_number: Option<u16>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        cue_id: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        cue_number: Option<f64>,
        validate_active_context: bool,
    },
    Preset {
        #[schemars(length(min = 1, max = 256))]
        object_id: String,
    },
    Group {
        #[schemars(length(min = 1, max = 256))]
        object_id: String,
    },
}

impl ProgrammingUpdateTarget {
    pub const fn family(&self) -> ProgrammingUpdateTargetFamily {
        match self {
            Self::Cue { .. } => ProgrammingUpdateTargetFamily::Cue,
            Self::Preset { .. } => ProgrammingUpdateTargetFamily::Preset,
            Self::Group { .. } => ProgrammingUpdateTargetFamily::Group,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingUpdateTargetFamily {
    Cue,
    Preset,
    Group,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateCueIdentity {
    pub id: Uuid,
    pub number: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateTargetIdentity {
    pub family: ProgrammingUpdateTargetFamily,
    #[schemars(length(min = 1, max = 256))]
    pub object_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub playback_number: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub cue: Option<ProgrammingUpdateCueIdentity>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingUpdateObjectKind {
    CueList,
    Preset,
    Group,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateObjectIdentity {
    pub kind: ProgrammingUpdateObjectKind,
    #[schemars(length(min = 1, max = 256))]
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingUpdateTargetFilter {
    EligibleForUpdateExisting,
    ShowAllActive,
}

pub(super) fn mode_matches_family(
    mode: ProgrammingUpdateMode,
    family: ProgrammingUpdateTargetFamily,
) -> bool {
    matches!(
        (mode, family),
        (
            ProgrammingUpdateMode::Cue(_),
            ProgrammingUpdateTargetFamily::Cue
        ) | (
            ProgrammingUpdateMode::ExistingContent(_),
            ProgrammingUpdateTargetFamily::Preset | ProgrammingUpdateTargetFamily::Group
        )
    )
}

#[cfg(test)]
mod tests;
