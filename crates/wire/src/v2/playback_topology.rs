//! Strict portable Cuelist, Playback, and Page topology action DTOs.

use super::events::ShowObjectKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize, TS)]
#[serde(untagged)]
pub enum PlaybackTopologyObjectIdentity {
    Present(String),
    Absent(()),
}

impl PlaybackTopologyObjectIdentity {
    pub fn into_option(self) -> Option<String> {
        match self {
            Self::Present(value) => Some(value),
            Self::Absent(()) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PlaybackTopologyActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub action: PlaybackTopologyAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaybackTopologyAction {
    SaveCueList {
        cue_list_id: Uuid,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_revision: u64,
        #[ts(type = "string | null")]
        expected_object_id: PlaybackTopologyObjectIdentity,
        /// Extensible portable body; adapters strictly decode its known Cuelist fields.
        #[ts(type = "unknown")]
        body: Value,
    },
    ConfigureSlot {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1, max = 127))]
        slot: u8,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "string | null")]
        expected_page_object_id: PlaybackTopologyObjectIdentity,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_playback_revision: u64,
        #[ts(type = "string | null")]
        expected_playback_object_id: PlaybackTopologyObjectIdentity,
        playback: PlaybackTopologyPlaybackDefinition,
    },
    MapExistingPlayback {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1, max = 127))]
        slot: u8,
        #[schemars(range(min = 1, max = 1000))]
        playback_number: u16,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "string | null")]
        expected_page_object_id: PlaybackTopologyObjectIdentity,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_playback_revision: u64,
        #[ts(type = "string | null")]
        expected_playback_object_id: PlaybackTopologyObjectIdentity,
    },
    CreatePage {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "string | null")]
        expected_page_object_id: PlaybackTopologyObjectIdentity,
    },
    RenamePage {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(length(min = 1, max = 80))]
        name: String,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "string | null")]
        expected_page_object_id: PlaybackTopologyObjectIdentity,
    },
    ClearMappedPlayback {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1, max = 127))]
        slot: u8,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "string | null")]
        expected_page_object_id: PlaybackTopologyObjectIdentity,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_playback_revision: u64,
        #[ts(type = "string | null")]
        expected_playback_object_id: PlaybackTopologyObjectIdentity,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PlaybackTopologyPlaybackDefinition {
    #[schemars(range(max = 1000))]
    pub number: u16,
    #[schemars(length(min = 1, max = 80))]
    pub name: String,
    pub target: PlaybackTopologyTarget,
    pub buttons: [PlaybackTopologyButtonAction; 3],
    #[schemars(range(max = 3))]
    pub button_count: u8,
    pub fader: PlaybackTopologyFaderMode,
    pub has_fader: bool,
    pub go_activates: bool,
    pub auto_off: bool,
    #[ts(type = "number")]
    pub xfade_millis: u64,
    pub color: String,
    pub flash_release: PlaybackTopologyFlashReleaseMode,
    pub protect_from_swap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub presentation_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub presentation_image: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaybackTopologyTarget {
    CueList { cue_list_id: Uuid },
    Group { group_id: String },
    SpeedGroup { group: String },
    ProgrammerFade {},
    CueFade {},
    GrandMaster {},
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackTopologyButtonAction {
    On,
    Off,
    Toggle,
    Go,
    GoMinus,
    FastForward,
    FastRewind,
    Flash,
    Temp,
    Swap,
    Select,
    SelectContents,
    SelectDereferenced,
    Learn,
    Double,
    Half,
    Pause,
    Blackout,
    PauseDynamics,
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackTopologyFaderMode {
    Master,
    Temp,
    Speed,
    XFade,
    DirectBpm,
    CenteredRelative,
    LearnedPercentage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackTopologyFlashReleaseMode {
    ReleaseAll,
    ReleaseIntensityOnly,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackTopologyActionOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub show_revision: u64,
    pub resolution: PlaybackTopologyResolution,
    #[serde(flatten)]
    pub outcome: PlaybackTopologyActionState,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaybackTopologyActionState {
    Changed {
        objects: Vec<PlaybackTopologyObjectProjection>,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange {
        objects: Vec<PlaybackTopologyObjectProjection>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaybackTopologyResolution {
    CueList {
        cue_list_id: Uuid,
    },
    PageSlot {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1, max = 127))]
        slot: u8,
        #[schemars(range(min = 1, max = 1000))]
        playback_number: Option<u16>,
    },
    Page {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaybackTopologyObjectProjection {
    Present {
        kind: ShowObjectKind,
        object_id: String,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown")]
        body: Value,
    },
    Deleted {
        kind: ShowObjectKind,
        object_id: String,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        object_revision: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PlaybackTopologyErrorResponse {
    pub kind: PlaybackTopologyErrorKind,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_related_revision: Option<u64>,
    pub retryable: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackTopologyErrorKind {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Unavailable,
    Internal,
}

#[cfg(test)]
#[path = "playback_topology/tests.rs"]
mod tests;
