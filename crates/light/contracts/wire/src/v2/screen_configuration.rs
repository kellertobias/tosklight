//! Typed desk-store screen configuration snapshots and edit intents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenFixtureIncludedHeads {
    All,
    NoSubHeads,
    NoMasterHeads,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenFixtureOrder {
    FixtureId,
    Active,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenFixtureColumn {
    Id,
    Icon,
    Name,
    Patch,
    Dimmer,
    Color,
    Position,
    Beam,
    Focus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenStageRenderQuality {
    LinesOnly,
    LinesAndBeams,
    Full,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenTextMode {
    Plain,
    Markdown,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FixedScreenPane {
    FixtureSheet {
        included_heads: FixedScreenFixtureIncludedHeads,
        order: FixedScreenFixtureOrder,
        active_only: bool,
        cue_list_id: Option<Uuid>,
        #[schemars(length(min = 1, max = 9))]
        columns: Vec<FixedScreenFixtureColumn>,
        show_type: bool,
        show_group_shortcuts: bool,
    },
    #[serde(rename = "stage_2d")]
    Stage2d {
        follow_preload: bool,
        show_floor_grid: bool,
    },
    #[serde(rename = "stage_3d")]
    Stage3d {
        follow_preload: bool,
        show_floor_grid: bool,
        show_beam_guides: bool,
        render_quality: FixedScreenStageRenderQuality,
        #[schemars(range(min = 0.0, max = 1.0))]
        environment_brightness: f64,
    },
    Cues {
        #[serde(default)]
        cue_list_id: String,
    },
    Text {
        #[serde(default)]
        #[schemars(length(max = 128))]
        root: String,
        #[serde(default)]
        #[schemars(length(max = 4096))]
        path: String,
        mode: FixedScreenTextMode,
    },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScreenContent {
    #[default]
    Desktop,
    FixedPane {
        pane: FixedScreenPane,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenPlaybackSurfaceRow {
    pub first_playback_slot: u8,
    pub has_fader: bool,
    pub button_count: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenPlaybackSurfaceLayout {
    pub playbacks_per_row: u8,
    pub rows: Vec<ScreenPlaybackSurfaceRow>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ScreenPageMode {
    FollowMain,
    Independent,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfiguration {
    pub id: Uuid,
    pub name: String,
    #[ts(type = "unknown")]
    pub layout: Value,
    pub show_dock: bool,
    pub show_playbacks: bool,
    pub playback_count: u8,
    pub playback_rows: u8,
    pub first_playback_slot: u8,
    pub page_mode: ScreenPageMode,
    pub show_page_controls: bool,
    pub desired_open: bool,
    pub display_id: Option<String>,
    #[ts(type = "unknown")]
    pub bounds: Option<Value>,
    pub fullscreen: bool,
    pub playback_layout: Option<ScreenPlaybackSurfaceLayout>,
    #[serde(default)]
    pub content: ScreenContent,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationSnapshot {
    pub screens: Vec<ScreenConfiguration>,
    #[ts(type = "Record<string, number>")]
    pub active_pages: BTreeMap<Uuid, u8>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub action: ScreenConfigurationAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationCreateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub configuration: ScreenConfiguration,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationUpdateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub patch: ScreenConfigurationPatch,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationDeleteRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScreenConfigurationAction {
    Create {
        configuration: ScreenConfiguration,
    },
    Update {
        screen_id: Uuid,
        patch: ScreenConfigurationPatch,
    },
    Delete {
        screen_id: Uuid,
    },
    SetPage {
        screen_id: Uuid,
        #[schemars(range(min = 1, max = 127))]
        page: u8,
    },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationPatch {
    pub name: Option<String>,
    #[ts(type = "unknown")]
    pub layout: Option<Value>,
    pub show_dock: Option<bool>,
    pub show_playbacks: Option<bool>,
    pub playback_count: Option<u8>,
    pub playback_rows: Option<u8>,
    pub first_playback_slot: Option<u8>,
    pub page_mode: Option<ScreenPageMode>,
    pub show_page_controls: Option<bool>,
    pub desired_open: Option<bool>,
    pub display_id: Option<String>,
    #[serde(default)]
    pub clear_display_id: bool,
    #[ts(type = "unknown")]
    pub bounds: Option<Value>,
    #[serde(default)]
    pub clear_bounds: bool,
    pub fullscreen: Option<bool>,
    pub playback_layout: Option<ScreenPlaybackSurfaceLayout>,
    #[serde(default)]
    pub clear_playback_layout: bool,
    pub content: Option<ScreenContent>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationActionOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub screen: Option<ScreenConfiguration>,
    pub active_page: Option<u8>,
}
