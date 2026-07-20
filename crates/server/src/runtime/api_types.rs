use super::*;

#[derive(Deserialize)]
pub(super) struct ThumbnailRequest {
    #[serde(default = "default_media_library_type")]
    pub(super) library_type: u8,
    #[serde(default)]
    pub(super) library_level: u8,
    #[serde(default)]
    pub(super) library_1: u8,
    #[serde(default)]
    pub(super) library_2: u8,
    #[serde(default)]
    pub(super) library_3: u8,
    pub(super) elements: Vec<u8>,
    #[serde(default = "default_media_width")]
    pub(super) width: u16,
    #[serde(default = "default_media_height")]
    pub(super) height: u16,
}
#[derive(Deserialize)]
pub(super) struct ThumbnailQuery {
    #[serde(default = "default_media_library_type")]
    pub(super) library_type: u8,
    #[serde(default)]
    pub(super) library_level: u8,
    #[serde(default)]
    pub(super) library_1: u8,
    #[serde(default)]
    pub(super) library_2: u8,
    #[serde(default)]
    pub(super) library_3: u8,
    pub(super) element: u8,
}
#[derive(Deserialize)]
pub(super) struct PreviewRequest {
    pub(super) source: u16,
    #[serde(default = "default_media_width")]
    pub(super) width: u16,
    #[serde(default = "default_media_height")]
    pub(super) height: u16,
}
pub(super) fn default_media_library_type() -> u8 {
    1
}
pub(super) fn default_media_width() -> u16 {
    320
}
pub(super) fn default_media_height() -> u16 {
    180
}
#[derive(Deserialize)]
pub(super) struct CreateSession {
    pub(super) username: String,
    pub(super) desk_id: Option<Uuid>,
    pub(super) client_id: Option<Uuid>,
}
#[derive(Deserialize)]
pub(super) struct UserInput {
    pub(super) name: String,
    #[serde(default = "default_true")]
    pub(super) enabled: bool,
}
pub(super) fn default_true() -> bool {
    true
}
#[derive(Serialize)]
pub(super) struct SessionResponse {
    pub(super) session_id: SessionId,
    pub(super) client_id: Uuid,
    pub(super) token: String,
    pub(super) user: DeskUser,
    pub(super) desk: ControlDesk,
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
pub(super) struct ProgrammerSet {
    pub(super) fixture_id: light_core::FixtureId,
    pub(super) attribute: String,
    pub(super) value: f32,
}
#[derive(Deserialize)]
pub(super) struct ProgrammerSetMany {
    pub(super) assignments: Vec<ProgrammerSet>,
}
#[derive(Deserialize)]
pub(super) struct MasterInput {
    pub(super) grand_master: Option<f32>,
    pub(super) blackout: Option<bool>,
}
#[derive(Deserialize)]
pub(super) struct RawDmxOverrideInput {
    pub(super) universe: light_core::Universe,
    pub(super) address: light_core::DmxAddress,
    pub(super) value: Option<u8>,
}
#[derive(Deserialize)]
pub(super) struct PresetStoreInput {
    pub(super) mode: light_programmer::PresetStoreMode,
    pub(super) preset: serde_json::Value,
}
#[derive(Deserialize)]
pub(super) struct PreloadStoreInput {
    pub(super) target: String,
    pub(super) target_id: String,
    pub(super) cue_number: Option<f64>,
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
    pub(super) cue_number: Option<f64>,
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

#[derive(Serialize)]
pub(super) struct UpdatePreviewResponse {
    pub(super) revision: u64,
    pub(super) show_revision: u64,
    pub(super) programmer_revision: String,
    #[serde(flatten)]
    pub(super) preview: update::UpdatePreview,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct UpdateTargetsQuery {
    #[serde(default)]
    pub(super) filter: update::UpdateTargetFilter,
}

#[derive(Serialize)]
pub(super) struct UpdateMenuResponseEntry {
    pub(super) target: UpdateApiTarget,
    pub(super) revision: u64,
    pub(super) active_or_referenced: bool,
    pub(super) existing_preview: UpdatePreviewResponse,
    pub(super) add_new_preview: UpdatePreviewResponse,
}
#[derive(Debug, Deserialize)]
pub(super) struct WsCommand {
    pub(super) protocol_version: u16,
    pub(super) request_id: String,
    pub(super) session_id: SessionId,
    pub(super) expected_revision: Option<u64>,
    pub(super) command: String,
    #[serde(default)]
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
}
#[derive(Serialize)]
pub(super) struct BootstrapHighlightState {
    pub(super) session_id: SessionId,
    pub(super) desk_id: Uuid,
    pub(super) user_id: light_core::UserId,
    pub(super) state: HighlightState,
}

#[derive(Serialize)]
pub(super) struct Bootstrap {
    pub(super) api_version: &'static str,
    pub(super) attribute_registry: &'static [light_core::AttributeDescriptor],
    pub(super) users: Vec<DeskUser>,
    pub(super) desks: Vec<ControlDesk>,
    pub(super) clients: Vec<ClientSummary>,
    pub(super) active_show: Option<ShowEntry>,
    pub(super) active_programmers: Vec<light_programmer::ProgrammerState>,
    pub(super) highlight_states: Vec<BootstrapHighlightState>,
    pub(super) frame_rate_hz: u16,
    pub(super) output_health: OutputHealth,
    pub(super) active_timecode_source: Option<String>,
    pub(super) active_timecode: Option<String>,
    pub(super) active_show_error: Option<String>,
    pub(super) hardware_connected: bool,
}

#[derive(Clone, Serialize)]
pub(super) struct ClientSummary {
    pub(super) client_id: Uuid,
    pub(super) name: String,
    pub(super) connected: bool,
    pub(super) last_connected_at: Option<String>,
    pub(super) desk: ControlDesk,
    pub(super) can_remove: bool,
}
