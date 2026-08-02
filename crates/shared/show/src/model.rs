use light_core::{Revision, SessionId, ShowId, UserId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenFixtureIncludedHeads {
    All,
    NoSubHeads,
    NoMasterHeads,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenFixtureOrder {
    FixtureId,
    Active,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenFixtureCompactMode {
    #[default]
    Off,
    IconOnly,
    TextOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenStageRenderQuality {
    LinesOnly,
    LinesAndBeams,
    Full,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixedScreenTextMode {
    Plain,
    Markdown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FixedScreenPane {
    FixtureSheet {
        included_heads: FixedScreenFixtureIncludedHeads,
        order: FixedScreenFixtureOrder,
        active_only: bool,
        #[serde(default)]
        compact_mode: FixedScreenFixtureCompactMode,
        cue_list_id: Option<Uuid>,
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
        environment_brightness: f64,
    },
    Cues {
        #[serde(default)]
        cue_list_id: String,
    },
    Text {
        #[serde(default)]
        root: String,
        #[serde(default)]
        path: String,
        mode: FixedScreenTextMode,
    },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScreenContent {
    #[default]
    Desktop,
    FixedPane {
        pane: FixedScreenPane,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeskUser {
    pub id: UserId,
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlaybackSurfaceRow {
    pub first_playback_slot: u8,
    pub has_fader: bool,
    pub button_count: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlaybackSurfaceLayout {
    pub playbacks_per_row: u8,
    pub rows: Vec<PlaybackSurfaceRow>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ControlDesk {
    pub id: Uuid,
    pub name: String,
    pub osc_alias: String,
    pub columns: u8,
    pub rows: u8,
    pub buttons: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_layout: Option<PlaybackSurfaceLayout>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClientDesk {
    pub client_id: Option<Uuid>,
    pub last_connected_at: Option<String>,
    pub desk: ControlDesk,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScreenConfiguration {
    pub id: Uuid,
    pub name: String,
    pub layout: serde_json::Value,
    pub show_dock: bool,
    pub show_playbacks: bool,
    pub playback_count: u8,
    pub playback_rows: u8,
    pub first_playback_slot: u8,
    pub page_mode: String,
    pub show_page_controls: bool,
    pub desired_open: bool,
    pub display_id: Option<String>,
    pub bounds: Option<serde_json::Value>,
    pub fullscreen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_layout: Option<PlaybackSurfaceLayout>,
    #[serde(default)]
    pub content: ScreenContent,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShowEntry {
    pub id: ShowId,
    pub name: String,
    pub path: String,
    pub revision: Revision,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_copy: Option<RevisionCopySource>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RevisionCopySource {
    pub show_id: ShowId,
    pub show_name: String,
    pub revision: Revision,
    pub revision_name: String,
    pub copied_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShowRevision {
    pub show_id: ShowId,
    pub revision: Revision,
    pub name: String,
    #[serde(skip_serializing)]
    pub path: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PersistedSession {
    pub id: SessionId,
    pub user_id: UserId,
    pub token: String,
    pub programmer_json: String,
    pub connected: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VersionedObject {
    pub kind: String,
    pub id: String,
    pub body: serde_json::Value,
    pub revision: Revision,
    pub updated_at: String,
}
