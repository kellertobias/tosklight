use super::*;

#[derive(Deserialize)]
pub(super) struct UserInput {
    pub(super) name: String,
    #[serde(default = "default_true")]
    pub(super) enabled: bool,
}
pub(super) fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct DeskLockConfiguration {
    pub(super) locked: bool,
    pub(super) message: String,
    pub(super) wallpaper: Option<String>,
    pub(super) unlock_mode: String,
    pub(super) pin_salt: Option<String>,
    pub(super) pin_hash: Option<String>,
}

impl Default for DeskLockConfiguration {
    fn default() -> Self {
        Self {
            locked: false,
            message: "Desk locked".into(),
            wallpaper: None,
            unlock_mode: "button".into(),
            pin_salt: None,
            pin_hash: None,
        }
    }
}

#[derive(Serialize)]
pub(super) struct DeskLockResponse {
    pub(super) locked: bool,
    pub(super) message: String,
    pub(super) wallpaper: Option<String>,
    pub(super) unlock_mode: String,
}

#[derive(Deserialize)]
pub(super) struct DeskLockUpdate {
    pub(super) message: String,
    pub(super) wallpaper: Option<String>,
    pub(super) unlock_mode: String,
    pub(super) pin: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct DeskUnlockInput {
    pub(super) pin: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct UploadShow {
    pub(super) name: String,
    pub(super) data_base64: Option<String>,
    pub(super) overwrite: bool,
}
#[derive(Deserialize)]
pub(super) struct OpenShow {
    pub(super) transition: Option<Transition>,
    pub(super) transition_millis: Option<u64>,
}
#[derive(Deserialize)]
pub(super) struct SaveShowRevision {
    pub(super) name: String,
}
#[derive(Deserialize)]
pub(super) struct RenameShow {
    pub(super) name: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Transition {
    HoldCurrent,
    TimedFade,
    SafeBlackout,
}
#[derive(Deserialize)]
pub(super) struct MasterInput {
    pub(super) grand_master: Option<f32>,
    pub(super) blackout: Option<bool>,
}
#[derive(Deserialize)]
pub(super) struct PreloadStoreInput {
    pub(super) target: String,
    pub(super) target_id: String,
    pub(super) cue_number: Option<String>,
    pub(super) name: Option<String>,
    pub(super) mode: Option<light_programmer::PresetStoreMode>,
    pub(super) family: Option<light_programmer::PresetFamily>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum UpdateApiTargetFamily {
    Cue,
    Preset,
    Group,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct UpdateApiTarget {
    pub(super) family: UpdateApiTargetFamily,
    #[serde(default, alias = "cue_list_id")]
    pub(super) object_id: Option<String>,
    #[serde(default)]
    pub(super) playback_number: Option<u16>,
    #[serde(default)]
    pub(super) cue_id: Option<Uuid>,
    #[serde(default)]
    pub(super) cue_number: Option<String>,
    /// Touch/menu targets are snapshots of a live playback context and must still match it when
    /// the operator confirms. Explicit command-line Cue addressing deliberately leaves this off.
    #[serde(default)]
    pub(super) validate_active_context: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct UpdateApiRequest {
    pub(super) target: UpdateApiTarget,
    pub(super) mode: update::UpdateMode,
    #[serde(default)]
    pub(super) expected_revision: Option<u64>,
    #[serde(default)]
    pub(super) expected_programmer_revision: Option<String>,
    #[serde(default)]
    pub(super) expected_show_revision: Option<u64>,
}

#[cfg(test)]
#[derive(Serialize)]
pub(super) struct UpdatePreviewResponse {
    pub(super) revision: u64,
    pub(super) show_revision: u64,
    pub(super) programmer_revision: String,
    #[serde(flatten)]
    pub(super) preview: update::UpdatePreview,
}

pub(super) struct WsActionRequest {
    pub(super) request_id: String,
    pub(super) payload: serde_json::Value,
}
#[derive(Debug, Serialize)]
pub(super) struct WsResponse {
    pub(super) protocol_version: u16,
    pub(super) request_id: String,
    pub(super) ok: bool,
    pub(super) revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) action_timing: Option<ActionTimingProjection>,
}
