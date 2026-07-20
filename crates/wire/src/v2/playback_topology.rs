//! Strict portable Cuelist, Playback, and Page topology action DTOs.

use super::events::ShowObjectKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

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
        #[ts(type = "number")]
        expected_revision: u64,
        /// Extensible portable body; adapters strictly decode its known Cuelist fields.
        #[ts(type = "unknown")]
        body: Value,
    },
    ConfigureSlot {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1, max = 127))]
        slot: u8,
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "number")]
        expected_playback_revision: u64,
        playback: PlaybackTopologyPlaybackDefinition,
    },
    ClearMappedPlayback {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[schemars(range(min = 1, max = 127))]
        slot: u8,
        #[ts(type = "number")]
        expected_page_revision: u64,
        #[ts(type = "number")]
        expected_playback_revision: u64,
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
        page: u8,
        slot: u8,
        playback_number: Option<u16>,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaybackTopologyObjectProjection {
    Present {
        kind: ShowObjectKind,
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown")]
        body: Value,
    },
    Deleted {
        kind: ShowObjectKind,
        object_id: String,
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
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_semantic_actions_have_strict_readable_discriminants() {
        let actions = [
            json!({
                "request_id":"save",
                "action":{"type":"save_cue_list","cue_list_id":Uuid::nil(),
                    "expected_revision":3,"body":{"id":Uuid::nil(),"future":true}}
            }),
            json!({
                "request_id":"configure",
                "action":{"type":"configure_slot","page":2,"slot":4,
                    "expected_page_revision":5,"expected_playback_revision":6,
                    "playback":playback_json()}
            }),
            json!({
                "request_id":"clear",
                "action":{"type":"clear_mapped_playback","page":2,"slot":4,
                    "expected_page_revision":5,"expected_playback_revision":6}
            }),
        ];
        for action in actions {
            serde_json::from_value::<PlaybackTopologyActionRequest>(action).unwrap();
        }
    }

    #[test]
    fn identity_and_nested_unknown_fields_are_rejected() {
        for forged in [
            "show_id",
            "user_id",
            "desk_id",
            "session_id",
            "correlation_id",
        ] {
            let mut request = configure_request();
            request[forged] = json!("forged");
            assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(request).is_err());
        }
        let mut action = configure_request();
        action["action"]["object_kind"] = json!("playback");
        assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(action).is_err());
        let mut playback = configure_request();
        playback["action"]["playback"]["future"] = json!(true);
        assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(playback).is_err());
        let mut target = configure_request();
        target["action"]["playback"]["target"]["future"] = json!(true);
        assert!(serde_json::from_value::<PlaybackTopologyActionRequest>(target).is_err());
    }

    #[test]
    fn changed_and_no_change_outcomes_enforce_event_and_projection_shapes() {
        let present = json!({"state":"present","kind":"playback","object_id":"7",
            "object_revision":4,"body":{"number":7}});
        let changed = json!({"request_id":"r","correlation_id":Uuid::nil(),"show_revision":9,
            "resolution":{"kind":"page_slot","page":1,"slot":2,"playback_number":7},
            "status":"changed","objects":[present.clone()],"event_sequence":10,"replayed":false});
        serde_json::from_value::<PlaybackTopologyActionOutcome>(changed).unwrap();
        let no_change = json!({"request_id":"r","correlation_id":Uuid::nil(),"show_revision":9,
            "resolution":{"kind":"page_slot","page":1,"slot":2,"playback_number":null},
            "status":"no_change","objects":[present],"replayed":true});
        serde_json::from_value::<PlaybackTopologyActionOutcome>(no_change).unwrap();
        let invalid = json!({"request_id":"r","correlation_id":Uuid::nil(),"show_revision":9,
            "resolution":{"kind":"cue_list","cue_list_id":Uuid::nil()},
            "status":"no_change","objects":[],"event_sequence":10,"replayed":false});
        assert!(serde_json::from_value::<PlaybackTopologyActionOutcome>(invalid).is_err());
        let unknown = json!({"request_id":"r","correlation_id":Uuid::nil(),"show_revision":9,
            "resolution":{"kind":"cue_list","cue_list_id":Uuid::nil()},
            "status":"no_change","objects":[],"future":true,"replayed":false});
        assert!(serde_json::from_value::<PlaybackTopologyActionOutcome>(unknown).is_err());
    }

    #[test]
    fn deleted_projection_cannot_smuggle_a_body() {
        let invalid = json!({"state":"deleted","kind":"playback","object_id":"2",
            "object_revision":4,"body":{"number":2}});
        assert!(serde_json::from_value::<PlaybackTopologyObjectProjection>(invalid).is_err());
    }

    fn configure_request() -> Value {
        json!({"request_id":"configure","action":{"type":"configure_slot","page":2,"slot":4,
            "expected_page_revision":5,"expected_playback_revision":6,
            "playback":playback_json()}})
    }

    fn playback_json() -> Value {
        json!({"number":7,"name":"Main","target":{"type":"grand_master"},
            "buttons":["blackout","pause_dynamics","flash"],"button_count":3,
            "fader":"master","has_fader":true,"go_activates":true,"auto_off":true,
            "xfade_millis":0,"color":"#20c997","flash_release":"release_all",
            "protect_from_swap":false})
    }
}
