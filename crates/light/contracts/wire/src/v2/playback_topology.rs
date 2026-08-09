//! Strict portable Cuelist, Playback, and Page topology action DTOs.

use super::{
    dynamics::{
        DynamicActivationPolicyProjection, DynamicDefinitionProjection, DynamicRationalProjection,
    },
    events::ShowObjectKind,
};
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
pub struct PlaybackTopologyActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub action: PlaybackTopologyAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
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
    UndoCueList {
        cue_list_id: Uuid,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_revision: u64,
        #[schemars(length(min = 1, max = 128))]
        expected_object_id: String,
    },
    RedoCueList {
        cue_list_id: Uuid,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_revision: u64,
        #[schemars(length(min = 1, max = 128))]
        expected_object_id: String,
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
    AssignGroupMaster {
        #[schemars(length(min = 1, max = 128))]
        group_object_id: String,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_group_revision: u64,
        address: PlaybackTopologyGroupMasterAddress,
    },
    ConfigureVirtual {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1001, max = 39100))]
        playback_number: u16,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "string | null")]
        expected_page_object_id: PlaybackTopologyObjectIdentity,
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
    ClearVirtual {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1001, max = 39100))]
        playback_number: u16,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "string | null")]
        expected_page_object_id: PlaybackTopologyObjectIdentity,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaybackTopologyGroupMasterAddress {
    Physical {
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
    Virtual {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1001, max = 39100))]
        playback_number: u16,
        #[schemars(range(max = 9007199254740991_u64))]
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "string | null")]
        expected_page_object_id: PlaybackTopologyObjectIdentity,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackTopologyPlaybackDefinition {
    #[schemars(range(max = 39100))]
    pub number: u16,
    #[schemars(length(min = 1, max = 80))]
    pub name: String,
    pub target: PlaybackTopologyTarget,
    pub buttons: [PlaybackTopologyButtonAction; 3],
    #[schemars(range(max = 3))]
    pub button_count: u8,
    pub fader: PlaybackTopologyFaderMode,
    pub has_fader: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub footprint: Option<PlaybackTopologyFootprint>,
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

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackTopologyTarget {
    CueList {
        cue_list_id: Uuid,
    },
    Dynamic {
        assignment: Box<PlaybackTopologyDynamicAssignment>,
    },
    Group {
        group_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(range(min = 0.0, max = 1.0))]
        #[ts(optional = nullable)]
        initial_master: Option<f32>,
    },
    SpeedGroup {
        group: String,
    },
    ProgrammerFade {},
    CueFade {},
    GrandMaster {},
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackTopologyDynamicAssignment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub dynamic_id: Option<Uuid>,
    pub last_known_pool_number: u16,
    pub embedded_fallback: DynamicDefinitionProjection,
    #[ts(type = "number")]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub target_scope: Option<PlaybackTopologyDynamicTargetScope>,
    pub fader_mode: PlaybackTopologyDynamicFaderMode,
    pub priority: i16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub activation_override: Option<DynamicActivationPolicyProjection>,
    pub resume_policy: PlaybackTopologyDynamicResumePolicy,
    pub local_speed_multiplier: DynamicRationalProjection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub learned_duration_millis: Option<u64>,
    pub crossfade_non_intensity: bool,
    pub auto_off_at_zero: bool,
    pub auto_off_flash_release: bool,
    pub auto_off_full_control: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackTopologyDynamicTargetScope {
    LiveGroup { group_id: String },
    FrozenTargets { targets: Vec<Uuid> },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackTopologyDynamicFaderMode {
    None,
    Master,
    Size,
    SizeAndMaster,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackTopologyDynamicResumePolicy {
    FollowDynamic,
    ResumeFrozenPhase,
    RejoinSynchronizedPosition,
    ResumeOnNextBoundary,
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
    DynamicRestart,
    DynamicDoubleSpeed,
    DynamicHalfSpeed,
    DynamicLearnSpeed,
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackTopologyFootprint {
    #[default]
    Normal,
    Taller {
        upper_button: PlaybackTopologyButtonAction,
    },
    Wider {
        right_buttons: [PlaybackTopologyButtonAction; 3],
        right_fader: PlaybackTopologyFaderMode,
    },
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
    Virtual {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1001, max = 39100))]
        playback_number: u16,
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
