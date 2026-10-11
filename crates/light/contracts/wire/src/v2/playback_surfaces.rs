//! Shared desk surface configuration and explicit edit intents.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackControlPresentation {
    Fader,
    Buttons,
    PushEncoder,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSurfaceRow {
    #[schemars(range(min = 1, max = 1000))]
    pub first_position: u16,
    #[schemars(range(min = 1, max = 1000))]
    pub count: u16,
    pub presentation: PlaybackControlPresentation,
    #[schemars(range(max = 3))]
    pub button_count: u8,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSurfaceLayout {
    pub rows: Vec<PlaybackSurfaceRow>,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackPageSource {
    Pinned {
        #[schemars(range(min = 1, max = 99))]
        page: u8,
    },
    Own {
        #[schemars(range(min = 1, max = 99))]
        page: u8,
    },
    Follow {
        surface_id: String,
    },
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSurfaceConfiguration {
    pub name: String,
    pub layout: PlaybackSurfaceLayout,
    pub page_source: PlaybackPageSource,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSurfacesSnapshot {
    pub playback_surfaces: BTreeMap<String, PlaybackSurfaceConfiguration>,
    pub selected_surface_id: String,
    pub resolved_pages: BTreeMap<String, u8>,
    pub command_page: u8,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSurfacesUpdateRequest {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<PlaybackSurfaceLayout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_source: Option<PlaybackPageSource>,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSurfaceSelectRequest {
    pub surface_id: String,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSurfacePageRequest {
    #[schemars(range(min = 1, max = 99))]
    pub page: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSurfacePageLiveActionRequest {
    pub surface_id: String,
    #[schemars(range(min = 1, max = 99))]
    pub page: u8,
}
