use super::{ProgrammingUpdateCueMode, ProgrammingUpdateExistingContentMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingUpdateSettings {
    pub cue_mode: ProgrammingUpdateCueMode,
    pub preset_mode: ProgrammingUpdateExistingContentMode,
    pub group_mode: ProgrammingUpdateExistingContentMode,
    pub show_update_modal_on_touch: bool,
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
