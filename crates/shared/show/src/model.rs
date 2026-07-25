use light_core::{Revision, SessionId, ShowId, UserId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
