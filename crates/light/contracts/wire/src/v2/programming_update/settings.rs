use super::{ProgrammingUpdateCueMode, ProgrammingUpdateExistingContentMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How a plain RECORD or UPDATE stores the programmer when the operator names no mode.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingRecordUpdateOption {
    #[default]
    Smart,
    Merge,
    AddExisting,
    AddCue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateSettings {
    pub cue_mode: ProgrammingUpdateCueMode,
    pub preset_mode: ProgrammingUpdateExistingContentMode,
    pub group_mode: ProgrammingUpdateExistingContentMode,
    pub show_update_modal_on_touch: bool,
    pub record_default: ProgrammingRecordUpdateOption,
    pub update_default: ProgrammingRecordUpdateOption,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateSettingsProjection {
    pub settings: ProgrammingUpdateSettings,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingUpdateSettingsUpdateRequest {
    pub request_id: String,
    pub settings: ProgrammingUpdateSettings,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingUpdateSettingsUpdateOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub settings: ProgrammingUpdateSettings,
}
