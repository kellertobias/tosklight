use super::{
    ProgrammingUpdateMode, ProgrammingUpdateObjectIdentity, ProgrammingUpdateTarget,
    ProgrammingUpdateTargetFilter, ProgrammingUpdateTargetIdentity, mode_matches_family,
};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingUpdateAddress {
    FixtureAttribute { fixture_id: Uuid, attribute: String },
    GroupAttribute { group_id: String, attribute: String },
    GroupMembership { fixture_id: Uuid },
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateCueSource {
    pub cue_id: Uuid,
    pub cue_number: f64,
    #[ts(type = "number")]
    pub cue_index: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingUpdateIgnoreReason {
    NewAddress,
    NotInCurrentCue,
    NotInActiveTrackedState,
    NewGroupMember,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingUpdateItemOutcome {
    ChangeAtSource {
        source: ProgrammingUpdateCueSource,
    },
    ChangeInCurrentCue {
        cue: ProgrammingUpdateCueSource,
    },
    AddToCurrentCue {
        cue: ProgrammingUpdateCueSource,
    },
    AddNewToCurrentCue {
        cue: ProgrammingUpdateCueSource,
    },
    UpdateExisting,
    AddNew,
    Unchanged {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        source: Option<ProgrammingUpdateCueSource>,
    },
    Ignored {
        reason: ProgrammingUpdateIgnoreReason,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdatePreviewItem {
    pub address: ProgrammingUpdateAddress,
    pub outcome: ProgrammingUpdateItemOutcome,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdatePreview {
    pub target: ProgrammingUpdateTargetIdentity,
    pub mode: ProgrammingUpdateMode,
    pub items: Vec<ProgrammingUpdatePreviewItem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProgrammingUpdatePreview {
    target: ProgrammingUpdateTargetIdentity,
    mode: ProgrammingUpdateMode,
    items: Vec<ProgrammingUpdatePreviewItem>,
}

impl<'de> Deserialize<'de> for ProgrammingUpdatePreview {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawProgrammingUpdatePreview::deserialize(deserializer)?;
        if !mode_matches_family(raw.mode, raw.target.family) {
            return Err(D::Error::custom("Update mode does not match target family"));
        }
        Ok(Self {
            target: raw.target,
            mode: raw.mode,
            items: raw.items,
        })
    }
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdatePreviewRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub target: ProgrammingUpdateTarget,
    pub mode: ProgrammingUpdateMode,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProgrammingUpdatePreviewRequest {
    request_id: String,
    target: ProgrammingUpdateTarget,
    mode: ProgrammingUpdateMode,
}

impl<'de> Deserialize<'de> for ProgrammingUpdatePreviewRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawProgrammingUpdatePreviewRequest::deserialize(deserializer)?;
        if !mode_matches_family(raw.mode, raw.target.family()) {
            return Err(D::Error::custom("Update mode does not match target family"));
        }
        Ok(Self {
            request_id: raw.request_id,
            target: raw.target,
            mode: raw.mode,
        })
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdatePreviewResponse {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub object: ProgrammingUpdateObjectIdentity,
    pub programmer_revision: String,
    pub preview: ProgrammingUpdatePreview,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateTargetsRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub filter: ProgrammingUpdateTargetFilter,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateTargetEntry {
    pub request_target: ProgrammingUpdateTarget,
    pub object: ProgrammingUpdateObjectIdentity,
    pub programmer_revision: String,
    pub active_or_referenced: bool,
    pub existing_preview: ProgrammingUpdatePreview,
    pub add_new_preview: ProgrammingUpdatePreview,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateTargetsResponse {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub targets: Vec<ProgrammingUpdateTargetEntry>,
}
