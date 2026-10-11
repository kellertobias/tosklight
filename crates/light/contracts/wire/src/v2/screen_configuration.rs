//! Typed desk-store screen configuration snapshots and edit intents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    #[serde(alias = "dimmer")]
    Intensity,
    Color,
    Position,
    Beam,
    Shapers,
    Focus,
    Control,
    Media,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenFixtureCompactMode {
    #[default]
    Off,
    IconOnly,
    TextOnly,
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
        #[serde(default)]
        compact_mode: FixedScreenFixtureCompactMode,
        cue_list_id: Option<Uuid>,
        #[schemars(length(min = 1, max = 12))]
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
    /// Controls only. Older desk data stored the equivalent empty screen as `none`.
    #[serde(alias = "none")]
    ControlSurface,
    FixedPane {
        pane: FixedScreenPane,
    },
    /// A full-height fixed widget on one side; the control region fills the rest.
    /// Older desk data carried a `base` discriminator here, which is now ignored.
    FixedSidePane {
        pane: FixedScreenPane,
        side: FixedScreenSide,
        /// Share of the window width, so the pane keeps its proportion on every display.
        #[schemars(range(min = 10, max = 80))]
        width_percent: u8,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfiguration {
    pub id: Uuid,
    pub name: String,
    #[ts(type = "unknown")]
    pub layout: Value,
    pub show_dock: bool,
    pub show_playbacks: bool,
    pub surface_id: String,
    pub show_page_controls: bool,
    /// Programmer command line above this optional screen's encoders.
    #[serde(default)]
    pub show_programmer: bool,
    pub desired_open: bool,
    pub display_id: Option<String>,
    #[ts(type = "unknown")]
    pub bounds: Option<Value>,
    pub fullscreen: bool,
    #[serde(default)]
    pub content: ScreenContent,
    /// Whether this screen may change programming.
    ///
    /// A Not Editable screen is a guest: it presents the desk and operates playback, macros and
    /// timecodes, but cannot record, update or assign. Screen-local presentation configuration,
    /// not a second desk or a second user.
    #[serde(default)]
    pub not_editable: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationSnapshot {
    pub screens: Vec<ScreenConfiguration>,
    pub surfaces: super::playback_surfaces::PlaybackSurfacesSnapshot,
    pub programmer_control_surface: ProgrammerControlSurfaceConfiguration,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerControlSurfaceConfiguration {
    pub owner_screen_id: Option<Uuid>,
    pub visible_encoders: u8,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerControlSurfacePatch {
    pub owner_screen_id: Option<Uuid>,
    #[serde(default)]
    pub assign_to_main: bool,
    pub visible_encoders: Option<u8>,
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
    UpdateProgrammerControlSurface {
        patch: ProgrammerControlSurfacePatch,
    },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationPatch {
    pub name: Option<String>,
    #[ts(type = "unknown")]
    pub layout: Option<Value>,
    pub show_dock: Option<bool>,
    pub show_playbacks: Option<bool>,
    pub surface_id: Option<String>,
    pub show_page_controls: Option<bool>,
    pub show_programmer: Option<bool>,
    pub desired_open: Option<bool>,
    pub display_id: Option<String>,
    #[serde(default)]
    pub clear_display_id: bool,
    #[ts(type = "unknown")]
    pub bounds: Option<Value>,
    #[serde(default)]
    pub clear_bounds: bool,
    pub fullscreen: Option<bool>,
    pub content: Option<ScreenContent>,
    pub not_editable: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenConfigurationActionOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub screen: Option<ScreenConfiguration>,
    pub programmer_control_surface: Option<ProgrammerControlSurfaceConfiguration>,
}
