#![forbid(unsafe_code)]

mod bootstrap;
#[path = "command_http.rs"]
mod command_http;
#[path = "cue_transfer.rs"]
mod cue_transfer;
#[path = "default_show.rs"]
mod default_show;
#[path = "file_manager.rs"]
mod file_manager;
#[path = "file_manager_support.rs"]
mod file_manager_support;
#[path = "help.rs"]
mod help;
mod http_router;
#[path = "matter.rs"]
mod matter;
mod output_scheduler;
mod startup_options;
mod startup_state;

use crate::highlight::{
    HighlightAction, HighlightError, HighlightFixture, HighlightMode, HighlightRegistry,
    HighlightSelectionWrite, HighlightState, HighlightTransition, is_duplicate_osc_action,
};
use crate::update;
use axum::extract::ws::{Message, WebSocket};
use axum::{
    Json, Router,
    extract::Request,
    extract::{DefaultBodyLimit, Path, Query, State, WebSocketUpgrade},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use bytes::Bytes;
use light_application::{EventBus, publish_automatic_playback_events};
use light_control::speed::{
    SoundObservation, SoundToLightConfig, SpeedGroupController, SpeedSnapshot,
};
use light_control::{
    ControlAction, ControlEvent, ControlInput, FrameRate, MidiControlInput, OscArgument,
    RtpMidiInput, SmpteTimecode, TimecodeRouter, TimecodeSourceConfig, UdpControlInput,
    UdpInputProtocol, encode_osc_message,
};
use light_core::{ATTRIBUTE_REGISTRY, ApplicationClock, ManualClock, SessionId};
use light_engine::{Engine, EngineSnapshot, RenderOptions};
use light_media::{CitpClient, LibraryId, MediaCache, PreviewKey, ThumbnailKey};
use light_output::{NetworkOutput, OutputHealth};
use light_programmer::ProgrammerRegistry;
use light_show::{
    AtomicObjectDelete, AtomicObjectWrite, ControlDesk, DeskStore, DeskUser, PersistedSession,
    RevisionCopySource, ScreenConfiguration, ShowEntry, ShowRevision, ShowStore, initialise_show,
    validate_show_file,
};
use parking_lot::{Mutex, RwLock};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path as FsPath, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU16, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use uuid::Uuid;

use cue_transfer::{CueTransferMode, destination_cue};

#[derive(Clone)]
struct AppState {
    desk: Arc<Mutex<DeskStore>>,
    fixture_library: Arc<Mutex<light_fixture::FixtureLibrary>>,
    data_dir: PathBuf,
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    session_clients: Arc<RwLock<HashMap<SessionId, Uuid>>>,
    ws_connections: Arc<Mutex<HashMap<SessionId, u32>>>,
    programmers: ProgrammerRegistry,
    engine: Arc<Engine>,
    highlight: Arc<HighlightRegistry>,
    patch_preview_highlights: Arc<Mutex<HashMap<SessionId, HashSet<light_core::FixtureId>>>>,
    output_health: Arc<std::sync::Mutex<OutputHealth>>,
    output_rate: Arc<AtomicU16>,
    configuration: Arc<RwLock<DeskConfiguration>>,
    matter_bridge: Arc<matter::MatterBridgeAdapter>,
    matter_transport: Option<Arc<matter::MatterTransport>>,
    output_control: Arc<Mutex<OutputControl>>,
    activation_lock: Arc<tokio::sync::Mutex<()>>,
    playback_action_lock: Arc<Mutex<()>>,
    timecode_router: Arc<Mutex<TimecodeRouter>>,
    active_show: Arc<RwLock<Option<ShowEntry>>>,
    active_show_error: Arc<RwLock<Option<String>>>,
    events: broadcast::Sender<Event>,
    application_events: EventBus,
    audit_events: Arc<Mutex<VecDeque<Event>>>,
    command_history: Arc<Mutex<HashMap<Uuid, VecDeque<CommandHistoryEntry>>>>,
    command_http: command_http::CommandHttpState,
    event_revision: Arc<AtomicU64>,
    desk_token: Option<Arc<str>>,
    shutdown: CancellationToken,
    media_cache: Arc<Mutex<MediaCache>>,
    media_status: Arc<RwLock<HashMap<light_core::FixtureId, MediaServerStatus>>>,
    input_locks: Arc<Mutex<HashMap<String, (light_core::UserId, Instant)>>>,
    file_input_contexts: Arc<Mutex<HashMap<Uuid, file_manager::FileInputContext>>>,
    osc_subscribers: Arc<Mutex<HashMap<String, OscSubscriber>>>,
    osc_feedback: Option<Arc<std::net::UdpSocket>>,
    #[cfg(test)]
    osc_feedback_capture: Arc<Mutex<Vec<CapturedOscMessage>>>,
    mvr_imports: Arc<Mutex<HashMap<Uuid, StagedMvrImport>>>,
    network_output: Option<Arc<NetworkOutput>>,
    output_sequences: Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
    manual_clock: Option<Arc<ManualClock>>,
    speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
    sound_capture_owners: Arc<Mutex<[Option<SoundCaptureOwner>; 5]>>,
}

#[cfg(test)]
type CapturedOscMessage = (SocketAddr, String, Vec<OscArgument>);

#[derive(Clone, Copy)]
struct SoundCaptureOwner {
    desk_id: Uuid,
    last_seen_millis: u64,
}

#[derive(Serialize)]
struct SpeedGroupResponse {
    group: String,
    configuration: SoundToLightConfig,
    snapshot: SpeedSnapshot,
}

#[derive(Deserialize)]
struct SpeedGroupActionInput {
    action: String,
    captured_at_millis: Option<u64>,
}

#[derive(Clone)]
struct StagedMvrImport {
    document: light_mvr::MvrDocument,
    created: Instant,
}

#[derive(Deserialize, Default)]
struct MvrPreviewQuery {
    show_id: Option<Uuid>,
}

#[derive(Clone, Serialize)]
struct MvrImportPreview {
    token: Uuid,
    fixtures: Vec<MvrPreviewFixture>,
    scenery: usize,
    missing_profiles: Vec<String>,
    warnings: Vec<String>,
    address_conflicts: Vec<String>,
}
#[derive(Clone, Serialize)]
struct MvrPreviewFixture {
    uuid: Uuid,
    name: String,
    gdtf_spec: String,
    gdtf_mode: String,
    universe: Option<u16>,
    address: Option<u16>,
    matched: bool,
}

#[derive(Deserialize)]
struct ApplyMvrImport {
    new_show: Option<NewMvrShow>,
    existing_show_id: Option<Uuid>,
    #[serde(default)]
    resolutions: HashMap<Uuid, MvrResolution>,
}
#[derive(Deserialize)]
struct NewMvrShow {
    name: String,
    #[serde(default = "default_true")]
    open_after_import: bool,
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum MvrResolution {
    Import,
    Skip,
    ImportUnpatched,
    Replace,
    Address { universe: u16, address: u16 },
}

#[derive(Serialize)]
struct ApplyMvrResult {
    show: ShowEntry,
    imported_fixtures: usize,
    unresolved_fixtures: usize,
    imported_scenery: usize,
    opened: bool,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct MvrExportPreview {
    fixtures: usize,
    scenery: usize,
    embedded_profiles: usize,
    missing_profiles: Vec<String>,
    omitted: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Clone)]
struct OscSubscriber {
    desk_alias: String,
    target: SocketAddr,
    command_source: SocketAddr,
    session_id: SessionId,
    last_seen: Instant,
    shifted: bool,
    shift_held: bool,
    update_record_started: Option<Instant>,
    update_first_release: Option<Instant>,
    last_highlight_action: Option<(String, Instant)>,
}

#[derive(RustEmbed)]
#[folder = "$LIGHT_CONTROL_FRONTEND_DIR"]
struct ControlUiAssets;
#[derive(Default)]
struct OutputControl {
    options: RenderOptions,
    grand_master_flash: bool,
    hold: bool,
    last_frames: HashMap<light_core::Universe, light_output::DmxFrame>,
    raw_overrides: HashMap<(light_core::Universe, light_core::DmxAddress), u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
struct PersistedOutputRuntime {
    grand_master: f32,
    blackout: bool,
    dynamics_paused_at: Option<chrono::DateTime<chrono::Utc>>,
    group_masters: HashMap<String, f32>,
}

impl Default for PersistedOutputRuntime {
    fn default() -> Self {
        Self {
            grand_master: 1.0,
            blackout: false,
            dynamics_paused_at: None,
            group_masters: HashMap::new(),
        }
    }
}

impl PersistedOutputRuntime {
    fn is_valid(&self) -> bool {
        self.grand_master.is_finite()
            && (0.0..=1.0).contains(&self.grand_master)
            && self
                .group_masters
                .values()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    }
}
impl OutputControl {
    fn render_options(&self) -> RenderOptions {
        RenderOptions {
            grand_master: if self.grand_master_flash {
                1.0
            } else {
                self.options.grand_master
            },
            ..self.options
        }
    }
}
#[derive(Clone, Serialize)]
struct Session {
    id: SessionId,
    user: DeskUser,
    token: String,
    connected: bool,
    desk: ControlDesk,
}
#[derive(Clone, Serialize)]
struct Event {
    revision: u64,
    kind: String,
    payload: serde_json::Value,
}
#[derive(Clone, Serialize)]
struct CommandHistoryEntry {
    id: String,
    desk_id: Uuid,
    session_id: SessionId,
    command: String,
    status: String,
    feedback: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    at: String,
}
#[derive(Deserialize)]
struct AuditQuery {
    #[serde(default)]
    after: u64,
}
#[derive(Clone, Default, Serialize)]
struct MediaServerStatus {
    online: bool,
    last_success: Option<String>,
    last_error: Option<String>,
}
#[derive(Deserialize)]
struct ThumbnailRequest {
    #[serde(default = "default_media_library_type")]
    library_type: u8,
    #[serde(default)]
    library_level: u8,
    #[serde(default)]
    library_1: u8,
    #[serde(default)]
    library_2: u8,
    #[serde(default)]
    library_3: u8,
    elements: Vec<u8>,
    #[serde(default = "default_media_width")]
    width: u16,
    #[serde(default = "default_media_height")]
    height: u16,
}
#[derive(Deserialize)]
struct ThumbnailQuery {
    #[serde(default = "default_media_library_type")]
    library_type: u8,
    #[serde(default)]
    library_level: u8,
    #[serde(default)]
    library_1: u8,
    #[serde(default)]
    library_2: u8,
    #[serde(default)]
    library_3: u8,
    element: u8,
}
#[derive(Deserialize)]
struct PreviewRequest {
    source: u16,
    #[serde(default = "default_media_width")]
    width: u16,
    #[serde(default = "default_media_height")]
    height: u16,
}
fn default_media_library_type() -> u8 {
    1
}
fn default_media_width() -> u16 {
    320
}
fn default_media_height() -> u16 {
    180
}
#[derive(Deserialize)]
struct CreateSession {
    username: String,
    desk_id: Option<Uuid>,
    client_id: Option<Uuid>,
}
#[derive(Deserialize)]
struct UserInput {
    name: String,
    #[serde(default = "default_true")]
    enabled: bool,
}
fn default_true() -> bool {
    true
}
#[derive(Serialize)]
struct SessionResponse {
    session_id: SessionId,
    client_id: Uuid,
    token: String,
    user: DeskUser,
    desk: ControlDesk,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
struct DeskLockConfiguration {
    locked: bool,
    message: String,
    wallpaper: Option<String>,
    unlock_mode: String,
    pin_salt: Option<String>,
    pin_hash: Option<String>,
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
struct DeskLockResponse {
    locked: bool,
    message: String,
    wallpaper: Option<String>,
    unlock_mode: String,
}

#[derive(Deserialize)]
struct DeskLockUpdate {
    message: String,
    wallpaper: Option<String>,
    unlock_mode: String,
    pin: Option<String>,
}

#[derive(Deserialize)]
struct DeskUnlockInput {
    pin: Option<String>,
}
#[derive(Deserialize)]
struct UploadShow {
    name: String,
    data_base64: Option<String>,
    overwrite: bool,
}
#[derive(Deserialize)]
struct OpenShow {
    transition: Option<Transition>,
    transition_millis: Option<u64>,
}
#[derive(Deserialize)]
struct SaveShowRevision {
    name: String,
}
#[derive(Deserialize)]
struct RenameShow {
    name: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Transition {
    HoldCurrent,
    TimedFade,
    SafeBlackout,
}
#[derive(Deserialize)]
struct ProgrammerSet {
    fixture_id: light_core::FixtureId,
    attribute: String,
    value: f32,
}
#[derive(Deserialize)]
struct ProgrammerSetMany {
    assignments: Vec<ProgrammerSet>,
}
#[derive(Deserialize)]
struct MasterInput {
    grand_master: Option<f32>,
    blackout: Option<bool>,
}
#[derive(Deserialize)]
struct RawDmxOverrideInput {
    universe: light_core::Universe,
    address: light_core::DmxAddress,
    value: Option<u8>,
}
#[derive(Deserialize)]
struct PresetStoreInput {
    mode: light_programmer::PresetStoreMode,
    preset: serde_json::Value,
}
#[derive(Deserialize)]
struct PreloadStoreInput {
    target: String,
    target_id: String,
    cue_number: Option<f64>,
    name: Option<String>,
    mode: Option<light_programmer::PresetStoreMode>,
    family: Option<light_programmer::PresetFamily>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum UpdateApiTargetFamily {
    Cue,
    Preset,
    Group,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UpdateApiTarget {
    family: UpdateApiTargetFamily,
    #[serde(default, alias = "cue_list_id")]
    object_id: Option<String>,
    #[serde(default)]
    playback_number: Option<u16>,
    #[serde(default)]
    cue_id: Option<Uuid>,
    #[serde(default)]
    cue_number: Option<f64>,
    /// Touch/menu targets are snapshots of a live playback context and must still match it when
    /// the operator confirms. Explicit command-line Cue addressing deliberately leaves this off.
    #[serde(default)]
    validate_active_context: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct UpdateApiRequest {
    target: UpdateApiTarget,
    mode: update::UpdateMode,
    #[serde(default)]
    expected_revision: Option<u64>,
    #[serde(default)]
    expected_programmer_revision: Option<String>,
}

#[derive(Serialize)]
struct UpdatePreviewResponse {
    revision: u64,
    programmer_revision: String,
    #[serde(flatten)]
    preview: update::UpdatePreview,
}

#[derive(Debug, Default, Deserialize)]
struct UpdateTargetsQuery {
    #[serde(default)]
    filter: update::UpdateTargetFilter,
}

#[derive(Serialize)]
struct UpdateMenuResponseEntry {
    target: UpdateApiTarget,
    revision: u64,
    active_or_referenced: bool,
    existing_preview: UpdatePreviewResponse,
    add_new_preview: UpdatePreviewResponse,
}
#[derive(Debug, Deserialize)]
struct WsCommand {
    protocol_version: u16,
    request_id: String,
    session_id: SessionId,
    expected_revision: Option<u64>,
    command: String,
    #[serde(default)]
    payload: serde_json::Value,
}
#[derive(Debug, Serialize)]
struct WsResponse {
    protocol_version: u16,
    request_id: String,
    ok: bool,
    revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}
#[derive(Serialize)]
struct BootstrapHighlightState {
    session_id: SessionId,
    desk_id: Uuid,
    user_id: light_core::UserId,
    state: HighlightState,
}

#[derive(Serialize)]
struct Bootstrap {
    api_version: &'static str,
    attribute_registry: &'static [light_core::AttributeDescriptor],
    users: Vec<DeskUser>,
    desks: Vec<ControlDesk>,
    clients: Vec<ClientSummary>,
    active_show: Option<ShowEntry>,
    active_programmers: Vec<light_programmer::ProgrammerState>,
    highlight_states: Vec<BootstrapHighlightState>,
    frame_rate_hz: u16,
    output_health: OutputHealth,
    active_timecode_source: Option<String>,
    active_timecode: Option<String>,
    active_show_error: Option<String>,
    hardware_connected: bool,
}

#[derive(Clone, Serialize)]
struct ClientSummary {
    client_id: Uuid,
    name: String,
    connected: bool,
    last_connected_at: Option<String>,
    desk: ControlDesk,
    can_remove: bool,
}

fn default_speed_groups() -> [f64; 5] {
    [120.0, 90.0, 60.0, 30.0, 15.0]
}

fn default_sound_to_light() -> [SoundToLightConfig; 5] {
    std::array::from_fn(|_| SoundToLightConfig::default())
}
fn deserialize_speed_groups<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<[f64; 5], D::Error> {
    let values = Vec::<f64>::deserialize(deserializer)?;
    if !(values.len() == 4 || values.len() == 5) {
        return Err(serde::de::Error::custom(
            "speed_groups_bpm requires four or five values",
        ));
    }
    let mut result = default_speed_groups();
    result[..values.len()].copy_from_slice(&values);
    Ok(result)
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
struct DeskConfiguration {
    frame_rate_hz: u16,
    output_bind_ip: IpAddr,
    osc_bind: Option<SocketAddr>,
    art_timecode_bind: Option<SocketAddr>,
    midi_inputs: Vec<String>,
    rtp_midi_bind: Option<SocketAddr>,
    timecode_sources: Vec<TimecodeSourceConfig>,
    osc_timecode: Option<OscTimecodeConfig>,
    backup_retention: usize,
    #[serde(
        default = "default_speed_groups",
        deserialize_with = "deserialize_speed_groups"
    )]
    speed_groups_bpm: [f64; 5],
    #[serde(default = "default_sound_to_light")]
    speed_group_sound_to_light: [SoundToLightConfig; 5],
    programmer_fade_millis: u64,
    sequence_master_fade_millis: u64,
    preload_programmer_changes: bool,
    preload_physical_playback_actions: bool,
    preload_virtual_playback_actions: bool,
    /// Allow Show Patch's scoped Stage preview selection to identify fixtures on DMX.
    patch_preview_highlight_dmx: bool,
    /// Desk-persistent opt-in for the global page/playback Matter bridge.
    matter_enabled: bool,
    /// Workflow defaults belong to a concrete desk rather than to portable show data.
    update_settings_by_desk: HashMap<Uuid, update::UpdateSettings>,
    file_manager_system_picker_fallback: bool,
    file_manager_roots: Vec<file_manager::ConfiguredRoot>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct OscTimecodeConfig {
    address: String,
    rate: FrameRate,
}
impl Default for DeskConfiguration {
    fn default() -> Self {
        Self {
            frame_rate_hz: 44,
            output_bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            osc_bind: Some(SocketAddr::from(([127, 0, 0, 1], 9000))),
            art_timecode_bind: None,
            midi_inputs: Vec::new(),
            rtp_midi_bind: None,
            timecode_sources: vec![
                TimecodeSourceConfig {
                    source_prefix: "artnet:".into(),
                    priority: 30,
                    fallback: false,
                    loss_timeout_millis: 500,
                },
                TimecodeSourceConfig {
                    source_prefix: "midi:".into(),
                    priority: 20,
                    fallback: true,
                    loss_timeout_millis: 500,
                },
                TimecodeSourceConfig {
                    source_prefix: "rtp:".into(),
                    priority: 20,
                    fallback: true,
                    loss_timeout_millis: 500,
                },
                TimecodeSourceConfig {
                    source_prefix: "osc:".into(),
                    priority: 10,
                    fallback: true,
                    loss_timeout_millis: 500,
                },
            ],
            osc_timecode: None,
            backup_retention: 20,
            speed_groups_bpm: default_speed_groups(),
            speed_group_sound_to_light: default_sound_to_light(),
            programmer_fade_millis: 3_000,
            sequence_master_fade_millis: 3_000,
            preload_programmer_changes: true,
            preload_physical_playback_actions: true,
            preload_virtual_playback_actions: false,
            patch_preview_highlight_dmx: false,
            matter_enabled: false,
            update_settings_by_desk: HashMap::new(),
            file_manager_system_picker_fallback: false,
            file_manager_roots: Vec::new(),
        }
    }
}
impl DeskConfiguration {
    fn validate(&self) -> Result<(), ApiError> {
        if !(40..=44).contains(&self.frame_rate_hz) {
            return Err(ApiError::bad_request("frame_rate_hz must be 40-44"));
        }
        if self.backup_retention == 0 || self.backup_retention > 1_000 {
            return Err(ApiError::bad_request("backup_retention must be 1-1000"));
        }
        if self
            .speed_groups_bpm
            .iter()
            .any(|bpm| !bpm.is_finite() || !(0.1..=999.0).contains(bpm))
        {
            return Err(ApiError::bad_request(
                "speed_groups_bpm values must be finite and within 0.1-999",
            ));
        }
        for sound in &self.speed_group_sound_to_light {
            sound
                .validate()
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        if self.programmer_fade_millis > 60_000 || self.sequence_master_fade_millis > 60_000 {
            return Err(ApiError::bad_request(
                "fade times must be 0-60000 milliseconds",
            ));
        }
        let mut root_ids = std::collections::HashSet::new();
        for root in &self.file_manager_roots {
            if root.id.trim().is_empty() || root.label.trim().is_empty() || !root.path.is_absolute()
            {
                return Err(ApiError::bad_request(
                    "File Manager roots require a stable ID, label, and absolute server path",
                ));
            }
            if !root_ids.insert(&root.id) {
                return Err(ApiError::bad_request(
                    "File Manager root IDs must be unique",
                ));
            }
        }
        Ok(())
    }
}

fn open_fixture_library_for_startup(
    data_dir: &FsPath,
    fixture_package_dir: Option<&FsPath>,
) -> Result<light_fixture::FixtureLibrary, light_fixture::FixtureError> {
    tracing::info!(path=%data_dir.join("fixtures.sqlite").display(), "opening fixture library");
    let library = light_fixture::FixtureLibrary::open(data_dir.join("fixtures.sqlite"))?;
    if let Some(path) = fixture_package_dir {
        let report = library.load_fixture_package_directory(path)?;
        tracing::info!(
            path = %path.display(),
            installed = report.installed,
            updated = report.updated,
            unchanged = report.unchanged,
            preserved_operator_revisions = report.preserved_operator_revisions,
            "loaded transferable fixture packages"
        );
    }
    for warning in library.migration_warnings()? {
        tracing::warn!(%warning, "fixture library migration requires operator attention");
    }
    tracing::info!("fixture library ready");
    Ok(library)
}

fn sibling_fixture_package_dir(executable: &FsPath) -> Option<PathBuf> {
    let directory = executable.parent()?.join("fixture-library");
    directory.is_dir().then_some(directory)
}

fn rebase_desk_show_paths(desk: &DeskStore, data_dir: &FsPath) -> anyhow::Result<()> {
    for entry in desk.library()? {
        let destination = data_dir.join("shows").join(format!("{}.show", entry.name));
        let source = FsPath::new(&entry.path);
        if source == destination {
            continue;
        }
        if destination.exists() {
            if validate_show_file(&destination).is_ok() {
                desk.relocate_show(entry.id, &destination.display().to_string())?;
            }
        } else if source.exists() {
            ShowStore::open(source)?.backup_to(&destination)?;
            desk.relocate_show(entry.id, &destination.display().to_string())?;
        }
    }
    for entry in desk.library()? {
        for revision in desk.show_revisions(entry.id)? {
            let Some(file_name) = FsPath::new(&revision.path).file_name() else {
                continue;
            };
            let destination = data_dir
                .join("revisions")
                .join(entry.id.0.to_string())
                .join(file_name);
            let source = FsPath::new(&revision.path);
            if source == destination {
                continue;
            }
            if destination.exists() {
                if validate_show_file(&destination).is_ok() {
                    desk.relocate_show_revision(
                        entry.id,
                        revision.revision,
                        &destination.display().to_string(),
                    )?;
                }
            } else if source.exists() {
                std::fs::create_dir_all(destination.parent().expect("revision directory"))?;
                ShowStore::open(source)?.backup_to(&destination)?;
                desk.relocate_show_revision(
                    entry.id,
                    revision.revision,
                    &destination.display().to_string(),
                )?;
            }
        }
    }
    Ok(())
}

fn preserve_invalid_default_show(data_dir: &FsPath, path: &FsPath) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let backup_directory = data_dir.join("backups");
    std::fs::create_dir_all(&backup_directory)?;
    let backup = backup_directory.join(format!(
        "Default Stage Show-unloadable-{}.show",
        chrono::Utc::now().timestamp_millis()
    ));
    std::fs::rename(path, &backup)?;
    tracing::warn!(original=%path.display(), preserved=%backup.display(), "preserved an unloadable default show before restoring the built-in default");
    Ok(())
}

fn ensure_default_show_available(desk: &DeskStore, data_dir: &FsPath) -> anyhow::Result<ShowEntry> {
    let path = data_dir
        .join("shows")
        .join(format!("{}.show", default_show::name()));
    let existing = desk
        .library()?
        .into_iter()
        .find(|entry| entry.name == default_show::name());
    if validate_show_file(&path).is_err() {
        preserve_invalid_default_show(data_dir, &path)?;
        default_show::initialise(&path)?;
    }
    let entry = if let Some(existing) = existing {
        ShowStore::open(&path)?.set_identity(existing.id, &existing.name, None)?;
        desk.relocate_show(existing.id, &path.display().to_string())?
    } else {
        let entry = desk.upsert_show(default_show::name(), &path.display().to_string(), false)?;
        ShowStore::open(&path)?.set_identity(entry.id, &entry.name, None)?;
        entry
    };
    Ok(entry)
}

pub async fn run() -> anyhow::Result<()> {
    bootstrap::run().await
}

fn router(state: AppState) -> Router {
    http_router::build(state)
}

fn fixed_test_time() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
        .expect("fixed test timestamp is valid")
        .with_timezone(&chrono::Utc)
}

async fn reset_test_clock(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let clock = state
        .manual_clock
        .as_ref()
        .ok_or_else(|| ApiError::not_found("test clock"))?;
    clock.set(fixed_test_time());
    state.programmers.reset_all();
    state.engine.clear_programmer_transitions();
    state.output_sequences.lock().await.clear();
    state.osc_subscribers.lock().clear();
    {
        let configuration = state.configuration.read().clone();
        *state.speed_groups.lock() = std::array::from_fn(|index| {
            SpeedGroupController::new(
                configuration.speed_groups_bpm[index],
                configuration.speed_group_sound_to_light[index].clone(),
            )
            .expect("validated Speed Group configuration")
        });
        *state.sound_capture_owners.lock() = [None; 5];
    }
    refresh_speed_group_engine(&state);
    emit(
        &state,
        "hardware_connection_changed",
        serde_json::json!({"connected":false}),
    );
    Ok(Json(serde_json::json!({"now":clock.now()})))
}

#[derive(Deserialize)]
struct AdvanceTestClock {
    #[serde(default)]
    millis: i64,
}

async fn advance_test_clock(
    State(state): State<AppState>,
    Json(input): Json<AdvanceTestClock>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !(0..=604_800_000).contains(&input.millis) {
        return Err(ApiError::bad_request("millis must be within 0-604800000"));
    }
    let clock = state
        .manual_clock
        .as_ref()
        .ok_or_else(|| ApiError::not_found("test clock"))?;
    let now = clock.advance_millis(input.millis);
    refresh_speed_group_engine(&state);
    let rendered = state
        .engine
        .render(state.output_control.lock().render_options())
        .map_err(|error| ApiError::internal(error.to_string()))?;
    publish_automatic_playback_events(
        &state.application_events,
        rendered.automatic_playback_transitions,
    );
    let frames = {
        let mut control = state.output_control.lock();
        if control.hold {
            control.last_frames.clone()
        } else {
            let mut frames = rendered.universes;
            for (&(universe, address), &value) in &control.raw_overrides {
                if let Some(frame) = frames.get_mut(&universe) {
                    frame[usize::from(address - 1)] = value;
                }
            }
            control.last_frames = frames.clone();
            frames
        }
    };
    let snapshot = state.engine.snapshot();
    let output = state
        .network_output
        .as_ref()
        .ok_or_else(|| ApiError::unavailable("network output is unavailable"))?;
    let packets = output
        .send_routes(
            &snapshot.routes,
            &frames,
            &rendered.patched_slots,
            &mut *state.output_sequences.lock().await,
        )
        .await
        .map_err(ApiError::io)?;
    {
        let mut health = state
            .output_health
            .lock()
            .expect("output health mutex poisoned");
        health.frames_sent += 1;
        health.packets_sent += packets;
        health.send_errors += output.take_send_errors();
    }
    send_osc_feedback(&state, true);
    Ok(Json(serde_json::json!({
        "now": now,
        "revision": rendered.revision,
        "packets_sent": packets,
        "universes": frames.into_iter().map(|(universe, slots)| serde_json::json!({"universe":universe,"slots":slots.to_vec()})).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct TestOutputFailure {
    destination: SocketAddr,
    enabled: bool,
}

async fn set_test_output_failure(
    State(state): State<AppState>,
    Json(input): Json<TestOutputFailure>,
) -> Result<StatusCode, ApiError> {
    state
        .network_output
        .as_ref()
        .ok_or_else(|| ApiError::unavailable("network output is unavailable"))?
        .inject_failure(input.destination, input.enabled);
    Ok(StatusCode::NO_CONTENT)
}

async fn list_fixture_library(
    State(state): State<AppState>,
) -> Result<Json<Vec<light_fixture::FixtureDefinition>>, ApiError> {
    Ok(Json(
        state
            .fixture_library
            .lock()
            .definitions()
            .map_err(ApiError::fixture)?,
    ))
}

async fn put_fixture_library(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(definition): Json<light_fixture::FixtureDefinition>,
) -> Result<Json<light_fixture::FixtureDefinition>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let json = serde_json::to_string(&definition)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let stored = state
        .fixture_library
        .lock()
        .import_json(&json)
        .map_err(ApiError::fixture)?;
    emit(
        &state,
        "fixture_library_changed",
        serde_json::json!({"id":stored.id,"revision":stored.revision}),
    );
    Ok(Json(stored))
}

async fn delete_fixture_library(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if state
        .fixture_library
        .lock()
        .delete(id, revision)
        .map_err(ApiError::fixture)?
    {
        emit(
            &state,
            "fixture_library_changed",
            serde_json::json!({"id":id,"revision":revision,"deleted":true}),
        );
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("fixture definition"))
    }
}

async fn list_fixture_profiles(
    State(state): State<AppState>,
) -> Result<Json<Vec<light_fixture::FixtureProfile>>, ApiError> {
    Ok(Json(
        state
            .fixture_library
            .lock()
            .profiles()
            .map_err(ApiError::fixture)?,
    ))
}

async fn list_fixture_profile_warnings(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(
        state
            .fixture_library
            .lock()
            .migration_warnings()
            .map_err(ApiError::fixture)?,
    ))
}

async fn list_fixture_profile_revisions(
    State(state): State<AppState>,
    Path(id): Path<light_core::FixtureId>,
) -> Result<Json<Vec<light_fixture::FixtureProfile>>, ApiError> {
    let library = state.fixture_library.lock();
    let revisions = library.profile_revisions(id).map_err(ApiError::fixture)?;
    let profiles = revisions
        .into_iter()
        .map(|revision| {
            library
                .profile(id, revision)
                .map_err(ApiError::fixture)?
                .ok_or_else(|| ApiError::not_found("fixture profile revision"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(profiles))
}

async fn put_fixture_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(profile): Json<light_fixture::FixtureProfile>,
) -> Result<Json<light_fixture::FixtureProfile>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let expected = u32::try_from(expected)
        .map_err(|_| ApiError::bad_request("fixture profile revision exceeds u32"))?;
    let stored = state
        .fixture_library
        .lock()
        .save_profile(profile, expected)
        .map_err(ApiError::fixture)?;
    emit(
        &state,
        "fixture_profile_changed",
        serde_json::json!({"id":stored.id,"revision":stored.revision}),
    );
    Ok(Json(stored))
}

async fn import_fixture_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<light_fixture::FixtureProfile>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if body.is_empty() {
        return Err(ApiError::bad_request("fixture package is empty"));
    }
    let stored = state
        .fixture_library
        .lock()
        .import_fixture_package(&body)
        .map_err(ApiError::fixture)?;
    emit(
        &state,
        "fixture_profile_changed",
        serde_json::json!({"id":stored.id,"revision":stored.revision,"imported_package":true}),
    );
    Ok(Json(stored))
}

async fn export_fixture_package(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let library = state.fixture_library.lock();
    let profile = library
        .profile(id, revision)
        .map_err(ApiError::fixture)?
        .ok_or_else(|| ApiError::not_found("fixture profile revision"))?;
    let bytes = library
        .export_fixture_package(id, revision)
        .map_err(ApiError::fixture)?
        .ok_or_else(|| ApiError::not_found("fixture profile revision"))?;
    let filename = format!(
        "{}-{}.toskfixture",
        profile
            .manufacturer
            .chars()
            .chain(std::iter::once('-'))
            .chain(profile.name.chars())
            .map(|character| if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            })
            .collect::<String>()
            .trim_matches('-'),
        revision
    );
    Ok((
        [
            (
                header::CONTENT_TYPE,
                light_fixture::FIXTURE_PACKAGE_MIME_TYPE,
            ),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn delete_fixture_profile(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if state
        .fixture_library
        .lock()
        .delete_profile(id, revision)
        .map_err(ApiError::fixture)?
    {
        emit(
            &state,
            "fixture_profile_changed",
            serde_json::json!({"id":id,"revision":revision,"deleted":true}),
        );
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("fixture profile revision"))
    }
}

async fn put_fixture_profile_source_gdtf(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if body.is_empty() {
        return Err(ApiError::bad_request("GDTF source archive is empty"));
    }
    if !state
        .fixture_library
        .lock()
        .set_profile_source_gdtf(id, revision, &body)
        .map_err(ApiError::fixture)?
    {
        return Err(ApiError::not_found("fixture profile revision"));
    }
    emit(
        &state,
        "fixture_profile_changed",
        serde_json::json!({"id":id,"revision":revision,"source_gdtf":true}),
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn desk_boundary(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(required) = &state.desk_token else {
        return next.run(request).await;
    };
    let ticketed_file_stream = request.method() == Method::GET
        && request.uri().path().starts_with("/api/v1/files/")
        && request.uri().path().ends_with("/content")
        && request.uri().query().is_some_and(|query| {
            query
                .split('&')
                .any(|part| part.starts_with("ticket=") && part.len() > "ticket=".len())
        });
    if request.uri().path() == "/"
        || request.uri().path().starts_with("/assets/")
        || request.uri().path().starts_with("/api/v1/help/assets/")
        // Native audio elements cannot attach the desk-boundary header. The
        // content handler still validates the path-bound, expiring stream
        // capability and its active authenticated session.
        || ticketed_file_stream
    {
        return next.run(request).await;
    }
    let supplied_header = request
        .headers()
        .get("x-light-desk-token")
        .and_then(|value| value.to_str().ok());
    let supplied_ws = request
        .headers()
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .split(',')
                .map(str::trim)
                .find_map(|value| value.strip_prefix("light.desk.b64."))
        })
        .and_then(|encoded| URL_SAFE_NO_PAD.decode(encoded).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok());
    if supplied_header == Some(required.as_ref())
        || supplied_ws.as_deref() == Some(required.as_ref())
    {
        next.run(request).await
    } else {
        ApiError::unauthorized("desk boundary token is required").into_response()
    }
}

fn desk_lock_key(id: Uuid) -> String {
    format!("desk_lock:{id}")
}

fn read_desk_lock(state: &AppState, id: Uuid) -> DeskLockConfiguration {
    state
        .desk
        .lock()
        .setting(&desk_lock_key(id))
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

fn write_desk_lock(
    state: &AppState,
    id: Uuid,
    configuration: &DeskLockConfiguration,
) -> Result<(), ApiError> {
    let value = serde_json::to_string(configuration)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .desk
        .lock()
        .set_setting(&desk_lock_key(id), &value)
        .map_err(ApiError::store)
}

fn desk_lock_response(configuration: DeskLockConfiguration) -> DeskLockResponse {
    DeskLockResponse {
        locked: configuration.locked,
        message: configuration.message,
        wallpaper: configuration.wallpaper,
        unlock_mode: configuration.unlock_mode,
    }
}

fn pin_hash(salt: &str, pin: &str) -> String {
    let digest = Sha256::digest(format!("{salt}:{pin}").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn desk_lock_boundary(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if request.method() == Method::GET
        || request.method() == Method::OPTIONS
        || path == "/api/v1/sessions"
        || path.starts_with("/api/v1/desk-lock")
    {
        return next.run(request).await;
    }
    let session = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|token| authenticate_token(&state, token).ok());
    if session
        .as_ref()
        .is_some_and(|session| read_desk_lock(&state, session.desk.id).locked)
    {
        return ApiError::conflict("desk is locked").into_response();
    }
    next.run(request).await
}

async fn desk_lock(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(desk_lock_response(read_desk_lock(
        &state,
        session.desk.id,
    ))))
}

async fn update_desk_lock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<DeskLockUpdate>,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let operation_lock = state.command_http.operation_lock(session.desk.id);
    let _operation = operation_lock.lock();
    let mut configuration = read_desk_lock(&state, session.desk.id);
    if configuration.locked {
        return Err(ApiError::conflict(
            "unlock the desk before changing its lock configuration",
        ));
    }
    if !matches!(input.unlock_mode.as_str(), "button" | "pin") {
        return Err(ApiError::bad_request("unlock mode must be button or pin"));
    }
    if input.message.len() > 500 {
        return Err(ApiError::bad_request(
            "lock message must not exceed 500 characters",
        ));
    }
    configuration.message = input.message;
    configuration.wallpaper = input.wallpaper.filter(|value| !value.trim().is_empty());
    configuration.unlock_mode = input.unlock_mode;
    if configuration.unlock_mode == "pin" {
        if let Some(pin) = input.pin {
            if !(4..=12).contains(&pin.len())
                || !pin.chars().all(|character| character.is_ascii_digit())
            {
                return Err(ApiError::bad_request("PIN must contain 4-12 digits"));
            }
            let salt = Uuid::new_v4().to_string();
            configuration.pin_hash = Some(pin_hash(&salt, &pin));
            configuration.pin_salt = Some(salt);
        }
        if configuration.pin_hash.is_none() {
            return Err(ApiError::bad_request(
                "PIN required mode needs a configured PIN",
            ));
        }
    } else {
        configuration.pin_hash = None;
        configuration.pin_salt = None;
    }
    write_desk_lock(&state, session.desk.id, &configuration)?;
    emit(
        &state,
        "desk_lock_changed",
        serde_json::json!({"desk_id":session.desk.id,"locked":false}),
    );
    Ok(Json(desk_lock_response(configuration)))
}

async fn lock_desk(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let operation_lock = state.command_http.operation_lock(session.desk.id);
    let _operation = operation_lock.lock();
    let mut configuration = read_desk_lock(&state, session.desk.id);
    configuration.locked = true;
    write_desk_lock(&state, session.desk.id, &configuration)?;
    emit(
        &state,
        "desk_lock_changed",
        serde_json::json!({"desk_id":session.desk.id,"locked":true}),
    );
    Ok(Json(desk_lock_response(configuration)))
}

async fn unlock_desk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<DeskUnlockInput>,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let operation_lock = state.command_http.operation_lock(session.desk.id);
    let _operation = operation_lock.lock();
    let mut configuration = read_desk_lock(&state, session.desk.id);
    if configuration.unlock_mode == "pin" {
        let Some(pin) = input.pin else {
            return Err(ApiError::unauthorized("PIN is required"));
        };
        let valid = configuration
            .pin_salt
            .as_deref()
            .zip(configuration.pin_hash.as_deref())
            .is_some_and(|(salt, expected)| pin_hash(salt, &pin) == expected);
        if !valid {
            return Err(ApiError::unauthorized("incorrect PIN"));
        }
    }
    configuration.locked = false;
    write_desk_lock(&state, session.desk.id, &configuration)?;
    emit(
        &state,
        "desk_lock_changed",
        serde_json::json!({"desk_id":session.desk.id,"locked":false}),
    );
    Ok(Json(desk_lock_response(configuration)))
}

async fn force_unlock_desk(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DeskLockResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let operation_lock = state.command_http.operation_lock(session.desk.id);
    let _operation = operation_lock.lock();
    let supplied = headers
        .get("x-light-admin-recovery")
        .and_then(|value| value.to_str().ok());
    let expected = env::var("LIGHT_ADMIN_RECOVERY_TOKEN").ok();
    if expected
        .as_deref()
        .is_none_or(|expected| supplied != Some(expected))
    {
        return Err(ApiError::unauthorized(
            "administrative recovery token is required",
        ));
    }
    let mut configuration = read_desk_lock(&state, session.desk.id);
    configuration.locked = false;
    write_desk_lock(&state, session.desk.id, &configuration)?;
    emit(
        &state,
        "desk_lock_changed",
        serde_json::json!({"desk_id":session.desk.id,"locked":false,"forced":true}),
    );
    Ok(Json(desk_lock_response(configuration)))
}

async fn operator_ui() -> Response {
    embedded_asset("index.html")
}
async fn operator_asset(Path(path): Path<String>) -> Response {
    embedded_asset(&format!("assets/{path}"))
}
fn embedded_asset(path: &str) -> Response {
    let Some(asset) = ControlUiAssets::get(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else {
        "application/octet-stream"
    };
    (
        [(header::CONTENT_TYPE, content_type)],
        asset.data.into_owned(),
    )
        .into_response()
}
async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok", "service":"light-server", "api_version":"v1"}))
}
async fn version() -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"service":"light-server","version":env!("CARGO_PKG_VERSION"),"api_version":"v1","show_schema":3,"desk_schema":6}),
    )
}
async fn readiness(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let active_show_error = state.active_show_error.read().clone();
    let recovery_mode = active_show_error.is_some();
    if !recovery_mode && let Some(show) = state.active_show.read().as_ref() {
        validate_show_file(&show.path).map_err(|error| ApiError::unavailable(error.to_string()))?;
    }
    Ok(Json(
        serde_json::json!({"status":"ready","active_show":state.active_show.read().as_ref().map(|show|show.id),"active_show_error":active_show_error,"recovery_mode":recovery_mode,"snapshot_revision":state.engine.snapshot().revision}),
    ))
}
async fn diagnostics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    // Refresh derived runtime state at the same application timestamp before exposing it. This is
    // especially important under the manually advanced Playwright clock, where no output frame is
    // guaranteed to have rendered between two exact MIB checkpoints.
    let _ = state.engine.resolved_values();
    let route_send_errors = state
        .network_output
        .as_ref()
        .map(|output| output.route_send_errors())
        .unwrap_or_default();
    let output_routes = NetworkOutput::route_diagnostics(&state.engine.snapshot().routes);
    let output_bind_ip = state.configuration.read().output_bind_ip;
    Ok(Json(
        serde_json::json!({"output":state.output_health.lock().expect("output health mutex poisoned").clone(),"output_bind_ip":output_bind_ip,"output_routes":output_routes,"route_send_errors":route_send_errors,"event_queue_pressure":state.events.len(),"active_programmers":state.programmers.active(),"active_playbacks":state.engine.playback().read().active(),"move_in_black":state.engine.move_in_black_runtime(),"timecode_source":state.timecode_router.lock().active_source(),"media_servers":state.media_status.read().clone(),"snapshot_revision":state.engine.snapshot().revision}),
    ))
}
async fn bootstrap(State(state): State<AppState>) -> Json<Bootstrap> {
    let (users, desks, client_desks) = {
        let desk = state.desk.lock();
        (
            desk.users().unwrap_or_default(),
            desk.desks().unwrap_or_default(),
            desk.client_desks().unwrap_or_default(),
        )
    };
    let sessions = state.sessions.read();
    let session_clients = state.session_clients.read();
    let mut clients = client_desks
        .into_iter()
        .map(|entry| {
            let client_id = entry.client_id.unwrap_or(entry.desk.id);
            let connected = sessions
                .values()
                .any(|session| session_clients.get(&session.id) == Some(&client_id));
            let desk_in_use = sessions
                .values()
                .any(|session| session.desk.id == entry.desk.id);
            ClientSummary {
                client_id,
                name: entry.desk.name.clone(),
                connected,
                last_connected_at: entry.last_connected_at,
                desk: entry.desk,
                can_remove: !connected && !desk_in_use,
            }
        })
        .collect::<Vec<_>>();
    drop(sessions);
    drop(session_clients);
    clients.sort_by(|left, right| {
        right
            .connected
            .cmp(&left.connected)
            .then_with(|| right.last_connected_at.cmp(&left.last_connected_at))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.client_id.cmp(&right.client_id))
    });
    let (active_timecode_source, active_timecode) = {
        let router = state.timecode_router.lock();
        (
            router.active_source().map(str::to_owned),
            router.current().map(|timecode| {
                format!(
                    "{:02}:{:02}:{:02}:{:02}",
                    timecode.hours, timecode.minutes, timecode.seconds, timecode.frames
                )
            }),
        )
    };
    let snapshot = state.engine.snapshot();
    let highlight_fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let highlight_groups = highlight_groups(&snapshot);
    let highlight_states = state
        .sessions
        .read()
        .values()
        .filter_map(|session| {
            let programmer = state.programmers.get(session.id)?;
            let selection = state.programmers.selection(session.id)?;
            let transition = state.highlight.status(
                session.desk.id,
                session.user.id,
                Some(&session.user.name),
                &selection,
                &highlight_fixtures,
                &highlight_groups,
                programmer.blind || programmer.preview,
            );
            Some(BootstrapHighlightState {
                session_id: session.id,
                desk_id: session.desk.id,
                user_id: session.user.id,
                state: transition.state,
            })
        })
        .collect();
    Json(Bootstrap {
        api_version: "v1",
        attribute_registry: ATTRIBUTE_REGISTRY,
        users,
        desks,
        clients,
        active_show: state.active_show.read().clone(),
        active_programmers: state.programmers.active_for_sessions(),
        highlight_states,
        frame_rate_hz: state.output_rate.load(Ordering::Relaxed),
        output_health: state
            .output_health
            .lock()
            .expect("output health mutex poisoned")
            .clone(),
        active_timecode_source,
        active_timecode,
        active_show_error: state.active_show_error.read().clone(),
        hardware_connected: !state.osc_subscribers.lock().is_empty(),
    })
}
async fn patch_snapshot(State(state): State<AppState>) -> Json<serde_json::Value> {
    let snapshot = state.engine.snapshot();
    Json(
        serde_json::json!({"revision":snapshot.revision,"fixtures":snapshot.fixtures,"routes":snapshot.routes}),
    )
}
async fn visualization_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<VisualizationQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let snapshot = state.engine.snapshot();
    let options = state.output_control.lock().render_options();
    let mut resolved = state.engine.resolved_values();
    if query.preload
        && let Some(programmer) = state.programmers.get(session.id)
    {
        for value in programmer
            .preload_active
            .iter()
            .chain(&programmer.preload_pending)
        {
            resolved.insert(
                (value.fixture_id, value.attribute.clone()),
                value.value.clone(),
            );
        }
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for (group_id, attributes) in programmer
            .preload_group_active
            .iter()
            .chain(&programmer.preload_group_pending)
        {
            if let Ok(fixtures) = light_programmer::resolve_group(group_id, &groups) {
                for fixture in fixtures {
                    for (attribute, value) in attributes {
                        resolved.insert((fixture, attribute.clone()), value.value.clone());
                    }
                }
            }
        }
    }
    let profile_output_values = state
        .engine
        .profile_visualization_values(&resolved, options)
        .map_err(|error| ApiError::internal(error.to_string()))?
        .into_iter()
        .map(|((fixture_id, attribute), value)| {
            serde_json::json!({
                "fixture_id": fixture_id,
                "attribute": attribute,
                "value": value,
            })
        })
        .collect::<Vec<_>>();
    let values = resolved
        .into_iter()
        .map(|((fixture_id, attribute), value)| {
            serde_json::json!({
                "fixture_id": fixture_id,
                "attribute": attribute,
                "value": value,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({
        "revision": snapshot.revision,
        "generated_at": chrono::Utc::now(),
        "grand_master": options.grand_master,
        "blackout": options.blackout,
        "preload": query.preload,
        "values": values,
        "profile_output_values": profile_output_values,
    })))
}
#[derive(Default, Deserialize)]
struct VisualizationQuery {
    #[serde(default)]
    preload: bool,
}
async fn media_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let statuses = state.media_status.read();
    let fixtures = state
        .engine
        .snapshot()
        .fixtures
        .iter()
        .filter_map(|fixture| {
            fixture.direct_control.as_ref().map(|endpoint| {
                let status = statuses.get(&fixture.fixture_id).cloned().unwrap_or_default();
                serde_json::json!({
                    "fixture_id": fixture.fixture_id,
                    "name": format!("{} {}", fixture.definition.manufacturer, fixture.definition.model),
                    "endpoint": endpoint,
                    "layers": fixture.logical_heads,
                    "status": status,
                })
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({ "fixtures": fixtures })))
}

async fn refresh_media_thumbnails(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    headers: HeaderMap,
    Json(input): Json<ThumbnailRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if !(1..=2).contains(&input.library_type) || input.library_level > 3 {
        return Err(ApiError::bad_request("invalid CITP library type or level"));
    }
    let library = library_id(&input);
    let address = media_endpoint(&state, fixture_id)?;
    let result = async {
        let mut client = CitpClient::connect(address, Duration::from_secs(3)).await?;
        client
            .request_thumbnail(
                input.library_type,
                library,
                &input.elements,
                input.width,
                input.height,
            )
            .await
    }
    .await;
    match result {
        Ok(images) => {
            let count = images.len();
            let mut cache = state.media_cache.lock();
            for (element, image) in images {
                cache
                    .put_thumbnail(
                        ThumbnailKey {
                            fixture: fixture_id.0.to_string(),
                            library_type: input.library_type,
                            library,
                            element,
                        },
                        image,
                    )
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            }
            update_media_status(&state, fixture_id, None);
            emit(
                &state,
                "media_thumbnails_refreshed",
                serde_json::json!({"session_id":session.id,"fixture_id":fixture_id,"count":count}),
            );
            Ok(Json(
                serde_json::json!({"fixture_id":fixture_id,"count":count}),
            ))
        }
        Err(error) => {
            update_media_status(&state, fixture_id, Some(error.to_string()));
            emit(
                &state,
                "media_server_offline",
                serde_json::json!({"fixture_id":fixture_id,"error":error.to_string()}),
            );
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

async fn media_thumbnail(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    Query(query): Query<ThumbnailQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let key = ThumbnailKey {
        fixture: fixture_id.0.to_string(),
        library_type: query.library_type,
        library: LibraryId {
            level: query.library_level,
            ids: [query.library_1, query.library_2, query.library_3],
        },
        element: query.element,
    };
    cached_image_response(state.media_cache.lock().thumbnail(&key), "thumbnail")
}

async fn refresh_media_preview(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    headers: HeaderMap,
    Json(input): Json<PreviewRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let address = media_endpoint(&state, fixture_id)?;
    let result = async {
        let mut client = CitpClient::connect(address, Duration::from_secs(3)).await?;
        client
            .request_preview(input.source, input.width, input.height)
            .await
    }
    .await;
    match result {
        Ok(image) => {
            let format = image.format;
            let width = image.width;
            let height = image.height;
            state
                .media_cache
                .lock()
                .put_preview(
                    PreviewKey {
                        fixture: fixture_id.0.to_string(),
                        source: input.source,
                    },
                    image,
                )
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            update_media_status(&state, fixture_id, None);
            emit(
                &state,
                "media_preview_refreshed",
                serde_json::json!({"session_id":session.id,"fixture_id":fixture_id,"source":input.source}),
            );
            Ok(Json(
                serde_json::json!({"fixture_id":fixture_id,"source":input.source,"format":format,"width":width,"height":height}),
            ))
        }
        Err(error) => {
            update_media_status(&state, fixture_id, Some(error.to_string()));
            emit(
                &state,
                "media_server_offline",
                serde_json::json!({"fixture_id":fixture_id,"error":error.to_string()}),
            );
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

async fn media_preview(
    State(state): State<AppState>,
    Path((fixture_id, source)): Path<(light_core::FixtureId, u16)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    cached_image_response(
        state.media_cache.lock().preview(&PreviewKey {
            fixture: fixture_id.0.to_string(),
            source,
        }),
        "preview",
    )
}

fn media_endpoint(
    state: &AppState,
    fixture_id: light_core::FixtureId,
) -> Result<SocketAddr, ApiError> {
    let snapshot = state.engine.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == fixture_id)
        .ok_or_else(|| ApiError::not_found("fixture"))?;
    let endpoint = fixture
        .direct_control
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("fixture has no direct-control endpoint"))?;
    Ok(SocketAddr::new(endpoint.ip_address, endpoint.port))
}
fn library_id(input: &ThumbnailRequest) -> LibraryId {
    LibraryId {
        level: input.library_level,
        ids: [input.library_1, input.library_2, input.library_3],
    }
}
fn update_media_status(state: &AppState, fixture_id: light_core::FixtureId, error: Option<String>) {
    let mut statuses = state.media_status.write();
    let status = statuses.entry(fixture_id).or_default();
    status.online = error.is_none();
    if let Some(error) = error {
        status.last_error = Some(error);
    } else {
        status.last_success = Some(chrono::Utc::now().to_rfc3339());
        status.last_error = None;
    }
}
fn cached_image_response(
    image: Option<light_media::CachedImage>,
    kind: &str,
) -> Result<Response, ApiError> {
    let image = image.ok_or_else(|| ApiError::not_found(format!("cached media {kind}")))?;
    let mut response = image.image.bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(image.image.format.mime()),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, max-age=5"),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-light-image-width"),
        header::HeaderValue::from_str(&image.image.width.to_string()).expect("valid width header"),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-light-image-height"),
        header::HeaderValue::from_str(&image.image.height.to_string())
            .expect("valid height header"),
    );
    Ok(response)
}
async fn dmx_snapshot(State(state): State<AppState>) -> Json<serde_json::Value> {
    let control = state.output_control.lock();
    let mut universes = control
        .last_frames
        .iter()
        .map(|(&universe, frame)| serde_json::json!({"universe":universe,"slots":frame.to_vec()}))
        .collect::<Vec<_>>();
    universes.sort_by_key(|universe| universe["universe"].as_u64().unwrap_or_default());
    Json(serde_json::json!({
        "revision":state.engine.snapshot().revision,
        "universes":universes,
        "overrides":control.raw_overrides.iter().map(|(&(universe,address),&value)| serde_json::json!({"universe":universe,"address":address,"value":value})).collect::<Vec<_>>()
    }))
}
async fn update_dmx_override(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RawDmxOverrideInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if input.universe == 0 || !(1..=512).contains(&input.address) {
        return Err(ApiError::bad_request(
            "universe and DMX address must be non-zero and address must be within 1-512",
        ));
    }
    let mut control = state.output_control.lock();
    match input.value {
        Some(value) => {
            control
                .raw_overrides
                .insert((input.universe, input.address), value);
        }
        None => {
            control
                .raw_overrides
                .remove(&(input.universe, input.address));
        }
    }
    drop(control);
    emit(
        &state,
        "dmx_override_changed",
        serde_json::json!({"session_id":session.id,"universe":input.universe,"address":input.address,"value":input.value}),
    );
    Ok(Json(serde_json::json!({"updated":true})))
}
async fn shutdown_server(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    emit(
        &state,
        "server_shutdown_requested",
        serde_json::json!({"session_id":session.id}),
    );
    state.shutdown.cancel();
    Ok(Json(serde_json::json!({"shutting_down":true})))
}
async fn configuration(State(state): State<AppState>) -> Json<serde_json::Value> {
    let matter = refresh_matter_bridge(&state);
    Json(
        serde_json::json!({"configuration":state.configuration.read().clone(),"output_health":state.output_health.lock().expect("output health mutex poisoned").clone(),"matter":matter}),
    )
}

async fn matter_bridge_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<matter::MatterBridgeStatus>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(refresh_matter_bridge(&state)))
}

fn refresh_matter_bridge(state: &AppState) -> matter::MatterBridgeStatus {
    let enabled = state.configuration.read().matter_enabled;
    let adapter = if !enabled {
        state
            .matter_bridge
            .reconcile(false, &[], &[], &HashMap::new());
        state.matter_bridge.status()
    } else {
        let snapshot = state.engine.snapshot();
        let values = matter_playback_values(state, &snapshot);
        state
            .matter_bridge
            .reconcile(true, &snapshot.playback_pages, &snapshot.playbacks, &values)
    };
    let Some(transport) = &state.matter_transport else {
        return adapter;
    };
    let transport = transport.reconcile(enabled, &adapter.lights);
    state.matter_bridge.apply_transport_snapshot(&transport)
}

fn spawn_matter_bridge_sync(
    state: AppState,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => break,
                _ = interval.tick() => {
                    refresh_matter_bridge(&state);
                    if let Some(transport) = &state.matter_transport {
                        let writes = transport.drain_remote_writes();
                        for remote in &writes {
                            if let Err(error) = apply_matter_playback_write(
                                &state,
                                remote.endpoint_id,
                                remote.write,
                            ) {
                                emit(
                                    &state,
                                    "matter_write_rejected",
                                    serde_json::json!({"endpoint_id":remote.endpoint_id,"error":error.message}),
                                );
                            }
                        }
                        if !writes.is_empty() {
                            refresh_matter_bridge(&state);
                        }
                    }
                }
            }
        }
        if let Some(transport) = &state.matter_transport {
            transport.stop();
        }
    })
}

fn matter_playback_values(
    state: &AppState,
    snapshot: &EngineSnapshot,
) -> HashMap<u16, matter::PlaybackValue> {
    let runtime = state
        .engine
        .playback()
        .read()
        .runtime_status()
        .into_iter()
        .filter_map(|status| {
            status
                .playback
                .playback_number
                .map(|number| (number, status))
        })
        .collect::<HashMap<_, _>>();
    let now = application_millis(state);
    let speeds = {
        let controllers = state.speed_groups.lock();
        std::array::from_fn::<_, 5, _>(|index| controllers[index].snapshot(now))
    };
    let configuration = state.configuration.read().clone();
    let grand_master = state.output_control.lock().options.grand_master;
    snapshot
        .playbacks
        .iter()
        .map(|definition| {
            use light_playback::PlaybackTarget;
            let value = match &definition.target {
                PlaybackTarget::CueList { .. } => runtime
                    .get(&definition.number)
                    .map(|status| match definition.fader {
                        light_playback::PlaybackFaderMode::Temp => matter::PlaybackValue::new(
                            status.temporary_master,
                            status.temporary_active,
                        ),
                        light_playback::PlaybackFaderMode::XFade => matter::PlaybackValue::new(
                            status.playback.manual_xfade_position,
                            status.playback.enabled,
                        ),
                        _ => matter::PlaybackValue::new(
                            status.playback.master,
                            status.playback.enabled,
                        ),
                    })
                    .unwrap_or_default(),
                PlaybackTarget::Group { group_id } => snapshot
                    .groups
                    .iter()
                    .find(|group| group.id == *group_id)
                    .map(|group| matter::PlaybackValue::new(group.master, group.master > 0.0))
                    .unwrap_or_default(),
                PlaybackTarget::SpeedGroup { group } => speed_group_index(group)
                    .ok()
                    .map(|index| {
                        let level = matter_speed_fader_level(speeds[index], definition.fader);
                        matter::PlaybackValue::new(level, level > 0.0)
                    })
                    .unwrap_or_default(),
                PlaybackTarget::ProgrammerFade => {
                    let level =
                        (configuration.programmer_fade_millis as f32 / 20_000.0).clamp(0.0, 1.0);
                    matter::PlaybackValue::new(level, level > 0.0)
                }
                PlaybackTarget::CueFade => {
                    let level = (configuration.sequence_master_fade_millis as f32 / 60_000.0)
                        .clamp(0.0, 1.0);
                    matter::PlaybackValue::new(level, level > 0.0)
                }
                PlaybackTarget::GrandMaster => {
                    matter::PlaybackValue::new(grand_master, grand_master > 0.0)
                }
            };
            (definition.number, value)
        })
        .collect()
}

fn matter_speed_fader_level(
    snapshot: SpeedSnapshot,
    fader: light_playback::PlaybackFaderMode,
) -> f32 {
    use light_playback::PlaybackFaderMode;
    let level = match fader {
        PlaybackFaderMode::DirectBpm => {
            if snapshot.speed_master_scale == 0.0 {
                0.0
            } else {
                snapshot.manual_bpm / 300.0
            }
        }
        PlaybackFaderMode::CenteredRelative => {
            snapshot.speed_master_scale.max(f64::MIN_POSITIVE).log(4.0) / 2.0 + 0.5
        }
        PlaybackFaderMode::LearnedPercentage | PlaybackFaderMode::Speed => {
            snapshot.speed_master_scale
        }
        _ => 0.0,
    };
    level.clamp(0.0, 1.0) as f32
}

/// Apply the protocol-independent result of a Matter On/Off or Level Control write through the
/// same global playback dispatcher used by attached desk surfaces. A protocol transport can call
/// this seam after commissioning without acquiring a desk-local current-page context.
#[allow(dead_code)]
fn apply_matter_playback_write(
    state: &AppState,
    endpoint_id: u16,
    write: matter::MatterPlaybackWrite,
) -> Result<matter::MatterBridgeStatus, ApiError> {
    refresh_matter_bridge(state);
    let resolved = state
        .matter_bridge
        .resolve_write(endpoint_id, write)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let definition = state
        .engine
        .snapshot()
        .playbacks
        .iter()
        .find(|definition| definition.number == resolved.playback_number)
        .cloned()
        .ok_or_else(|| ApiError::not_found("playback"))?;
    let changed = dispatch_playback_action(
        state,
        None,
        None,
        &definition,
        "fader",
        &PoolPlaybackInput {
            value: Some(resolved.level),
            surface: Some("matter".into()),
            ..PoolPlaybackInput::default()
        },
        "matter",
    )?;
    if changed {
        emit(
            state,
            "playback_changed",
            serde_json::json!({
                "page":resolved.page,
                "playback":resolved.playback,
                "playback_number":resolved.playback_number,
                "action":"fader",
                "source":"matter"
            }),
        );
    }
    Ok(refresh_matter_bridge(state))
}

fn speed_group_index(group: &str) -> Result<usize, ApiError> {
    match group.to_ascii_uppercase().as_str() {
        "A" | "1" => Ok(0),
        "B" | "2" => Ok(1),
        "C" | "3" => Ok(2),
        "D" | "4" => Ok(3),
        "E" | "5" => Ok(4),
        _ => Err(ApiError::bad_request("Speed Group must be A-E")),
    }
}

fn speed_group_name(index: usize) -> String {
    char::from(b'A' + index as u8).to_string()
}

fn linked_speed_group(controllers: &[SpeedGroupController; 5], index: usize) -> Option<usize> {
    controllers[index]
        .synchronized_with()
        .and_then(|group| usize::from(group).checked_sub(1))
        .filter(|peer| *peer < controllers.len() && *peer != index)
}

/// Detaches one group from its reciprocal phase link. The group which received the manual
/// action starts a new independent phase at `now_millis`; its untouched peer keeps its existing
/// phase origin and BPM.
fn unlink_speed_group(controllers: &mut [SpeedGroupController; 5], index: usize, now_millis: u64) {
    let peer = linked_speed_group(controllers, index);
    controllers[index].break_synchronization(now_millis);
    if let Some(peer) = peer
        && controllers[peer].synchronized_with() == Some((index + 1) as u8)
    {
        controllers[peer].clear_synchronization();
    }
}

fn synchronize_speed_groups(
    controllers: &mut [SpeedGroupController; 5],
    source: usize,
    target: usize,
    now_millis: u64,
) -> Result<(), ApiError> {
    if source == target {
        return Err(ApiError::bad_request(
            "source and target Speed Groups must be different",
        ));
    }

    let source_snapshot = controllers[source].snapshot(now_millis);
    let source_phase_reference = controllers[source].phase_reference_millis(now_millis);
    // Relinking does not itself count as the independent action that resets a beat. Preserve the
    // source origin, while removing any older links from both addressed groups.
    if let Some(peer) = linked_speed_group(controllers, source) {
        controllers[source].clear_synchronization();
        if controllers[peer].synchronized_with() == Some((source + 1) as u8) {
            controllers[peer].clear_synchronization();
        }
    }
    if let Some(peer) = linked_speed_group(controllers, target) {
        controllers[target].clear_synchronization();
        if controllers[peer].synchronized_with() == Some((target + 1) as u8) {
            controllers[peer].clear_synchronization();
        }
    }

    controllers[source]
        .set_manual_bpm(source_snapshot.manual_bpm)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    controllers[target]
        .set_manual_bpm(source_snapshot.manual_bpm)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    for index in [source, target] {
        controllers[index]
            .set_speed_master_scale(1.0)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        controllers[index].set_paused_at(source_snapshot.paused, now_millis);
    }
    controllers[source].synchronize_phase(
        (target + 1) as u8,
        source_snapshot.phase_origin_millis,
        source_phase_reference,
    );
    controllers[target].synchronize_phase(
        (source + 1) as u8,
        source_snapshot.phase_origin_millis,
        source_phase_reference,
    );
    Ok(())
}

fn speed_group_action_indices(controllers: &[SpeedGroupController; 5], index: usize) -> Vec<usize> {
    let mut affected = vec![index];
    if let Some(peer) = linked_speed_group(controllers, index)
        && controllers[peer].synchronized_with() == Some((index + 1) as u8)
    {
        affected.push(peer);
    }
    affected
}

fn copy_speed_group_runtime_to_configuration(
    state: &AppState,
    controllers: &[SpeedGroupController; 5],
    indices: &[usize],
) {
    let mut configuration = state.configuration.write();
    for &index in indices {
        configuration.speed_groups_bpm[index] = controllers[index].manual_bpm();
        configuration.speed_group_sound_to_light[index] = controllers[index].sound_config().clone();
    }
}

fn application_millis(state: &AppState) -> u64 {
    state
        .engine
        .playback()
        .read()
        .clock()
        .now()
        .timestamp_millis()
        .max(0) as u64
}

/// Propagates the authoritative Speed Group controllers into both chaser scheduling and runtime
/// pause state. The controller retains the useful BPM while paused; the engine receives a
/// separate phase-advancing flag so resuming does not lose that rate.
fn refresh_speed_group_engine(state: &AppState) -> [SpeedSnapshot; 5] {
    let now = application_millis(state);
    let snapshots = {
        let controllers = state.speed_groups.lock();
        std::array::from_fn(|index| controllers[index].snapshot(now))
    };
    let timing = state.configuration.read().clone();
    let effective_bpm = snapshots.map(|snapshot| snapshot.effective_bpm.clamp(0.1, 999.0));
    state.engine.set_control_timing(
        effective_bpm,
        timing.programmer_fade_millis,
        timing.sequence_master_fade_millis,
    );
    state
        .engine
        .set_speed_groups_paused(snapshots.map(|snapshot| !snapshot.phase_advancing));
    snapshots
}

fn persist_server_configuration(state: &AppState) -> Result<(), ApiError> {
    let configuration = state.configuration.read().clone();
    let encoded = serde_json::to_string(&configuration)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .desk
        .lock()
        .set_setting("server_configuration", &encoded)
        .map_err(ApiError::store)
}

fn speed_group_response(
    state: &AppState,
    index: usize,
    snapshots: [SpeedSnapshot; 5],
) -> SpeedGroupResponse {
    let configuration = state.speed_groups.lock()[index].sound_config().clone();
    SpeedGroupResponse {
        group: speed_group_name(index),
        configuration,
        snapshot: snapshots[index],
    }
}

async fn speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    let snapshots = refresh_speed_group_engine(&state);
    Ok(Json(speed_group_response(&state, index, snapshots)))
}

async fn update_speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    Json(configuration): Json<SoundToLightConfig>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    configuration
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state.speed_groups.lock()[index]
        .set_sound_config(configuration.clone())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state.configuration.write().speed_group_sound_to_light[index] = configuration.clone();
    if !configuration.enabled {
        state.sound_capture_owners.lock()[index] = None;
    }
    persist_server_configuration(&state)?;
    let snapshots = refresh_speed_group_engine(&state);
    emit(
        &state,
        "speed_group_changed",
        serde_json::json!({"group":speed_group_name(index),"desk_id":session.desk.id,"configuration":configuration}),
    );
    Ok(Json(speed_group_response(&state, index, snapshots)))
}

async fn observe_speed_group(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    Json(mut observation): Json<SoundObservation>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    let now = application_millis(&state);
    if !state.speed_groups.lock()[index].sound_config().enabled {
        return Err(ApiError::conflict(
            "enable Sound to Light before submitting observations",
        ));
    }
    {
        let mut owners = state.sound_capture_owners.lock();
        if owners[index].is_some_and(|owner| {
            owner.desk_id != session.desk.id && now.saturating_sub(owner.last_seen_millis) <= 3_000
        }) {
            return Err(ApiError::conflict(
                "this Speed Group is receiving audio from another desk",
            ));
        }
        owners[index] = Some(SoundCaptureOwner {
            desk_id: session.desk.id,
            last_seen_millis: now,
        });
    }
    // Browser clocks and capture callback timestamps are not comparable across desks. The server
    // stamps every accepted sample with the shared application clock used by playback.
    observation.captured_at_millis = now;
    state.speed_groups.lock()[index].observe_sound(observation);
    let snapshots = refresh_speed_group_engine(&state);
    emit(
        &state,
        "speed_group_sound_observed",
        serde_json::json!({"group":speed_group_name(index),"desk_id":session.desk.id,"snapshot":snapshots[index]}),
    );
    Ok(Json(speed_group_response(&state, index, snapshots)))
}

async fn speed_group_action(
    State(state): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    Json(input): Json<SpeedGroupActionInput>,
) -> Result<Json<SpeedGroupResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let index = speed_group_index(&group)?;
    let now = application_millis(&state);
    let mut controller = state.speed_groups.lock();
    let affected = match input.action.as_str() {
        "learn" => {
            // The optional browser timestamp is deliberately advisory only; all desk surfaces use
            // the same application clock so an attached OSC surface and the UI behave identically.
            let _browser_timestamp = input.captured_at_millis;
            unlink_speed_group(&mut controller, index, now);
            controller[index].tap_learn(now);
            vec![index]
        }
        "double" => {
            let affected = speed_group_action_indices(&controller, index);
            for &affected_index in &affected {
                controller[affected_index].double();
            }
            affected
        }
        "half" => {
            let affected = speed_group_action_indices(&controller, index);
            for &affected_index in &affected {
                controller[affected_index].half();
            }
            affected
        }
        "pause" => {
            let paused = !controller[index].snapshot(now).paused;
            let affected = speed_group_action_indices(&controller, index);
            for &affected_index in &affected {
                controller[affected_index].set_paused_at(paused, now);
            }
            affected
        }
        _ => {
            return Err(ApiError::bad_request(
                "Speed Group action must be learn, double, half, or pause",
            ));
        }
    };
    copy_speed_group_runtime_to_configuration(&state, &controller, &affected);
    drop(controller);
    if input.action == "learn" {
        state.sound_capture_owners.lock()[index] = None;
    }
    persist_server_configuration(&state)?;
    let snapshots = refresh_speed_group_engine(&state);
    emit(
        &state,
        "speed_group_action",
        serde_json::json!({"group":speed_group_name(index),"desk_id":session.desk.id,"action":input.action,"snapshot":snapshots[index]}),
    );
    Ok(Json(speed_group_response(&state, index, snapshots)))
}

async fn midi_inputs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(
        light_control::available_midi_inputs().map_err(ApiError::internal)?,
    ))
}
async fn update_configuration(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut configuration): Json<DeskConfiguration>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    configuration.validate()?;
    let previous = state.configuration.read().clone();
    let now = application_millis(&state);
    {
        let mut controllers = state.speed_groups.lock();
        for index in 0..controllers.len() {
            if configuration.speed_groups_bpm[index] != previous.speed_groups_bpm[index] {
                // A direct value entered through Configuration is the same manual action as the
                // Speed Group UI or OSC surface and therefore takes ownership from Sound.
                unlink_speed_group(&mut controllers, index, now);
                controllers[index]
                    .set_manual_bpm(configuration.speed_groups_bpm[index])
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                controllers[index]
                    .set_speed_master_scale(1.0)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                controllers[index].set_paused_at(false, now);
                configuration.speed_group_sound_to_light[index].enabled = false;
                state.sound_capture_owners.lock()[index] = None;
            } else {
                controllers[index]
                    .set_manual_fallback_bpm(configuration.speed_groups_bpm[index])
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            }
            controllers[index]
                .set_sound_config(configuration.speed_group_sound_to_light[index].clone())
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
    }
    state
        .output_rate
        .store(configuration.frame_rate_hz, Ordering::Relaxed);
    state
        .timecode_router
        .lock()
        .configure(configuration.timecode_sources.clone());
    let requires_restart = configuration.output_bind_ip != previous.output_bind_ip
        || configuration.osc_bind != previous.osc_bind
        || configuration.art_timecode_bind != previous.art_timecode_bind
        || configuration.midi_inputs != previous.midi_inputs
        || configuration.rtp_midi_bind != previous.rtp_midi_bind;
    *state.configuration.write() = configuration.clone();
    if !configuration.patch_preview_highlight_dmx {
        state.patch_preview_highlights.lock().clear();
        sync_highlight_output(&state);
    }
    persist_server_configuration(&state)?;
    refresh_speed_group_engine(&state);
    let matter = refresh_matter_bridge(&state);
    emit(
        &state,
        "server_configuration_changed",
        serde_json::json!({"configuration":configuration,"requires_restart":requires_restart,"matter":&matter}),
    );
    Ok(Json(
        serde_json::json!({"configuration":configuration,"requires_restart":requires_restart,"matter":matter}),
    ))
}
async fn create_session(
    State(state): State<AppState>,
    Json(input): Json<CreateSession>,
) -> Result<Json<SessionResponse>, ApiError> {
    let client_id = input
        .client_id
        .or_else(|| {
            input.desk_id.and_then(|desk_id| {
                state
                    .desk
                    .lock()
                    .client_desks()
                    .ok()?
                    .into_iter()
                    .find(|entry| entry.desk.id == desk_id)?
                    .client_id
            })
        })
        .unwrap_or_else(Uuid::new_v4);
    let user = state
        .desk
        .lock()
        .find_user(&input.username)
        .map_err(ApiError::store)?
        .filter(|u| u.enabled)
        .ok_or_else(|| ApiError::not_found("enabled user"))?;
    let desk = state
        .desk
        .lock()
        .resolve_client_desk(client_id, input.desk_id)
        .map_err(ApiError::store)?;
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: Uuid::new_v4().to_string(),
        connected: true,
        desk: desk.clone(),
    };
    state.session_clients.write().insert(session.id, client_id);
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());
    persist_programmer(&state, &session)?;
    emit(
        &state,
        "session_started",
        serde_json::json!({"session_id":session.id,"user":user.name}),
    );
    Ok(Json(SessionResponse {
        session_id: session.id,
        client_id,
        token: session.token,
        user,
        desk,
    }))
}
async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UserInput>,
) -> Result<(StatusCode, Json<DeskUser>), ApiError> {
    let _session = authenticate(&state, &headers)?;
    let mut user = state
        .desk
        .lock()
        .add_user(&input.name)
        .map_err(ApiError::store)?;
    if !input.enabled {
        user = state
            .desk
            .lock()
            .update_user(user.id, &user.name, false)
            .map_err(ApiError::store)?;
    }
    emit(
        &state,
        "desk_user_changed",
        serde_json::json!({"user":user}),
    );
    Ok((StatusCode::CREATED, Json(user)))
}
async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UserInput>,
) -> Result<Json<DeskUser>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let user = state
        .desk
        .lock()
        .update_user(light_core::UserId(id), &input.name, input.enabled)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "desk_user_changed",
        serde_json::json!({"user":user}),
    );
    Ok(Json(user))
}
async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let session = authenticate(&state, &headers)?;
    let id = light_core::UserId(id);
    if session.user.id == id {
        return Err(ApiError::conflict(
            "the current session cannot delete its own user",
        ));
    }
    if !state.desk.lock().delete_user(id).map_err(ApiError::store)? {
        return Err(ApiError::not_found("user"));
    }
    state.highlight.clear_user(id);
    sync_highlight_output(&state);
    emit(
        &state,
        "desk_user_deleted",
        serde_json::json!({"user_id":id}),
    );
    Ok(StatusCode::NO_CONTENT)
}
async fn close_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let id = SessionId(id);
    let caller = authenticate(&state, &headers)?;
    if caller.id != id {
        return Err(ApiError::conflict("a session may only disconnect itself"));
    }
    let Some(session) = state.sessions.write().remove(&id) else {
        return Err(ApiError::not_found("session"));
    };
    if let Some(client_id) = state.session_clients.write().remove(&id) {
        state
            .desk
            .lock()
            .touch_client(client_id)
            .map_err(ApiError::store)?;
    }
    let same_context_connected = state.sessions.read().values().any(|candidate| {
        candidate.user.id == session.user.id && candidate.desk.id == session.desk.id
    });
    if !same_context_connected {
        state
            .highlight
            .clear_context(session.desk.id, session.user.id);
        sync_highlight_output(&state);
    }
    state.patch_preview_highlights.lock().remove(&id);
    sync_highlight_output(&state);
    file_manager::release_session_input(&state, &session, "session_closed");
    persist_programmer(&state, &session)?;
    state.programmers.disconnect(id);
    emit(
        &state,
        "session_disconnected",
        serde_json::json!({"session_id":id}),
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_client(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let caller = authenticate(&state, &headers)?;
    let target = state
        .desk
        .lock()
        .client_desks()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.desk.id == id)
        .ok_or_else(|| ApiError::not_found("client"))?;
    let target_client_id = target.client_id.unwrap_or(target.desk.id);
    let caller_client_id = state.session_clients.read().get(&caller.id).copied();
    if caller_client_id == Some(target_client_id) || caller.desk.id == id {
        return Err(ApiError::conflict(
            "the current client cannot remove itself",
        ));
    }
    let sessions = state.sessions.read();
    let session_clients = state.session_clients.read();
    if sessions.values().any(|session| {
        session_clients.get(&session.id) == Some(&target_client_id)
            || session.desk.id == target.desk.id
    }) {
        return Err(ApiError::conflict(
            "an actively connected client cannot be removed",
        ));
    }
    drop(sessions);
    drop(session_clients);
    if !state
        .desk
        .lock()
        .remove_client_desk(id)
        .map_err(ApiError::store)?
    {
        return Err(ApiError::not_found("client"));
    }
    state
        .configuration
        .write()
        .update_settings_by_desk
        .remove(&id);
    state.highlight.clear_desk(id);
    sync_highlight_output(&state);
    emit(
        &state,
        "client_removed",
        serde_json::json!({"client_id":target_client_id,"desk_id":id}),
    );
    Ok(StatusCode::NO_CONTENT)
}
async fn list_shows(State(state): State<AppState>) -> Result<Json<Vec<ShowEntry>>, ApiError> {
    Ok(Json(state.desk.lock().library().map_err(ApiError::store)?))
}
async fn list_show_revisions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<ShowRevision>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let id = light_core::ShowId(id);
    if state
        .desk
        .lock()
        .show(id)
        .map_err(ApiError::store)?
        .is_none()
    {
        return Err(ApiError::not_found("show"));
    }
    Ok(Json(
        state
            .desk
            .lock()
            .show_revisions(id)
            .map_err(ApiError::store)?,
    ))
}
async fn save_show_revision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<SaveShowRevision>,
) -> Result<(StatusCode, Json<ShowRevision>), ApiError> {
    let _session = authenticate(&state, &headers)?;
    let id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    if input.name.trim().is_empty() || input.name.trim().len() > 120 {
        return Err(ApiError::bad_request(
            "revision name must contain 1-120 characters",
        ));
    }
    let directory = state
        .data_dir
        .join("revisions")
        .join(entry.id.0.to_string());
    std::fs::create_dir_all(&directory).map_err(ApiError::io)?;
    let destination = directory.join(format!("{}.show", Uuid::new_v4()));
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .backup_to(&destination)
        .map_err(ApiError::store)?;
    let revision = match state.desk.lock().add_show_revision(
        entry.id,
        input.name.trim(),
        &destination.display().to_string(),
    ) {
        Ok(revision) => revision,
        Err(error) => {
            let _ = std::fs::remove_file(destination);
            return Err(ApiError::store(error));
        }
    };
    emit(
        &state,
        "show_revision_saved",
        serde_json::json!({"show_id":entry.id,"revision":revision}),
    );
    Ok((StatusCode::CREATED, Json(revision)))
}
async fn open_show_revision(
    State(state): State<AppState>,
    Path((id, revision)): Path<(Uuid, u64)>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let saved_revision = state
        .desk
        .lock()
        .show_revision(id, revision)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show revision"))?;
    if !FsPath::new(&saved_revision.path).exists() {
        return Err(ApiError::bad_request("saved show revision is unavailable"));
    }
    validate_show_file(&saved_revision.path).map_err(ApiError::store)?;
    let revision_entry = ShowEntry {
        path: saved_revision.path.clone(),
        ..entry.clone()
    };
    let compiled = load_engine_snapshot(&revision_entry).map_err(ApiError::internal)?;
    compiled
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let copied_at = chrono::Utc::now();
    let revision_copy = RevisionCopySource {
        show_id: entry.id,
        show_name: entry.name.clone(),
        revision: saved_revision.revision,
        revision_name: saved_revision.name.clone(),
        copied_at: copied_at.to_rfc3339(),
    };
    let copy_name = revision_copy_name(&state, &entry.name, revision, copied_at.date_naive())?;
    let copy_path = state
        .data_dir
        .join("shows")
        .join(format!("{copy_name}.show"));
    ShowStore::open(&saved_revision.path)
        .map_err(ApiError::store)?
        .backup_to(&copy_path)
        .map_err(ApiError::store)?;
    let copy = match state.desk.lock().upsert_show_with_revision_copy(
        &copy_name,
        &copy_path.display().to_string(),
        false,
        Some(&revision_copy),
    ) {
        Ok(copy) => copy,
        Err(error) => {
            let _ = std::fs::remove_file(&copy_path);
            return Err(ApiError::store(error));
        }
    };
    if let Err(error) = ShowStore::open(&copy.path)
        .and_then(|store| store.set_identity(copy.id, &copy.name, copy.revision_copy.as_ref()))
    {
        let _ = state.desk.lock().remove_show(copy.id);
        let _ = std::fs::remove_file(&copy_path);
        return Err(ApiError::store(error));
    }
    let _activation = state.activation_lock.lock().await;
    let previous = state.active_show.read().clone();
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    if let Err(error) =
        activate_snapshot(&state, compiled, &transition, input.transition_millis).await
    {
        let _ = state.desk.lock().remove_show(copy.id);
        let _ = std::fs::remove_file(&copy_path);
        return Err(error);
    }
    state
        .desk
        .lock()
        .set_active_show(Some(copy.id))
        .map_err(ApiError::store)?;
    if let Some(previous) = &previous
        && previous.id != copy.id
    {
        state
            .desk
            .lock()
            .set_setting("previous_active_show_id", &previous.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    *state.active_show.write() = Some(copy.clone());
    *state.active_show_error.write() = None;
    emit(
        &state,
        "show_opened",
        serde_json::json!({"show":copy,"revision_copy":revision_copy,"transition":transition}),
    );
    Ok(Json(copy))
}
async fn upload_show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UploadShow>,
) -> Result<(StatusCode, Json<ShowEntry>), ApiError> {
    let _session = authenticate(&state, &headers)?;
    validate_show_name(&input.name)?;
    let path = state
        .data_dir
        .join("shows")
        .join(format!("{}.show", input.name));
    let mut uploaded_revision_copy = None;
    if let Some(data) = input.data_base64 {
        let bytes = STANDARD
            .decode(data)
            .map_err(|_| ApiError::bad_request("data_base64 is invalid"))?;
        if bytes.len() < 100 || !bytes.starts_with(b"SQLite format 3\0") {
            return Err(ApiError::bad_request(
                "uploaded show is not a SQLite database",
            ));
        }
        let staged = state
            .data_dir
            .join("shows")
            .join(format!(".upload-{}.tmp", Uuid::new_v4()));
        std::fs::write(&staged, bytes).map_err(ApiError::io)?;
        if let Err(error) = validate_show_file(&staged) {
            let _ = std::fs::remove_file(&staged);
            return Err(ApiError::store(error));
        }
        uploaded_revision_copy = ShowStore::open(&staged)
            .map_err(ApiError::store)?
            .revision_copy_source()
            .map_err(ApiError::store)?;
        if path.exists() && !input.overwrite {
            let _ = std::fs::remove_file(&staged);
            return Err(ApiError::conflict("a show with that name already exists"));
        }
        if path.exists()
            && let Some(existing) = state
                .desk
                .lock()
                .library()
                .map_err(ApiError::store)?
                .into_iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(&input.name))
        {
            backup_show(&state, &existing)?;
        }
        std::fs::rename(&staged, &path).map_err(ApiError::io)?;
    } else if !path.exists() {
        if input.name == default_show::name() {
            default_show::initialise(&path).map_err(ApiError::store)?;
        } else {
            initialise_show(&path, &input.name).map_err(ApiError::store)?;
        }
    }
    let entry = state
        .desk
        .lock()
        .upsert_show_with_revision_copy(
            &input.name,
            &path.display().to_string(),
            input.overwrite,
            uploaded_revision_copy.as_ref(),
        )
        .map_err(ApiError::store)?;
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .set_identity(entry.id, &entry.name, entry.revision_copy.as_ref())
        .map_err(ApiError::store)?;
    emit(&state, "show_uploaded", serde_json::json!({"show":entry}));
    Ok((StatusCode::CREATED, Json(entry)))
}
async fn rename_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RenameShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let name = input.name.trim();
    validate_show_name(name)?;
    let id = light_core::ShowId(id);
    let current = state
        .active_show
        .read()
        .clone()
        .filter(|show| show.id == id)
        .ok_or_else(|| ApiError::conflict("only the active show can be renamed"))?;
    if current.name == name {
        return Ok(Json(current));
    }
    if state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .any(|show| show.id != id && show.name.eq_ignore_ascii_case(name))
    {
        return Err(ApiError::conflict("a show with that name already exists"));
    }

    let destination = state.data_dir.join("shows").join(format!("{name}.show"));
    if destination.exists() {
        return Err(ApiError::conflict(
            "a show file with that name already exists",
        ));
    }
    let staged = state
        .data_dir
        .join("shows")
        .join(format!(".rename-{}.tmp", Uuid::new_v4()));
    let stage_result = ShowStore::open(&current.path)
        .and_then(|store| store.backup_to(&staged))
        .and_then(|_| ShowStore::open(&staged))
        .and_then(|store| store.set_identity(current.id, name, current.revision_copy.as_ref()))
        .and_then(|_| validate_show_file(&staged).map(|_| ()));
    if let Err(error) = stage_result {
        let _ = std::fs::remove_file(&staged);
        return Err(ApiError::store(error));
    }
    if let Err(error) = std::fs::rename(&staged, &destination) {
        let _ = std::fs::remove_file(&staged);
        return Err(ApiError::io(error));
    }
    let renamed =
        match state
            .desk
            .lock()
            .rename_show(current.id, name, &destination.display().to_string())
        {
            Ok(entry) => entry,
            Err(error) => {
                let _ = std::fs::remove_file(&destination);
                return Err(ApiError::store(error));
            }
        };
    *state.active_show.write() = Some(renamed.clone());
    if let Err(error) = std::fs::remove_file(&current.path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(path=%current.path, %error, "renamed show retained its superseded file");
    }
    emit(
        &state,
        "show_renamed",
        serde_json::json!({"previous_name":current.name,"show":renamed}),
    );
    Ok(Json(renamed))
}
async fn overwrite_show(
    State(state): State<AppState>,
    Path((source_id, destination_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if source_id == destination_id {
        return Err(ApiError::bad_request(
            "source and overwrite destination must be different shows",
        ));
    }
    if state
        .active_show
        .read()
        .as_ref()
        .is_none_or(|show| show.id.0 != source_id)
    {
        return Err(ApiError::conflict(
            "only the active show can be saved over another show",
        ));
    }
    let (source, destination) = {
        let desk = state.desk.lock();
        let source = desk
            .show(light_core::ShowId(source_id))
            .map_err(ApiError::store)?
            .ok_or_else(|| ApiError::not_found("source show"))?;
        let destination = desk
            .show(light_core::ShowId(destination_id))
            .map_err(ApiError::store)?
            .ok_or_else(|| ApiError::not_found("overwrite destination"))?;
        (source, destination)
    };
    let staged = state
        .data_dir
        .join("shows")
        .join(format!(".overwrite-{}.tmp", Uuid::new_v4()));
    let stage_result = ShowStore::open(&source.path)
        .and_then(|store| store.backup_to(&staged))
        .and_then(|_| ShowStore::open(&staged))
        .and_then(|store| {
            store.set_identity(
                destination.id,
                &destination.name,
                destination.revision_copy.as_ref(),
            )
        })
        .and_then(|_| validate_show_file(&staged).map(|_| ()));
    if let Err(error) = stage_result {
        let _ = std::fs::remove_file(&staged);
        return Err(ApiError::store(error));
    }
    if let Err(error) = backup_show(&state, &destination) {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&staged, &destination.path) {
        let _ = std::fs::remove_file(&staged);
        return Err(ApiError::io(error));
    }
    let destination = state
        .desk
        .lock()
        .mark_show_updated(destination.id)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "show_overwritten",
        serde_json::json!({"source_show":source,"destination_show":destination}),
    );
    Ok(Json(destination))
}
async fn delete_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let id = light_core::ShowId(id);
    if state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|show| show.id == id)
    {
        return Err(ApiError::conflict("the active show cannot be deleted"));
    }
    let entry = state
        .desk
        .lock()
        .show(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let revisions = state
        .desk
        .lock()
        .show_revisions(id)
        .map_err(ApiError::store)?;
    state.desk.lock().remove_show(id).map_err(ApiError::store)?;
    if let Err(error) = std::fs::remove_file(&entry.path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(ApiError::io(error));
    }
    for revision in revisions {
        if let Err(error) = std::fs::remove_file(revision.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(ApiError::io(error));
        }
    }
    emit(&state, "show_deleted", serde_json::json!({"show_id":id}));
    Ok(StatusCode::NO_CONTENT)
}
async fn open_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let entry = state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.id.0 == id)
        .ok_or_else(|| ApiError::not_found("show"))?;
    if !FsPath::new(&entry.path).exists() {
        return Err(ApiError::bad_request("show file is unavailable"));
    }
    validate_show_file(&entry.path).map_err(ApiError::store)?;
    let compiled = load_engine_snapshot(&entry).map_err(ApiError::internal)?;
    compiled
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let _activation = state.activation_lock.lock().await;
    let previous = state.active_show.read().clone();
    if let Some(previous) = &previous {
        state
            .desk
            .lock()
            .set_setting("previous_active_show_id", &previous.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    activate_snapshot(&state, compiled, &transition, input.transition_millis).await?;
    state
        .desk
        .lock()
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    *state.active_show.write() = Some(entry.clone());
    *state.active_show_error.write() = None;
    emit(
        &state,
        "show_opened",
        serde_json::json!({"show":entry,"transition":transition,"previous_show":previous}),
    );
    Ok(Json(entry))
}
async fn open_clean_default_show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let name = available_show_name(&state, "Default Stage Show Clean Copy")?;
    let path = state.data_dir.join("shows").join(format!("{name}.show"));
    default_show::initialise(&path).map_err(ApiError::store)?;
    let entry = match state
        .desk
        .lock()
        .upsert_show(&name, &path.display().to_string(), false)
    {
        Ok(entry) => entry,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(ApiError::store(error));
        }
    };
    if let Err(error) =
        ShowStore::open(&path).and_then(|store| store.set_identity(entry.id, &entry.name, None))
    {
        let _ = state.desk.lock().remove_show(entry.id);
        let _ = std::fs::remove_file(&path);
        return Err(ApiError::store(error));
    }
    let compiled = match load_engine_snapshot(&entry) {
        Ok(compiled) => compiled,
        Err(error) => {
            let _ = state.desk.lock().remove_show(entry.id);
            let _ = std::fs::remove_file(&path);
            return Err(ApiError::internal(error));
        }
    };
    compiled
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let _activation = state.activation_lock.lock().await;
    let previous = state.active_show.read().clone();
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    activate_snapshot(&state, compiled, &transition, input.transition_millis).await?;
    state
        .desk
        .lock()
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    if let Some(previous) = &previous {
        state
            .desk
            .lock()
            .set_setting("previous_active_show_id", &previous.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    *state.active_show.write() = Some(entry.clone());
    *state.active_show_error.write() = None;
    emit(
        &state,
        "show_opened",
        serde_json::json!({"show":entry,"transition":transition,"previous_show":previous,"source":"built_in_default"}),
    );
    Ok(Json(entry))
}
async fn rollback_show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<OpenShow>,
) -> Result<Json<ShowEntry>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let previous_id = state
        .desk
        .lock()
        .setting("previous_active_show_id")
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("previous active show"))?;
    let previous_id = light_core::ShowId(
        Uuid::parse_str(&previous_id)
            .map_err(|_| ApiError::bad_request("stored rollback show ID is invalid"))?,
    );
    let entry = state
        .desk
        .lock()
        .show(previous_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("rollback show"))?;
    let compiled = load_engine_snapshot(&entry).map_err(ApiError::internal)?;
    compiled
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let _activation = state.activation_lock.lock().await;
    let current = state.active_show.read().clone();
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    activate_snapshot(&state, compiled, &transition, input.transition_millis).await?;
    state
        .desk
        .lock()
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    if let Some(current) = current {
        state
            .desk
            .lock()
            .set_setting("previous_active_show_id", &current.id.0.to_string())
            .map_err(ApiError::store)?;
    }
    *state.active_show.write() = Some(entry.clone());
    *state.active_show_error.write() = None;
    emit(
        &state,
        "show_rolled_back",
        serde_json::json!({"show":entry,"transition":transition}),
    );
    Ok(Json(entry))
}
async fn download_show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let entry = state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.id.0 == id)
        .ok_or_else(|| ApiError::not_found("show"))?;
    let export = state
        .data_dir
        .join(format!(".export-{}.show", Uuid::new_v4()));
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .backup_to(&export)
        .map_err(ApiError::store)?;
    let data = std::fs::read(&export).map_err(ApiError::io)?;
    let _ = std::fs::remove_file(export);
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.light.show"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}.show\"", entry.name),
            ),
        ],
        data,
    )
        .into_response())
}

async fn preview_mvr_import(
    State(state): State<AppState>,
    Query(query): Query<MvrPreviewQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<MvrImportPreview>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let document =
        light_mvr::read(&body).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let (definitions, _) = mvr_definitions(&state, &document)?;
    let mut existing = Vec::new();
    if let Some(id) = query.show_id
        && let Some(show) = state
            .desk
            .lock()
            .show(light_core::ShowId(id))
            .map_err(ApiError::store)?
    {
        existing = ShowStore::open(show.path)
            .map_err(ApiError::store)?
            .objects("patched_fixture")
            .map_err(ApiError::store)?
            .into_iter()
            .filter_map(|o| serde_json::from_value::<light_fixture::PatchedFixture>(o.body).ok())
            .collect();
    }
    let missing_profiles = document
        .fixtures
        .iter()
        .filter(|f| resolve_mvr_definition(&definitions, f).is_none())
        .map(|f| format!("{} · {}", f.gdtf_spec, f.gdtf_mode))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut address_conflicts = Vec::new();
    for fixture in &document.fixtures {
        if let (Some(u), Some(a), Some(definition)) = (
            fixture.universe,
            fixture.address,
            resolve_mvr_definition(&definitions, fixture),
        ) {
            let end = a.saturating_add(definition.footprint.saturating_sub(1));
            if existing.iter().any(|e| {
                e.universe == Some(u)
                    && e.address.is_some_and(|start| {
                        start <= end
                            && start.saturating_add(e.definition.footprint.saturating_sub(1)) >= a
                    })
            }) {
                address_conflicts.push(format!(
                    "{} conflicts at universe {} address {}-{}",
                    fixture.name, u, a, end
                ));
            }
        }
    }
    let token = Uuid::new_v4();
    let now = Instant::now();
    let mut imports = state.mvr_imports.lock();
    imports.retain(|_, item| now.duration_since(item.created) < Duration::from_secs(30 * 60));
    imports.insert(
        token,
        StagedMvrImport {
            document: document.clone(),
            created: now,
        },
    );
    Ok(Json(MvrImportPreview {
        token,
        fixtures: document
            .fixtures
            .iter()
            .map(|f| MvrPreviewFixture {
                uuid: f.uuid,
                name: f.name.clone(),
                gdtf_spec: f.gdtf_spec.clone(),
                gdtf_mode: f.gdtf_mode.clone(),
                universe: f.universe,
                address: f.address,
                matched: resolve_mvr_definition(&definitions, f).is_some(),
            })
            .collect(),
        scenery: document.geometry.len(),
        missing_profiles,
        warnings: address_conflicts.clone(),
        address_conflicts,
    }))
}

fn resolve_mvr_definition(
    definitions: &[light_fixture::FixtureDefinition],
    fixture: &light_mvr::MvrFixture,
) -> Option<light_fixture::FixtureDefinition> {
    let spec = fixture
        .gdtf_spec
        .rsplit('/')
        .next()
        .unwrap_or(&fixture.gdtf_spec)
        .trim_end_matches(".gdtf");
    definitions
        .iter()
        .find(|d| {
            d.mode.eq_ignore_ascii_case(&fixture.gdtf_mode)
                && (d.model.eq_ignore_ascii_case(spec)
                    || d.name.eq_ignore_ascii_case(spec)
                    || format!("{}@{}", d.manufacturer, d.model).eq_ignore_ascii_case(spec))
        })
        .cloned()
}

type MvrDefinitions = (
    Vec<light_fixture::FixtureDefinition>,
    Vec<(light_fixture::FixtureDefinition, Vec<u8>)>,
);

fn mvr_definitions(
    state: &AppState,
    document: &light_mvr::MvrDocument,
) -> Result<MvrDefinitions, ApiError> {
    let mut definitions = state
        .fixture_library
        .lock()
        .definitions()
        .map_err(ApiError::fixture)?;
    let mut imported = Vec::new();
    for fixture in &document.fixtures {
        if resolve_mvr_definition(&definitions, fixture).is_some() {
            continue;
        }
        let name = fixture.gdtf_spec.to_ascii_lowercase();
        let Some(bytes) = document.files.get(&name).or_else(|| {
            document
                .files
                .iter()
                .find(|(path, _)| path.ends_with(&format!("/{name}")))
                .map(|(_, data)| data)
        }) else {
            continue;
        };
        let Ok(modes) = light_mvr::read_gdtf(bytes) else {
            continue;
        };
        for mode in modes {
            let footprint = mode
                .channels
                .iter()
                .flat_map(|c| c.offsets.iter())
                .max()
                .copied()
                .unwrap_or(0)
                + 1;
            let parameters = mode
                .channels
                .into_iter()
                .map(|channel| {
                    let normalized = channel
                        .attribute
                        .replace([' ', '_'], ".")
                        .to_ascii_lowercase();
                    light_fixture::Parameter {
                        attribute: light_core::AttributeKey(normalized.clone()),
                        components: channel
                            .offsets
                            .into_iter()
                            .map(|offset| light_fixture::ChannelComponent {
                                offset,
                                byte_order: light_fixture::ByteOrder::MsbFirst,
                            })
                            .collect(),
                        default: 0.0,
                        virtual_dimmer: false,
                        metadata: light_fixture::ParameterMetadata {
                            wrap: normalized.contains("pan"),
                            ..Default::default()
                        },
                        capabilities: Vec::new(),
                    }
                })
                .collect();
            let definition = light_fixture::FixtureDefinition {
                schema_version: 1,
                id: light_core::FixtureId::new(),
                revision: 1,
                manufacturer: mode.manufacturer,
                device_type: "other".into(),
                name: mode.model.clone(),
                model: mode.model,
                mode: mode.name,
                footprint,
                heads: vec![light_fixture::LogicalHead {
                    index: 0,
                    name: "Main".into(),
                    shared: true,
                    parameters,
                }],
                color_calibration: None,
                physical: Default::default(),
                model_asset: None,
                icon_asset: None,
                hazardous: false,
                direct_control_protocols: Vec::new(),
                signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
                safe_values: Default::default(),
                profile_id: None,
                mode_id: None,
                profile_snapshot: None,
            };
            definitions.push(definition.clone());
            imported.push((definition, bytes.clone()));
        }
    }
    Ok((definitions, imported))
}

fn mvr_transform(
    matrix: [f64; 12],
) -> (light_fixture::FixtureLocation, light_fixture::FixtureVector) {
    let location = light_fixture::FixtureLocation {
        x: matrix[9]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        y: matrix[10]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        z: matrix[11]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
    };
    let rotation = light_fixture::FixtureVector {
        x: (matrix[9].atan2(matrix[10]).to_degrees()) as f32,
        y: (-matrix[8].asin().to_degrees()) as f32,
        z: (matrix[4].atan2(matrix[0]).to_degrees()) as f32,
    };
    (location, rotation)
}

fn apply_mvr_to_store(
    store: &ShowStore,
    document: &light_mvr::MvrDocument,
    definitions: &[light_fixture::FixtureDefinition],
    resolutions: &HashMap<Uuid, MvrResolution>,
) -> Result<(usize, usize, Vec<String>), ApiError> {
    let existing_objects = store.objects("patched_fixture").map_err(ApiError::store)?;
    let mut occupied: Vec<(u16, u16, u16, String)> = existing_objects
        .iter()
        .filter_map(|o| {
            serde_json::from_value::<light_fixture::PatchedFixture>(o.body.clone())
                .ok()
                .and_then(|f| {
                    Some((
                        f.universe?,
                        f.address?,
                        f.definition.footprint,
                        o.id.clone(),
                    ))
                })
        })
        .collect();
    let metadata = store.objects("mvr_fixture").map_err(ApiError::store)?;
    let ids: HashMap<Uuid, String> = metadata
        .iter()
        .filter_map(|o| {
            Uuid::parse_str(&o.id).ok().and_then(|uuid| {
                o.body
                    .get("fixture_id")?
                    .as_str()
                    .map(|id| (uuid, id.to_owned()))
            })
        })
        .collect();
    let mut imported = 0;
    let mut unresolved = 0;
    let mut warnings = Vec::new();
    for source in &document.fixtures {
        if matches!(resolutions.get(&source.uuid), Some(MvrResolution::Skip)) {
            continue;
        }
        let Some(definition) = resolve_mvr_definition(definitions, source) else {
            let current = store
                .objects("unresolved_mvr_fixture")
                .map_err(ApiError::store)?
                .into_iter()
                .find(|o| o.id == source.uuid.to_string())
                .map(|o| o.revision)
                .unwrap_or(0);
            store
                .put_object(
                    "unresolved_mvr_fixture",
                    &source.uuid.to_string(),
                    &serde_json::to_value(source)
                        .map_err(|e| ApiError::bad_request(e.to_string()))?,
                    current,
                )
                .map_err(ApiError::store)?;
            unresolved += 1;
            warnings.push(format!(
                "{} requires {} mode {}",
                source.name, source.gdtf_spec, source.gdtf_mode
            ));
            continue;
        };
        let fixture_id = ids
            .get(&source.uuid)
            .and_then(|id| Uuid::parse_str(id).ok())
            .map(light_core::FixtureId)
            .unwrap_or_default();
        let (location, rotation) = mvr_transform(source.matrix);
        let mut universe = source.universe;
        let mut address = source.address;
        if let Some(MvrResolution::Address {
            universe: u,
            address: a,
        }) = resolutions.get(&source.uuid)
        {
            universe = Some(*u);
            address = Some(*a);
        }
        if matches!(
            resolutions.get(&source.uuid),
            Some(MvrResolution::ImportUnpatched)
        ) {
            universe = None;
            address = None;
        }
        if let (Some(u), Some(a)) = (universe, address) {
            let end = a.saturating_add(definition.footprint.saturating_sub(1));
            let conflict = occupied
                .iter()
                .find(|(eu, ea, ef, id)| {
                    *eu == u
                        && *id != fixture_id.0.to_string()
                        && *ea <= end
                        && ea.saturating_add(ef.saturating_sub(1)) >= a
                })
                .cloned();
            if let Some((_, _, _, id)) = conflict {
                if matches!(resolutions.get(&source.uuid), Some(MvrResolution::Replace)) {
                    store
                        .delete_object("patched_fixture", &id)
                        .map_err(ApiError::store)?;
                    occupied.retain(|item| item.3 != id);
                } else {
                    universe = None;
                    address = None;
                    warnings.push(format!(
                        "{} imported unpatched because its requested address conflicts",
                        source.name
                    ));
                }
            }
        }
        let heads = definition
            .heads
            .iter()
            .filter(|h| !h.shared)
            .map(|h| light_fixture::PatchedHead {
                head_index: h.index,
                fixture_id: light_core::FixtureId::new(),
            })
            .collect();
        let existing_mib = existing_objects
            .iter()
            .find(|object| object.id == fixture_id.0.to_string())
            .and_then(|object| {
                serde_json::from_value::<light_fixture::PatchedFixture>(object.body.clone()).ok()
            })
            .map(|fixture| {
                (
                    fixture.move_in_black_enabled,
                    fixture.move_in_black_delay_millis,
                )
            });
        let patched = light_fixture::PatchedFixture {
            fixture_id,
            fixture_number: source
                .fixture_id
                .as_deref()
                .and_then(|value| value.parse().ok()),
            virtual_fixture_number: None,
            name: source.name.clone(),
            definition: definition.clone(),
            universe,
            address,
            split_patches: Vec::new(),
            layer_id: source.layer.clone().unwrap_or_else(|| "default".into()),
            direct_control: None,
            location,
            rotation,
            logical_heads: heads,
            // MIB is show-local extension data. Reimporting the same MVR fixture updates its
            // exchange fields without silently resetting the operator's safety settings.
            move_in_black_enabled: existing_mib.is_none_or(|settings| settings.0),
            move_in_black_delay_millis: existing_mib.map_or(0, |settings| settings.1),
            highlight_overrides: Default::default(),
            multipatch: Vec::new(),
        };
        let id = fixture_id.0.to_string();
        let current = existing_objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.revision)
            .unwrap_or(0);
        store
            .put_object(
                "patched_fixture",
                &id,
                &serde_json::to_value(&patched)
                    .map_err(|e| ApiError::bad_request(e.to_string()))?,
                current,
            )
            .map_err(ApiError::store)?;
        let meta_current = metadata
            .iter()
            .find(|o| o.id == source.uuid.to_string())
            .map(|o| o.revision)
            .unwrap_or(0);
        store.put_object("mvr_fixture",&source.uuid.to_string(),&serde_json::json!({"fixture_id":id,"gdtf_spec":source.gdtf_spec,"gdtf_mode":source.gdtf_mode}),meta_current).map_err(ApiError::store)?;
        if let (Some(u), Some(a)) = (universe, address) {
            occupied.push((u, a, definition.footprint, id));
        }
        imported += 1;
    }
    if !document.geometry.is_empty() {
        warnings.push(
            "MVR scene geometry was not imported. Add scenery from the Venue fixture library in Show Patch."
                .into(),
        );
    }
    Ok((imported, unresolved, warnings))
}

async fn apply_mvr_import(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ApplyMvrImport>,
) -> Result<Json<ApplyMvrResult>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let staged = state
        .mvr_imports
        .lock()
        .remove(&token)
        .ok_or_else(|| ApiError::not_found("MVR import preview"))?;
    if staged.created.elapsed() > Duration::from_secs(30 * 60) {
        return Err(ApiError::bad_request("MVR import preview expired"));
    }
    if input.new_show.is_some() == input.existing_show_id.is_some() {
        return Err(ApiError::bad_request(
            "choose exactly one MVR import destination",
        ));
    }
    let (entry, is_new, open_after) = if let Some(new) = input.new_show {
        validate_show_name(&new.name)?;
        let path = state
            .data_dir
            .join("shows")
            .join(format!("{}.show", new.name));
        if path.exists() {
            return Err(ApiError::conflict("a show with that name already exists"));
        }
        initialise_show(&path, &new.name).map_err(ApiError::store)?;
        (
            state
                .desk
                .lock()
                .upsert_show(&new.name, &path.display().to_string(), false)
                .map_err(ApiError::store)?,
            true,
            new.open_after_import,
        )
    } else {
        let id = light_core::ShowId(input.existing_show_id.unwrap());
        (
            state
                .desk
                .lock()
                .show(id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("show"))?,
            false,
            false,
        )
    };
    let temporary = state
        .data_dir
        .join("shows")
        .join(format!(".mvr-{}.show", Uuid::new_v4()));
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .backup_to(&temporary)
        .map_err(ApiError::store)?;
    let (definitions, new_definitions) = mvr_definitions(&state, &staged.document)?;
    let result = (|| {
        let store = ShowStore::open(&temporary).map_err(ApiError::store)?;
        let applied =
            apply_mvr_to_store(&store, &staged.document, &definitions, &input.resolutions)?;
        validate_show_file(&temporary).map_err(ApiError::store)?;
        let probe = ShowEntry {
            path: temporary.display().to_string(),
            ..entry.clone()
        };
        load_engine_snapshot(&probe)
            .map_err(ApiError::bad_request)?
            .validate()
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        Ok::<_, ApiError>(applied)
    })();
    let (imported, unresolved, warnings) = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            if is_new {
                let _ = state.desk.lock().remove_show(entry.id);
                let _ = std::fs::remove_file(&entry.path);
            }
            return Err(e);
        }
    };
    if !is_new {
        backup_show(&state, &entry)?;
    }
    std::fs::rename(&temporary, &entry.path).map_err(ApiError::io)?;
    for (definition, source) in new_definitions {
        let json =
            serde_json::to_string(&definition).map_err(|e| ApiError::internal(e.to_string()))?;
        state
            .fixture_library
            .lock()
            .import_json_with_source(&json, Some(&source))
            .map_err(ApiError::fixture)?;
    }
    let should_open = open_after
        || state
            .active_show
            .read()
            .as_ref()
            .is_some_and(|s| s.id == entry.id);
    if should_open {
        let compiled = load_engine_snapshot(&entry).map_err(ApiError::bad_request)?;
        let _lock = state.activation_lock.lock().await;
        activate_snapshot(&state, compiled, &Transition::HoldCurrent, None).await?;
        state
            .desk
            .lock()
            .set_active_show(Some(entry.id))
            .map_err(ApiError::store)?;
        *state.active_show.write() = Some(entry.clone());
    }
    emit(
        &state,
        "mvr_imported",
        serde_json::json!({"show":entry,"fixtures":imported,"unresolved":unresolved,"scenery":0}),
    );
    Ok(Json(ApplyMvrResult {
        show: entry,
        imported_fixtures: imported,
        unresolved_fixtures: unresolved,
        imported_scenery: 0,
        opened: should_open,
        warnings,
    }))
}

fn build_mvr_export(
    state: &AppState,
    id: Uuid,
) -> Result<(ShowEntry, light_mvr::MvrDocument, MvrExportPreview), ApiError> {
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let metas: HashMap<String, serde_json::Value> = store
        .objects("mvr_fixture")
        .map_err(ApiError::store)?
        .into_iter()
        .filter_map(|o| {
            let id = o.body.get("fixture_id")?.as_str()?.to_owned();
            Some((id, o.body))
        })
        .collect();
    let fixtures = store
        .objects("patched_fixture")
        .map_err(ApiError::store)?
        .into_iter()
        .filter_map(|o| {
            serde_json::from_value::<light_fixture::PatchedFixture>(o.body)
                .ok()
                .map(|f| (o.id, f))
        })
        .collect::<Vec<_>>();
    let mut doc = light_mvr::MvrDocument::default();
    let mut missing = Vec::new();
    let mut embedded = 0;
    for (id, f) in &fixtures {
        let meta = metas.get(id);
        let gdtf = meta
            .and_then(|m| m.get("gdtf_spec"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!("{}@{}.gdtf", f.definition.manufacturer, f.definition.model)
            });
        if let Some(source) = state
            .fixture_library
            .lock()
            .source_gdtf(f.definition.id, f.definition.revision)
            .map_err(ApiError::fixture)?
        {
            doc.files.entry(gdtf.to_ascii_lowercase()).or_insert(source);
            embedded += 1;
        } else {
            missing.push(format!(
                "{} · {}",
                f.definition.manufacturer, f.definition.model
            ));
        }
        let uuid = metas
            .iter()
            .find(|(_, m)| m.get("fixture_id").and_then(|v| v.as_str()) == Some(id))
            .and_then(|(uuid, _)| Uuid::parse_str(uuid).ok())
            .unwrap_or(f.fixture_id.0);
        let rx = f64::from(f.rotation.x).to_radians();
        let ry = f64::from(f.rotation.y).to_radians();
        let rz = f64::from(f.rotation.z).to_radians();
        let (sx, cx) = rx.sin_cos();
        let (sy, cy) = ry.sin_cos();
        let (sz, cz) = rz.sin_cos();
        doc.fixtures.push(light_mvr::MvrFixture {
            uuid,
            name: if f.name.is_empty() {
                f.definition.name.clone()
            } else {
                f.name.clone()
            },
            fixture_id: Some(id.clone()),
            gdtf_spec: gdtf,
            gdtf_mode: f.definition.mode.clone(),
            universe: f.universe,
            address: f.address,
            matrix: [
                cy * cz,
                cz * sx * sy - cx * sz,
                sx * sz + cx * cz * sy,
                cy * sz,
                cx * cz + sx * sy * sz,
                cx * sy * sz - cz * sx,
                -sy,
                cy * sx,
                cx * cy,
                f64::from(f.location.x),
                f64::from(f.location.y),
                f64::from(f.location.z),
            ],
            layer: Some(f.layer_id.clone()),
            class: None,
        });
    }
    let warnings = if missing.is_empty() {
        vec![]
    } else {
        vec!["Some fixture profiles have no retained source GDTF and are referenced but not embedded".into()]
    };
    let preview = MvrExportPreview {
        fixtures: doc.fixtures.len(),
        scenery: doc.geometry.len(),
        embedded_profiles: embedded,
        missing_profiles: missing,
        omitted: vec!["cues, presets, playbacks, users, and desk layouts".into()],
        warnings,
    };
    Ok((entry, doc, preview))
}
async fn preview_mvr_export(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<MvrExportPreview>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    Ok(Json(build_mvr_export(&state, id)?.2))
}
async fn export_mvr(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _ = authenticate(&state, &headers)?;
    let (entry, doc, _) = build_mvr_export(&state, id)?;
    let data = light_mvr::write(&doc).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}.mvr\"", entry.name),
            ),
        ],
        data,
    )
        .into_response())
}

async fn list_objects(
    State(state): State<AppState>,
    Path((id, kind)): Path<(Uuid, String)>,
) -> Result<Json<Vec<light_show::VersionedObject>>, ApiError> {
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let mut objects = ShowStore::open(entry.path)
        .map_err(ApiError::store)?
        .objects(&kind)
        .map_err(ApiError::store)?;
    if kind == "group" {
        materialize_derived_group_memberships(&mut objects);
    }
    if kind == "preset" {
        materialize_preset_addresses(&mut objects)?;
    }
    Ok(Json(objects))
}
async fn get_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let mut objects = ShowStore::open(entry.path)
        .map_err(ApiError::store)?
        .objects(&kind)
        .map_err(ApiError::store)?;
    if kind == "group" {
        materialize_derived_group_memberships(&mut objects);
    }
    if kind == "preset" {
        materialize_preset_addresses(&mut objects)?;
    }
    let object = objects
        .into_iter()
        .find(|object| object.id == object_id)
        .ok_or_else(|| ApiError::not_found("show object"))?;
    Ok((
        [(header::ETAG, format!("\"{}\"", object.revision))],
        Json(object),
    )
        .into_response())
}

fn materialize_derived_group_memberships(objects: &mut [light_show::VersionedObject]) {
    let groups = objects
        .iter()
        .filter_map(|object| {
            serde_json::from_value::<light_programmer::GroupDefinition>(object.body.clone())
                .ok()
                .map(|mut group| {
                    group.id = object.id.clone();
                    (group.id.clone(), group)
                })
        })
        .collect::<HashMap<_, _>>();
    for object in objects {
        let Ok(mut group) =
            serde_json::from_value::<light_programmer::GroupDefinition>(object.body.clone())
        else {
            continue;
        };
        group.id = object.id.clone();
        let Ok(fixtures) = light_programmer::resolve_group(&group.id, &groups) else {
            continue;
        };
        group.fixtures = fixtures;
        if let Ok(body) = serde_json::to_value(group) {
            object.body = body;
        }
    }
}
fn materialize_preset_addresses(
    objects: &mut [light_show::VersionedObject],
) -> Result<(), ApiError> {
    for object in objects {
        let (_, preset) = decode_preset_object(object).map_err(ApiError::bad_request)?;
        object.body = serialize_preset_preserving_extensions(&object.body, &preset)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }
    Ok(())
}
async fn put_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
    Json(mut body): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    if kind == "patched_fixture" {
        let mut fixture = serde_json::from_value::<light_fixture::PatchedFixture>(body)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        light_fixture::migrate_patched_fixture_to_v2(&mut fixture).map_err(ApiError::fixture)?;
        body =
            serde_json::to_value(fixture).map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if kind == "cue_list" {
        let mut cue_list = serde_json::from_value::<light_playback::CueList>(body)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        cue_list.migrate_legacy_chaser_xfade(&state.configuration.read().speed_groups_bpm);
        cue_list.validate().map_err(ApiError::bad_request)?;
        body = serde_json::to_value(cue_list)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if kind == "group" {
        let mut group = serde_json::from_value::<light_programmer::GroupDefinition>(body)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        group.id = object_id.clone();
        body =
            serde_json::to_value(group).map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if kind == "preset" {
        let address =
            light_programmer::PresetAddress::parse(&object_id).map_err(ApiError::bad_request)?;
        let mut preset = serde_json::from_value::<light_programmer::Preset>(body)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if preset.family != address.family {
            return Err(ApiError::bad_request(
                "preset family must match its pool address",
            ));
        }
        if preset.number != 0 && preset.number != address.number {
            return Err(ApiError::bad_request(
                "preset number must match its pool-local address",
            ));
        }
        preset.number = address.number;
        body =
            serde_json::to_value(preset).map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if kind == "playback" {
        let playback = serde_json::from_value::<light_playback::PlaybackDefinition>(body)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if object_id != playback.number.to_string() {
            return Err(ApiError::bad_request(
                "playback object id must match its playback number",
            ));
        }
        playback.validate().map_err(ApiError::bad_request)?;
        body = serde_json::to_value(playback)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if kind == "playback_page" {
        let page = serde_json::from_value::<light_playback::PlaybackPage>(body)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if object_id != page.number.to_string() {
            return Err(ApiError::bad_request(
                "playback page object id must match its page number",
            ));
        }
        page.validate().map_err(ApiError::bad_request)?;
        body = serde_json::to_value(page).map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if kind == "route" {
        let mut route = serde_json::from_value::<light_output::OutputRoute>(body)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if route.delivery_mode.is_none() {
            route.delivery_mode = Some(route.resolved_delivery_mode());
        }
        route.validate().map_err(ApiError::bad_request)?;
        body =
            serde_json::to_value(route).map_err(|error| ApiError::internal(error.to_string()))?;
    }
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id);
    let route_to_terminate = if active && kind == "route" {
        let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
        let previous = store
            .objects("route")
            .map_err(ApiError::store)?
            .into_iter()
            .find(|object| object.id == object_id)
            .and_then(|object| {
                serde_json::from_value::<light_output::OutputRoute>(object.body).ok()
            });
        let next = serde_json::from_value::<light_output::OutputRoute>(body.clone()).ok();
        previous.filter(|old| {
            old.enabled
                && next.as_ref().is_none_or(|new| {
                    !new.enabled
                        || old.protocol != new.protocol
                        || old.destination_universe != new.destination_universe
                        || old.resolved_delivery_mode() != new.resolved_delivery_mode()
                        || old.destination != new.destination
                })
        })
    } else {
        None
    };
    if active
        || matches!(
            kind.as_str(),
            "patched_fixture" | "playback" | "playback_page"
        )
    {
        let candidate =
            load_engine_snapshot_with_override(&entry, Some((&kind, &object_id, &body)))
                .map_err(ApiError::internal)?;
        if active || matches!(kind.as_str(), "playback" | "playback_page") {
            state
                .engine
                .validate_snapshot_for_runtime(&candidate)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        } else {
            candidate
                .validate()
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
    }
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    backup_show(&state, &entry)?;
    let revision = store
        .put_object(&kind, &object_id, &body, expected)
        .map_err(ApiError::store)?;
    if active {
        if let (Some(output), Some(route)) = (&state.network_output, route_to_terminate) {
            let _ = output
                .terminate_routes(&[route], &mut *state.output_sequences.lock().await)
                .await;
        }
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).map_err(ApiError::internal)?)
            .map_err(|error| ApiError::internal(error.to_string()))?;
        if kind == "patched_fixture"
            && let Ok(fixture) =
                serde_json::from_value::<light_fixture::PatchedFixture>(body.clone())
        {
            state
                .media_cache
                .lock()
                .clear_fixture(&fixture.fixture_id.0.to_string());
            state.media_status.write().remove(&fixture.fixture_id);
        }
    }
    emit(
        &state,
        "show_object_changed",
        serde_json::json!({"show_id":show_id,"kind":kind,"id":object_id,"revision":revision}),
    );
    Ok((
        [(header::ETAG, format!("\"{revision}\""))],
        Json(serde_json::json!({"revision":revision})),
    )
        .into_response())
}

async fn delete_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if kind != "route" {
        return Err(ApiError::bad_request(
            "generic object deletion is currently limited to output routes",
        ));
    }
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let object = store
        .objects(&kind)
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| object.id == object_id)
        .ok_or_else(|| ApiError::not_found("show object"))?;
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id);
    let route = active
        .then(|| serde_json::from_value::<light_output::OutputRoute>(object.body.clone()))
        .transpose()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    backup_show(&state, &entry)?;
    store
        .mutate_objects_atomically(
            &[],
            &[AtomicObjectDelete {
                kind: &kind,
                id: &object_id,
                expected,
            }],
        )
        .map_err(ApiError::store)?;
    if active {
        if let (Some(output), Some(route)) = (&state.network_output, route) {
            let _ = output
                .terminate_routes(&[route], &mut *state.output_sequences.lock().await)
                .await;
        }
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).map_err(ApiError::internal)?)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }
    emit(
        &state,
        "show_object_changed",
        serde_json::json!({"show_id":show_id,"kind":kind,"id":object_id,"revision":expected + 1,"deleted":true}),
    );
    Ok(StatusCode::NO_CONTENT)
}
async fn undo_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let revision = ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .undo_object(&kind, &object_id, expected)
        .map_err(ApiError::store)?;
    if state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id)
    {
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).map_err(ApiError::internal)?)
            .map_err(|error| ApiError::internal(error.to_string()))?;
        if kind == "patched_fixture" {
            state.media_cache.lock().retain_fixtures(
                &state
                    .engine
                    .snapshot()
                    .fixtures
                    .iter()
                    .filter(|fixture| fixture.direct_control.is_some())
                    .map(|fixture| fixture.fixture_id.0.to_string())
                    .collect(),
            );
            state.media_status.write().retain(|fixture, _| {
                state.engine.snapshot().fixtures.iter().any(|patched| {
                    patched.fixture_id == *fixture && patched.direct_control.is_some()
                })
            });
        }
    }
    emit(
        &state,
        "show_object_undone",
        serde_json::json!({"show_id":show_id,"kind":kind,"id":object_id,"revision":revision}),
    );
    Ok(Json(serde_json::json!({"revision":revision})))
}
async fn store_preset(
    State(state): State<AppState>,
    Path((id, preset_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(input): Json<PresetStoreInput>,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let family_supplied = input.preset.get("family").is_some();
    let mut incoming: light_programmer::Preset = serde_json::from_value(input.preset)
        .map_err(|error| ApiError::bad_request(format!("invalid incoming preset: {error}")))?;
    let address = light_programmer::PresetAddress::from_storage_key(&preset_id, incoming.family)
        .map_err(ApiError::bad_request)?;
    if !family_supplied {
        incoming.family = address.family;
    }
    if incoming.number != 0 && incoming.number != address.number {
        return Err(ApiError::bad_request(
            "preset body number does not match its pool-local address",
        ));
    }
    if incoming.family != address.family {
        return Err(ApiError::bad_request(
            "preset body family does not match its pool address",
        ));
    }
    incoming.number = address.number;
    let storage_key = address.storage_key();
    let existing = store
        .objects("preset")
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| {
            object.id == storage_key
                || decode_preset_object(object)
                    .is_ok_and(|(stored_address, _)| stored_address == address)
        });
    let persisted_key = existing
        .as_ref()
        .map(|object| object.id.clone())
        .unwrap_or(storage_key);
    let mut preset = existing
        .as_ref()
        .map(decode_preset_object)
        .transpose()
        .map_err(ApiError::bad_request)?
        .map(|(_, preset)| preset)
        .unwrap_or_else(|| light_programmer::Preset {
            family: address.family,
            number: address.number,
            ..Default::default()
        });
    preset.store(incoming, input.mode);
    backup_show(&state, &entry)?;
    let revision = store
        .put_object(
            "preset",
            &persisted_key,
            &serde_json::to_value(&preset)
                .map_err(|error| ApiError::internal(error.to_string()))?,
            expected,
        )
        .map_err(ApiError::store)?;
    emit(
        &state,
        "preset_stored",
        serde_json::json!({"show_id":show_id,"preset_address":address,"revision":revision,"source_session":session.id}),
    );
    Ok((
        [(header::ETAG, format!("\"{revision}\""))],
        Json(serde_json::json!({"revision":revision,"preset":preset,"source_session":session.id})),
    )
        .into_response())
}
async fn store_preload(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<PreloadStoreInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let use_active_preload = programmer.preload_pending.is_empty()
        && programmer.preload_group_pending.is_empty()
        && (!programmer.preload_active.is_empty() || !programmer.preload_group_active.is_empty());
    let fixture_values = if use_active_preload {
        &programmer.preload_active
    } else {
        &programmer.preload_pending
    };
    let group_values = if use_active_preload {
        &programmer.preload_group_active
    } else {
        &programmer.preload_group_pending
    };
    if fixture_values.is_empty() && group_values.is_empty() {
        return Err(ApiError::bad_request(
            "the pending and active preload scenes are empty",
        ));
    }
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let revision = match input.target.as_str() {
        "preset" => {
            let hinted_family = input.family.unwrap_or_else(|| {
                command_preset_family(&input.target_id)
                    .unwrap_or(light_programmer::PresetFamily::Mixed)
            });
            let address =
                light_programmer::PresetAddress::from_storage_key(&input.target_id, hinted_family)
                    .map_err(ApiError::bad_request)?;
            if input.family.is_some_and(|family| family != address.family) {
                return Err(ApiError::bad_request(
                    "preset family does not match its pool address",
                ));
            }
            let storage_key = address.storage_key();
            let mut preset = light_programmer::Preset {
                name: input
                    .name
                    .unwrap_or_else(|| format!("Preset {}", address.number)),
                family: address.family,
                number: address.number,
                ..Default::default()
            };
            for value in fixture_values {
                preset
                    .values
                    .entry(value.fixture_id)
                    .or_default()
                    .insert(value.attribute.clone(), value.value.clone());
            }
            for (group_id, pending) in group_values {
                let attributes = preset.group_values.entry(group_id.clone()).or_default();
                for (attribute, scoped) in pending {
                    attributes.insert(attribute.clone(), scoped.value.clone());
                }
            }
            preset.retain_family_attributes();
            let existing = store
                .objects("preset")
                .map_err(ApiError::store)?
                .into_iter()
                .find(|object| object.id == storage_key);
            let had_existing = existing.is_some();
            let mut merged = existing
                .as_ref()
                .map(decode_preset_object)
                .transpose()
                .map_err(ApiError::bad_request)?
                .map(|(_, preset)| preset)
                .unwrap_or_else(|| light_programmer::Preset {
                    family: address.family,
                    number: address.number,
                    ..Default::default()
                });
            if input.family.is_none() && had_existing {
                preset.family = merged.family;
            }
            merged.store(
                preset,
                input
                    .mode
                    .unwrap_or(light_programmer::PresetStoreMode::Merge),
            );
            store
                .put_object(
                    "preset",
                    &storage_key,
                    &serde_json::to_value(merged)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                    expected,
                )
                .map_err(ApiError::store)?
        }
        "cue" => {
            let object = store
                .objects("cue_list")
                .map_err(ApiError::store)?
                .into_iter()
                .find(|object| object.id == input.target_id)
                .ok_or_else(|| ApiError::not_found("Cuelist"))?;
            let mut cue_list: light_playback::CueList = serde_json::from_value(object.body)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            let number = input
                .cue_number
                .ok_or_else(|| ApiError::bad_request("cue_number is required for cue storage"))?;
            let index = cue_list
                .cues
                .iter()
                .position(|cue| cue.number == number)
                .unwrap_or_else(|| {
                    cue_list.cues.push(light_playback::Cue::new(number));
                    cue_list.cues.sort_by(|a, b| a.number.total_cmp(&b.number));
                    cue_list
                        .cues
                        .iter()
                        .position(|cue| cue.number == number)
                        .expect("inserted cue exists")
                });
            let cue = &mut cue_list.cues[index];
            if let Some(name) = input.name {
                cue.name = name;
            }
            for value in fixture_values {
                cue.changes.retain(|change| {
                    change.fixture_id != value.fixture_id || change.attribute != value.attribute
                });
                cue.changes.push(light_playback::CueChange::set(
                    value.fixture_id,
                    value.attribute.clone(),
                    value.value.clone(),
                ));
            }
            for (group_id, pending) in group_values {
                for (attribute, scoped) in pending {
                    cue.group_changes.retain(|change| {
                        change.group_id != *group_id || change.attribute != *attribute
                    });
                    cue.group_changes.push(light_playback::GroupCueChange {
                        group_id: group_id.clone(),
                        attribute: attribute.clone(),
                        value: Some(scoped.value.clone()),
                        automatic_restore: false,
                        fade_millis: scoped.fade_millis,
                        delay_millis: scoped.delay_millis,
                    });
                }
            }
            store
                .put_object(
                    "cue_list",
                    &input.target_id,
                    &serde_json::to_value(cue_list)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                    expected,
                )
                .map_err(ApiError::store)?
        }
        _ => return Err(ApiError::bad_request("target must be preset or cue")),
    };
    if state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id)
    {
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).map_err(ApiError::internal)?)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if use_active_preload {
        state.programmers.release_preload(session.id);
        persist_programmer(&state, &session)?;
    }
    emit(
        &state,
        "preload_stored",
        serde_json::json!({"session_id":session.id,"target":input.target,"target_id":input.target_id,"revision":revision,"source":if use_active_preload { "active_preload" } else { "pending_preload" }}),
    );
    Ok(Json(serde_json::json!({"revision":revision})))
}
async fn playback_action(
    State(state): State<AppState>,
    Path((id, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let id = light_core::CueListId(id);
    let mut playback = state.engine.playback().write();
    let result = match action.as_str() {
        "go" => serde_json::to_value(playback.go(id).map_err(ApiError::bad_request)?),
        "back" => serde_json::to_value(playback.back(id).map_err(ApiError::bad_request)?),
        "pause" => {
            playback.pause(id).map_err(ApiError::bad_request)?;
            serde_json::to_value(playback.active())
        }
        "release" => {
            let released = playback.release(id);
            Ok(serde_json::json!({"released":released}))
        }
        _ => return Err(ApiError::not_found("playback action")),
    }
    .map_err(|error| ApiError::internal(error.to_string()))?;
    drop(playback);
    persist_active_playbacks(&state)?;
    emit(
        &state,
        "playback_changed",
        serde_json::json!({"cue_list_id":id,"action":action,"session_id":session.id}),
    );
    Ok(Json(result))
}
async fn playbacks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let snapshot = state.engine.snapshot();
    let active_page = state
        .active_show
        .read()
        .as_ref()
        .and_then(|show| state.desk.lock().desk_page(session.desk.id, show.id).ok())
        .unwrap_or(1);
    let selected_playback = state.active_show.read().as_ref().and_then(|show| {
        state
            .desk
            .lock()
            .selected_playback(session.desk.id, show.id)
            .ok()
            .flatten()
    });
    Ok(Json(serde_json::json!({
        "cue_lists":snapshot.cue_lists,
        "pool":snapshot.playbacks,
        "pages":snapshot.playback_pages,
        "active":state.engine.playback().read().runtime_status(),
        "desk":session.desk,
        "active_page":active_page,
        "selected_playback":selected_playback,
        "authoritative_controls":authoritative_playback_controls(&state)
    })))
}

fn authoritative_playback_controls(state: &AppState) -> serde_json::Value {
    let now = application_millis(state);
    let speed_groups = {
        let controllers = state.speed_groups.lock();
        std::array::from_fn::<_, 5, _>(|index| controllers[index].snapshot(now))
    };
    let snapshot = state.engine.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| {
            serde_json::json!({
                "id":group.id,
                "master":group.master,
                "flash_level":state.engine.group_master_flash(&group.id)
            })
        })
        .collect::<Vec<_>>();
    let control = state.output_control.lock();
    let timing = state.configuration.read();
    serde_json::json!({
        "speed_groups":speed_groups,
        "groups":groups,
        "grand_master":{
            "level":control.options.grand_master,
            "effective_level":if control.grand_master_flash {1.0} else {control.options.grand_master},
            "blackout":control.options.blackout,
            "flash_active":control.grand_master_flash,
            "dynamics_paused":state.engine.playback().read().dynamics_paused()
        },
        "programmer_fade_millis":timing.programmer_fade_millis,
        "cue_fade_millis":timing.sequence_master_fade_millis
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct VirtualPlaybackExclusionZone {
    id: String,
    name: String,
    slots: Vec<u8>,
}

#[derive(Deserialize)]
struct VirtualPlaybackExclusionZoneInput {
    zones: Vec<VirtualPlaybackExclusionZone>,
}

type VirtualPlaybackExclusionSurfaces = HashMap<String, Vec<VirtualPlaybackExclusionZone>>;
type VirtualPlaybackExclusionStore = HashMap<String, VirtualPlaybackExclusionSurfaces>;

fn virtual_playback_exclusion_setting(show_id: light_core::ShowId) -> String {
    format!("virtual_playback_exclusion_zones:{}", show_id.0)
}

fn read_virtual_playback_exclusion_store(
    desk: &DeskStore,
    show_id: light_core::ShowId,
) -> VirtualPlaybackExclusionStore {
    desk.setting(&virtual_playback_exclusion_setting(show_id))
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

async fn virtual_playback_exclusion_zones(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let surfaces = read_virtual_playback_exclusion_store(&state.desk.lock(), show.id)
        .remove(&session.desk.id.to_string())
        .unwrap_or_default();
    Ok(Json(serde_json::json!({
        "show_id": show.id,
        "desk_id": session.desk.id,
        "surfaces": surfaces,
    })))
}

async fn put_virtual_playback_exclusion_zones(
    State(state): State<AppState>,
    Path(surface_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<VirtualPlaybackExclusionZoneInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    if surface_id.trim().is_empty() || surface_id.len() > 128 {
        return Err(ApiError::bad_request(
            "surface id must contain 1-128 characters",
        ));
    }
    let mut zone_ids = HashSet::new();
    let mut zones = Vec::with_capacity(input.zones.len());
    for mut zone in input.zones {
        zone.id = zone.id.trim().to_owned();
        zone.name = zone.name.trim().to_owned();
        if zone.id.is_empty() || zone.id.len() > 128 || !zone_ids.insert(zone.id.clone()) {
            return Err(ApiError::bad_request(
                "zone ids must be unique and contain 1-128 characters",
            ));
        }
        if zone.name.is_empty() || zone.name.len() > 80 {
            return Err(ApiError::bad_request(
                "zone names must contain 1-80 characters",
            ));
        }
        let mut seen = HashSet::new();
        zone.slots
            .retain(|slot| (1..=144).contains(slot) && seen.insert(*slot));
        if zone.slots.len() < 2 {
            return Err(ApiError::bad_request(
                "an exclusion zone needs at least two cells",
            ));
        }
        zones.push(zone);
    }
    let desk = state.desk.lock();
    let mut stored = read_virtual_playback_exclusion_store(&desk, show.id);
    let surfaces = stored.entry(session.desk.id.to_string()).or_default();
    if zones.is_empty() {
        surfaces.remove(&surface_id);
    } else {
        surfaces.insert(surface_id.clone(), zones.clone());
    }
    if surfaces.is_empty() {
        stored.remove(&session.desk.id.to_string());
    }
    desk.set_setting(
        &virtual_playback_exclusion_setting(show.id),
        &serde_json::to_string(&stored).map_err(|error| ApiError::internal(error.to_string()))?,
    )
    .map_err(ApiError::store)?;
    drop(desk);
    emit(
        &state,
        "virtual_playback_exclusion_zones_changed",
        serde_json::json!({"desk_id":session.desk.id,"show_id":show.id,"surface_id":surface_id,"zones":zones}),
    );
    Ok(Json(
        serde_json::json!({"surface_id":surface_id,"zones":zones}),
    ))
}

fn virtual_playback_zone_numbers(state: &AppState, desk_id: Uuid) -> Vec<Vec<u16>> {
    let Some(show) = state.active_show.read().clone() else {
        return Vec::new();
    };
    let (page_number, surfaces) = {
        let desk = state.desk.lock();
        let page = desk.desk_page(desk_id, show.id).unwrap_or(1);
        let surfaces = read_virtual_playback_exclusion_store(&desk, show.id)
            .remove(&desk_id.to_string())
            .unwrap_or_default();
        (page, surfaces)
    };
    let snapshot = state.engine.snapshot();
    let Some(page) = snapshot
        .playback_pages
        .iter()
        .find(|candidate| candidate.number == page_number)
    else {
        return Vec::new();
    };
    surfaces
        .into_values()
        .flat_map(|zones| zones.into_iter())
        .map(|zone| {
            let mut seen = HashSet::new();
            zone.slots
                .into_iter()
                .filter_map(|slot| page.slots.get(&slot).copied())
                .filter(|number| seen.insert(*number))
                .collect::<Vec<_>>()
        })
        .filter(|numbers| numbers.len() >= 2)
        .collect()
}

fn enforce_virtual_playback_exclusions(
    state: &AppState,
    desk_id: Uuid,
    activated_number: u16,
) -> Vec<u16> {
    let zones = virtual_playback_zone_numbers(state, desk_id);
    let mut playback = state.engine.playback().write();
    enforce_virtual_playback_exclusions_on(&mut playback, &zones, activated_number)
}

/// Apply one desk's virtual-playback exclusion zones to an arbitrary playback engine.
///
/// Keeping this operation independent from [`AppState`] lets Preload validate a complete batch
/// against an isolated engine before publishing any live playback state.
fn enforce_virtual_playback_exclusions_on(
    playback: &mut light_playback::PlaybackEngine,
    zones: &[Vec<u16>],
    activated_number: u16,
) -> Vec<u16> {
    if !zones.iter().any(|zone| zone.contains(&activated_number)) {
        return Vec::new();
    }
    if !playback
        .runtime()
        .iter()
        .any(|active| active.playback_number == Some(activated_number) && active.enabled)
    {
        return Vec::new();
    }
    let mut released = HashSet::new();
    for number in zones
        .iter()
        .filter(|zone| zone.contains(&activated_number))
        .flat_map(|zone| zone.iter().copied())
        .filter(|number| *number != activated_number)
    {
        if released.insert(number) {
            let _ = playback.off(number);
        }
    }
    let mut released = released.into_iter().collect::<Vec<_>>();
    released.sort_unstable();
    released
}

fn normalize_restored_virtual_playback_exclusions(state: &AppState) {
    let Some(show) = state.active_show.read().clone() else {
        return;
    };
    let desks = read_virtual_playback_exclusion_store(&state.desk.lock(), show.id)
        .keys()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect::<Vec<_>>();
    let zones = desks
        .into_iter()
        .flat_map(|desk_id| virtual_playback_zone_numbers(state, desk_id))
        .collect::<Vec<_>>();
    let mut active = state
        .engine
        .playback()
        .read()
        .runtime()
        .into_iter()
        .filter(|playback| {
            playback.enabled
                && playback
                    .playback_number
                    .is_some_and(|number| zones.iter().any(|zone| zone.contains(&number)))
        })
        .collect::<Vec<_>>();
    active.sort_by_key(|playback| (playback.activated_at, playback.playback_number));
    // Replay the retained activation order against every configured zone. This is independent of
    // HashMap/surface iteration and gives overlapping zones the same last-serialized-wins result as
    // live dispatch. Non-conflicting members may remain active.
    let mut retained = HashSet::new();
    for number in active
        .iter()
        .filter_map(|playback| playback.playback_number)
    {
        for other in zones
            .iter()
            .filter(|zone| zone.contains(&number))
            .flat_map(|zone| zone.iter())
        {
            retained.remove(other);
        }
        retained.insert(number);
    }
    let mut changed = false;
    let mut playback = state.engine.playback().write();
    for number in active
        .into_iter()
        .filter_map(|candidate| candidate.playback_number)
        .filter(|number| !retained.contains(number))
    {
        changed |= playback.off(number).unwrap_or(false);
    }
    drop(playback);
    if changed {
        let _ = persist_active_playbacks(state);
    }
}

#[derive(Default, Deserialize)]
struct PoolPlaybackInput {
    value: Option<f32>,
    cue_number: Option<f64>,
    pressed: Option<bool>,
    button: Option<u8>,
    surface: Option<String>,
}

#[derive(Deserialize)]
struct PlaybackSlotUpsertInput {
    playback: light_playback::PlaybackDefinition,
    #[serde(default)]
    expected_playback_revision: u64,
    #[serde(default)]
    expected_page_revision: u64,
}

#[derive(Deserialize)]
struct PlaybackSlotClearInput {
    expected_playback_revision: u64,
    expected_page_revision: u64,
}

async fn upsert_playback_slot(
    State(state): State<AppState>,
    Path((page_number, slot)): Path<(u8, u8)>,
    headers: HeaderMap,
    Json(input): Json<PlaybackSlotUpsertInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if !(1..=light_playback::MAX_PLAYBACK_PAGES).contains(&page_number)
        || !(1..=light_playback::MAX_PAGE_SLOTS).contains(&slot)
    {
        return Err(ApiError::bad_request(
            "page and slot must each be within 1-127",
        ));
    }
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let store = ShowStore::open(&show.path).map_err(ApiError::store)?;
    let playback_objects = store.objects("playback").map_err(ApiError::store)?;
    let page_objects = store.objects("playback_page").map_err(ApiError::store)?;
    let stored_page = page_objects
        .iter()
        .find(|object| object.id == page_number.to_string());
    let mut page = stored_page
        .map(|object| {
            serde_json::from_value::<light_playback::PlaybackPage>(object.body.clone())
                .map_err(|error| ApiError::bad_request(error.to_string()))
        })
        .transpose()?
        .unwrap_or(light_playback::PlaybackPage {
            number: page_number,
            name: format!("Page {page_number}"),
            slots: HashMap::new(),
        });
    let current_page_revision = stored_page.map_or(0, |object| object.revision);
    if current_page_revision != input.expected_page_revision {
        return Err(ApiError::store(light_show::StoreError::RevisionConflict {
            expected: input.expected_page_revision,
            current: current_page_revision,
        }));
    }
    let existing_number = page.slots.get(&slot).copied();
    let number = if let Some(number) = existing_number {
        number
    } else {
        let used = playback_objects
            .iter()
            .filter_map(|object| object.id.parse::<u16>().ok())
            .collect::<std::collections::HashSet<_>>();
        (1..=light_playback::MAX_PLAYBACKS)
            .find(|number| !used.contains(number))
            .ok_or_else(|| ApiError::bad_request("playback pool is full"))?
    };
    let existing_playback = playback_objects
        .iter()
        .find(|object| object.id == number.to_string());
    let current_playback_revision = existing_playback.map_or(0, |object| object.revision);
    if current_playback_revision != input.expected_playback_revision {
        return Err(ApiError::store(light_show::StoreError::RevisionConflict {
            expected: input.expected_playback_revision,
            current: current_playback_revision,
        }));
    }
    let mut playback = input.playback;
    playback.number = number;
    playback.validate().map_err(ApiError::bad_request)?;
    page.number = page_number;
    page.slots.insert(slot, number);
    page.validate().map_err(ApiError::bad_request)?;

    let mut candidate = (*state.engine.snapshot()).clone();
    candidate
        .playbacks
        .retain(|definition| definition.number != number);
    candidate.playbacks.push(playback.clone());
    if let Some(candidate_page) = candidate
        .playback_pages
        .iter_mut()
        .find(|candidate| candidate.number == page_number)
    {
        *candidate_page = page.clone();
    } else {
        candidate.playback_pages.push(page.clone());
    }
    state
        .engine
        .validate_snapshot_for_runtime(&candidate)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let playback_body =
        serde_json::to_value(&playback).map_err(|error| ApiError::internal(error.to_string()))?;
    let page_body =
        serde_json::to_value(&page).map_err(|error| ApiError::internal(error.to_string()))?;
    let playback_id = number.to_string();
    let page_id = page_number.to_string();
    backup_show(&state, &show)?;
    let revisions = store
        .mutate_objects_atomically(
            &[
                AtomicObjectWrite {
                    kind: "playback",
                    id: &playback_id,
                    body: &playback_body,
                    expected: current_playback_revision,
                },
                AtomicObjectWrite {
                    kind: "playback_page",
                    id: &page_id,
                    body: &page_body,
                    expected: current_page_revision,
                },
            ],
            &[],
        )
        .map_err(ApiError::store)?;
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&show).map_err(ApiError::internal)?)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    emit(
        &state,
        "playback_slot_changed",
        serde_json::json!({"session_id":session.id,"page":page_number,"slot":slot,"playback_number":number}),
    );
    Ok(Json(serde_json::json!({
        "playback": playback,
        "playback_revision": revisions[0],
        "page": page,
        "page_revision": revisions[1]
    })))
}

async fn clear_playback_slot(
    State(state): State<AppState>,
    Path((page_number, slot)): Path<(u8, u8)>,
    headers: HeaderMap,
    Json(input): Json<PlaybackSlotClearInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let store = ShowStore::open(&show.path).map_err(ApiError::store)?;
    let playback_objects = store.objects("playback").map_err(ApiError::store)?;
    let page_objects = store.objects("playback_page").map_err(ApiError::store)?;
    let primary_page = page_objects
        .iter()
        .find(|object| object.id == page_number.to_string())
        .ok_or_else(|| ApiError::not_found("playback page"))?;
    if primary_page.revision != input.expected_page_revision {
        return Err(ApiError::store(light_show::StoreError::RevisionConflict {
            expected: input.expected_page_revision,
            current: primary_page.revision,
        }));
    }
    let primary_definition: light_playback::PlaybackPage =
        serde_json::from_value(primary_page.body.clone())
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let number = primary_definition
        .slots
        .get(&slot)
        .copied()
        .ok_or_else(|| ApiError::not_found("paged playback"))?;
    let playback_object = playback_objects
        .iter()
        .find(|object| object.id == number.to_string())
        .ok_or_else(|| ApiError::not_found("playback"))?;
    if playback_object.revision != input.expected_playback_revision {
        return Err(ApiError::store(light_show::StoreError::RevisionConflict {
            expected: input.expected_playback_revision,
            current: playback_object.revision,
        }));
    }

    let mut page_updates = Vec::new();
    for object in page_objects {
        let mut definition: light_playback::PlaybackPage =
            serde_json::from_value(object.body.clone())
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let before = definition.slots.len();
        definition.slots.retain(|_, playback| *playback != number);
        if definition.slots.len() != before {
            page_updates.push((
                object.id,
                serde_json::to_value(definition)
                    .map_err(|error| ApiError::internal(error.to_string()))?,
                object.revision,
            ));
        }
    }
    let writes = page_updates
        .iter()
        .map(|(id, body, expected)| AtomicObjectWrite {
            kind: "playback_page",
            id,
            body,
            expected: *expected,
        })
        .collect::<Vec<_>>();
    let playback_id = number.to_string();
    let deletes = [AtomicObjectDelete {
        kind: "playback",
        id: &playback_id,
        expected: playback_object.revision,
    }];
    backup_show(&state, &show)?;
    let revisions = store
        .mutate_objects_atomically(&writes, &deletes)
        .map_err(ApiError::store)?;
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&show).map_err(ApiError::internal)?)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    emit(
        &state,
        "playback_slot_cleared",
        serde_json::json!({"session_id":session.id,"page":page_number,"slot":slot,"playback_number":number}),
    );
    Ok(Json(serde_json::json!({
        "cleared": true,
        "page": page_number,
        "slot": slot,
        "playback_number": number,
        "page_revisions": revisions
    })))
}
async fn pool_playback_state(
    State(state): State<AppState>,
    Path(number): Path<u16>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    let snapshot = state.engine.snapshot();
    let definition = snapshot
        .playbacks
        .iter()
        .find(|playback| playback.number == number)
        .ok_or_else(|| ApiError::not_found("playback"))?;
    let runtime = state
        .engine
        .playback()
        .read()
        .active()
        .into_iter()
        .find(|active| active.playback_number == Some(number));
    Ok(Json(
        serde_json::json!({"playback":definition,"runtime":runtime}),
    ))
}
#[derive(Deserialize)]
struct DeskPageInput {
    page: u8,
}
#[derive(Deserialize)]
struct ControlDeskInput {
    name: String,
    osc_alias: String,
    columns: u8,
    rows: u8,
    buttons: u8,
    #[serde(default)]
    playback_layout: Option<light_show::PlaybackSurfaceLayout>,
}
async fn update_control_desk(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ControlDeskInput>,
) -> Result<Json<ControlDesk>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if session.desk.id != id {
        return Err(ApiError::bad_request(
            "session is not attached to this desk",
        ));
    }
    let desk = state
        .desk
        .lock()
        .update_desk(
            id,
            &input.name,
            &input.osc_alias,
            input.columns,
            input.rows,
            input.buttons,
            input.playback_layout,
        )
        .map_err(ApiError::store)?;
    for session in state
        .sessions
        .write()
        .values_mut()
        .filter(|session| session.desk.id == id)
    {
        session.desk = desk.clone();
    }
    emit(
        &state,
        "control_desk_changed",
        serde_json::json!({"desk":desk}),
    );
    Ok(Json(desk))
}

async fn update_desk_page(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DeskPageInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if session.desk.id != id {
        return Err(ApiError::bad_request(
            "session is not attached to this desk",
        ));
    }
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    if !ensure_playback_page_for_advance(&state, &show, input.page)? {
        return Err(ApiError::bad_request("playback page does not exist"));
    }
    state
        .desk
        .lock()
        .set_desk_page(id, show.id, input.page)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "playback_page_changed",
        serde_json::json!({"desk_id":id,"show_id":show.id,"page":input.page}),
    );
    send_osc_feedback(&state, false);
    Ok(Json(serde_json::json!({"desk_id":id,"page":input.page})))
}

fn ensure_playback_page_for_advance(
    state: &AppState,
    show: &ShowEntry,
    requested: u8,
) -> Result<bool, ApiError> {
    let snapshot = state.engine.snapshot();
    if snapshot
        .playback_pages
        .iter()
        .any(|page| page.number == requested)
    {
        return Ok(true);
    }
    let Some(last) = snapshot
        .playback_pages
        .iter()
        .max_by_key(|page| page.number)
    else {
        return Ok(false);
    };
    if last.slots.is_empty() || last.number.checked_add(1) != Some(requested) {
        return Ok(false);
    }
    let page = light_playback::PlaybackPage {
        number: requested,
        name: format!("Page {requested}"),
        slots: HashMap::new(),
    };
    let body =
        serde_json::to_value(&page).map_err(|error| ApiError::internal(error.to_string()))?;
    let store = ShowStore::open(&show.path).map_err(ApiError::store)?;
    backup_show(state, show)?;
    let revision = store
        .put_object("playback_page", &requested.to_string(), &body, 0)
        .map_err(ApiError::store)?;
    state
        .engine
        .replace_snapshot(load_engine_snapshot(show).map_err(ApiError::internal)?)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    emit(
        state,
        "show_object_changed",
        serde_json::json!({"show_id":show.id,"kind":"playback_page","id":requested.to_string(),"revision":revision}),
    );
    Ok(true)
}

async fn list_screens(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show = state.active_show.read().clone();
    let store = state.desk.lock();
    let screens = store.screens().map_err(ApiError::store)?;
    let mut pages = serde_json::Map::new();
    if let Some(show) = show {
        for screen in &screens {
            let page = if screen.page_mode == "follow_main" {
                store.desk_page(session.desk.id, show.id)
            } else {
                store.screen_page(screen.id, show.id)
            }
            .map_err(ApiError::store)?;
            pages.insert(screen.id.to_string(), serde_json::json!(page));
        }
    }
    Ok(Json(
        serde_json::json!({"screens":screens,"active_pages":pages}),
    ))
}
async fn put_screen(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(mut input): Json<ScreenConfiguration>,
) -> Result<Json<ScreenConfiguration>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    input.id = id;
    let screen = state
        .desk
        .lock()
        .put_screen(input)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "screen_configuration_changed",
        serde_json::json!({"screen":screen}),
    );
    Ok(Json(screen))
}
async fn delete_screen(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _ = authenticate(&state, &headers)?;
    state
        .desk
        .lock()
        .delete_screen(id)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "screen_configuration_changed",
        serde_json::json!({"screen_id":id,"deleted":true}),
    );
    Ok(StatusCode::NO_CONTENT)
}
async fn update_screen_page(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DeskPageInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    if !state
        .engine
        .snapshot()
        .playback_pages
        .iter()
        .any(|page| page.number == input.page)
    {
        return Err(ApiError::bad_request("playback page does not exist"));
    }
    let store = state.desk.lock();
    let screen = store
        .screen(id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("screen"))?;
    if screen.page_mode != "independent" {
        return Err(ApiError::bad_request("screen follows the main page"));
    }
    store
        .set_screen_page(id, show.id, input.page)
        .map_err(ApiError::store)?;
    drop(store);
    emit(
        &state,
        "screen_page_changed",
        serde_json::json!({"screen_id":id,"show_id":show.id,"page":input.page}),
    );
    Ok(Json(serde_json::json!({"screen_id":id,"page":input.page})))
}

async fn paged_playback_action(
    State(state): State<AppState>,
    Path((id, slot, action)): Path<(Uuid, u8, String)>,
    headers: HeaderMap,
    input: Option<Json<PoolPlaybackInput>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if session.desk.id != id {
        return Err(ApiError::bad_request(
            "session is not attached to this desk",
        ));
    }
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let page_number = state
        .desk
        .lock()
        .desk_page(id, show.id)
        .map_err(ApiError::store)?;
    let number = cuelist_for_page_playback(&state.engine.snapshot(), page_number, slot)
        .ok_or_else(|| ApiError::not_found("paged playback"))?;
    pool_playback_action(State(state), Path((number, action)), headers, input).await
}

async fn pool_playback_action(
    State(state): State<AppState>,
    Path((number, action)): Path<(u16, String)>,
    headers: HeaderMap,
    input: Option<Json<PoolPlaybackInput>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let input = input.map(|Json(value)| value).unwrap_or_default();
    let definition = state
        .engine
        .snapshot()
        .playbacks
        .iter()
        .find(|playback| playback.number == number)
        .cloned()
        .ok_or_else(|| ApiError::not_found("playback"))?;
    let surface = input.surface.as_deref().unwrap_or("physical");
    let temp_active = predicted_preload_temp_state(&state, session.id, number);
    let pending_action =
        preload_capture_action_with_temp_state(&definition, &action, &input, temp_active)?;
    let capture = state
        .programmers
        .get(session.id)
        .is_some_and(|programmer| programmer.blind)
        && pending_action.is_some()
        && if surface == "virtual" {
            state.configuration.read().preload_virtual_playback_actions
        } else {
            state.configuration.read().preload_physical_playback_actions
        };
    if capture {
        let pending_action = pending_action.expect("capture requires a retained action verb");
        state.programmers.queue_preload_playback_action(
            session.id,
            number,
            pending_action.to_owned(),
            surface.to_owned(),
        );
        persist_programmer(&state, &session)?;
        emit(
            &state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"preload_playback_action":pending_action,"playback_number":number,"surface":surface}),
        );
        return Ok(Json(
            serde_json::json!({"pending":true,"action":pending_action,"playback":definition}),
        ));
    }
    let changed = dispatch_playback_action(
        &state,
        Some(&session),
        Some(&session.desk),
        &definition,
        &action,
        &input,
        "ui",
    )?;
    if changed {
        persist_active_playbacks(&state)?;
        emit(
            &state,
            "playback_changed",
            serde_json::json!({"playback_number":number,"action":action,"session_id":session.id}),
        );
    }
    let snapshot = state.engine.snapshot();
    Ok(Json(serde_json::json!({
        "playback":definition,
        "active":state.engine.playback().read().runtime_status(),
        "groups":snapshot.groups,
        "authoritative_controls":authoritative_playback_controls(&state),
        "changed":changed
    })))
}

fn predicted_preload_temp_state(state: &AppState, session: SessionId, number: u16) -> bool {
    let mut active = state
        .engine
        .playback()
        .read()
        .runtime_status()
        .into_iter()
        .find(|status| status.playback.playback_number == Some(number))
        .is_some_and(|status| status.temporary_active);
    if let Some(programmer) = state.programmers.get(session) {
        for pending in programmer
            .preload_playback_pending
            .iter()
            .filter(|pending| pending.playback_number == number)
        {
            match pending.action.as_str() {
                "temp-on" => active = true,
                "temp-off" => active = false,
                _ => {}
            }
        }
    }
    active
}

fn requested_playback_button_action(
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
) -> Result<Option<light_playback::PlaybackButtonAction>, ApiError> {
    use light_playback::PlaybackButtonAction as Action;
    let mapped = match action {
        "button" => {
            let button = input
                .button
                .ok_or_else(|| ApiError::bad_request("button number is required"))?;
            if button == 0 || button > definition.button_count {
                return Err(ApiError::bad_request(
                    "button is not present on this playback",
                ));
            }
            *definition
                .buttons
                .get(usize::from(button - 1))
                .ok_or_else(|| ApiError::bad_request("button must be within 1-3"))?
        }
        "on" => Action::On,
        "off" => Action::Off,
        "toggle" => Action::Toggle,
        "go" | "go-plus" => Action::Go,
        "go-minus" | "back" => Action::GoMinus,
        "fast-forward" => Action::FastForward,
        "fast-rewind" => Action::FastRewind,
        "flash" => Action::Flash,
        "temp" => Action::Temp,
        "swap" => Action::Swap,
        "select" => Action::Select,
        "select-contents" => Action::SelectContents,
        "select-dereferenced" => Action::SelectDereferenced,
        "learn" => Action::Learn,
        "double" => Action::Double,
        "half" => Action::Half,
        "pause" => Action::Pause,
        "blackout" => Action::Blackout,
        "pause-dynamics" => Action::PauseDynamics,
        "none" => Action::None,
        "master" | "fader" | "go-to" | "load" | "xfade-on" | "xfade-off" => {
            return Ok(None);
        }
        _ => return Err(ApiError::not_found("playback action")),
    };
    Ok(Some(mapped))
}

/// Returns the exact action verb retained by Preload, after resolving a configured physical or
/// virtual button. A configured Temp toggle is canonicalized to its next explicit on/off state;
/// Flash, faders, and implicit fader activation are never representable in the pending list.
#[cfg(test)]
fn preload_capture_action(
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
) -> Result<Option<&'static str>, ApiError> {
    preload_capture_action_with_temp_state(definition, action, input, false)
}

fn preload_capture_action_with_temp_state(
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
    temp_active: bool,
) -> Result<Option<&'static str>, ApiError> {
    use light_playback::PlaybackButtonAction as Action;
    if action == "temp-on" {
        return Ok(input.pressed.unwrap_or(true).then_some("temp-on"));
    }
    if action == "temp-off" {
        return Ok(Some("temp-off"));
    }
    if !input.pressed.unwrap_or(true) {
        return Ok(None);
    }
    Ok(
        match requested_playback_button_action(definition, action, input)? {
            Some(Action::Toggle) => Some("toggle"),
            Some(Action::Go) => Some("go"),
            Some(Action::GoMinus) => Some("go-minus"),
            Some(Action::Off) => Some("off"),
            Some(Action::On) => Some("on"),
            Some(Action::Temp) => Some(if temp_active { "temp-off" } else { "temp-on" }),
            _ => None,
        },
    )
}

fn select_cuelist_contents(
    state: &AppState,
    session: &Session,
    cue_list_id: light_core::CueListId,
) -> Result<(), ApiError> {
    let snapshot = state.engine.snapshot();
    let cue_list = snapshot
        .cue_lists
        .iter()
        .find(|cue_list| cue_list.id == cue_list_id)
        .ok_or_else(|| ApiError::bad_request("playback cue list does not exist"))?;
    let mut fixture_ids = std::collections::HashSet::new();
    let mut group_ids = std::collections::HashSet::new();
    let mut items = Vec::new();
    for cue in &cue_list.cues {
        for change in &cue.changes {
            if fixture_ids.insert(change.fixture_id) {
                items.push(light_programmer::SelectionReference::Fixture {
                    fixture_id: change.fixture_id,
                });
            }
        }
        for change in &cue.group_changes {
            if group_ids.insert(change.group_id.clone()) {
                items.push(light_programmer::SelectionReference::LiveGroup {
                    group_id: change.group_id.clone(),
                });
            }
        }
    }
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let fixtures = light_programmer::resolve_selection_references(&items, &groups);
    state.programmers.select_expression(
        session.id,
        fixtures,
        light_programmer::SelectionExpression::PlaybackContents { items },
    );
    persist_programmer(state, session)?;
    reconcile_highlight_selection(state, session, "playback_contents_selection");
    Ok(())
}

fn select_group_playback(
    state: &AppState,
    session: &Session,
    group_id: &str,
    live: bool,
) -> Result<(), ApiError> {
    let groups = state
        .engine
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let fixtures =
        light_programmer::resolve_group(group_id, &groups).map_err(ApiError::bad_request)?;
    if live {
        state.programmers.select_expression(
            session.id,
            fixtures,
            light_programmer::SelectionExpression::LiveGroup {
                group_id: group_id.to_owned(),
                rule: light_programmer::SelectionRule::All,
            },
        );
    } else {
        state.programmers.select(session.id, fixtures);
    }
    persist_programmer(state, session)?;
    reconcile_highlight_selection(state, session, "group_playback_selection");
    Ok(())
}

fn set_group_playback_master(state: &AppState, group_id: &str, value: f32) -> Result<(), ApiError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ApiError::bad_request("playback master must be within 0-1"));
    }
    let mut next = (*state.engine.snapshot()).clone();
    let group = next
        .groups
        .iter_mut()
        .find(|group| group.id == group_id)
        .ok_or_else(|| ApiError::bad_request("group does not exist"))?;
    group.master = value;
    state
        .engine
        .replace_snapshot(next)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

/// The one authoritative playback action path for UI, OSC, attached hardware, and deferred
/// preload actions. Desk selection is intentionally context-local; programmer selection remains
/// shared by the registry's user identity.
fn dispatch_playback_action(
    state: &AppState,
    session: Option<&Session>,
    desk: Option<&ControlDesk>,
    definition: &light_playback::PlaybackDefinition,
    action_name: &str,
    input: &PoolPlaybackInput,
    source: &str,
) -> Result<bool, ApiError> {
    let _serialized = state.playback_action_lock.lock();
    let was_enabled = state
        .engine
        .playback()
        .read()
        .runtime()
        .iter()
        .any(|playback| playback.playback_number == Some(definition.number) && playback.enabled);
    let changed = dispatch_playback_action_inner(
        state,
        session,
        desk,
        definition,
        action_name,
        input,
        source,
    )?;
    let now_enabled = state
        .engine
        .playback()
        .read()
        .runtime()
        .iter()
        .any(|playback| playback.playback_number == Some(definition.number) && playback.enabled);
    if changed
        && !was_enabled
        && now_enabled
        && let Some(desk) = desk
    {
        let released = enforce_virtual_playback_exclusions(state, desk.id, definition.number);
        if !released.is_empty() {
            emit(
                state,
                "playback_exclusion_applied",
                serde_json::json!({"desk_id":desk.id,"activated_playback":definition.number,"released_playbacks":released,"source":source}),
            );
        }
    }
    if changed {
        persist_active_playbacks(state)?;
        persist_output_runtime(state)?;
    }
    Ok(changed)
}

fn dispatch_playback_action_inner(
    state: &AppState,
    session: Option<&Session>,
    desk: Option<&ControlDesk>,
    definition: &light_playback::PlaybackDefinition,
    action_name: &str,
    input: &PoolPlaybackInput,
    source: &str,
) -> Result<bool, ApiError> {
    use light_playback::{PlaybackButtonAction as Action, PlaybackTarget};
    let pressed = input.pressed.unwrap_or(true);
    if matches!(action_name, "master" | "fader") {
        let virtual_fader = source == "matter" && !definition.has_fader;
        if !definition.has_fader && !virtual_fader {
            return Err(ApiError::bad_request("playback does not have a fader"));
        }
        let value = input
            .value
            .ok_or_else(|| ApiError::bad_request("master value is required"))?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ApiError::bad_request("playback master must be within 0-1"));
        }
        match &definition.target {
            PlaybackTarget::CueList { .. } => {
                let mut playback = state.engine.playback().write();
                if virtual_fader {
                    playback
                        .set_virtual_master(definition.number, value)
                        .map_err(ApiError::bad_request)?;
                } else {
                    playback
                        .set_master(definition.number, value)
                        .map_err(ApiError::bad_request)?;
                }
            }
            PlaybackTarget::Group { group_id } => {
                set_group_playback_master(state, group_id, value)?
            }
            PlaybackTarget::SpeedGroup { group } => {
                apply_speed_group_playback_action(state, group, "master", input, definition.fader)?
            }
            PlaybackTarget::GrandMaster => {
                state.output_control.lock().options.grand_master = value;
            }
            PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
                {
                    let mut configuration = state.configuration.write();
                    if matches!(definition.target, PlaybackTarget::ProgrammerFade) {
                        configuration.programmer_fade_millis =
                            (f64::from(value) * 20_000.0).round() as u64;
                    } else {
                        configuration.sequence_master_fade_millis =
                            (f64::from(value) * 60_000.0).round() as u64;
                    }
                }
                persist_server_configuration(state)?;
                refresh_speed_group_engine(state);
            }
        }
        return Ok(true);
    }

    if action_name == "go-to" {
        state
            .engine
            .playback()
            .write()
            .goto_playback(
                definition.number,
                input
                    .cue_number
                    .ok_or_else(|| ApiError::bad_request("cue_number is required"))?,
            )
            .map_err(ApiError::bad_request)?;
        return Ok(true);
    }
    if action_name == "load" {
        state
            .engine
            .playback()
            .write()
            .load_playback(
                definition.number,
                input
                    .cue_number
                    .ok_or_else(|| ApiError::bad_request("cue_number is required"))?,
            )
            .map_err(ApiError::bad_request)?;
        return Ok(true);
    }
    if matches!(action_name, "xfade-on" | "xfade-off") {
        state
            .engine
            .playback()
            .write()
            .xfade(definition.number, action_name == "xfade-on")
            .map_err(ApiError::bad_request)?;
        return Ok(true);
    }
    if matches!(action_name, "temp-on" | "temp-off") {
        if !matches!(definition.target, PlaybackTarget::CueList { .. }) {
            return Err(ApiError::bad_request(
                "Temp is available only for a Cuelist playback",
            ));
        }
        state
            .engine
            .playback()
            .write()
            .set_temp_button(definition.number, action_name == "temp-on")
            .map_err(ApiError::bad_request)?;
        return Ok(true);
    }

    let action = requested_playback_button_action(definition, action_name, input)?
        .ok_or_else(|| ApiError::not_found("playback action"))?;
    if !pressed && !matches!(action, Action::Flash | Action::Swap) {
        return Ok(false);
    }
    if action == Action::Select
        && matches!(
            definition.target,
            PlaybackTarget::CueList { .. } | PlaybackTarget::Group { .. }
        )
    {
        let desk = desk.ok_or_else(|| ApiError::bad_request("playback selection needs a desk"))?;
        let show = state
            .active_show
            .read()
            .clone()
            .ok_or_else(|| ApiError::bad_request("no show is open"))?;
        state
            .desk
            .lock()
            .set_selected_playback(desk.id, show.id, Some(definition.number))
            .map_err(ApiError::store)?;
    }
    match &definition.target {
        PlaybackTarget::CueList { cue_list_id } => match action {
            Action::On => state
                .engine
                .playback()
                .write()
                .on(definition.number)
                .map_err(ApiError::bad_request)?,
            Action::Off => {
                state
                    .engine
                    .playback()
                    .write()
                    .off(definition.number)
                    .map_err(ApiError::bad_request)?;
            }
            Action::Toggle => {
                state
                    .engine
                    .playback()
                    .write()
                    .toggle(definition.number)
                    .map_err(ApiError::bad_request)?;
            }
            Action::Go => {
                state
                    .engine
                    .playback()
                    .write()
                    .go_playback(definition.number)
                    .map_err(ApiError::bad_request)?;
            }
            Action::GoMinus => {
                state
                    .engine
                    .playback()
                    .write()
                    .back_playback(definition.number)
                    .map_err(ApiError::bad_request)?;
            }
            Action::Pause => {
                let paused = state
                    .engine
                    .playback()
                    .read()
                    .runtime()
                    .iter()
                    .any(|runtime| {
                        runtime.playback_number == Some(definition.number) && runtime.paused
                    });
                if paused {
                    state
                        .engine
                        .playback()
                        .write()
                        .go_playback(definition.number)
                        .map_err(ApiError::bad_request)?;
                } else {
                    state
                        .engine
                        .playback()
                        .write()
                        .pause_playback(definition.number)
                        .map_err(ApiError::bad_request)?;
                }
            }
            Action::FastForward => {
                state
                    .engine
                    .playback()
                    .write()
                    .fast_forward_playback(definition.number)
                    .map_err(ApiError::bad_request)?;
            }
            Action::FastRewind => {
                state
                    .engine
                    .playback()
                    .write()
                    .fast_rewind_playback(definition.number)
                    .map_err(ApiError::bad_request)?;
            }
            Action::Flash => state
                .engine
                .playback()
                .write()
                .set_flash(definition.number, pressed)
                .map_err(ApiError::bad_request)?,
            Action::Temp => {
                state
                    .engine
                    .playback()
                    .write()
                    .toggle_temp(definition.number)
                    .map_err(ApiError::bad_request)?;
            }
            Action::Swap => state
                .engine
                .playback()
                .write()
                .set_swap(definition.number, pressed)
                .map_err(ApiError::bad_request)?,
            Action::Select => {}
            Action::SelectContents => {
                let session =
                    session.ok_or_else(|| ApiError::bad_request("selection needs a session"))?;
                select_cuelist_contents(state, session, *cue_list_id)?;
            }
            Action::None => return Ok(false),
            _ => {
                return Err(ApiError::bad_request(
                    "action is incompatible with a Cuelist playback",
                ));
            }
        },
        PlaybackTarget::Group { group_id } => match action {
            Action::Select => {
                let session =
                    session.ok_or_else(|| ApiError::bad_request("selection needs a session"))?;
                select_group_playback(state, session, group_id, true)?;
            }
            Action::SelectDereferenced => {
                let session =
                    session.ok_or_else(|| ApiError::bad_request("selection needs a session"))?;
                select_group_playback(state, session, group_id, false)?;
            }
            Action::Flash => state
                .engine
                .set_group_master_flash(group_id.clone(), if pressed { 1.0 } else { 0.0 }),
            Action::None => return Ok(false),
            _ => {
                return Err(ApiError::bad_request(
                    "action is incompatible with a Group Master playback",
                ));
            }
        },
        PlaybackTarget::SpeedGroup { group } => {
            let speed_action = match action {
                Action::Learn => "learn",
                Action::Double => "double",
                Action::Half => "half",
                Action::Pause => "pause",
                Action::None => return Ok(false),
                _ => {
                    return Err(ApiError::bad_request(
                        "action is incompatible with a Speed Group playback",
                    ));
                }
            };
            apply_speed_group_playback_action(state, group, speed_action, input, definition.fader)?;
        }
        PlaybackTarget::GrandMaster => match action {
            Action::Blackout => {
                let current = state.output_control.lock().options.blackout;
                state.output_control.lock().options.blackout = !current;
            }
            Action::Flash => state.output_control.lock().grand_master_flash = pressed,
            Action::PauseDynamics => {
                state.engine.playback().write().toggle_dynamics_paused();
            }
            Action::None => return Ok(false),
            _ => {
                return Err(ApiError::bad_request(
                    "action is incompatible with a Grand Master playback",
                ));
            }
        },
        PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => {
            let mut configuration = state.configuration.write();
            let (time, maximum) = if matches!(definition.target, PlaybackTarget::ProgrammerFade) {
                (&mut configuration.programmer_fade_millis, 20_000)
            } else {
                (&mut configuration.sequence_master_fade_millis, 60_000)
            };
            match action {
                Action::Double => *time = time.saturating_mul(2).min(maximum),
                Action::Half => *time /= 2,
                Action::Off => *time = 0,
                Action::None => return Ok(false),
                _ => {
                    return Err(ApiError::bad_request(
                        "action is incompatible with a time-master playback",
                    ));
                }
            }
            drop(configuration);
            persist_server_configuration(state)?;
            refresh_speed_group_engine(state);
        }
    }
    Ok(true)
}

fn apply_speed_group_playback_action(
    state: &AppState,
    group: &str,
    action: &str,
    input: &PoolPlaybackInput,
    fader: light_playback::PlaybackFaderMode,
) -> Result<(), ApiError> {
    let index = speed_group_index(group)?;
    let now = application_millis(state);
    let mut controllers = state.speed_groups.lock();
    let affected = match action {
        "learn" => {
            unlink_speed_group(&mut controllers, index, now);
            controllers[index].tap_learn(now);
            state.sound_capture_owners.lock()[index] = None;
            vec![index]
        }
        "double" => {
            let affected = speed_group_action_indices(&controllers, index);
            for &affected_index in &affected {
                controllers[affected_index].double();
            }
            affected
        }
        "half" => {
            let affected = speed_group_action_indices(&controllers, index);
            for &affected_index in &affected {
                controllers[affected_index].half();
            }
            affected
        }
        "pause" => {
            let paused = !controllers[index].snapshot(now).paused;
            let affected = speed_group_action_indices(&controllers, index);
            for &affected_index in &affected {
                controllers[affected_index].set_paused_at(paused, now);
            }
            affected
        }
        "master" => {
            let value = input
                .value
                .ok_or_else(|| ApiError::bad_request("master value is required"))?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(ApiError::bad_request("playback master must be within 0-1"));
            }
            match fader {
                light_playback::PlaybackFaderMode::DirectBpm => {
                    unlink_speed_group(&mut controllers, index, now);
                    if value == 0.0 {
                        controllers[index]
                            .set_speed_master_scale(0.0)
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                        controllers[index].set_paused_at(true, now);
                    } else {
                        controllers[index]
                            .set_manual_bpm(f64::from(value) * 300.0)
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                        controllers[index]
                            .set_speed_master_scale(1.0)
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                        controllers[index].set_paused_at(false, now);
                        state.sound_capture_owners.lock()[index] = None;
                    }
                    vec![index]
                }
                light_playback::PlaybackFaderMode::CenteredRelative => {
                    let scale = 4_f64.powf((f64::from(value) - 0.5) * 2.0);
                    let affected = speed_group_action_indices(&controllers, index);
                    for &affected_index in &affected {
                        controllers[affected_index]
                            .set_speed_master_scale(scale)
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                    }
                    affected
                }
                light_playback::PlaybackFaderMode::LearnedPercentage
                | light_playback::PlaybackFaderMode::Speed => {
                    let affected = speed_group_action_indices(&controllers, index);
                    for &affected_index in &affected {
                        controllers[affected_index]
                            .set_speed_master_scale(f64::from(value))
                            .map_err(|error| ApiError::bad_request(error.to_string()))?;
                        controllers[affected_index].set_paused_at(value == 0.0, now);
                    }
                    affected
                }
                _ => {
                    return Err(ApiError::bad_request(
                        "the configured fader mode is not available for a Speed Group",
                    ));
                }
            }
        }
        _ => {
            return Err(ApiError::bad_request(
                "action is not available for a Speed Group playback",
            ));
        }
    };
    copy_speed_group_runtime_to_configuration(state, &controllers, &affected);
    drop(controllers);
    persist_server_configuration(state)?;
    refresh_speed_group_engine(state);
    Ok(())
}

async fn list_programmers(
    State(state): State<AppState>,
) -> Json<Vec<light_programmer::ProgrammerState>> {
    Json(state.programmers.active_for_sessions())
}

fn update_settings_for(state: &AppState, desk_id: Uuid) -> update::UpdateSettings {
    state
        .configuration
        .read()
        .update_settings_by_desk
        .get(&desk_id)
        .cloned()
        .unwrap_or_default()
}

fn command_line_arms_update(command_line: &str) -> bool {
    command_line
        .split_whitespace()
        .next()
        .is_some_and(|token| token.eq_ignore_ascii_case("UPDATE"))
}

fn emit_update_armed_transition(
    state: &AppState,
    session: &Session,
    was_armed: bool,
    is_armed: bool,
    source: &str,
) {
    if was_armed == is_armed {
        return;
    }
    emit(
        state,
        "update_armed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "armed":is_armed,
            "source":source,
        }),
    );
}

async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<update::UpdateSettings>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(update_settings_for(&state, session.desk.id)))
}

async fn put_update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(settings): Json<update::UpdateSettings>,
) -> Result<Json<update::UpdateSettings>, ApiError> {
    let session = authenticate(&state, &headers)?;
    state
        .configuration
        .write()
        .update_settings_by_desk
        .insert(session.desk.id, settings.clone());
    persist_server_configuration(&state)?;
    emit(
        &state,
        "update_settings_changed",
        serde_json::json!({"desk_id":session.desk.id,"settings":settings}),
    );
    Ok(Json(settings))
}

fn active_update_cue_contexts(state: &AppState) -> Vec<update::ActiveCueContext> {
    state
        .engine
        .playback()
        .read()
        .active()
        .into_iter()
        .filter_map(|playback| {
            Some(update::ActiveCueContext {
                playback_number: playback.playback_number?,
                cue_list_id: playback.cue_list_id,
                cue_id: playback.current_cue_id?,
                cue_number: playback.current_cue_number?,
            })
        })
        .collect()
}

fn parse_update_cue_list_id(target: &UpdateApiTarget) -> Result<light_core::CueListId, ApiError> {
    let id = target
        .object_id
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("Cue Update requires a Cuelist object_id"))?;
    Ok(light_core::CueListId(Uuid::parse_str(id).map_err(
        |_| ApiError::bad_request("Cue Update object_id is not a Cuelist UUID"),
    )?))
}

fn resolve_update_cue_target(
    target: &UpdateApiTarget,
    active: &[update::ActiveCueContext],
) -> Result<update::ResolvedCueTarget, ApiError> {
    if target.validate_active_context {
        let playback_number = target.playback_number.ok_or_else(|| {
            ApiError::bad_request("a live Update target requires playback_number")
        })?;
        let cue_list_id = parse_update_cue_list_id(target)?;
        let context = active
            .iter()
            .find(|context| context.playback_number == playback_number)
            .ok_or_else(|| {
                ApiError::conflict("the touched playback is no longer active; preview Update again")
            })?;
        if context.cue_list_id != cue_list_id
            || target.cue_id.is_some_and(|cue_id| context.cue_id != cue_id)
            || target
                .cue_number
                .is_some_and(|number| context.cue_number != number)
        {
            return Err(ApiError::conflict(
                "the touched playback/Cue context changed; preview Update again",
            ));
        }
        return Ok(update::ResolvedCueTarget::from(context));
    }
    let request = if let Some(cue_id) = target.cue_id {
        if let Some(object_id) = target.object_id.as_deref() {
            let cue_list_id = light_core::CueListId(Uuid::parse_str(object_id).map_err(|_| {
                ApiError::bad_request("Cue Update object_id is not a Cuelist UUID")
            })?);
            update::CueTargetRequest::Explicit(update::ResolvedCueTarget {
                cue_list_id,
                playback_number: target.playback_number,
                cue_id,
                cue_number: target.cue_number.unwrap_or_default(),
            })
        } else {
            let context = active
                .iter()
                .find(|context| {
                    context.cue_id == cue_id
                        && target
                            .playback_number
                            .is_none_or(|number| context.playback_number == number)
                })
                .ok_or_else(|| ApiError::bad_request("explicit Cue context is no longer active"))?;
            update::CueTargetRequest::Explicit(update::ResolvedCueTarget::from(context))
        }
    } else if let Some(playback_number) = target.playback_number {
        update::CueTargetRequest::ActivePlayback { playback_number }
    } else {
        update::CueTargetRequest::PoolCueList {
            cue_list_id: parse_update_cue_list_id(target)?,
        }
    };
    update::resolve_cue_target(&request, active).map_err(update_api_error)
}

fn update_content_revision(
    content: &light_programmer::ProgrammerUpdateContent,
) -> Result<String, ApiError> {
    let encoded = serde_json::to_vec(content).map_err(|error| {
        ApiError::internal(format!("could not fingerprint programmer: {error}"))
    })?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

fn update_api_error(error: update::UpdateError) -> ApiError {
    match error {
        update::UpdateError::StaleRevision { .. } => ApiError::conflict(error.to_string()),
        update::UpdateError::MissingTarget { .. } => ApiError::not_found(error.to_string()),
        _ => ApiError::bad_request(error.to_string()),
    }
}

fn stored_update_object(
    store: &ShowStore,
    kind: &str,
    id: &str,
) -> Result<light_show::VersionedObject, ApiError> {
    store
        .objects(kind)
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| object.id == id)
        .ok_or_else(|| ApiError::not_found(format!("{kind} {id}")))
}

fn preview_update_request(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
) -> Result<UpdatePreviewResponse, ApiError> {
    let (_, store) = active_show_store(state).map_err(ApiError::bad_request)?;
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let content = programmer.update_content();
    let programmer_revision = update_content_revision(&content)?;
    let (revision, preview) = match request.target.family {
        UpdateApiTargetFamily::Cue => {
            let update::UpdateMode::Cue(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Cue targets require one of the four Cue Update modes",
                ));
            };
            let target =
                resolve_update_cue_target(&request.target, &active_update_cue_contexts(state))?;
            let id = target.cue_list_id.0.to_string();
            let object = stored_update_object(&store, "cue_list", &id)?;
            let cue_list = serde_json::from_value::<light_playback::CueList>(object.body)
                .map_err(|error| ApiError::bad_request(format!("invalid Cuelist: {error}")))?;
            (
                object.revision,
                update::preview_cue_update(&cue_list, &target, mode, &content)
                    .map_err(update_api_error)?,
            )
        }
        UpdateApiTargetFamily::Preset => {
            let update::UpdateMode::ExistingContent(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Preset targets require Update Existing or Add New",
                ));
            };
            let id = request
                .target
                .object_id
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("Preset Update requires object_id"))?;
            let object = stored_update_object(&store, "preset", id)?;
            let preset = serde_json::from_value::<light_programmer::Preset>(object.body)
                .map_err(|error| ApiError::bad_request(format!("invalid Preset: {error}")))?;
            (
                object.revision,
                update::preview_preset_update(id, &preset, mode, &content)
                    .map_err(update_api_error)?,
            )
        }
        UpdateApiTargetFamily::Group => {
            let update::UpdateMode::ExistingContent(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Group targets require Update Existing or Add New",
                ));
            };
            let id = request
                .target
                .object_id
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("Group Update requires object_id"))?;
            let object = stored_update_object(&store, "group", id)?;
            let mut group =
                serde_json::from_value::<light_programmer::GroupDefinition>(object.body)
                    .map_err(|error| ApiError::bad_request(format!("invalid Group: {error}")))?;
            group.id = id.to_owned();
            let groups = state
                .engine
                .snapshot()
                .groups
                .iter()
                .cloned()
                .map(|candidate| (candidate.id.clone(), candidate))
                .collect::<HashMap<_, _>>();
            let membership =
                light_programmer::resolve_group(id, &groups).map_err(ApiError::bad_request)?;
            (
                object.revision,
                update::preview_group_update(&group, &membership, mode, &content)
                    .map_err(update_api_error)?,
            )
        }
    };
    Ok(UpdatePreviewResponse {
        revision,
        programmer_revision,
        preview,
    })
}

async fn preview_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateApiRequest>,
) -> Result<Json<UpdatePreviewResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(preview_update_request(&state, &session, &request)?))
}

fn plan_update_request(
    state: &AppState,
    session: &Session,
    store: &ShowStore,
    request: &UpdateApiRequest,
) -> Result<update::AtomicUpdatePlan, ApiError> {
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let content = programmer.update_content();
    let programmer_revision = update_content_revision(&content)?;
    if request
        .expected_programmer_revision
        .as_ref()
        .is_some_and(|expected| expected != &programmer_revision)
    {
        return Err(ApiError::conflict(
            "programmer content changed after the Update preview; preview again",
        ));
    }
    match request.target.family {
        UpdateApiTargetFamily::Cue => {
            let update::UpdateMode::Cue(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Cue targets require one of the four Cue Update modes",
                ));
            };
            let target =
                resolve_update_cue_target(&request.target, &active_update_cue_contexts(state))?;
            let id = target.cue_list_id.0.to_string();
            let object = stored_update_object(store, "cue_list", &id)?;
            let cue_list = serde_json::from_value::<light_playback::CueList>(object.body)
                .map_err(|error| ApiError::bad_request(format!("invalid Cuelist: {error}")))?;
            update::plan_cue_update(
                &cue_list,
                object.revision,
                request.expected_revision.unwrap_or(object.revision),
                &target,
                mode,
                &content,
            )
            .map_err(update_api_error)
        }
        UpdateApiTargetFamily::Preset => {
            let update::UpdateMode::ExistingContent(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Preset targets require Update Existing or Add New",
                ));
            };
            let id = request
                .target
                .object_id
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("Preset Update requires object_id"))?;
            let object = stored_update_object(store, "preset", id)?;
            let preset = serde_json::from_value::<light_programmer::Preset>(object.body)
                .map_err(|error| ApiError::bad_request(format!("invalid Preset: {error}")))?;
            update::plan_preset_update(
                id,
                &preset,
                object.revision,
                request.expected_revision.unwrap_or(object.revision),
                mode,
                &content,
            )
            .map_err(update_api_error)
        }
        UpdateApiTargetFamily::Group => {
            let update::UpdateMode::ExistingContent(mode) = request.mode else {
                return Err(ApiError::bad_request(
                    "Group targets require Update Existing or Add New",
                ));
            };
            let id = request
                .target
                .object_id
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("Group Update requires object_id"))?;
            let object = stored_update_object(store, "group", id)?;
            let mut group =
                serde_json::from_value::<light_programmer::GroupDefinition>(object.body)
                    .map_err(|error| ApiError::bad_request(format!("invalid Group: {error}")))?;
            group.id = id.to_owned();
            let groups = state
                .engine
                .snapshot()
                .groups
                .iter()
                .cloned()
                .map(|candidate| (candidate.id.clone(), candidate))
                .collect::<HashMap<_, _>>();
            let membership =
                light_programmer::resolve_group(id, &groups).map_err(ApiError::bad_request)?;
            update::plan_group_update(
                &group,
                &membership,
                object.revision,
                request.expected_revision.unwrap_or(object.revision),
                mode,
                &content,
            )
            .map_err(update_api_error)
        }
    }
}

fn perform_update(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
) -> Result<update::UpdateResult, ApiError> {
    let (entry, store) = active_show_store(state).map_err(ApiError::bad_request)?;
    let plan = plan_update_request(state, session, &store, request)?;
    let kind = plan.object_kind().to_owned();
    let id = plan.object_id().to_owned();
    let body = plan
        .body()
        .map_err(|error| ApiError::internal(error.to_string()))?;
    backup_show(state, &entry)?;
    let revision = store
        .put_object(&kind, &id, &body, plan.expected_revision)
        .map_err(ApiError::store)?;
    let result = plan.complete(revision);
    refresh_command_show(state, &entry).map_err(ApiError::internal)?;
    emit(
        state,
        "show_object_changed",
        serde_json::json!({
            "show_id":entry.id,
            "kind":kind,
            "id":id,
            "revision":revision,
            "source":"update",
            "result":result,
            "session_id":session.id,
        }),
    );
    Ok(result)
}

async fn apply_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateApiRequest>,
) -> Result<Json<update::UpdateResult>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(perform_update(&state, &session, &request)?))
}

fn referenced_update_targets(
    state: &AppState,
    session: &Session,
) -> Result<Vec<UpdateApiTarget>, ApiError> {
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let mut targets = active_update_cue_contexts(state)
        .into_iter()
        .map(|context| UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(context.cue_list_id.0.to_string()),
            playback_number: Some(context.playback_number),
            cue_id: Some(context.cue_id),
            cue_number: Some(context.cue_number),
            validate_active_context: true,
        })
        .collect::<Vec<_>>();
    if let Some(id) = programmer
        .active_context
        .as_deref()
        .and_then(|context| context.strip_prefix("preset:"))
    {
        targets.push(UpdateApiTarget {
            family: UpdateApiTargetFamily::Preset,
            object_id: Some(id.to_owned()),
            playback_number: None,
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        });
    }
    let mut group_ids = programmer.group_values.keys().cloned().collect::<Vec<_>>();
    match programmer.selection_expression {
        Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) => {
            group_ids.push(group_id)
        }
        Some(light_programmer::SelectionExpression::Sources { items }) => {
            group_ids.extend(items.into_iter().filter_map(|item| match item {
                light_programmer::SelectionReference::LiveGroup { group_id }
                | light_programmer::SelectionReference::RemoveLiveGroup { group_id } => {
                    Some(group_id)
                }
                _ => None,
            }));
        }
        _ => {}
    }
    group_ids.sort();
    group_ids.dedup();
    targets.extend(group_ids.into_iter().map(|id| UpdateApiTarget {
        family: UpdateApiTargetFamily::Group,
        object_id: Some(id),
        playback_number: None,
        cue_id: None,
        cue_number: None,
        validate_active_context: false,
    }));
    Ok(targets)
}

async fn update_targets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<UpdateTargetsQuery>,
) -> Result<Json<Vec<UpdateMenuResponseEntry>>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let mut entries = Vec::new();
    for target in referenced_update_targets(&state, &session)? {
        let modes = match target.family {
            UpdateApiTargetFamily::Cue => (
                update::UpdateMode::Cue(update::CueUpdateMode::ExistingOnly),
                update::UpdateMode::Cue(update::CueUpdateMode::AddNew),
            ),
            UpdateApiTargetFamily::Preset | UpdateApiTargetFamily::Group => (
                update::UpdateMode::ExistingContent(update::ExistingContentMode::UpdateExisting),
                update::UpdateMode::ExistingContent(update::ExistingContentMode::AddNew),
            ),
        };
        let existing = preview_update_request(
            &state,
            &session,
            &UpdateApiRequest {
                target: target.clone(),
                mode: modes.0,
                expected_revision: None,
                expected_programmer_revision: None,
            },
        );
        let add_new = preview_update_request(
            &state,
            &session,
            &UpdateApiRequest {
                target: target.clone(),
                mode: modes.1,
                expected_revision: None,
                expected_programmer_revision: None,
            },
        );
        let (Ok(existing), Ok(add_new)) = (existing, add_new) else {
            continue;
        };
        if query.filter == update::UpdateTargetFilter::EligibleForUpdateExisting
            && !existing.preview.has_real_change()
        {
            continue;
        }
        entries.push(UpdateMenuResponseEntry {
            target,
            revision: existing.revision,
            active_or_referenced: true,
            existing_preview: existing,
            add_new_preview: add_new,
        });
    }
    Ok(Json(entries))
}

#[derive(Deserialize)]
struct HighlightActionInput {
    action: HighlightAction,
}

async fn highlight_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<HighlightState>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let transition = current_highlight_transition(&state, &session)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    if apply_highlight_selection_write(&state, &session, transition.working_selection.as_ref())? {
        emit(
            &state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"source":"highlight_status_reconcile"}),
        );
    }
    sync_highlight_output(&state);
    Ok(Json(transition.state))
}

async fn highlight_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<HighlightActionInput>,
) -> Result<Json<HighlightState>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let selection = state
        .programmers
        .selection(session.id)
        .ok_or_else(|| ApiError::not_found("programmer selection"))?;
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    let transition = state
        .highlight
        .action_guarded(
            session.desk.id,
            session.user.id,
            Some(&session.user.name),
            input.action,
            &selection,
            &fixtures,
            &groups,
            programmer.blind || programmer.preview,
        )
        .map_err(|error| match error {
            HighlightError::OwnedByAnotherUser(_) => ApiError::conflict(error.to_string()),
        })?;
    if apply_highlight_selection_write(&state, &session, transition.working_selection.as_ref())? {
        emit(
            &state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"source":"highlight","action":input.action}),
        );
    }
    sync_highlight_output(&state);
    emit(
        &state,
        "highlight_changed",
        serde_json::json!({
            "desk_id": session.desk.id,
            "user_id": session.user.id,
            "action": input.action,
            "state": &transition.state,
        }),
    );
    send_osc_feedback(&state, false);
    Ok(Json(transition.state))
}

fn highlight_fixture_summaries(
    fixtures: &[light_fixture::PatchedFixture],
) -> Vec<HighlightFixture> {
    let mut summaries = Vec::new();
    let mut seen = HashSet::new();
    for fixture in fixtures {
        let base_name = if fixture.name.trim().is_empty() {
            fixture.definition.display_name()
        } else {
            &fixture.name
        };
        if seen.insert(fixture.fixture_id) {
            summaries.push(HighlightFixture {
                fixture_id: fixture.fixture_id,
                name: Some(base_name.to_owned()),
                number: fixture.fixture_number,
            });
        }
        for patched_head in &fixture.logical_heads {
            if !seen.insert(patched_head.fixture_id) {
                continue;
            }
            let head_name = fixture
                .definition
                .heads
                .iter()
                .find(|head| head.index == patched_head.head_index)
                .map(|head| head.name.as_str())
                .unwrap_or("Head");
            summaries.push(HighlightFixture {
                fixture_id: patched_head.fixture_id,
                name: Some(format!("{base_name} / {head_name}")),
                number: fixture.fixture_number,
            });
        }
    }
    summaries
}

fn highlight_groups(
    snapshot: &EngineSnapshot,
) -> HashMap<String, light_programmer::GroupDefinition> {
    snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect()
}

fn apply_highlight_selection_write(
    state: &AppState,
    session: &Session,
    write: Option<&HighlightSelectionWrite>,
) -> Result<bool, ApiError> {
    let Some(write) = write else {
        return Ok(false);
    };
    match write.expression.clone() {
        Some(expression) => {
            state
                .programmers
                .select_expression(session.id, write.selected.clone(), expression);
        }
        None => {
            state.programmers.select(session.id, write.selected.clone());
        }
    }
    let selection = state
        .programmers
        .selection(session.id)
        .ok_or_else(|| ApiError::not_found("programmer selection"))?;
    state
        .highlight
        .acknowledge_internal_selection(session.desk.id, session.user.id, &selection);
    persist_programmer(state, session)?;
    Ok(true)
}

fn current_highlight_transition(
    state: &AppState,
    session: &Session,
) -> Option<HighlightTransition> {
    let programmer = state.programmers.get(session.id)?;
    let selection = state.programmers.selection(session.id)?;
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    Some(state.highlight.status(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        &selection,
        &fixtures,
        &groups,
        programmer.blind || programmer.preview,
    ))
}

fn reconcile_highlight_selection(
    state: &AppState,
    session: &Session,
    source: &str,
) -> Option<HighlightState> {
    let transition = current_highlight_transition(state, session)?;
    let selection_changed = match apply_highlight_selection_write(
        state,
        session,
        transition.working_selection.as_ref(),
    ) {
        Ok(changed) => changed,
        Err(error) => {
            emit(
                state,
                "highlight_rejected",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "user_id":session.user.id,
                    "source":source,
                    "error":error.message,
                }),
            );
            return None;
        }
    };
    if selection_changed {
        emit(
            state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id,"source":source,"action":"highlight_selection_reconcile"}),
        );
    }
    sync_highlight_output(state);
    emit(
        state,
        "highlight_changed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "user_id":session.user.id,
            "source":source,
            "state":&transition.state,
        }),
    );
    send_osc_feedback(state, false);
    Some(transition.state)
}

fn sync_highlight_output(state: &AppState) {
    let mut fixtures = state
        .highlight
        .output_fixtures()
        .into_iter()
        .collect::<HashSet<_>>();
    for preview in state.patch_preview_highlights.lock().values() {
        fixtures.extend(preview.iter().copied());
    }
    state.engine.set_highlighted_fixtures(fixtures);
}

#[derive(Deserialize)]
struct PatchPreviewHighlightInput {
    active: bool,
    #[serde(default)]
    fixture_ids: Vec<light_core::FixtureId>,
}

async fn patch_preview_highlight(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PatchPreviewHighlightInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let allowed = state.configuration.read().patch_preview_highlight_dmx;
    let mut active = false;
    if allowed && input.active && !input.fixture_ids.is_empty() {
        let known = state
            .engine
            .snapshot()
            .fixtures
            .iter()
            .flat_map(selectable_fixture_ids)
            .collect::<HashSet<_>>();
        let fixtures = input
            .fixture_ids
            .into_iter()
            .filter(|fixture| known.contains(fixture))
            .collect::<HashSet<_>>();
        active = !fixtures.is_empty();
        if active {
            state
                .patch_preview_highlights
                .lock()
                .insert(session.id, fixtures);
        } else {
            state.patch_preview_highlights.lock().remove(&session.id);
        }
    } else {
        state.patch_preview_highlights.lock().remove(&session.id);
    }
    sync_highlight_output(&state);
    emit(
        &state,
        "patch_preview_highlight_changed",
        serde_json::json!({"session_id":session.id,"active":active}),
    );
    Ok(Json(serde_json::json!({"active":active,"allowed":allowed})))
}

fn reconcile_highlight_capture_mode(
    state: &AppState,
    session: &Session,
    source: &str,
) -> Option<HighlightState> {
    reconcile_highlight_selection(state, session, source)
}
async fn audit_events(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Event>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(
        state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.revision > query.after)
            .cloned()
            .collect(),
    ))
}

const COMMAND_HISTORY_LIMIT: usize = 50;

async fn command_history(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CommandHistoryEntry>>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let entries = state
        .command_history
        .lock()
        .get(&session.desk.id)
        .map(|history| history.iter().cloned().collect())
        .unwrap_or_default();
    Ok(Json(entries))
}
async fn clear_programmer(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let session_id = SessionId(id);
    let user_id = state
        .programmers
        .get(session_id)
        .map(|programmer| programmer.user_id);
    if !state.programmers.clear(session_id) {
        return Ok(StatusCode::NOT_FOUND);
    }
    if let Err(error) = state.desk.lock().delete_session(session_id) {
        tracing::error!(%error, "failed to remove persisted programmer");
        return Ok(StatusCode::INTERNAL_SERVER_ERROR);
    }
    // Values belong to the user's shared programmer. Recreate that value layer
    // while keeping a desk-local command projection for every live session.
    if let Some(user_id) = user_id {
        let connected = state
            .sessions
            .read()
            .values()
            .filter(|candidate| candidate.user.id == user_id)
            .cloned()
            .collect::<Vec<_>>();
        for connected_session in connected {
            state.programmers.start(connected_session.id, user_id);
            attach_session_command_context(&state, &connected_session);
            persist_programmer(&state, &connected_session)?;
        }
    }
    emit(
        &state,
        "programmer_cleared",
        serde_json::json!({"session_id":id}),
    );
    Ok(StatusCode::NO_CONTENT)
}
async fn set_programmer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ProgrammerSet>,
) -> Result<StatusCode, ApiError> {
    let session = authenticate(&state, &headers)?;
    state.programmers.set(
        session.id,
        input.fixture_id,
        light_core::AttributeKey(input.attribute),
        light_core::AttributeValue::Normalized(input.value),
    );
    persist_programmer(&state, &session)?;
    emit(
        &state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id}),
    );
    Ok(StatusCode::NO_CONTENT)
}
async fn update_master(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<MasterInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let mut control = state.output_control.lock();
    if let Some(level) = input.grand_master {
        if !level.is_finite() || !(0.0..=1.0).contains(&level) {
            return Err(ApiError::bad_request("grand_master must be within 0-1"));
        }
        control.options.grand_master = level;
    }
    if let Some(blackout) = input.blackout {
        control.options.blackout = blackout;
    }
    let result = serde_json::json!({"grand_master":control.options.grand_master,"blackout":control.options.blackout});
    drop(control);
    persist_output_runtime(&state)?;
    emit(
        &state,
        "master_changed",
        serde_json::json!({"session_id":session.id,"state":result}),
    );
    Ok(Json(result))
}
async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let protocols = headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let token = protocols
        .split(',')
        .map(str::trim)
        .find_map(|protocol| protocol.strip_prefix("light.token."))
        .ok_or_else(|| ApiError::unauthorized("WebSocket session token protocol is missing"))?;
    let session = authenticate_token(&state, token)?;
    Ok(ws
        .protocols(["light.v1"])
        .on_upgrade(move |socket| handle_socket(socket, state, session))
        .into_response())
}
async fn handle_socket(mut socket: WebSocket, state: AppState, session: Session) {
    {
        let mut connections = state.ws_connections.lock();
        *connections.entry(session.id).or_insert(0) += 1;
    }
    state.programmers.connect(session.id);
    let _ = persist_programmer(&state, &session);
    let mut receiver = state.events.subscribe();
    loop {
        tokio::select! { event = receiver.recv() => match event { Ok(event) => { let Ok(json)=serde_json::to_string(&event) else { continue; }; if socket.send(Message::Text(json.into())).await.is_err() { break; } }, Err(_) => break }, incoming = socket.recv() => match incoming { Some(Ok(Message::Close(_))) | None => break, Some(Ok(Message::Ping(v))) => { let _ = socket.send(Message::Pong(v)).await; }, Some(Ok(Message::Text(text))) => { let response = match serde_json::from_str::<WsCommand>(&text) { Ok(command) => dispatch_ws_command(&state, &session, command), Err(error) => WsResponse { protocol_version: 1, request_id: String::new(), ok: false, revision: state.engine.snapshot().revision, payload: None, error: Some(format!("invalid command envelope: {error}")) } }; let Ok(json)=serde_json::to_string(&response) else { continue; }; if socket.send(Message::Text(json.into())).await.is_err() { break; } }, _ => {} } }
    }
    finish_event_socket(&state, &session);
}

fn finish_event_socket(state: &AppState, session: &Session) {
    let disconnected = {
        let mut connections = state.ws_connections.lock();
        if let Some(count) = connections.get_mut(&session.id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                connections.remove(&session.id);
                true
            } else {
                false
            }
        } else {
            false
        }
    };
    if disconnected {
        // An event socket is only one transport attached to the authenticated
        // control-desk session. Short-lived command sockets and browser
        // reconnects must retain the Desk's input context; only close_session
        // ends that session and releases its owned context.
        let _ = persist_programmer(state, session);
    }
}

fn lock_live_input(state: &AppState, session: &Session, key: String) -> Result<(), String> {
    let now = Instant::now();
    let mut locks = state.input_locks.lock();
    locks.retain(|_, (_, expires)| *expires > now);
    if let Some((owner, _)) = locks.get(&key)
        && *owner != session.user.id
    {
        return Err(format!(
            "input {key} is currently controlled by another user"
        ));
    }
    locks.insert(key, (session.user.id, now + Duration::from_secs(1)));
    Ok(())
}

#[derive(Debug)]
struct StagedPreloadPlaybackAction {
    playback_number: u16,
    action: String,
    surface: String,
    released_playbacks: Vec<u16>,
}

fn apply_preload_playback_verb(
    playback: &mut light_playback::PlaybackEngine,
    number: u16,
    action: &str,
) -> Result<(), String> {
    match action {
        "toggle" => playback.toggle(number).map(|_| ()),
        "go" => playback.go_playback(number).map(|_| ()),
        "go-minus" => playback.back_playback(number).map(|_| ()),
        "off" => playback.off(number).map(|_| ()),
        "on" => playback.on(number).map(|_| ()),
        "temp-on" => playback.set_temp_button(number, true),
        "temp-off" => playback.set_temp_button(number, false),
        _ => Err(format!("unsupported queued Preload action {action}")),
    }
}

/// Build one complete Preload playback result without changing the live engine. A rejected verb,
/// stale definition, or timing error therefore discards only this clone, even when it follows
/// actions that would otherwise have succeeded.
fn stage_preload_playback_batch(
    current: &light_playback::PlaybackEngine,
    definitions: &[(
        light_programmer::PreloadPlaybackAction,
        light_playback::PlaybackDefinition,
    )],
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
    exclusion_zones: &[Vec<u16>],
) -> Result<
    (
        light_playback::PlaybackEngine,
        Vec<StagedPreloadPlaybackAction>,
    ),
    String,
> {
    let mut staged = current.clone();
    let mut actions = Vec::with_capacity(definitions.len());
    for (pending, definition) in definitions {
        let previous = staged
            .runtime()
            .into_iter()
            .find(|playback| playback.playback_number == Some(definition.number))
            .map(|playback| (playback.enabled, playback.master));
        let was_enabled = previous.is_some_and(|(enabled, _)| enabled);

        apply_preload_playback_verb(&mut staged, definition.number, &pending.action)?;
        let now_enabled = staged.runtime().into_iter().any(|playback| {
            playback.playback_number == Some(definition.number) && playback.enabled
        });
        let released_playbacks = if !was_enabled && now_enabled {
            enforce_virtual_playback_exclusions_on(&mut staged, exclusion_zones, definition.number)
        } else {
            Vec::new()
        };
        staged.apply_preload_timing(
            definition.number,
            &pending.action,
            committed_at,
            programmer_fade_millis,
            previous,
        )?;
        actions.push(StagedPreloadPlaybackAction {
            playback_number: definition.number,
            action: pending.action.clone(),
            surface: pending.surface.clone(),
            released_playbacks,
        });
    }
    Ok((staged, actions))
}

fn record_preload_persistence_failure(
    state: &AppState,
    session: &Session,
    domain: &str,
    error: ApiError,
) -> String {
    let warning = format!(
        "Preload committed but {domain} persistence failed: {}",
        error.message
    );
    emit(
        state,
        "preload_persistence_failed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "domain":domain,
            "source":"preload",
            "accepted":true,
            "error":error.message,
        }),
    );
    warning
}

fn commit_preload(state: &AppState, session: &Session) -> Result<serde_json::Value, String> {
    // Use the same lock ordering as normal playback actions: playback serialization first, then
    // the user's reentrant Programmer transaction gate. This keeps queued actions stable while
    // the candidate playback engine is validated.
    let _serialized = state.playback_action_lock.lock();
    state
        .programmers
        .with_transaction(session.id, || commit_preload_transaction(state, session))
}

fn commit_preload_transaction(
    state: &AppState,
    session: &Session,
) -> Result<serde_json::Value, String> {
    let pending = state
        .programmers
        .get(session.id)
        .ok_or_else(|| "programmer does not exist".to_owned())?
        .preload_playback_pending;
    let snapshot = state.engine.snapshot();
    let definitions = pending
        .iter()
        .map(|action| {
            snapshot
                .playbacks
                .iter()
                .find(|definition| definition.number == action.playback_number)
                .cloned()
                .map(|definition| (action.clone(), definition))
                .ok_or_else(|| format!("playback {} no longer exists", action.playback_number))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let committed_at = state.programmers.clock().now();
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    let exclusion_zones = virtual_playback_zone_numbers(state, session.desk.id);
    let playback = state.engine.playback();
    let mut live_playback = playback.write();
    let (staged_playback, staged_actions) = stage_preload_playback_batch(
        &live_playback,
        &definitions,
        committed_at,
        programmer_fade_millis,
        &exclusion_zones,
    )?;

    // Nothing live has changed before this point. The Programmer transaction restores its exact
    // checkpoint if the queue somehow differs despite holding the per-user mutation gate.
    state
        .programmers
        .activate_preload_at(session.id, committed_at);
    let drained = state.programmers.take_preload_playback_actions(session.id);
    if drained != pending {
        return Err("the Preload queue changed while GO was being prepared".into());
    }

    // Publishing the already validated clone is the only live playback mutation and cannot fail.
    // Engine resolution acquires Playback before reading Programmer sources, so retaining this
    // write guard across Programmer activation also exposes the combined result at one render
    // boundary instead of allowing a torn frame between the two domains.
    *live_playback = staged_playback;
    drop(live_playback);

    for action in &staged_actions {
        if !action.released_playbacks.is_empty() {
            emit(
                state,
                "playback_exclusion_applied",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "activated_playback":action.playback_number,
                    "released_playbacks":action.released_playbacks,
                    "source":"preload",
                }),
            );
        }
    }
    let executed = staged_actions
        .into_iter()
        .map(|action| {
            serde_json::json!({
                "playback_number":action.playback_number,
                "action":action.action,
                "surface":action.surface,
                "started_at":committed_at,
                "fallback_millis":programmer_fade_millis
            })
        })
        .collect::<Vec<_>>();

    // Persistence is deliberately downstream of the commit point. A disk/store error is an
    // accepted operation with an explicit warning and audit event, never a false rejection after
    // the live Programmer and Playback states have changed.
    let mut warnings = Vec::new();
    if let Err(error) = persist_programmer(state, session) {
        warnings.push(record_preload_persistence_failure(
            state,
            session,
            "programmer",
            error,
        ));
    }
    if !executed.is_empty() {
        if let Err(error) = persist_active_playbacks(state) {
            warnings.push(record_preload_persistence_failure(
                state,
                session,
                "active playbacks",
                error,
            ));
        }
        if let Err(error) = persist_output_runtime(state) {
            warnings.push(record_preload_persistence_failure(
                state,
                session,
                "output runtime",
                error,
            ));
        }
    }

    let mut payload = serde_json::json!({
        "session_id":session.id,
        "application_timestamp":committed_at,
        "programmer_fade_millis":programmer_fade_millis,
        "playback_actions":executed
    });
    if !warnings.is_empty() {
        payload["warnings"] = serde_json::json!(warnings);
    }
    emit(state, "preload_committed", payload.clone());
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"preload_committed_at":committed_at}),
    );
    if !executed.is_empty() {
        emit(
            state,
            "playback_changed",
            serde_json::json!({"session_id":session.id,"source":"preload","application_timestamp":committed_at,"actions":executed}),
        );
    }
    let mut response = serde_json::json!({
        "active":true,
        "application_timestamp":committed_at,
        "programmer_fade_millis":programmer_fade_millis,
        "playback_actions":payload["playback_actions"],
        "programmer":state.programmers.get(session.id)
    });
    if let Some(warnings) = payload.get("warnings") {
        response["warnings"] = warnings.clone();
    }
    Ok(response)
}

fn validate_programmer_attribute_value(value: &light_core::AttributeValue) -> Result<(), String> {
    match value {
        light_core::AttributeValue::Normalized(value)
            if !value.is_finite() || !(0.0..=1.0).contains(value) =>
        {
            return Err("normalized value must be within 0-1".into());
        }
        light_core::AttributeValue::Spread(_) => {
            return Err("spread values require a Group programming command".into());
        }
        light_core::AttributeValue::Discrete(value) if value.trim().is_empty() => {
            return Err("discrete value must contain a semantic identifier".into());
        }
        light_core::AttributeValue::ColorXyz(value)
            if !value.x.is_finite()
                || !value.y.is_finite()
                || !value.z.is_finite()
                || value.x < 0.0
                || value.y < 0.0
                || value.z < 0.0 =>
        {
            return Err("XYZ color components must be finite and non-negative".into());
        }
        _ => {}
    }
    Ok(())
}

fn profile_head_owner(
    fixture: &light_fixture::PatchedFixture,
    mode: &light_fixture::FixtureMode,
    head_id: Uuid,
) -> Result<light_core::FixtureId, String> {
    let (head_index, head) = mode
        .heads
        .iter()
        .enumerate()
        .find(|(_, head)| head.id == head_id)
        .ok_or("fixture profile channel references a missing head")?;
    if head.master_shared {
        return Ok(fixture.fixture_id);
    }
    fixture
        .logical_heads
        .iter()
        .find(|head| usize::from(head.head_index) == head_index)
        .or_else(|| {
            fixture
                .logical_heads
                .iter()
                .find(|head| usize::from(head.head_index) == head_index + 1)
        })
        .map(|head| head.fixture_id)
        .ok_or_else(|| {
            format!(
                "fixture {} is missing logical head {head_index}",
                fixture.fixture_id.0
            )
        })
}

type ControlActionProgrammerAssignment = (
    light_core::FixtureId,
    light_core::AttributeKey,
    light_core::AttributeValue,
);

type ControlActionProgrammerValues = (
    Vec<ControlActionProgrammerAssignment>,
    Option<u64>,
    light_fixture::ControlActionKind,
);

fn control_action_programmer_values(
    snapshot: &EngineSnapshot,
    fixture_id: light_core::FixtureId,
    action_id: Uuid,
    active: bool,
) -> Result<ControlActionProgrammerValues, String> {
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| {
            fixture.fixture_id == fixture_id
                || fixture
                    .logical_heads
                    .iter()
                    .any(|head| head.fixture_id == fixture_id)
        })
        .ok_or("fixture does not exist")?;
    let profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .ok_or("fixture does not use a schema-v2 profile")?;
    let mode_id = fixture
        .definition
        .mode_id
        .ok_or("fixture profile mode is unavailable")?;
    let mode = profile
        .mode(mode_id)
        .ok_or("fixture profile mode does not exist")?;
    let action = mode
        .control_actions
        .iter()
        .find(|action| action.id == action_id)
        .ok_or("control action does not exist")?;
    let duration = (active && action.kind == light_fixture::ControlActionKind::TimedPulse)
        .then_some(action.duration_millis.unwrap_or(0));
    let assignments = mode
        .control_action_values(action_id, active)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|(channel_id, value)| {
            let channel = mode
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .ok_or("control action references a missing channel")?;
            Ok((
                profile_head_owner(fixture, mode, channel.head_id)?,
                light_fixture::FixtureMode::control_action_attribute(channel.id),
                light_core::AttributeValue::RawDmxExact(value),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((assignments, duration, action.kind))
}

#[derive(Clone, Debug)]
struct GeneratedProfilePreset {
    semantic_id: String,
    name: String,
    family: String,
    values: HashMap<
        light_core::FixtureId,
        HashMap<light_core::AttributeKey, light_core::AttributeValue>,
    >,
}

fn generated_profile_preset_family(attribute: &light_core::AttributeKey) -> &'static str {
    match light_core::attribute_descriptor(attribute).family {
        light_core::AttributeClass::Intensity => "Intensity",
        light_core::AttributeClass::Position => "Position",
        light_core::AttributeClass::Color => "Color",
        light_core::AttributeClass::Beam | light_core::AttributeClass::Focus => "Beam",
        light_core::AttributeClass::Control | light_core::AttributeClass::Custom => "Mixed",
    }
}

fn generated_profile_presets(
    snapshot: &EngineSnapshot,
    selected: &HashSet<light_core::FixtureId>,
) -> Result<Vec<GeneratedProfilePreset>, String> {
    let mut generated = BTreeMap::<(String, String), GeneratedProfilePreset>::new();
    for fixture in &snapshot.fixtures {
        let physical_selected = selected.contains(&fixture.fixture_id);
        if !physical_selected
            && !fixture
                .logical_heads
                .iter()
                .any(|head| selected.contains(&head.fixture_id))
        {
            continue;
        }
        let Some(profile) = fixture.definition.profile_snapshot.as_deref() else {
            continue;
        };
        let Some(mode) = fixture
            .definition
            .mode_id
            .and_then(|mode_id| profile.mode(mode_id))
        else {
            continue;
        };
        for channel in &mode.channels {
            let owner = profile_head_owner(fixture, mode, channel.head_id)?;
            if !physical_selected && !selected.contains(&owner) {
                continue;
            }
            for function in &channel.functions {
                let (semantic_id, label) = match &function.behavior {
                    light_fixture::ChannelFunctionBehavior::Fixed {
                        semantic_id, label, ..
                    }
                    | light_fixture::ChannelFunctionBehavior::Indexed {
                        semantic_id, label, ..
                    } => (semantic_id, label),
                    _ => continue,
                };
                let family = generated_profile_preset_family(&function.attribute).to_owned();
                let preset = generated
                    .entry((family.clone(), semantic_id.clone()))
                    .or_insert_with(|| GeneratedProfilePreset {
                        semantic_id: semantic_id.clone(),
                        name: label.clone(),
                        family,
                        values: HashMap::new(),
                    });
                if label < &preset.name {
                    preset.name.clone_from(label);
                }
                preset.values.entry(owner).or_default().insert(
                    function.attribute.clone(),
                    light_core::AttributeValue::Discrete(semantic_id.clone()),
                );
            }
        }
    }
    Ok(generated.into_values().collect())
}

fn generate_profile_presets(
    state: &AppState,
    fixture_ids: Vec<light_core::FixtureId>,
) -> Result<serde_json::Value, String> {
    if fixture_ids.is_empty() {
        return Err("select at least one fixture before generating presets".into());
    }
    let generated =
        generated_profile_presets(&state.engine.snapshot(), &fixture_ids.into_iter().collect())?;
    if generated.is_empty() {
        return Err("the selected fixtures have no fixed or indexed values".into());
    }
    let (entry, store) = active_show_store(state)?;
    let existing = store.objects("preset").map_err(|error| error.to_string())?;
    let mut used = HashMap::<light_programmer::PresetFamily, HashSet<u32>>::new();
    for object in &existing {
        let (address, _) = decode_preset_object(object)?;
        used.entry(address.family)
            .or_default()
            .insert(address.number);
    }
    let mut ids = Vec::with_capacity(generated.len());
    let mut bodies = Vec::with_capacity(generated.len());
    let mut created = Vec::with_capacity(generated.len());
    for preset in generated {
        let family: light_programmer::PresetFamily =
            serde_json::from_value(serde_json::Value::String(preset.family.clone()))
                .map_err(|error| format!("invalid generated preset family: {error}"))?;
        let family_used = used.entry(family).or_default();
        let mut number = 1_u32;
        while family_used.contains(&number) {
            number += 1;
        }
        let address = light_programmer::PresetAddress::new(family, number)?;
        let storage_key = address.storage_key();
        family_used.insert(number);
        let mut body = serde_json::to_value(light_programmer::Preset {
            name: preset.name.clone(),
            family,
            number,
            values: preset.values,
            group_values: HashMap::new(),
        })
        .map_err(|error| error.to_string())?;
        body["generated_from_fixture_profile"] = serde_json::json!({
            "semantic_id":preset.semantic_id,
        });
        created.push(serde_json::json!({
            "address":address,
            "number":number,
            "name":preset.name,
            "family":preset.family,
        }));
        ids.push(storage_key);
        bodies.push(body);
    }
    let writes = ids
        .iter()
        .zip(&bodies)
        .map(|(id, body)| AtomicObjectWrite {
            kind: "preset",
            id,
            body,
            expected: 0,
        })
        .collect::<Vec<_>>();
    backup_show(state, &entry).map_err(|error| error.message)?;
    let revisions = store
        .mutate_objects_atomically(&writes, &[])
        .map_err(|error| error.to_string())?;
    refresh_command_show(state, &entry)?;
    for ((id, revision), item) in ids.iter().zip(revisions).zip(&created) {
        emit_command_object_changed(state, &entry, "preset", id, revision);
        emit(
            state,
            "preset_generated",
            serde_json::json!({"show_id":entry.id,"preset":item,"revision":revision}),
        );
    }
    Ok(serde_json::json!({"created":created}))
}

fn dispatch_ws_command(state: &AppState, session: &Session, command: WsCommand) -> WsResponse {
    let revision = state.engine.snapshot().revision;
    let fail = |message: String| WsResponse {
        protocol_version: 1,
        request_id: command.request_id.clone(),
        ok: false,
        revision,
        payload: None,
        error: Some(message),
    };
    if read_desk_lock(state, session.desk.id).locked {
        return fail("desk is locked".into());
    }
    if command.protocol_version != 1 {
        return fail("unsupported protocol_version".into());
    }
    if command.session_id != session.id {
        return fail("session_id does not own this connection".into());
    }
    let live_absolute = matches!(
        command.command.as_str(),
        "selection.set"
            | "selection.gesture"
            | "selection.macro"
            | "group.select"
            | "programmer.set"
            | "programmer.set_many"
            | "programmer.set_value"
            | "programmer.control_action"
            | "programmer.priority"
            | "programmer.release"
            | "programmer.group.set"
            | "programmer.group.release"
            | "programmer.align"
            | "programmer.command_line"
            | "programmer.command_target"
            | "programmer.execute"
            | "programmer.clear"
            | "programmer.undo"
            | "programmer.redo"
            | "programmer.mode"
            | "master.set"
            | "group.master.set"
            | "group.master.flash"
            | "preload.enter"
            | "preload.group.set"
            | "preload.go"
            | "preload.clear"
            | "preload.release"
            | "playback.go"
            | "playback.back"
            | "playback.pause"
            | "playback.release"
            | "preset.apply"
    );
    let command_operation =
        live_absolute.then(|| state.command_http.operation_lock(session.desk.id));
    let _command_operation_guard = command_operation.as_ref().map(|lock| lock.lock());
    let selection_revision_before = state
        .programmers
        .selection(session.id)
        .map(|selection| selection.revision);
    if !live_absolute
        && command
            .expected_revision
            .is_some_and(|expected| expected != revision)
    {
        return fail(format!("revision conflict: current revision is {revision}"));
    }
    let result: Result<serde_json::Value, String> = (|| match command.command.as_str() {
        "selection.set" => {
            #[derive(Deserialize)]
            struct Input {
                fixtures: Vec<light_core::FixtureId>,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let snapshot = state.engine.snapshot();
            state.programmers.select(
                session.id,
                expand_selectable_fixture_ids(&snapshot.fixtures, input.fixtures),
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "selection.gesture" => {
            #[derive(Deserialize)]
            #[serde(tag = "type", rename_all = "snake_case")]
            enum Source {
                Fixture { fixture_id: light_core::FixtureId },
                LiveGroup { group_id: String },
                DereferencedGroup { group_id: String },
            }
            #[derive(Deserialize)]
            struct Input {
                source: Source,
                #[serde(default)]
                remove: bool,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let snapshot = state.engine.snapshot();
            let groups = snapshot
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.clone()))
                .collect::<HashMap<_, _>>();
            let references = match input.source {
                Source::Fixture { fixture_id } => {
                    let Some(fixture) = snapshot.fixtures.iter().find(|fixture| {
                        fixture.fixture_id == fixture_id
                            || fixture
                                .logical_heads
                                .iter()
                                .any(|head| head.fixture_id == fixture_id)
                    }) else {
                        return Err("fixture does not exist".into());
                    };
                    let selectable = if fixture.fixture_id == fixture_id {
                        selectable_fixture_ids(fixture)
                    } else {
                        vec![fixture_id]
                    };
                    selectable
                        .into_iter()
                        .map(|fixture_id| {
                            if input.remove {
                                light_programmer::SelectionReference::RemoveFixture { fixture_id }
                            } else {
                                light_programmer::SelectionReference::Fixture { fixture_id }
                            }
                        })
                        .collect()
                }
                Source::LiveGroup { group_id } => {
                    light_programmer::resolve_group(&group_id, &groups)?;
                    vec![if input.remove {
                        light_programmer::SelectionReference::RemoveLiveGroup { group_id }
                    } else {
                        light_programmer::SelectionReference::LiveGroup { group_id }
                    }]
                }
                Source::DereferencedGroup { group_id } => {
                    light_programmer::resolve_group(&group_id, &groups)?
                        .into_iter()
                        .map(|fixture_id| {
                            if input.remove {
                                light_programmer::SelectionReference::RemoveFixture { fixture_id }
                            } else {
                                light_programmer::SelectionReference::Fixture { fixture_id }
                            }
                        })
                        .collect()
                }
            };
            state
                .programmers
                .apply_selection_gesture(session.id, references, &groups);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "group.select" => {
            #[derive(Deserialize)]
            struct Input {
                group_id: String,
                #[serde(default)]
                frozen: bool,
                rule: Option<light_programmer::SelectionRule>,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let snapshot = state.engine.snapshot();
            let groups = snapshot
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.clone()))
                .collect::<HashMap<_, _>>();
            let mut fixtures = light_programmer::resolve_group(&input.group_id, &groups)?;
            let rule = input.rule.unwrap_or(light_programmer::SelectionRule::All);
            rule.validate()?;
            fixtures = light_programmer::apply_selection_rule(&fixtures, &rule);
            let expression = if input.frozen {
                light_programmer::SelectionExpression::FrozenGroup {
                    group_id: input.group_id,
                    source_revision: snapshot.revision,
                }
            } else {
                light_programmer::SelectionExpression::LiveGroup {
                    group_id: input.group_id,
                    rule,
                }
            };
            state
                .programmers
                .select_expression(session.id, fixtures, expression);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "selection.macro" => {
            #[derive(Deserialize)]
            struct Input {
                rule: light_programmer::SelectionRule,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            input.rule.validate()?;
            let current = state
                .programmers
                .get(session.id)
                .ok_or("programmer does not exist")?;
            let (base, expression) = match current.selection_expression {
                Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) => {
                    let snapshot = state.engine.snapshot();
                    let groups = snapshot
                        .groups
                        .iter()
                        .map(|group| (group.id.clone(), group.clone()))
                        .collect::<HashMap<_, _>>();
                    let base = light_programmer::resolve_group(&group_id, &groups)?;
                    (
                        base,
                        light_programmer::SelectionExpression::LiveGroup {
                            group_id,
                            rule: input.rule.clone(),
                        },
                    )
                }
                _ => (
                    current.selected,
                    light_programmer::SelectionExpression::Static,
                ),
            };
            let fixtures = light_programmer::apply_selection_rule(&base, &input.rule);
            state
                .programmers
                .select_expression(session.id, fixtures, expression);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "programmer.align" => {
            #[derive(Deserialize)]
            struct Input {
                attribute: String,
                mode: String,
                #[serde(default)]
                from: f32,
                #[serde(default = "one_f32")]
                to: f32,
            }
            fn one_f32() -> f32 {
                1.0
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let selected = state
                .programmers
                .get(session.id)
                .ok_or("programmer does not exist")?
                .selected;
            let snapshot = state.engine.snapshot();
            let mut supported = Vec::new();
            let mut unsupported = Vec::new();
            for fixture_id in selected {
                let parameter = snapshot.fixtures.iter().find_map(|fixture| {
                    let owns_parent = fixture.fixture_id == fixture_id;
                    fixture.definition.heads.iter().find_map(|head| {
                        let owns_head = head.shared && owns_parent
                            || fixture.logical_heads.iter().any(|patched| {
                                patched.fixture_id == fixture_id && patched.head_index == head.index
                            });
                        owns_head
                            .then(|| {
                                head.parameters
                                    .iter()
                                    .find(|parameter| parameter.attribute.0 == input.attribute)
                            })
                            .flatten()
                    })
                });
                match parameter {
                    Some(parameter) if parameter.capabilities.is_empty() => {
                        supported.push((fixture_id, parameter.metadata.wrap))
                    }
                    Some(_) => {
                        return Err(format!(
                            "{} is discrete and cannot be aligned",
                            input.attribute
                        ));
                    }
                    None => unsupported.push(fixture_id),
                }
            }
            if supported.is_empty() {
                return Err(format!(
                    "none of the selected fixtures support {}",
                    input.attribute
                ));
            }
            for (index, (fixture, wraps)) in supported.iter().enumerate() {
                let value = aligned_normalized(
                    &input.mode,
                    index,
                    supported.len(),
                    input.from,
                    input.to,
                    *wraps,
                )?;
                state.programmers.set(
                    session.id,
                    *fixture,
                    light_core::AttributeKey(input.attribute.clone()),
                    light_core::AttributeValue::Normalized(value),
                );
            }
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(
                serde_json::json!({"programmer":state.programmers.get(session.id),"unsupported_fixtures":unsupported}),
            )
        }
        "programmer.group.set" => {
            #[derive(Deserialize)]
            struct Input {
                group_id: String,
                attribute: String,
                value: serde_json::Value,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let value = if let Some(value) = input.value.as_f64() {
                light_core::AttributeValue::Normalized(value as f32)
            } else {
                serde_json::from_value::<light_core::AttributeValue>(input.value)
                    .map_err(|error| format!("group value is invalid: {error}"))?
            };
            match &value {
                light_core::AttributeValue::Normalized(value)
                    if !value.is_finite() || !(0.0..=1.0).contains(value) =>
                {
                    return Err("value must be within 0-1".into());
                }
                light_core::AttributeValue::Spread(points)
                    if points.len() < 2
                        || points
                            .iter()
                            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) =>
                {
                    return Err("spread requires at least two values within 0-1".into());
                }
                light_core::AttributeValue::Normalized(_)
                | light_core::AttributeValue::Spread(_) => {}
                _ => return Err("group value must be normalized or spread".into()),
            }
            if !state
                .engine
                .snapshot()
                .groups
                .iter()
                .any(|group| group.id == input.group_id)
            {
                return Err("group does not exist".into());
            }
            let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
            state.programmers.set_group_faded_with_timing(
                session.id,
                input.group_id,
                light_core::AttributeKey(input.attribute),
                value,
                Some(programmer_fade_millis),
                None,
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "programmer.group.release" => {
            #[derive(Deserialize)]
            struct Input {
                group_id: String,
                attribute: String,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            if !state
                .engine
                .snapshot()
                .groups
                .iter()
                .any(|group| group.id == input.group_id)
            {
                return Err("group does not exist".into());
            }
            let released = state.programmers.release_group_attribute(
                session.id,
                &input.group_id,
                &light_core::AttributeKey(input.attribute),
            );
            if released {
                persist_programmer(state, session).map_err(|e| e.message)?;
            }
            Ok(
                serde_json::json!({"released":released,"programmer":state.programmers.get(session.id)}),
            )
        }
        "programmer.priority" => {
            #[derive(Deserialize)]
            struct Input {
                priority: i16,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            if !state.programmers.set_priority(session.id, input.priority) {
                return Err("programmer does not exist".into());
            }
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "programmer.set" => {
            let input: ProgrammerSet =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            if !input.value.is_finite() || !(0.0..=1.0).contains(&input.value) {
                return Err("value must be within 0-1".into());
            }
            let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
            state.programmers.set_faded_with_timing(
                session.id,
                input.fixture_id,
                light_core::AttributeKey(input.attribute),
                light_core::AttributeValue::Normalized(input.value),
                Some(programmer_fade_millis),
                None,
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "programmer.set_many" => {
            let input: ProgrammerSetMany =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let snapshot = state.engine.snapshot();
            let fixture_exists = |fixture_id: light_core::FixtureId| {
                snapshot.fixtures.iter().any(|fixture| {
                    fixture.fixture_id == fixture_id
                        || fixture
                            .logical_heads
                            .iter()
                            .any(|head| head.fixture_id == fixture_id)
                })
            };
            let mut assignments = Vec::with_capacity(input.assignments.len());
            for assignment in input.assignments {
                if assignment.attribute.trim().is_empty() {
                    return Err("attribute is required".into());
                }
                if !assignment.value.is_finite() || !(0.0..=1.0).contains(&assignment.value) {
                    return Err("value must be within 0-1".into());
                }
                if !fixture_exists(assignment.fixture_id) {
                    return Err("fixture does not exist".into());
                }
                assignments.push((
                    assignment.fixture_id,
                    light_core::AttributeKey(assignment.attribute),
                    light_core::AttributeValue::Normalized(assignment.value),
                ));
            }
            let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
            state.programmers.set_many_faded_with_timing(
                session.id,
                assignments,
                Some(programmer_fade_millis),
                None,
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "programmer.set_value" => {
            #[derive(Deserialize)]
            struct Input {
                fixture_id: light_core::FixtureId,
                attribute: String,
                value: light_core::AttributeValue,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            if input.attribute.trim().is_empty() {
                return Err("attribute is required".into());
            }
            validate_programmer_attribute_value(&input.value)?;
            if !state.engine.snapshot().fixtures.iter().any(|fixture| {
                fixture.fixture_id == input.fixture_id
                    || fixture
                        .logical_heads
                        .iter()
                        .any(|head| head.fixture_id == input.fixture_id)
            }) {
                return Err("fixture does not exist".into());
            }
            let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
            state.programmers.set_faded_with_timing(
                session.id,
                input.fixture_id,
                light_core::AttributeKey(input.attribute),
                input.value,
                Some(programmer_fade_millis),
                None,
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "programmer.control_action" => {
            #[derive(Deserialize)]
            struct Input {
                fixture_id: light_core::FixtureId,
                action_id: Uuid,
                active: bool,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let snapshot = state.engine.snapshot();
            let (assignments, pulse_duration, kind) = control_action_programmer_values(
                &snapshot,
                input.fixture_id,
                input.action_id,
                input.active,
            )?;
            let transient_source =
                format!("fixture-control:{}:{}", input.fixture_id.0, input.action_id);
            let transient_generation = match (kind, input.active) {
                (light_fixture::ControlActionKind::Latched, _) => {
                    state.programmers.set_many(session.id, assignments);
                    persist_programmer(state, session).map_err(|e| e.message)?;
                    None
                }
                (_, true) => state.programmers.set_transient_action(
                    session.id,
                    transient_source.clone(),
                    assignments,
                ),
                (_, false) => {
                    state
                        .programmers
                        .release_transient_action(session.id, &transient_source, None);
                    None
                }
            };
            if let (Some(duration_millis), Some(generation)) =
                (pulse_duration, transient_generation)
            {
                let state = state.clone();
                let session = session.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(duration_millis)).await;
                    if !state.programmers.release_transient_action(
                        session.id,
                        &transient_source,
                        Some(generation),
                    ) {
                        return;
                    }
                    emit(
                        &state,
                        "programmer_changed",
                        serde_json::json!({
                            "session_id":session.id,
                            "command":"programmer.control_action",
                            "action_id":input.action_id,
                            "active":false,
                            "timed_pulse_complete":true,
                        }),
                    );
                });
            }
            Ok(serde_json::json!({
                "action_id":input.action_id,
                "active":input.active,
                "kind":kind,
                "pulse_duration_millis":pulse_duration,
                "programmer":state.programmers.get(session.id),
            }))
        }
        "preset.generate_fixture_values" => {
            #[derive(Deserialize)]
            struct Input {
                fixture_ids: Vec<light_core::FixtureId>,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            generate_profile_presets(state, input.fixture_ids)
        }
        "programmer.release" => {
            #[derive(Deserialize)]
            struct Input {
                fixture_id: light_core::FixtureId,
                attribute: String,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            if !state.engine.snapshot().fixtures.iter().any(|fixture| {
                fixture.fixture_id == input.fixture_id
                    || fixture
                        .logical_heads
                        .iter()
                        .any(|head| head.fixture_id == input.fixture_id)
            }) {
                return Err("fixture does not exist".into());
            }
            let released = state.programmers.release_fixture_attribute(
                session.id,
                input.fixture_id,
                &light_core::AttributeKey(input.attribute),
            );
            if released {
                persist_programmer(state, session).map_err(|e| e.message)?;
            }
            Ok(
                serde_json::json!({"released":released,"programmer":state.programmers.get(session.id)}),
            )
        }
        "programmer.clear" => {
            state.programmers.clear_values(session.id);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"cleared":true}))
        }
        "preload.enter" => {
            let capture_programmer = state.configuration.read().preload_programmer_changes;
            state
                .programmers
                .arm_preload(session.id, capture_programmer);
            persist_programmer(state, session).map_err(|e| e.message)?;
            reconcile_highlight_capture_mode(state, session, "preload");
            emit(
                state,
                "programmer_changed",
                serde_json::json!({"session_id":session.id,"preload_armed":true}),
            );
            Ok(serde_json::json!({"blind":true}))
        }
        "preload.group.set" => {
            #[derive(Deserialize)]
            struct Input {
                group_id: String,
                attribute: String,
                value: f32,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            if !(0.0..=1.0).contains(&input.value) {
                return Err("value must be within 0-1".into());
            }
            state.programmers.set_group(
                session.id,
                input.group_id,
                light_core::AttributeKey(input.attribute),
                light_core::AttributeValue::Normalized(input.value),
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            let programmer = state.programmers.get(session.id);
            let pending = programmer.as_ref().is_some_and(|programmer| {
                programmer.blind && programmer.preload_capture_programmer
            });
            Ok(serde_json::json!({"pending":pending,"programmer":programmer}))
        }
        "preload.go" => commit_preload(state, session),
        "preload.clear" => {
            state.programmers.clear_preload_pending(session.id);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"pending_cleared":true,"active_unchanged":true}))
        }
        "preload.release" => {
            let released = state.programmers.release_preload(session.id);
            if released {
                persist_programmer(state, session).map_err(|e| e.message)?;
                emit(
                    state,
                    "programmer_changed",
                    serde_json::json!({"session_id":session.id,"preload_released":true}),
                );
            }
            Ok(serde_json::json!({"released":released}))
        }
        "programmer.undo" => Ok(serde_json::json!({"changed":state.programmers.undo(session.id)})),
        "programmer.redo" => Ok(serde_json::json!({"changed":state.programmers.redo(session.id)})),
        "programmer.command_line" => {
            #[derive(Deserialize)]
            struct Input {
                value: String,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let was_armed = state
                .programmers
                .get(session.id)
                .is_some_and(|programmer| command_line_arms_update(&programmer.command_line));
            let is_armed = command_line_arms_update(&input.value);
            state.programmers.set_command_line(session.id, input.value);
            persist_programmer(state, session).map_err(|e| e.message)?;
            emit_update_armed_transition(state, session, was_armed, is_armed, "software");
            Ok(serde_json::json!({"updated":true}))
        }
        "programmer.command_target" => {
            #[derive(Deserialize)]
            struct Input {
                value: String,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            if !state
                .programmers
                .set_command_target(session.id, input.value.to_ascii_uppercase())
            {
                return Err("command target must be FIXTURE or GROUP".into());
            }
            Ok(serde_json::json!({"updated":true}))
        }
        "programmer.execute" => {
            #[derive(Deserialize)]
            struct Input {
                value: String,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            match command_http::execute_existing_command(
                state,
                session,
                &input.value,
                "software",
                Some(&command.request_id),
                command_http::ExistingCommandPolicy::Compatibility,
            ) {
                command_http::ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
                    Ok(serde_json::json!({
                        "applied":0,
                        "pending_choice":pending_choice,
                        "programmer":state.programmers.get(session.id)
                    }))
                }
                command_http::ExistingCommandOutcome::Accepted {
                    applied,
                    persistence_warning,
                } => Ok(serde_json::json!({
                    "applied":applied,
                    "persistence_warning":persistence_warning,
                    "programmer":state.programmers.get(session.id)
                })),
                command_http::ExistingCommandOutcome::Rejected { error } => Err(error),
            }
        }
        "preset.apply" => {
            #[derive(Deserialize)]
            struct Input {
                #[serde(default)]
                preset_id: Option<String>,
                #[serde(default)]
                family: Option<light_programmer::PresetFamily>,
                #[serde(default)]
                number: Option<u32>,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let requested_address = match (input.family, input.number, input.preset_id.as_deref()) {
                (Some(family), Some(number), _) => {
                    light_programmer::PresetAddress::new(family, number)?
                }
                (_, _, Some(id)) => light_programmer::PresetAddress::parse(id)?,
                _ => return Err("preset.apply requires family and number".into()),
            };
            let storage_key = requested_address.storage_key();
            let active = state
                .active_show
                .read()
                .clone()
                .ok_or("no active show is loaded")?;
            let object = ShowStore::open(&active.path)
                .map_err(|e| e.to_string())?
                .objects("preset")
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|object| {
                    object.id == storage_key
                        || decode_preset_object(object)
                            .is_ok_and(|(address, _)| address == requested_address)
                })
                .ok_or("preset does not exist")?;
            let (stored_address, preset) = decode_preset_object(&object)?;
            if stored_address != requested_address {
                return Err("stored preset address does not match the requested pool entry".into());
            }
            let group_map = state
                .engine
                .snapshot()
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.clone()))
                .collect::<HashMap<_, _>>();
            let current = state
                .programmers
                .get(session.id)
                .ok_or("programmer does not exist")?;
            let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
            if current.selected.is_empty() {
                return Err("preset recall requires a current selection".into());
            }
            let live_group_targets = match current.selection_expression.clone() {
                Some(light_programmer::SelectionExpression::LiveGroup {
                    group_id,
                    rule: light_programmer::SelectionRule::All,
                }) => vec![group_id],
                Some(light_programmer::SelectionExpression::Sources { items })
                    if items.iter().all(|item| {
                        matches!(item, light_programmer::SelectionReference::LiveGroup { .. })
                    }) =>
                {
                    items
                        .into_iter()
                        .filter_map(|item| match item {
                            light_programmer::SelectionReference::LiveGroup { group_id } => {
                                Some(group_id)
                            }
                            _ => None,
                        })
                        .collect()
                }
                _ => Vec::new(),
            };
            for fixture_id in &current.selected {
                if let Some(attributes) = preset.values.get(fixture_id) {
                    for (attribute, value) in attributes {
                        state.programmers.set_faded_with_timing(
                            session.id,
                            *fixture_id,
                            attribute.clone(),
                            value.clone(),
                            Some(programmer_fade_millis),
                            None,
                        );
                    }
                }
                for (group_id, attributes) in preset
                    .group_values
                    .iter()
                    .filter(|(group_id, _)| !live_group_targets.contains(group_id))
                {
                    if !light_programmer::resolve_group(group_id, &group_map)
                        .is_ok_and(|members| members.contains(fixture_id))
                    {
                        continue;
                    }
                    for (attribute, value) in attributes {
                        state.programmers.set_faded_with_timing(
                            session.id,
                            *fixture_id,
                            attribute.clone(),
                            value.clone(),
                            Some(programmer_fade_millis),
                            None,
                        );
                    }
                }
            }
            for group_id in live_group_targets {
                let Some(attributes) = preset.group_values.get(&group_id) else {
                    continue;
                };
                for (attribute, value) in attributes {
                    state.programmers.set_group_faded_with_timing(
                        session.id,
                        group_id.clone(),
                        attribute.clone(),
                        value.clone(),
                        Some(programmer_fade_millis),
                        None,
                    );
                }
            }
            state.programmers.set_modes(
                session.id,
                None,
                None,
                None,
                Some(Some(format!("preset:{}", storage_key))),
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(
                serde_json::json!({"applied":current.selected.len(),"programmer":state.programmers.get(session.id)}),
            )
        }
        "programmer.mode" => {
            #[derive(Deserialize)]
            struct Input {
                blind: Option<bool>,
                preview: Option<bool>,
                highlight: Option<bool>,
                active_context: Option<Option<String>>,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            state.programmers.set_modes(
                session.id,
                input.blind,
                input.preview,
                None,
                input.active_context,
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            let mut highlight_state = None;
            if let Some(enabled) = input.highlight {
                let programmer = state
                    .programmers
                    .get(session.id)
                    .ok_or("programmer does not exist")?;
                let selection = state
                    .programmers
                    .selection(session.id)
                    .ok_or("programmer selection does not exist")?;
                let snapshot = state.engine.snapshot();
                let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
                let groups = highlight_groups(&snapshot);
                let transition = state
                    .highlight
                    .action_guarded(
                        session.desk.id,
                        session.user.id,
                        Some(&session.user.name),
                        if enabled {
                            HighlightAction::On
                        } else {
                            HighlightAction::Off
                        },
                        &selection,
                        &fixtures,
                        &groups,
                        programmer.blind || programmer.preview,
                    )
                    .map_err(|error| error.to_string())?;
                apply_highlight_selection_write(
                    state,
                    session,
                    transition.working_selection.as_ref(),
                )
                .map_err(|error| error.message)?;
                sync_highlight_output(state);
                emit(
                    state,
                    "highlight_changed",
                    serde_json::json!({"desk_id":session.desk.id,"user_id":session.user.id,"state":&transition.state}),
                );
                highlight_state = Some(transition.state);
            } else if input.blind.is_some() || input.preview.is_some() {
                highlight_state =
                    reconcile_highlight_capture_mode(state, session, "programmer_mode");
            }
            Ok(serde_json::json!({"updated":true,"highlight":highlight_state}))
        }
        "master.set" => {
            let input: MasterInput =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            lock_live_input(state, session, "desk:master".into())?;
            let mut control = state.output_control.lock();
            if let Some(level) = input.grand_master {
                if !level.is_finite() || !(0.0..=1.0).contains(&level) {
                    return Err("grand_master must be within 0-1".into());
                }
                control.options.grand_master = level;
            }
            if let Some(blackout) = input.blackout {
                control.options.blackout = blackout;
            }
            let result = serde_json::json!({"grand_master":control.options.grand_master,"blackout":control.options.blackout});
            drop(control);
            persist_output_runtime(state).map_err(|error| error.message)?;
            Ok(result)
        }
        "group.master.set" => {
            #[derive(Deserialize)]
            struct Input {
                group_id: String,
                value: f32,
            }
            let input: Input = serde_json::from_value(command.payload.clone())
                .map_err(|error| error.to_string())?;
            if !input.value.is_finite() || !(0.0..=1.0).contains(&input.value) {
                return Err("group master must be within 0-1".into());
            }
            lock_live_input(state, session, format!("group-master:{}", input.group_id))?;
            let mut snapshot = (*state.engine.snapshot()).clone();
            let group = snapshot
                .groups
                .iter_mut()
                .find(|group| group.id == input.group_id)
                .ok_or("group does not exist")?;
            group.master = input.value;
            state
                .engine
                .replace_snapshot(snapshot)
                .map_err(|error| error.to_string())?;
            persist_output_runtime(state).map_err(|error| error.message)?;
            Ok(serde_json::json!({"group_id":input.group_id,"master":input.value}))
        }
        "group.master.flash" => {
            #[derive(Deserialize)]
            struct Input {
                group_id: String,
                value: f32,
            }
            let input: Input = serde_json::from_value(command.payload.clone())
                .map_err(|error| error.to_string())?;
            if !input.value.is_finite() || !(0.0..=1.0).contains(&input.value) {
                return Err("group flash must be within 0-1".into());
            }
            if !state
                .engine
                .snapshot()
                .groups
                .iter()
                .any(|group| group.id == input.group_id)
            {
                return Err("group does not exist".into());
            }
            lock_live_input(state, session, format!("group-flash:{}", input.group_id))?;
            state
                .engine
                .set_group_master_flash(input.group_id.clone(), input.value);
            Ok(serde_json::json!({"group_id":input.group_id,"flash":input.value}))
        }
        "playback.go" | "playback.back" | "playback.pause" | "playback.release" => {
            #[derive(Deserialize)]
            struct Input {
                cue_list_id: light_core::CueListId,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let playback = state.engine.playback();
            let mut playback = playback.write();
            match command.command.as_str() {
                "playback.go" => {
                    serde_json::to_value(playback.go(input.cue_list_id).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())
                }
                "playback.back" => serde_json::to_value(
                    playback
                        .back(input.cue_list_id)
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string()),
                "playback.pause" => {
                    playback
                        .pause(input.cue_list_id)
                        .map_err(|e| e.to_string())?;
                    Ok(serde_json::json!({"paused":true}))
                }
                _ => Ok(serde_json::json!({"released":playback.release(input.cue_list_id)})),
            }
        }
        _ => Err("unknown command".into()),
    })();
    if result.is_ok()
        && state
            .programmers
            .selection(session.id)
            .map(|selection| selection.revision)
            != selection_revision_before
    {
        reconcile_highlight_selection(state, session, "programmer_selection");
    }
    if result.is_ok()
        && matches!(
            command.command.as_str(),
            "programmer.undo" | "programmer.redo"
        )
        && let Err(error) = persist_programmer(state, session)
    {
        return fail(error.message);
    }
    match result {
        Ok(payload) => {
            let no_op_release = command.command == "preload.release"
                && payload.get("released").and_then(serde_json::Value::as_bool) == Some(false);
            if !no_op_release {
                emit(
                    state,
                    "command_applied",
                    serde_json::json!({"request_id":command.request_id,"session_id":session.id,"command":command.command}),
                );
            }
            if matches!(
                command.command.as_str(),
                "programmer.set"
                    | "programmer.set_many"
                    | "programmer.set_value"
                    | "programmer.control_action"
                    | "programmer.release"
                    | "programmer.group.set"
                    | "programmer.group.release"
                    | "selection.set"
                    | "selection.gesture"
                    | "selection.macro"
                    | "group.select"
                    | "programmer.execute"
                    | "programmer.undo"
                    | "programmer.redo"
                    | "preload.group.set"
                    | "preload.clear"
            ) {
                emit(
                    state,
                    "programmer_changed",
                    serde_json::json!({"session_id":session.id,"command":command.command}),
                );
            }
            WsResponse {
                protocol_version: 1,
                request_id: command.request_id,
                ok: true,
                revision: state.engine.snapshot().revision,
                payload: Some(payload),
                error: None,
            }
        }
        Err(error) => fail(error),
    }
}
fn aligned_normalized(
    mode: &str,
    index: usize,
    count: usize,
    from: f32,
    to: f32,
    wraps: bool,
) -> Result<f32, String> {
    if !from.is_finite()
        || !to.is_finite()
        || !(0.0..=1.0).contains(&from)
        || !(0.0..=1.0).contains(&to)
    {
        return Err("alignment endpoints must be within 0-1".into());
    }
    let last = count.saturating_sub(1).max(1) as f32;
    let t = index as f32 / last;
    let shaped = match mode {
        "left" => t,
        "right" => 1.0 - t,
        "center" => (t - 0.5).abs() * 2.0,
        "out" => 1.0 - (t - 0.5).abs() * 2.0,
        _ => return Err("alignment mode must be left, right, center, or out".into()),
    };
    let mut delta = to - from;
    if wraps && delta.abs() > 0.5 {
        delta -= delta.signum();
    }
    let value = from + delta * shaped;
    Ok(if wraps {
        value.rem_euclid(1.0)
    } else {
        value.clamp(0.0, 1.0)
    })
}

fn resolve_fixture_reference(
    fixtures: &[light_fixture::PatchedFixture],
    reference: &str,
) -> Result<light_core::FixtureId, String> {
    let (number, head_number) = match reference.split_once('.') {
        Some((fixture, head)) => (
            fixture
                .parse::<u32>()
                .map_err(|_| "fixture number is invalid")?,
            Some(head.parse::<u16>().map_err(|_| "head number is invalid")?),
        ),
        None => (
            reference
                .parse::<u32>()
                .map_err(|_| "fixture number is invalid")?,
            None,
        ),
    };
    if number == 0 {
        return Err("fixture numbers start at 1".into());
    }
    let fixture = fixture_by_number(fixtures, number)
        .ok_or_else(|| format!("fixture {number} does not exist"))?;
    match head_number {
        None => Ok(fixture.fixture_id),
        Some(0) if !fixture.logical_heads.is_empty() => Ok(fixture.fixture_id),
        Some(0) => Err(format!("fixture {number} is not a multi-head fixture")),
        Some(head_number) => ordered_child_ids(fixture)
            .get(usize::from(head_number - 1))
            .copied()
            .ok_or_else(|| format!("fixture {number} has no head {head_number}")),
    }
}

fn fixture_by_number(
    fixtures: &[light_fixture::PatchedFixture],
    number: u32,
) -> Option<&light_fixture::PatchedFixture> {
    fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(number))
        .or_else(|| {
            fixtures.get(number.saturating_sub(1) as usize).filter(|_| {
                fixtures
                    .iter()
                    .all(|fixture| fixture.fixture_number.is_none())
            })
        })
}

fn ordered_child_ids(fixture: &light_fixture::PatchedFixture) -> Vec<light_core::FixtureId> {
    fixture
        .definition
        .heads
        .iter()
        .filter(|head| !head.shared)
        .filter_map(|head| {
            fixture
                .logical_heads
                .iter()
                .find(|patched| patched.head_index == head.index)
                .map(|patched| patched.fixture_id)
        })
        .collect()
}

fn selectable_fixture_ids(fixture: &light_fixture::PatchedFixture) -> Vec<light_core::FixtureId> {
    let children = ordered_child_ids(fixture);
    if children.is_empty() {
        vec![fixture.fixture_id]
    } else {
        children
    }
}

fn expand_selectable_fixture_ids(
    fixtures: &[light_fixture::PatchedFixture],
    fixture_ids: impl IntoIterator<Item = light_core::FixtureId>,
) -> Vec<light_core::FixtureId> {
    let mut expanded = Vec::new();
    for fixture_id in fixture_ids {
        if let Some(fixture) = fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == fixture_id)
        {
            for selectable in selectable_fixture_ids(fixture) {
                push_unique(&mut expanded, selectable);
            }
        } else {
            // A logical head is already an ordinary selectable identity. Preserve it (and retain
            // the existing validation behavior for unknown IDs) rather than looking for another
            // master expansion.
            push_unique(&mut expanded, fixture_id);
        }
    }
    expanded
}

fn push_unique(selected: &mut Vec<light_core::FixtureId>, fixture_id: light_core::FixtureId) {
    if !selected.contains(&fixture_id) {
        selected.push(fixture_id);
    }
}

#[derive(Clone, Copy)]
struct FixtureReference {
    number: u32,
    head: Option<u16>,
}

fn parse_fixture_reference_tokens(
    tokens: &[String],
    index: &mut usize,
    end: usize,
) -> Result<FixtureReference, String> {
    let number = tokens
        .get(*index)
        .ok_or("expected a fixture number")?
        .parse::<u32>()
        .map_err(|_| "fixture number is invalid")?;
    if number == 0 {
        return Err("fixture numbers start at 1".into());
    }
    *index += 1;
    let head = if *index < end && tokens[*index] == "." {
        *index += 1;
        let head = tokens
            .get(*index)
            .ok_or("fixture head reference requires a head number")?
            .parse::<u16>()
            .map_err(|_| "head number is invalid")?;
        *index += 1;
        Some(head)
    } else {
        None
    };
    Ok(FixtureReference { number, head })
}

fn parse_subset_rule(tokens: &[String]) -> Result<light_programmer::SelectionRule, String> {
    if tokens.is_empty() {
        return Ok(light_programmer::SelectionRule::All);
    }
    if tokens[0] != "DIV" {
        return Err("unexpected tokens after selection".into());
    }
    if tokens.get(1).is_some_and(|token| token == "DIV") {
        if tokens.len() != 2 {
            return Err("DIV DIV does not accept another offset".into());
        }
        return Ok(light_programmer::SelectionRule::Even);
    }
    let n = tokens.get(1).map_or(Ok(2), |token| {
        token
            .parse::<usize>()
            .map_err(|_| "DIV requires a positive number")
    })?;
    if n == 0 {
        return Err("DIV requires a positive number".into());
    }
    let has_offset = tokens.get(2).is_some();
    let offset = match tokens.get(2).map(String::as_str) {
        None => 0,
        Some("+") => tokens
            .get(3)
            .ok_or("+ requires an offset")?
            .parse::<usize>()
            .map_err(|_| "offset is invalid")?,
        _ => return Err("expected + before the subset offset".into()),
    };
    if tokens.len() > if has_offset { 4 } else { 2 } {
        return Err("unexpected tokens after subset".into());
    }
    Ok(light_programmer::SelectionRule::EveryNth { n, offset })
}

fn parse_fixture_selection(
    fixtures: &[light_fixture::PatchedFixture],
    tokens: &[String],
) -> Result<Vec<light_core::FixtureId>, String> {
    if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
        )
    }) {
        let normalized = tokens
            .iter()
            .filter(|token| {
                !matches!(
                    token.as_str(),
                    "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        return parse_fixture_selection(fixtures, &normalized);
    }
    let div = tokens
        .iter()
        .position(|token| token == "DIV")
        .unwrap_or(tokens.len());
    if let Some(minus) = tokens[..div].iter().position(|token| token == "-") {
        if minus == 0 || minus + 1 == div {
            return Err("- requires fixture selections on both sides".into());
        }
        let mut selected = parse_fixture_selection(fixtures, &tokens[..minus])?;
        let mut start = minus + 1;
        while start < div {
            let end = tokens[start..div]
                .iter()
                .position(|token| token == "-")
                .map_or(div, |offset| start + offset);
            if start == end {
                return Err("- requires a fixture selection".into());
            }
            let removed = parse_fixture_selection(fixtures, &tokens[start..end])?;
            selected.retain(|fixture| !removed.contains(fixture));
            start = end + 1;
        }
        let rule = parse_subset_rule(&tokens[div..])?;
        return Ok(light_programmer::apply_selection_rule(&selected, &rule));
    }
    let mut selected = Vec::new();
    let mut index = 0;
    while index < div {
        if tokens[index] == "+" {
            return Err("expected a fixture reference before +".into());
        }
        let first = parse_fixture_reference_tokens(tokens, &mut index, div)?;
        if tokens.get(index).is_some_and(|token| token == "THRU") {
            index += 1;
            let last = parse_fixture_reference_tokens(tokens, &mut index, div)?;
            if last.number < first.number {
                return Err("fixture range is invalid".into());
            }
            match (first.head, last.head) {
                (None, None) => {
                    for number in first.number..=last.number {
                        let Some(fixture) = fixture_by_number(fixtures, number) else {
                            continue;
                        };
                        let children = ordered_child_ids(fixture);
                        if children.is_empty() {
                            push_unique(&mut selected, fixture.fixture_id);
                        } else {
                            for child in children {
                                push_unique(&mut selected, child);
                            }
                        }
                    }
                }
                (Some(0), Some(0)) => {
                    for number in first.number..=last.number {
                        let fixture_id =
                            resolve_fixture_reference(fixtures, &format!("{number}.0"))?;
                        push_unique(&mut selected, fixture_id);
                    }
                }
                (Some(first_head), Some(last_head))
                    if first.number == last.number && first_head > 0 && last_head >= first_head =>
                {
                    for head in first_head..=last_head {
                        let fixture_id = resolve_fixture_reference(
                            fixtures,
                            &format!("{}.{}", first.number, head),
                        )?;
                        push_unique(&mut selected, fixture_id);
                    }
                }
                _ => {
                    return Err(
                        "head ranges must be .0 across fixtures or child heads within one fixture"
                            .into(),
                    );
                }
            }
        } else {
            match first.head {
                Some(head) => push_unique(
                    &mut selected,
                    resolve_fixture_reference(fixtures, &format!("{}.{}", first.number, head))?,
                ),
                None => {
                    if let Some(fixture) = fixture_by_number(fixtures, first.number) {
                        for selectable in selectable_fixture_ids(fixture) {
                            push_unique(&mut selected, selectable);
                        }
                    }
                }
            }
        }
        if index < div && tokens[index] != "+" {
            return Err("expected + between fixture ranges".into());
        }
        if index < div {
            index += 1;
            if index == div {
                return Err("expected a fixture reference after +".into());
            }
        }
    }
    let rule = parse_subset_rule(&tokens[div..])?;
    Ok(light_programmer::apply_selection_rule(&selected, &rule))
}

#[derive(Clone, Copy, Debug, Default)]
struct CommandTiming {
    fade_millis: Option<u64>,
    delay_millis: Option<u64>,
}

fn programmer_value_timing(state: &AppState, timing: CommandTiming) -> CommandTiming {
    CommandTiming {
        fade_millis: Some(
            timing
                .fade_millis
                .unwrap_or_else(|| state.configuration.read().programmer_fade_millis),
        ),
        ..timing
    }
}

fn set_command_fixture_intensities(
    state: &AppState,
    session: &Session,
    values: impl IntoIterator<Item = (light_core::FixtureId, f32)>,
    timing: CommandTiming,
) {
    state.programmers.set_many_faded_with_timing(
        session.id,
        values.into_iter().map(|(fixture_id, value)| {
            (
                fixture_id,
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(value),
            )
        }),
        timing.fade_millis,
        timing.delay_millis,
    );
}

fn command_time_millis(token: &str) -> Result<u64, String> {
    let seconds = token
        .parse::<f64>()
        .map_err(|_| "TIME and DELAY require seconds")?;
    if !seconds.is_finite() || !(0.0..=86_400.0).contains(&seconds) {
        return Err("TIME and DELAY must be within 0-86400 seconds".into());
    }
    Ok((seconds * 1_000.0).round() as u64)
}

fn command_time_at(tokens: &[String], index: usize) -> Result<(u64, usize), String> {
    let whole = tokens.get(index).ok_or("TIME and DELAY require seconds")?;
    if tokens.get(index + 1).is_some_and(|token| token == ".") {
        let fraction = tokens
            .get(index + 2)
            .ok_or("time decimal requires digits after the dot")?;
        return Ok((command_time_millis(&format!("{whole}.{fraction}"))?, 3));
    }
    Ok((command_time_millis(whole)?, 1))
}

fn extract_command_timing(tokens: &[String]) -> Result<(Vec<String>, CommandTiming), String> {
    let mut command = Vec::with_capacity(tokens.len());
    let mut timing = CommandTiming::default();
    let mut index = 0;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "TIME" if tokens.get(index + 1).is_some_and(|token| token == "TIME") => {
                let (value, used) = command_time_at(tokens, index + 2)?;
                timing.delay_millis = Some(value);
                index += 2 + used;
            }
            "TIME" => {
                let (value, used) = command_time_at(tokens, index + 1)?;
                timing.fade_millis = Some(value);
                index += 1 + used;
            }
            "DELAY" => {
                let (value, used) = command_time_at(tokens, index + 1)?;
                timing.delay_millis = Some(value);
                index += 1 + used;
            }
            _ => {
                command.push(tokens[index].clone());
                index += 1;
            }
        }
    }
    Ok((command, timing))
}

fn tokenize_programmer_command(command_line: &str) -> Result<(Vec<String>, CommandTiming), String> {
    let spaced = command_line
        .replace(',', ".")
        .replace('.', " . ")
        .replace('+', " + ")
        .replace('-', " - ");
    let mut raw_tokens = Vec::new();
    for token in spaced
        .split_whitespace()
        .map(|token| token.to_ascii_uppercase())
    {
        if token == "DEGRP" {
            raw_tokens.extend(["GROUP".to_owned(), "GROUP".to_owned()]);
            continue;
        }
        if token == "F" || token == "G" {
            raw_tokens.push(if token == "F" { "FIXTURE" } else { "GROUP" }.to_owned());
            continue;
        }
        if token.len() > 1 && matches!(token.as_bytes()[0], b'F' | b'G') {
            let (prefix, number) = token.split_at(1);
            if matches!(prefix, "F" | "G")
                && number.chars().all(|character| character.is_ascii_digit())
            {
                raw_tokens.push(if prefix == "F" { "FIXTURE" } else { "GROUP" }.to_owned());
                raw_tokens.push(number.to_owned());
                continue;
            }
        }
        raw_tokens.push(token);
    }
    extract_command_timing(&raw_tokens)
}

/// Return the same normalized first command token used by execution, after removing valid timing
/// clauses. Transport adapters use this to enforce capability ownership without maintaining a
/// second, subtly different command parser.
fn normalized_programmer_command_family(command_line: &str) -> Result<Option<String>, String> {
    tokenize_programmer_command(command_line).map(|(tokens, _)| tokens.into_iter().next())
}

fn active_show_store(state: &AppState) -> Result<(ShowEntry, ShowStore), String> {
    let entry = state
        .active_show
        .read()
        .clone()
        .ok_or("no active show is loaded")?;
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    Ok((entry, store))
}

fn refresh_command_show(state: &AppState, entry: &ShowEntry) -> Result<(), String> {
    let snapshot = load_engine_snapshot(entry)?;
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    state
        .engine
        .replace_snapshot(snapshot)
        .map_err(|error| error.to_string())?;
    state.programmers.refresh_live_selections(&groups);
    let mut reconciled = HashSet::new();
    for session in state.sessions.read().values().cloned().collect::<Vec<_>>() {
        if reconciled.insert((session.desk.id, session.user.id)) {
            reconcile_highlight_selection(state, &session, "show_selection_refresh");
        }
    }
    Ok(())
}

fn emit_command_object_changed(
    state: &AppState,
    entry: &ShowEntry,
    kind: &str,
    id: &str,
    revision: u64,
) {
    emit(
        state,
        "show_object_changed",
        serde_json::json!({"show_id":entry.id,"kind":kind,"id":id,"revision":revision}),
    );
}

fn decode_preset_object(
    object: &light_show::VersionedObject,
) -> Result<(light_programmer::PresetAddress, light_programmer::Preset), String> {
    let mut preset: light_programmer::Preset = serde_json::from_value(object.body.clone())
        .map_err(|error| format!("invalid stored preset: {error}"))?;
    let address = preset.reconcile_address(&object.id)?;
    Ok((address, preset))
}

fn serialize_preset_preserving_extensions(
    original: &serde_json::Value,
    preset: &light_programmer::Preset,
) -> Result<serde_json::Value, serde_json::Error> {
    let canonical = serde_json::to_value(preset)?;
    let mut merged = original.clone();
    let Some(merged_fields) = merged.as_object_mut() else {
        return Ok(canonical);
    };
    let Some(canonical_fields) = canonical.as_object() else {
        return Ok(canonical);
    };
    for (key, value) in canonical_fields {
        merged_fields.insert(key.clone(), value.clone());
    }
    Ok(merged)
}

fn apply_command_preset(
    state: &AppState,
    session: &Session,
    id: &str,
    selected: &[light_core::FixtureId],
) -> Result<(), String> {
    let (_, store) = active_show_store(state)?;
    let requested_address = light_programmer::PresetAddress::parse(id)?;
    let object = store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            object.id == id
                || decode_preset_object(object)
                    .is_ok_and(|(address, _)| address == requested_address)
        })
        .ok_or_else(|| format!("preset {id} does not exist"))?;
    let (_, preset) = decode_preset_object(&object)?;
    let groups = state
        .engine
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let current_expression = state
        .programmers
        .get(session.id)
        .and_then(|programmer| programmer.selection_expression);
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    let live_group_targets = match current_expression {
        Some(light_programmer::SelectionExpression::LiveGroup {
            group_id,
            rule: light_programmer::SelectionRule::All,
        }) => vec![group_id],
        Some(light_programmer::SelectionExpression::Sources { items })
            if items.iter().all(|item| {
                matches!(item, light_programmer::SelectionReference::LiveGroup { .. })
            }) =>
        {
            items
                .into_iter()
                .filter_map(|item| match item {
                    light_programmer::SelectionReference::LiveGroup { group_id } => Some(group_id),
                    _ => None,
                })
                .collect()
        }
        _ => Vec::new(),
    };
    for fixture in selected {
        if let Some(attributes) = preset.values.get(fixture) {
            for (attribute, value) in attributes {
                state.programmers.set_faded_with_timing(
                    session.id,
                    *fixture,
                    attribute.clone(),
                    value.clone(),
                    Some(programmer_fade_millis),
                    None,
                );
            }
        }
        for (group_id, attributes) in preset
            .group_values
            .iter()
            .filter(|(group_id, _)| !live_group_targets.contains(group_id))
        {
            if light_programmer::resolve_group(group_id, &groups)
                .is_ok_and(|members| members.contains(fixture))
            {
                for (attribute, value) in attributes {
                    state.programmers.set_faded_with_timing(
                        session.id,
                        *fixture,
                        attribute.clone(),
                        value.clone(),
                        Some(programmer_fade_millis),
                        None,
                    );
                }
            }
        }
    }
    for group_id in live_group_targets {
        let Some(attributes) = preset.group_values.get(&group_id) else {
            continue;
        };
        for (attribute, value) in attributes {
            state.programmers.set_group_faded_with_timing(
                session.id,
                group_id.clone(),
                attribute.clone(),
                value.clone(),
                Some(programmer_fade_millis),
                None,
            );
        }
    }
    state.programmers.set_modes(
        session.id,
        None,
        None,
        None,
        Some(Some(format!("preset:{id}"))),
    );
    Ok(())
}

fn command_preset_address(tokens: &[String]) -> Result<light_programmer::PresetAddress, String> {
    if tokens.len() != 3 || tokens[1] != "." {
        return Err("expected <preset-type> . <preset-number>".into());
    }
    light_programmer::PresetAddress::parse(&format!("{}.{}", tokens[0], tokens[2]))
}

fn command_preset_id(tokens: &[String]) -> Result<String, String> {
    Ok(command_preset_address(tokens)?.storage_key())
}

fn command_preset_family(id: &str) -> Result<light_programmer::PresetFamily, String> {
    Ok(light_programmer::PresetAddress::parse(id)?.family)
}

#[derive(Clone, Copy)]
struct CommandPlaybackAddress {
    playback: u16,
    cue: Option<f64>,
}

fn page_playback(snapshot: &EngineSnapshot, page: u8, slot: u8) -> Result<u16, String> {
    snapshot
        .playback_pages
        .iter()
        .find(|item| item.number == page)
        .and_then(|item| item.slots.get(&slot))
        .copied()
        .ok_or_else(|| format!("page {page} slot {slot} is not assigned"))
}

fn parse_playback_address(
    tokens: &[String],
    require_set: bool,
    snapshot: &EngineSnapshot,
) -> Result<(CommandPlaybackAddress, usize), String> {
    let mut index = 0;
    if require_set {
        if tokens.get(index).is_none_or(|token| token != "SET") {
            return Err("playback address must start with SET".into());
        }
        index += 1;
    }
    let first = tokens
        .get(index)
        .ok_or("playback number is required")?
        .parse::<u16>()
        .map_err(|_| "playback number is invalid")?;
    index += 1;
    let playback = if tokens.get(index).is_some_and(|token| token == ".") {
        index += 1;
        let slot = tokens
            .get(index)
            .ok_or("page playback number is required")?
            .parse::<u8>()
            .map_err(|_| "page playback number is invalid")?;
        index += 1;
        page_playback(
            snapshot,
            first.try_into().map_err(|_| "page number is invalid")?,
            slot,
        )?
    } else {
        first
    };
    let cue = if tokens.get(index).is_some_and(|token| token == "CUE") {
        index += 1;
        let mut cue = tokens
            .get(index)
            .ok_or("CUE requires a cue number")?
            .clone();
        index += 1;
        while tokens.get(index).is_some_and(|token| token == ".") {
            cue.push('.');
            index += 1;
            cue.push_str(tokens.get(index).ok_or("DOT requires another cue part")?);
            index += 1;
        }
        let cue = cue.parse::<f64>().map_err(|_| "cue number is invalid")?;
        if !cue.is_finite() || cue <= 0.0 {
            return Err("cue number must be positive".into());
        }
        Some(cue)
    } else {
        None
    };
    Ok((CommandPlaybackAddress { playback, cue }, index))
}

fn parse_update_playback_address(
    tokens: &[String],
    current_page: u8,
    snapshot: &EngineSnapshot,
) -> Result<CommandPlaybackAddress, String> {
    if tokens.first().is_none_or(|token| token != "SET") {
        return Err("playback address must start with SET".into());
    }

    // Update follows the control-surface playback model: SET <slot> addresses
    // that slot on this desk's current page, while SET <page> . <slot> keeps an
    // explicit page stable when the operator changes pages.
    let explicit = if tokens.get(2).is_some_and(|token| token == ".") {
        tokens.to_vec()
    } else {
        let slot = tokens
            .get(1)
            .ok_or("playback number is required")?
            .parse::<u8>()
            .map_err(|_| "playback number is invalid")?;
        if slot == 0 || slot > 127 {
            return Err("playback number must be within 1-127".into());
        }
        let mut explicit = vec![
            "SET".to_string(),
            current_page.to_string(),
            ".".to_string(),
            slot.to_string(),
        ];
        explicit.extend(tokens.iter().skip(2).cloned());
        explicit
    };
    let (address, used) = parse_playback_address(&explicit, true, snapshot)?;
    if used != explicit.len() {
        return Err("unexpected tokens after Update playback target".into());
    }
    Ok(address)
}

fn programmer_preset(
    programmer: &light_programmer::ProgrammerState,
    name: String,
    address: light_programmer::PresetAddress,
) -> light_programmer::Preset {
    let mut preset = light_programmer::Preset {
        name,
        family: address.family,
        number: address.number,
        ..Default::default()
    };
    for value in &programmer.values {
        preset
            .values
            .entry(value.fixture_id)
            .or_default()
            .insert(value.attribute.clone(), value.value.clone());
    }
    for (group, attributes) in &programmer.group_values {
        for (attribute, value) in attributes {
            preset
                .group_values
                .entry(group.clone())
                .or_default()
                .insert(attribute.clone(), value.value.clone());
        }
    }
    preset.retain_family_attributes();
    preset
}

fn programmer_cue(
    programmer: &light_programmer::ProgrammerState,
    number: f64,
    timing: CommandTiming,
) -> light_playback::Cue {
    let mut cue = light_playback::Cue::new(number);
    cue.fade_millis = timing.fade_millis.unwrap_or(0);
    cue.trigger = match timing.delay_millis {
        Some(0) => light_playback::CueTrigger::Follow { delay_millis: 0 },
        Some(delay_millis) => light_playback::CueTrigger::Wait { delay_millis },
        None => light_playback::CueTrigger::Manual,
    };
    cue.changes = programmer
        .values
        .iter()
        .map(|value| {
            let mut change = light_playback::CueChange::set(
                value.fixture_id,
                value.attribute.clone(),
                value.value.clone(),
            );
            change.fade_millis = value.fade_millis;
            change.delay_millis = value.delay_millis;
            change
        })
        .collect();
    cue.group_changes = programmer
        .group_values
        .iter()
        .flat_map(|(group, attributes)| {
            attributes
                .iter()
                .map(|(attribute, value)| light_playback::GroupCueChange {
                    group_id: group.clone(),
                    attribute: attribute.clone(),
                    value: Some(value.value.clone()),
                    automatic_restore: false,
                    fade_millis: value.fade_millis,
                    delay_millis: value.delay_millis,
                })
        })
        .collect();
    cue
}

fn cue_list_for_playback(
    store: &ShowStore,
    snapshot: &EngineSnapshot,
    playback: u16,
) -> Result<
    (
        light_playback::PlaybackDefinition,
        light_show::VersionedObject,
        light_playback::CueList,
    ),
    String,
> {
    let definition = snapshot
        .playbacks
        .iter()
        .find(|item| item.number == playback)
        .cloned()
        .ok_or_else(|| format!("Cuelist {playback} does not exist"))?;
    let light_playback::PlaybackTarget::CueList { cue_list_id } = definition.target else {
        return Err(format!("Cuelist {playback} does not contain Cues"));
    };
    let id = cue_list_id.0.to_string();
    let object = store
        .objects("cue_list")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == id)
        .ok_or("Cuelist does not exist")?;
    let cue_list =
        serde_json::from_value(object.body.clone()).map_err(|error| error.to_string())?;
    Ok((definition, object, cue_list))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecordOperation {
    Overwrite,
    Merge,
    Subtract,
}

fn store_cue_at(
    state: &AppState,
    session: &Session,
    playback: u16,
    requested: Option<f64>,
    timing: CommandTiming,
    operation: RecordOperation,
) -> Result<(), String> {
    let (entry, store) = active_show_store(state)?;
    let mut changed_objects = Vec::new();
    let snapshot = state.engine.snapshot();
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let programmer_is_empty = programmer.values.is_empty() && programmer.group_values.is_empty();
    if programmer_is_empty && operation != RecordOperation::Subtract {
        return Err("the programmer has no values to record".into());
    }
    if operation != RecordOperation::Overwrite && requested.is_none() {
        return Err("RECORD + and RECORD - require an explicit CUE target".into());
    }
    if let Some(definition) = snapshot
        .playbacks
        .iter()
        .find(|item| item.number == playback)
    {
        let (_, object, mut list) = cue_list_for_playback(&store, &snapshot, definition.number)?;
        let number = requested
            .unwrap_or_else(|| list.cues.last().map_or(1.0, |cue| cue.number.floor() + 1.0));
        if let Some(position) = list.cues.iter().position(|cue| cue.number == number) {
            if operation == RecordOperation::Subtract && programmer_is_empty {
                list.cues.remove(position);
                if list.cues.is_empty() {
                    return Err(
                        "cannot delete the only Cue; delete the Cuelist from its configuration instead"
                            .into(),
                    );
                }
            } else {
                let incoming = programmer_cue(&programmer, number, timing);
                let cue = &mut list.cues[position];
                match operation {
                    RecordOperation::Overwrite => {
                        cue.changes = incoming.changes;
                        cue.group_changes = incoming.group_changes;
                        cue.phasers = incoming.phasers;
                        cue.fade_millis = incoming.fade_millis;
                        cue.delay_millis = incoming.delay_millis;
                        cue.trigger = incoming.trigger;
                        cue.cue_only = incoming.cue_only;
                    }
                    RecordOperation::Merge => {
                        for change in incoming.changes {
                            cue.changes.retain(|existing| {
                                existing.fixture_id != change.fixture_id
                                    || existing.attribute != change.attribute
                            });
                            cue.changes.push(change);
                        }
                        for change in incoming.group_changes {
                            cue.group_changes.retain(|existing| {
                                existing.group_id != change.group_id
                                    || existing.attribute != change.attribute
                            });
                            cue.group_changes.push(change);
                        }
                    }
                    RecordOperation::Subtract => {
                        cue.changes.retain(|existing| {
                            !incoming.changes.iter().any(|remove| {
                                existing.fixture_id == remove.fixture_id
                                    && existing.attribute == remove.attribute
                            })
                        });
                        cue.group_changes.retain(|existing| {
                            !incoming.group_changes.iter().any(|remove| {
                                existing.group_id == remove.group_id
                                    && existing.attribute == remove.attribute
                            })
                        });
                    }
                }
            }
        } else if operation == RecordOperation::Subtract {
            return Err(format!("Cue {number} does not exist"));
        } else {
            list.cues.push(programmer_cue(&programmer, number, timing));
        }
        list.cues.sort_by(|a, b| a.number.total_cmp(&b.number));
        let revision = store
            .put_object(
                "cue_list",
                &object.id,
                &serde_json::to_value(list).map_err(|error| error.to_string())?,
                object.revision,
            )
            .map_err(|error| error.to_string())?;
        changed_objects.push(("cue_list", object.id, revision));
    } else {
        if operation == RecordOperation::Subtract {
            return Err(format!("Cuelist {playback} does not exist"));
        }
        if !(1..=light_playback::MAX_PLAYBACKS).contains(&playback) {
            return Err("Cuelist number must be within 1-1000".into());
        }
        let cue_list_id = light_core::CueListId::new();
        let number = requested.unwrap_or(1.0);
        let list = light_playback::CueList {
            id: cue_list_id,
            name: format!("Cuelist {playback}"),
            priority: 0,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![programmer_cue(&programmer, number, timing)],
        };
        let definition = light_playback::PlaybackDefinition {
            number: playback,
            name: list.name.clone(),
            target: light_playback::PlaybackTarget::CueList { cue_list_id },
            buttons: [
                light_playback::PlaybackButtonAction::GoMinus,
                light_playback::PlaybackButtonAction::Go,
                light_playback::PlaybackButtonAction::Flash,
            ],
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader: true,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::default(),
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        let cue_list_id = cue_list_id.0.to_string();
        let cue_list_revision = store
            .put_object(
                "cue_list",
                &cue_list_id,
                &serde_json::to_value(list).map_err(|error| error.to_string())?,
                0,
            )
            .map_err(|error| error.to_string())?;
        changed_objects.push(("cue_list", cue_list_id, cue_list_revision));
        let playback_id = playback.to_string();
        let playback_revision = store
            .put_object(
                "playback",
                &playback_id,
                &serde_json::to_value(definition).map_err(|error| error.to_string())?,
                0,
            )
            .map_err(|error| error.to_string())?;
        changed_objects.push(("playback", playback_id, playback_revision));
    }
    refresh_command_show(state, &entry)?;
    for (kind, id, revision) in changed_objects {
        emit_command_object_changed(state, &entry, kind, &id, revision);
    }
    Ok(())
}

fn execute_show_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    timing: CommandTiming,
) -> Result<usize, String> {
    let operation = match tokens[0].as_str() {
        "REC" => "RECORD",
        "DEL" => "DELETE",
        "MOV" => "MOVE",
        "CPY" => "COPY",
        value => value,
    };
    let mut body = &tokens[1..];
    let transfer_mode = match body.first().map(String::as_str) {
        Some("PLAIN") => {
            body = &body[1..];
            Some(CueTransferMode::Plain)
        }
        Some("STATUS") => {
            body = &body[1..];
            Some(CueTransferMode::Status)
        }
        _ => None,
    };
    let snapshot = state.engine.snapshot();
    if operation == "UPDATE" {
        let settings = update_settings_for(state, session.desk.id);
        let request = if body.first().is_some_and(|token| token == "SET") {
            let show = state.active_show.read().clone().ok_or("no show is open")?;
            let current_page = state
                .desk
                .lock()
                .desk_page(session.desk.id, show.id)
                .unwrap_or(1);
            let address = parse_update_playback_address(body, current_page, &snapshot)?;
            let definition = snapshot
                .playbacks
                .iter()
                .find(|definition| definition.number == address.playback)
                .ok_or_else(|| format!("playback {} does not exist", address.playback))?;
            let light_playback::PlaybackTarget::CueList { cue_list_id } = &definition.target else {
                return Err(format!(
                    "playback {} is not assigned to a Cuelist",
                    address.playback
                ));
            };
            let explicit = address
                .cue
                .map(|number| {
                    snapshot
                        .cue_lists
                        .iter()
                        .find(|list| list.id == *cue_list_id)
                        .and_then(|list| list.cues.iter().find(|cue| cue.number == number))
                        .map(|cue| (cue.id, cue.number))
                        .ok_or_else(|| format!("Cue {number} does not exist"))
                })
                .transpose()?;
            UpdateApiRequest {
                target: UpdateApiTarget {
                    family: UpdateApiTargetFamily::Cue,
                    object_id: Some(cue_list_id.0.to_string()),
                    playback_number: Some(address.playback),
                    cue_id: explicit.map(|cue| cue.0),
                    cue_number: explicit.map(|cue| cue.1),
                    validate_active_context: false,
                },
                mode: update::UpdateMode::Cue(settings.cue_mode),
                expected_revision: None,
                expected_programmer_revision: None,
            }
        } else if body.first().is_some_and(|token| token == "GROUP") {
            if body.len() != 2 {
                return Err("expected UPDATE GROUP <group-number>".into());
            }
            UpdateApiRequest {
                target: UpdateApiTarget {
                    family: UpdateApiTargetFamily::Group,
                    object_id: Some(body[1].clone()),
                    playback_number: None,
                    cue_id: None,
                    cue_number: None,
                    validate_active_context: false,
                },
                mode: update::UpdateMode::ExistingContent(settings.group_mode),
                expected_revision: None,
                expected_programmer_revision: None,
            }
        } else {
            let id = command_preset_id(body)?;
            UpdateApiRequest {
                target: UpdateApiTarget {
                    family: UpdateApiTargetFamily::Preset,
                    object_id: Some(id),
                    playback_number: None,
                    cue_id: None,
                    cue_number: None,
                    validate_active_context: false,
                },
                mode: update::UpdateMode::ExistingContent(settings.preset_mode),
                expected_revision: None,
                expected_programmer_revision: None,
            }
        };
        let result = perform_update(state, session, &request).map_err(|error| error.message)?;
        return Ok(result.changed_count);
    }
    if operation == "RECORD" {
        let record_operation = match body.first().map(String::as_str) {
            Some("+") => {
                body = &body[1..];
                RecordOperation::Merge
            }
            Some("-") => {
                body = &body[1..];
                RecordOperation::Subtract
            }
            _ => RecordOperation::Overwrite,
        };
        if body.first().is_some_and(|token| token == "GROUP") {
            if body.len() != 2 {
                return Err("expected RECORD [ + | - ] GROUP <group-number>".into());
            }
            let id = body[1].clone();
            let programmer = state
                .programmers
                .get(session.id)
                .ok_or("programmer does not exist")?;
            let (entry, store) = active_show_store(state)?;
            let existing = store
                .objects("group")
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|object| object.id == id);
            if record_operation == RecordOperation::Subtract && programmer.selected.is_empty() {
                let existing = existing.ok_or_else(|| format!("group {id} does not exist"))?;
                if let Some(dependent) = snapshot.groups.iter().find(|group| {
                    group
                        .derived_from
                        .as_ref()
                        .is_some_and(|derived| derived.source_group_id == id)
                }) {
                    return Err(format!(
                        "cannot delete group {id}; derived group {} depends on it",
                        dependent.id
                    ));
                }
                store
                    .delete_object("group", &existing.id)
                    .map_err(|error| error.to_string())?;
                refresh_command_show(state, &entry)?;
                return Ok(1);
            }
            let existing_group = existing
                .as_ref()
                .map(|object| {
                    serde_json::from_value::<light_programmer::GroupDefinition>(object.body.clone())
                        .map_err(|error| error.to_string())
                })
                .transpose()?;
            if record_operation != RecordOperation::Overwrite && existing_group.is_none() {
                return Err(format!("group {id} does not exist"));
            }
            let existing_membership = if existing_group.is_some() {
                let groups = snapshot
                    .groups
                    .iter()
                    .cloned()
                    .map(|group| (group.id.clone(), group))
                    .collect::<HashMap<_, _>>();
                light_programmer::resolve_group(&id, &groups)?
            } else {
                Vec::new()
            };
            let mut group = existing_group.unwrap_or_else(|| light_programmer::GroupDefinition {
                id: id.clone(),
                name: format!("Group {id}"),
                ..Default::default()
            });
            match record_operation {
                RecordOperation::Overwrite => {
                    group.fixtures = programmer.selected.clone();
                    group.derived_from = None;
                    group.frozen_from = None;
                    match programmer.selection_expression.clone() {
                        Some(light_programmer::SelectionExpression::LiveGroup {
                            group_id,
                            rule,
                        }) if group_id != id => {
                            group.derived_from = Some(light_programmer::DerivedGroup {
                                source_group_id: group_id,
                                rule,
                            });
                        }
                        Some(light_programmer::SelectionExpression::FrozenGroup {
                            group_id,
                            source_revision,
                        }) if group_id != id => {
                            group.frozen_from = Some(light_programmer::FrozenGroup {
                                source_group_id: group_id,
                                source_revision,
                                captured_at: chrono::Utc::now(),
                            });
                        }
                        _ => {}
                    }
                }
                RecordOperation::Merge => {
                    group.fixtures = existing_membership;
                    for fixture in &programmer.selected {
                        if !group.fixtures.contains(fixture) {
                            group.fixtures.push(*fixture);
                        }
                    }
                    group.derived_from = None;
                    group.frozen_from = None;
                }
                RecordOperation::Subtract => {
                    group.fixtures = existing_membership;
                    group
                        .fixtures
                        .retain(|fixture| !programmer.selected.contains(fixture));
                    group.derived_from = None;
                    group.frozen_from = None;
                }
            }
            if group.derived_from.is_some() {
                let mut groups = snapshot
                    .groups
                    .iter()
                    .cloned()
                    .map(|candidate| (candidate.id.clone(), candidate))
                    .collect::<HashMap<_, _>>();
                groups.insert(id.clone(), group.clone());
                if light_programmer::resolve_group(&id, &groups).is_err() {
                    group.derived_from = None;
                    group.frozen_from = None;
                    group.fixtures = programmer.selected.clone();
                }
            }
            store
                .put_object(
                    "group",
                    &id,
                    &serde_json::to_value(group).map_err(|error| error.to_string())?,
                    existing.map_or(0, |object| object.revision),
                )
                .map_err(|error| error.to_string())?;
            refresh_command_show(state, &entry)?;
            state.programmers.finish_selection_gesture(session.id);
            return Ok(programmer.selected.len());
        }
        if body.first().is_some_and(|token| token == "CUE") {
            let show = state.active_show.read().clone().ok_or("no show is open")?;
            let playback = state
                .desk
                .lock()
                .selected_playback(session.desk.id, show.id)
                .map_err(|error| error.to_string())?
                .ok_or("no playback is selected; use RECORD SET <playback> CUE <cue>")?;
            let number = parse_command_cue_number(&body[1..])?;
            store_cue_at(
                state,
                session,
                playback,
                Some(number),
                timing,
                record_operation,
            )?;
            return Ok(1);
        }
        if body.first().is_some_and(|token| token == "SET") {
            let (address, used) = parse_playback_address(body, true, &snapshot)?;
            if used != body.len() {
                return Err("unexpected tokens after cue target".into());
            }
            store_cue_at(
                state,
                session,
                address.playback,
                address.cue,
                timing,
                record_operation,
            )?;
            return Ok(1);
        }
        if record_operation != RecordOperation::Overwrite {
            return Err(
                "RECORD + and RECORD - currently require GROUP or SET ... CUE targets".into(),
            );
        }
        let address = command_preset_address(body)?;
        let id = address.storage_key();
        let programmer = state
            .programmers
            .get(session.id)
            .ok_or("programmer does not exist")?;
        let preset = programmer_preset(&programmer, format!("Preset {id}"), address);
        if preset.values.is_empty() && preset.group_values.is_empty() {
            return Err("the programmer has no values to record".into());
        }
        let (entry, store) = active_show_store(state)?;
        let existing = store
            .objects("preset")
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|object| {
                object.id == id
                    || decode_preset_object(object)
                        .is_ok_and(|(stored_address, _)| stored_address == address)
            });
        let storage_key = existing
            .as_ref()
            .map(|object| object.id.clone())
            .unwrap_or(id);
        store
            .put_object(
                "preset",
                &storage_key,
                &serde_json::to_value(preset).map_err(|error| error.to_string())?,
                existing.map_or(0, |object| object.revision),
            )
            .map_err(|error| error.to_string())?;
        refresh_command_show(state, &entry)?;
        return Ok(1);
    }
    if operation == "SET" {
        return execute_set_command(state, session, body);
    }
    let (entry, store) = active_show_store(state)?;
    if operation == "DELETE" && body.first().is_some_and(|token| token == "GROUP") {
        if body.len() != 2 {
            return Err("expected DELETE GROUP <group-number>".into());
        }
        let id = &body[1];
        if let Some(dependent) = snapshot.groups.iter().find(|group| {
            group
                .derived_from
                .as_ref()
                .is_some_and(|derived| &derived.source_group_id == id)
        }) {
            return Err(format!(
                "cannot delete group {id}; derived group {} depends on it",
                dependent.id
            ));
        }
        if !snapshot.groups.iter().any(|group| &group.id == id) {
            return Err(format!("group {id} does not exist"));
        }
        store
            .delete_object("group", id)
            .map_err(|error| error.to_string())?;
        refresh_command_show(state, &entry)?;
        return Ok(1);
    }
    if body.first().is_some_and(|token| token == "SET") {
        let at = body.iter().position(|token| token == "AT");
        let source_tokens = at.map_or(body, |index| &body[..index]);
        let (source, used) = parse_playback_address(source_tokens, true, &snapshot)?;
        if used != source_tokens.len() {
            return Err("unexpected cue source tokens".into());
        }
        let source_cue = source.cue.ok_or("cue source requires CUE <cue-number>")?;
        let (_, source_object, mut source_list) =
            cue_list_for_playback(&store, &snapshot, source.playback)?;
        let position = source_list
            .cues
            .iter()
            .position(|cue| cue.number == source_cue)
            .ok_or_else(|| format!("cue {source_cue} does not exist"))?;
        if operation == "DELETE" {
            source_list.cues.remove(position);
            if source_list.cues.is_empty() {
                return Err(
                    "cannot delete the only Cue; delete the Cuelist from its configuration instead"
                        .into(),
                );
            }
            let revision = store
                .put_object(
                    "cue_list",
                    &source_object.id,
                    &serde_json::to_value(source_list).map_err(|error| error.to_string())?,
                    source_object.revision,
                )
                .map_err(|error| error.to_string())?;
            refresh_command_show(state, &entry)?;
            emit_command_object_changed(state, &entry, "cue_list", &source_object.id, revision);
            return Ok(1);
        } else {
            let transfer_mode = transfer_mode.ok_or(
                "Cue MOVE/COPY requires an explicit PLAIN or STATUS choice after the operation",
            )?;
            let at = at.ok_or("MOVE and COPY require AT and a destination")?;
            let (destination, used) = parse_playback_address(&body[at + 1..], true, &snapshot)?;
            if used != body.len() - at - 1 {
                return Err("unexpected cue destination tokens".into());
            }
            let destination_number = destination
                .cue
                .ok_or("cue destination requires CUE <cue-number>")?;
            let mut cue =
                destination_cue(&source_list, position, destination_number, transfer_mode)?;
            if operation == "COPY" {
                cue.id = Uuid::new_v4();
            }
            if destination.playback == source.playback {
                if source_list
                    .cues
                    .iter()
                    .any(|item| item.number == destination_number)
                {
                    return Err("destination cue already exists".into());
                }
                if operation == "MOVE" {
                    source_list.cues.remove(position);
                }
                source_list.cues.push(cue);
                source_list
                    .cues
                    .sort_by(|a, b| a.number.total_cmp(&b.number));
                store
                    .put_object(
                        "cue_list",
                        &source_object.id,
                        &serde_json::to_value(source_list).map_err(|error| error.to_string())?,
                        source_object.revision,
                    )
                    .map_err(|error| error.to_string())?;
            } else {
                let (_, destination_object, mut destination_list) =
                    cue_list_for_playback(&store, &snapshot, destination.playback)?;
                if destination_list
                    .cues
                    .iter()
                    .any(|item| item.number == destination_number)
                {
                    return Err("destination cue already exists".into());
                }
                destination_list.cues.push(cue);
                destination_list
                    .cues
                    .sort_by(|a, b| a.number.total_cmp(&b.number));
                if operation == "MOVE" {
                    if source_list.cues.len() == 1 {
                        return Err(
                            "cannot move the only Cue out of a Cuelist; delete the Cuelist from its configuration instead"
                                .into(),
                        );
                    }
                    source_list.cues.remove(position);
                    let source_body =
                        serde_json::to_value(source_list).map_err(|error| error.to_string())?;
                    let destination_body = serde_json::to_value(destination_list)
                        .map_err(|error| error.to_string())?;
                    store
                        .mutate_objects_atomically(
                            &[
                                AtomicObjectWrite {
                                    kind: "cue_list",
                                    id: &source_object.id,
                                    body: &source_body,
                                    expected: source_object.revision,
                                },
                                AtomicObjectWrite {
                                    kind: "cue_list",
                                    id: &destination_object.id,
                                    body: &destination_body,
                                    expected: destination_object.revision,
                                },
                            ],
                            &[],
                        )
                        .map_err(|error| error.to_string())?;
                } else {
                    store
                        .put_object(
                            "cue_list",
                            &destination_object.id,
                            &serde_json::to_value(destination_list)
                                .map_err(|error| error.to_string())?,
                            destination_object.revision,
                        )
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        refresh_command_show(state, &entry)?;
        return Ok(1);
    }
    let at = body.iter().position(|token| token == "AT");
    let source_address = command_preset_address(at.map_or(body, |index| &body[..index]))?;
    let requested_source_id = source_address.storage_key();
    let source = store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            object.id == requested_source_id
                || decode_preset_object(object)
                    .is_ok_and(|(stored_address, _)| stored_address == source_address)
        })
        .ok_or_else(|| format!("preset {requested_source_id} does not exist"))?;
    let persisted_source_id = source.id.clone();
    if operation == "DELETE" {
        store
            .delete_object("preset", &persisted_source_id)
            .map_err(|error| error.to_string())?;
    } else {
        let at = at.ok_or("MOVE and COPY require AT and a destination number")?;
        if body.len() != at + 2 {
            return Err("preset destination must contain only its new number".into());
        }
        let destination_address = light_programmer::PresetAddress::new(
            source_address.family,
            body[at + 1]
                .parse::<u32>()
                .map_err(|_| "preset destination is invalid")?,
        )?;
        let destination = destination_address.storage_key();
        if store
            .objects("preset")
            .map_err(|error| error.to_string())?
            .iter()
            .any(|object| {
                object.id == destination
                    || decode_preset_object(object)
                        .is_ok_and(|(stored_address, _)| stored_address == destination_address)
            })
        {
            return Err(format!("preset {destination} already exists"));
        }
        let mut destination_body = source.body.clone();
        destination_body["number"] = serde_json::json!(destination_address.number);
        store
            .put_object("preset", &destination, &destination_body, 0)
            .map_err(|error| error.to_string())?;
        if operation == "MOVE" {
            store
                .delete_object("preset", &persisted_source_id)
                .map_err(|error| error.to_string())?;
        }
    }
    refresh_command_show(state, &entry)?;
    Ok(1)
}

fn execute_set_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
) -> Result<usize, String> {
    if tokens.first().is_some_and(|token| token == "GROUP") {
        if tokens.len() != 2 {
            return Err("expected SET GROUP <group-number>".into());
        }
        let group_id = &tokens[1];
        if !state
            .engine
            .snapshot()
            .groups
            .iter()
            .any(|group| &group.id == group_id)
        {
            return Err(format!("group {group_id} does not exist"));
        }
        emit(
            state,
            "group_configuration_requested",
            serde_json::json!({"group_id":group_id,"desk_id":session.desk.id}),
        );
        return Ok(0);
    }
    let at = tokens.iter().position(|token| token == "AT");
    if let Some(at) = at {
        let (entry, store) = active_show_store(state)?;
        if tokens.first().is_some_and(|token| token == "GROUP") {
            return Err(
                "playback pages accept Cuelists only; store the group in a Cuelist first".into(),
            );
        } else {
            let playback = tokens
                .first()
                .ok_or("playback number is required")?
                .parse::<u16>()
                .map_err(|_| "playback number is invalid")?;
            if !state
                .engine
                .snapshot()
                .playbacks
                .iter()
                .any(|item| item.number == playback)
            {
                return Err(format!("Cuelist {playback} does not exist"));
            }
            if !state.engine.snapshot().playbacks.iter().any(|item| {
                item.number == playback
                    && matches!(item.target, light_playback::PlaybackTarget::CueList { .. })
            }) {
                return Err(format!(
                    "Cuelist {playback} cannot be assigned to a playback"
                ));
            }
            let (page, slot) = parse_page_slot(&tokens[at + 1..])?;
            assign_page_slot(&store, &state.engine.snapshot(), page, slot, playback)?;
        }
        refresh_command_show(state, &entry)?;
        return Ok(1);
    }
    let snapshot = state.engine.snapshot();
    let (address, used) = parse_playback_address(tokens, false, &snapshot)?;
    if used != tokens.len() {
        return Err("unexpected tokens after playback selection".into());
    }
    emit(
        state,
        "playback_configuration_requested",
        serde_json::json!({"playback":address.playback,"cue":address.cue}),
    );
    Ok(0)
}

fn parse_page_slot(tokens: &[String]) -> Result<(u8, u8), String> {
    if tokens.len() != 3 || tokens[1] != "." {
        return Err("expected <page> . <page-playback>".into());
    }
    Ok((
        tokens[0].parse().map_err(|_| "page number is invalid")?,
        tokens[2]
            .parse()
            .map_err(|_| "page playback number is invalid")?,
    ))
}

fn assign_page_slot(
    store: &ShowStore,
    snapshot: &EngineSnapshot,
    page: u8,
    slot: u8,
    playback: u16,
) -> Result<(), String> {
    let object = store
        .objects("playback_page")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == page.to_string());
    let mut definition = if let Some(object) = &object {
        serde_json::from_value::<light_playback::PlaybackPage>(object.body.clone())
            .map_err(|error| error.to_string())?
    } else {
        snapshot
            .playback_pages
            .iter()
            .find(|item| item.number == page)
            .cloned()
            .unwrap_or(light_playback::PlaybackPage {
                number: page,
                name: format!("Page {page}"),
                slots: HashMap::new(),
            })
    };
    definition.slots.insert(slot, playback);
    store
        .put_object(
            "playback_page",
            &page.to_string(),
            &serde_json::to_value(definition).map_err(|error| error.to_string())?,
            object.map_or(0, |object| object.revision),
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Clone, Debug)]
struct ParsedMixedSelection {
    fixtures: Vec<light_core::FixtureId>,
    sources: Vec<light_programmer::SelectionReference>,
}

fn parse_group_mixed_selection(
    snapshot: &EngineSnapshot,
    tokens: &[String],
    default_to_group: bool,
) -> Result<ParsedMixedSelection, String> {
    fn fixture_by_number(
        snapshot: &EngineSnapshot,
        token: &str,
    ) -> Result<Vec<light_core::FixtureId>, String> {
        let number = token
            .parse::<u32>()
            .map_err(|_| "fixture number is invalid")?;
        snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_number == Some(number))
            .map(selectable_fixture_ids)
            .ok_or_else(|| format!("fixture {number} does not exist"))
    }
    fn group_members(
        snapshot: &EngineSnapshot,
        groups: &HashMap<String, light_programmer::GroupDefinition>,
        id: &str,
        skip_missing: bool,
    ) -> Result<Vec<light_core::FixtureId>, String> {
        if skip_missing && !groups.contains_key(id) {
            return Ok(Vec::new());
        }
        light_programmer::resolve_group(id, groups)
            .map_err(|error| {
                if skip_missing && error.contains("does not exist") {
                    String::new()
                } else {
                    error
                }
            })
            .map(|members| {
                if members.is_empty() && skip_missing && !groups.contains_key(id) {
                    Vec::new()
                } else {
                    let valid = snapshot
                        .fixtures
                        .iter()
                        .flat_map(|fixture| {
                            std::iter::once(fixture.fixture_id)
                                .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
                        })
                        .collect::<HashSet<_>>();
                    members
                        .into_iter()
                        .filter(|fixture| valid.contains(fixture))
                        .collect()
                }
            })
    }

    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    #[derive(Clone, Copy)]
    enum TermKind {
        Fixture,
        LiveGroup,
        DereferencedGroup,
    }

    fn reference_for_fixture(
        fixture_id: light_core::FixtureId,
        remove: bool,
    ) -> light_programmer::SelectionReference {
        if remove {
            light_programmer::SelectionReference::RemoveFixture { fixture_id }
        } else {
            light_programmer::SelectionReference::Fixture { fixture_id }
        }
    }

    fn reference_for_live_group(
        group_id: String,
        remove: bool,
    ) -> light_programmer::SelectionReference {
        if remove {
            light_programmer::SelectionReference::RemoveLiveGroup { group_id }
        } else {
            light_programmer::SelectionReference::LiveGroup { group_id }
        }
    }

    let mut sources = Vec::new();
    let mut index = 0;
    let mut operation = "+";
    let mut term_kind = if default_to_group {
        TermKind::LiveGroup
    } else {
        TermKind::Fixture
    };
    while index < tokens.len() {
        match tokens[index].as_str() {
            "+" | "-" => {
                operation = tokens[index].as_str();
                term_kind = TermKind::Fixture;
                index += 1;
            }
            "GROUP" => {
                // DEGRP tokenization produces GROUP GROUP. The outer GROUP has already been
                // consumed when a command starts in Group mode, so a leading GROUP in that form
                // is also a dereference marker.
                if tokens
                    .get(index + 1)
                    .is_some_and(|candidate| candidate == "GROUP")
                {
                    term_kind = TermKind::DereferencedGroup;
                    index += 2;
                } else if index == 0 && default_to_group {
                    term_kind = TermKind::DereferencedGroup;
                    index += 1;
                } else {
                    term_kind = TermKind::LiveGroup;
                    index += 1;
                }
            }
            "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS" => {
                term_kind = TermKind::Fixture;
                index += 1;
            }
            token => {
                if tokens
                    .get(index + 1)
                    .is_some_and(|candidate| candidate == "THRU")
                {
                    let end = tokens
                        .get(index + 2)
                        .ok_or("THRU requires an end reference")?
                        .parse::<i32>()
                        .map_err(|_| "range end is invalid")?;
                    let start = token.parse::<i32>().map_err(|_| "range start is invalid")?;
                    let step = if start <= end { 1 } else { -1 };
                    let mut current = start;
                    loop {
                        let id = current.to_string();
                        match term_kind {
                            TermKind::LiveGroup => {
                                if groups.contains_key(&id) {
                                    // Resolve now to reject invalid/cyclic stored Groups while the
                                    // reference itself remains live for future membership edits.
                                    group_members(snapshot, &groups, &id, true)?;
                                    sources.push(reference_for_live_group(id, operation == "-"));
                                }
                            }
                            TermKind::DereferencedGroup => {
                                for fixture in group_members(snapshot, &groups, &id, true)? {
                                    sources.push(reference_for_fixture(fixture, operation == "-"));
                                }
                            }
                            TermKind::Fixture => {
                                for fixture in fixture_by_number(snapshot, &id)? {
                                    sources.push(reference_for_fixture(fixture, operation == "-"));
                                }
                            }
                        }
                        if current == end {
                            break;
                        }
                        current += step;
                    }
                    index += 3;
                } else {
                    match term_kind {
                        TermKind::LiveGroup => {
                            group_members(snapshot, &groups, token, false)?;
                            sources
                                .push(reference_for_live_group(token.to_owned(), operation == "-"));
                        }
                        TermKind::DereferencedGroup => {
                            for fixture in group_members(snapshot, &groups, token, false)? {
                                sources.push(reference_for_fixture(fixture, operation == "-"));
                            }
                        }
                        TermKind::Fixture => {
                            for fixture in fixture_by_number(snapshot, token)? {
                                sources.push(reference_for_fixture(fixture, operation == "-"));
                            }
                        }
                    }
                    index += 1;
                }
            }
        }
    }
    let fixtures = light_programmer::resolve_selection_references(&sources, &groups);
    Ok(ParsedMixedSelection { fixtures, sources })
}

fn parse_spread_points(tokens: &[String]) -> Result<Vec<f32>, String> {
    if tokens.len() < 3 || tokens.len().is_multiple_of(2) {
        return Err("a spread requires levels separated by THRU".into());
    }
    let mut points = Vec::with_capacity(tokens.len().div_ceil(2));
    for (index, token) in tokens.iter().enumerate() {
        if index % 2 == 1 {
            if token != "THRU" {
                return Err("spread control points must be separated by THRU".into());
            }
            continue;
        }
        let percent = if token == "FULL" {
            100.0
        } else {
            token
                .parse::<f32>()
                .map_err(|_| "spread levels must be percentages or FULL")?
        };
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err("spread levels must be within 0-100".into());
        }
        points.push(percent / 100.0);
    }
    Ok(points)
}

fn spread_position(points: &[f32], index: usize, count: usize) -> f32 {
    if points.len() == 1 || count <= 1 {
        return points[0];
    }
    let position = index as f32 * (points.len() - 1) as f32 / (count - 1) as f32;
    let left = position.floor() as usize;
    let right = position.ceil() as usize;
    points[left] + (points[right] - points[left]) * (position - left as f32)
}

fn parse_command_cue_number(tokens: &[String]) -> Result<f64, String> {
    if tokens.is_empty() {
        return Err("CUE requires a cue number".into());
    }
    let value = tokens.join("");
    let number = value.parse::<f64>().map_err(|_| "cue number is invalid")?;
    if !number.is_finite() || number <= 0.0 {
        return Err("cue number must be positive".into());
    }
    Ok(number)
}

fn execute_cue_operation(
    state: &AppState,
    session: &Session,
    tokens: &[String],
) -> Result<usize, String> {
    let load = tokens.get(1).is_some_and(|token| token == "CUE");
    let start = if load { 2 } else { 1 };
    let snapshot = state.engine.snapshot();
    let (playback, cue_number) = if tokens.get(start).is_some_and(|token| token == "SET") {
        let (address, consumed) = parse_playback_address(&tokens[start..], true, &snapshot)?;
        if start + consumed != tokens.len() {
            return Err("unexpected tokens after Cue address".into());
        }
        (
            address.playback,
            address
                .cue
                .ok_or("explicit Cue address requires CUE and a Cue number")?,
        )
    } else {
        let show = state.active_show.read().clone().ok_or("no show is open")?;
        let selected = state
            .desk
            .lock()
            .selected_playback(session.desk.id, show.id)
            .map_err(|error| error.to_string())?
            .ok_or(
                "no playback is selected; select a playback or use CUE SET <playback> CUE <cue>",
            )?;
        (selected, parse_command_cue_number(&tokens[start..])?)
    };
    if !snapshot
        .playbacks
        .iter()
        .any(|definition| definition.number == playback)
    {
        return Err(format!("playback {playback} does not exist"));
    }
    let mut engine = state.engine.playback().write();
    if load {
        engine.load_playback(playback, cue_number)?;
    } else {
        engine.goto_playback(playback, cue_number)?;
    }
    drop(engine);
    emit(
        state,
        "playback_changed",
        serde_json::json!({"playback_number":playback,"action":if load {"load"} else {"go-to"},"cue_number":cue_number,"session_id":session.id}),
    );
    Ok(1)
}

fn pending_cue_transfer_choice(command_line: &str) -> Option<serde_json::Value> {
    let tokens = command_line
        .replace(',', ".")
        .replace('.', " . ")
        .split_whitespace()
        .map(|token| token.to_ascii_uppercase())
        .collect::<Vec<_>>();
    let operation = match tokens.first()?.as_str() {
        "COPY" | "CPY" => "COPY",
        "MOVE" | "MOV" => "MOVE",
        _ => return None,
    };
    if tokens
        .get(1)
        .is_some_and(|token| matches!(token.as_str(), "PLAIN" | "STATUS"))
    {
        return None;
    }
    let at = tokens.iter().position(|token| token == "AT")?;
    if tokens.get(1).is_none_or(|token| token != "SET")
        || !tokens[1..at].iter().any(|token| token == "CUE")
        || tokens.get(at + 1).is_none_or(|token| token != "SET")
        || !tokens[at + 1..].iter().any(|token| token == "CUE")
    {
        return None;
    }
    let title = if operation == "COPY" { "Copy" } else { "Move" };
    let suffix = tokens[1..].join(" ");
    Some(serde_json::json!({
        "type":"cue_move_copy",
        "operation":operation.to_ascii_lowercase(),
        "command":command_line,
        "options":[
            {
                "id":"plain",
                "label":format!("Plain {title}"),
                "command":format!("{operation} PLAIN {suffix}")
            },
            {
                "id":"status",
                "label":format!("Status {title}"),
                "command":format!("{operation} STATUS {suffix}")
            }
        ],
        "cancel_label":"Cancel"
    }))
}

fn command_speed_group_index(token: &str) -> Result<usize, String> {
    let group = token
        .parse::<usize>()
        .map_err(|_| "Speed Group number is invalid")?;
    if !(1..=5).contains(&group) {
        return Err("Speed Group number must be within 1-5".into());
    }
    Ok(group - 1)
}

fn command_bpm_at(tokens: &[String]) -> Result<(f64, usize), String> {
    let whole = tokens.first().ok_or("AT requires a BPM value")?;
    let (value, consumed) = if tokens.get(1).is_some_and(|token| token == ".") {
        let fraction = tokens
            .get(2)
            .ok_or("BPM decimal requires digits after the separator")?;
        (format!("{whole}.{fraction}"), 3)
    } else {
        (whole.clone(), 1)
    };
    let bpm = value.parse::<f64>().map_err(|_| "BPM value is invalid")?;
    if !bpm.is_finite() {
        return Err("BPM value must be finite".into());
    }
    Ok((bpm, consumed))
}

fn execute_speed_group_command(state: &AppState, tokens: &[String]) -> Result<usize, String> {
    if tokens.len() < 5 || tokens[0] != "SPD" || tokens[1] != "GRP" || tokens[3] != "AT" {
        return Err("expected SPD GRP <1-5> AT <BPM | +/- BPM | SPD GRP <1-5>>".into());
    }
    let source = command_speed_group_index(&tokens[2])?;
    let right = &tokens[4..];
    let now = application_millis(state);
    let mut controllers = state.speed_groups.lock();
    let affected = if right.first().is_some_and(|token| token == "SPD") {
        if right.len() != 3 || right[1] != "GRP" {
            return Err("synchronization target must be SPD GRP <1-5>".into());
        }
        let target = command_speed_group_index(&right[2])?;
        synchronize_speed_groups(&mut controllers, source, target, now)
            .map_err(|error| error.message)?;
        vec![source, target]
    } else {
        let (relative, value_tokens) = match right.first().map(String::as_str) {
            Some("+") => (1.0, &right[1..]),
            Some("-") => (-1.0, &right[1..]),
            _ => (0.0, right),
        };
        let (entered, consumed) = command_bpm_at(value_tokens)?;
        if consumed != value_tokens.len() {
            return Err("unexpected tokens after BPM value".into());
        }
        let bpm = if relative == 0.0 {
            entered
        } else {
            controllers[source].manual_bpm() + relative * entered
        };
        unlink_speed_group(&mut controllers, source, now);
        controllers[source]
            .set_manual_bpm(bpm)
            .map_err(|error| error.to_string())?;
        controllers[source]
            .set_speed_master_scale(1.0)
            .map_err(|error| error.to_string())?;
        controllers[source].set_paused_at(false, now);
        vec![source]
    };
    copy_speed_group_runtime_to_configuration(state, &controllers, &affected);
    let snapshots: [SpeedSnapshot; 5] =
        std::array::from_fn(|index| controllers[index].snapshot(now));
    drop(controllers);

    {
        let mut owners = state.sound_capture_owners.lock();
        for &index in &affected {
            owners[index] = None;
        }
    }
    persist_server_configuration(state).map_err(|error| error.message)?;
    refresh_speed_group_engine(state);
    emit(
        state,
        "speed_group_command",
        serde_json::json!({
            "command":tokens.join(" "),
            "groups":affected.iter().map(|index| speed_group_name(*index)).collect::<Vec<_>>(),
            "snapshots":affected.iter().map(|index| snapshots[*index]).collect::<Vec<_>>()
        }),
    );
    Ok(affected.len())
}

fn execute_programmer_command(
    state: &AppState,
    session: &Session,
    command_line: &str,
) -> Result<usize, String> {
    let (tokens, timing) = tokenize_programmer_command(command_line)?;
    if tokens.is_empty() {
        return Err("the command line is empty".into());
    }
    if tokens.first().is_some_and(|token| token == "CUE") {
        return execute_cue_operation(state, session, &tokens);
    }
    if tokens.first().is_some_and(|token| token == "AT") {
        return apply_current_selection_value(
            state,
            session,
            &tokens[1..],
            programmer_value_timing(state, timing),
        );
    }
    if tokens.first().is_some_and(|token| token == "SPD") {
        return execute_speed_group_command(state, &tokens);
    }
    if matches!(
        tokens[0].as_str(),
        "RECORD" | "REC" | "UPDATE" | "DELETE" | "DEL" | "MOVE" | "MOV" | "COPY" | "CPY" | "SET"
    ) {
        return execute_show_command(state, session, &tokens, timing);
    }
    let timing = programmer_value_timing(state, timing);
    if tokens.first().is_some_and(|token| token == "GROUP") {
        let frozen = tokens.get(1).is_some_and(|token| token == "GROUP");
        let id_index = if frozen { 2 } else { 1 };
        let at_index = tokens
            .iter()
            .position(|token| token == "AT")
            .unwrap_or(tokens.len());
        let mixes_address_types = tokens[..at_index].iter().any(|token| {
            matches!(
                token.as_str(),
                "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
            )
        });
        let mixed_address = tokens[id_index..at_index]
            .iter()
            .any(|token| matches!(token.as_str(), "THRU" | "+" | "-"))
            || mixes_address_types
            || tokens[1..at_index]
                .windows(2)
                .any(|pair| pair[0] == "GROUP" && pair[1] == "GROUP");
        if at_index < tokens.len() && mixed_address {
            let snapshot = state.engine.snapshot();
            let parsed = parse_group_mixed_selection(&snapshot, &tokens[1..at_index], true)?;
            let value = &tokens[at_index + 1..];
            let level = value.first().ok_or("AT requires a level")?;
            let percent = if level == "FULL" {
                100.0
            } else {
                level
                    .parse::<f32>()
                    .map_err(|_| "level must be a percentage or FULL")?
            };
            if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                return Err("level must be within 0-100".into());
            }
            if value.len() != 1 {
                return Err("unexpected tokens after level".into());
            }
            state.programmers.select_expression(
                session.id,
                parsed.fixtures.clone(),
                light_programmer::SelectionExpression::Sources {
                    items: parsed.sources.clone(),
                },
            );
            if parsed.sources.iter().all(|source| {
                matches!(
                    source,
                    light_programmer::SelectionReference::LiveGroup { .. }
                )
            }) {
                let mut programmed = HashSet::new();
                for source in &parsed.sources {
                    let light_programmer::SelectionReference::LiveGroup { group_id } = source
                    else {
                        unreachable!("all mixed sources were checked as live Groups")
                    };
                    if programmed.insert(group_id.clone()) {
                        state.programmers.set_group_faded_with_timing(
                            session.id,
                            group_id.clone(),
                            light_core::AttributeKey::intensity(),
                            light_core::AttributeValue::Normalized(percent / 100.0),
                            timing.fade_millis,
                            timing.delay_millis,
                        );
                    }
                }
            } else {
                set_command_fixture_intensities(
                    state,
                    session,
                    parsed
                        .fixtures
                        .iter()
                        .copied()
                        .map(|fixture_id| (fixture_id, percent / 100.0)),
                    timing,
                );
            }
            return Ok(parsed.fixtures.len());
        }
        if at_index == tokens.len()
            && mixed_address
            && !tokens[id_index..at_index]
                .iter()
                .any(|token| token == "DIV")
        {
            let snapshot = state.engine.snapshot();
            let parsed = parse_group_mixed_selection(&snapshot, &tokens[1..at_index], true)?;
            state.programmers.select_expression(
                session.id,
                parsed.fixtures.clone(),
                light_programmer::SelectionExpression::Sources {
                    items: parsed.sources,
                },
            );
            state
                .programmers
                .set_command_line(session.id, command_line.to_owned());
            return Ok(parsed.fixtures.len());
        }
        let group_id = tokens
            .get(id_index)
            .ok_or("GROUP requires a group number")?
            .clone();
        let snapshot = state.engine.snapshot();
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        let base = light_programmer::resolve_group(&group_id, &groups)?;
        let rule = parse_subset_rule(&tokens[id_index + 1..at_index])?;
        let fixtures = light_programmer::apply_selection_rule(&base, &rule);
        let expression = if frozen {
            light_programmer::SelectionExpression::FrozenGroup {
                group_id: group_id.clone(),
                source_revision: snapshot.revision,
            }
        } else {
            light_programmer::SelectionExpression::LiveGroup {
                group_id: group_id.clone(),
                rule,
            }
        };
        state
            .programmers
            .select_expression(session.id, fixtures.clone(), expression);
        if at_index < tokens.len() {
            let value = &tokens[at_index + 1..];
            if value.len() == 3 && value[1] == "." {
                apply_command_preset(
                    state,
                    session,
                    &format!("{}.{}", value[0], value[2]),
                    &fixtures,
                )?;
            } else if value.iter().any(|token| token == "THRU") {
                let points = parse_spread_points(value)?;
                if frozen {
                    let count = fixtures.len();
                    set_command_fixture_intensities(
                        state,
                        session,
                        fixtures.iter().enumerate().map(|(index, fixture_id)| {
                            (*fixture_id, spread_position(&points, index, count))
                        }),
                        timing,
                    );
                } else {
                    state.programmers.set_group_faded_with_timing(
                        session.id,
                        group_id.clone(),
                        light_core::AttributeKey::intensity(),
                        light_core::AttributeValue::Spread(points),
                        timing.fade_millis,
                        timing.delay_millis,
                    );
                }
            } else {
                let relative = value.len() == 2 && matches!(value[0].as_str(), "+" | "-");
                if relative && !frozen {
                    return Err("relative group values require GROUP GROUP so each fixture keeps its own offset".into());
                }
                let level = value
                    .get(usize::from(relative))
                    .ok_or("AT requires a level")?;
                let percent = if level == "FULL" && !relative {
                    100.0
                } else {
                    level
                        .parse::<f32>()
                        .map_err(|_| "level must be a percentage or FULL")?
                };
                if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                    return Err("level must be within 0-100".into());
                }
                if frozen {
                    let resolved = state.engine.resolved_values();
                    let values = fixtures.iter().map(|fixture_id| {
                        let target = if relative {
                            let current = resolved
                                .get(&(*fixture_id, light_core::AttributeKey::intensity()))
                                .and_then(light_core::AttributeValue::normalized)
                                .unwrap_or(0.0)
                                * 100.0;
                            (current + if value[0] == "+" { percent } else { -percent })
                                .clamp(0.0, 100.0)
                        } else {
                            percent
                        };
                        (*fixture_id, target / 100.0)
                    });
                    set_command_fixture_intensities(state, session, values, timing);
                } else {
                    state.programmers.set_group_faded_with_timing(
                        session.id,
                        group_id.clone(),
                        light_core::AttributeKey::intensity(),
                        light_core::AttributeValue::Normalized(percent / 100.0),
                        timing.fade_millis,
                        timing.delay_millis,
                    );
                }
            }
        }
        return Ok(fixtures.len());
    }
    let continuing = tokens[0] == "+";
    let start = if continuing {
        1
    } else {
        usize::from(matches!(
            tokens[0].as_str(),
            "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
        ))
    };
    if tokens.len() <= start {
        return Err("expected a fixture number".into());
    }
    let snapshot = state.engine.snapshot();
    let at_index = tokens
        .iter()
        .position(|token| token == "AT")
        .unwrap_or(tokens.len());
    let (mut fixture_ids, mut selection_sources) =
        if tokens[start..at_index].iter().any(|token| token == "GROUP") {
            let parsed = parse_group_mixed_selection(&snapshot, &tokens[start..at_index], false)?;
            (parsed.fixtures, parsed.sources)
        } else {
            let fixtures = parse_fixture_selection(&snapshot.fixtures, &tokens[start..at_index])?;
            let sources = fixtures
                .iter()
                .map(|fixture_id| light_programmer::SelectionReference::Fixture {
                    fixture_id: *fixture_id,
                })
                .collect();
            (fixtures, sources)
        };
    if continuing {
        let current = state
            .programmers
            .get(session.id)
            .ok_or("programmer does not exist")?;
        let mut combined_sources = match current.selection_expression {
            Some(light_programmer::SelectionExpression::Sources { items }) => items,
            Some(light_programmer::SelectionExpression::LiveGroup {
                group_id,
                rule: light_programmer::SelectionRule::All,
            }) => vec![light_programmer::SelectionReference::LiveGroup { group_id }],
            _ => current
                .selected
                .into_iter()
                .map(|fixture_id| light_programmer::SelectionReference::Fixture { fixture_id })
                .collect(),
        };
        combined_sources.extend(selection_sources);
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        fixture_ids = light_programmer::resolve_selection_references(&combined_sources, &groups);
        selection_sources = combined_sources;
    }
    let selection_expression = light_programmer::SelectionExpression::Sources {
        items: selection_sources,
    };
    if at_index == tokens.len() {
        state
            .programmers
            .select_expression(session.id, fixture_ids.clone(), selection_expression);
        state
            .programmers
            .set_command_line(session.id, command_line.to_owned());
        return Ok(fixture_ids.len());
    }
    let value = &tokens[at_index + 1..];
    if value.len() == 3 && value[1] == "." {
        state
            .programmers
            .select_expression(session.id, fixture_ids.clone(), selection_expression);
        apply_command_preset(
            state,
            session,
            &format!("{}.{}", value[0], value[2]),
            &fixture_ids,
        )?;
        return Ok(fixture_ids.len());
    }
    if value.iter().any(|token| token == "THRU") {
        let points = parse_spread_points(value)?;
        let count = fixture_ids.len();
        state
            .programmers
            .select_expression(session.id, fixture_ids.clone(), selection_expression);
        set_command_fixture_intensities(
            state,
            session,
            fixture_ids
                .iter()
                .enumerate()
                .map(|(index, fixture_id)| (*fixture_id, spread_position(&points, index, count))),
            timing,
        );
        return Ok(fixture_ids.len());
    }
    let relative = value.len() == 2 && matches!(value[0].as_str(), "+" | "-");
    let level_token = value
        .get(usize::from(relative))
        .ok_or("AT requires a level")?;
    let percent = if level_token == "FULL" && !relative {
        100.0
    } else {
        level_token
            .parse::<f32>()
            .map_err(|_| "level must be a percentage or FULL")?
    };
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err("level must be within 0-100".into());
    }
    state
        .programmers
        .select_expression(session.id, fixture_ids.clone(), selection_expression);
    state
        .programmers
        .set_command_line(session.id, command_line.to_owned());
    let resolved = relative.then(|| state.engine.resolved_values());
    let values = fixture_ids.iter().map(|fixture_id| {
        let target = if let Some(resolved) = &resolved {
            let current = resolved
                .get(&(*fixture_id, light_core::AttributeKey::intensity()))
                .and_then(light_core::AttributeValue::normalized)
                .unwrap_or(0.0)
                * 100.0;
            (current + if value[0] == "+" { percent } else { -percent }).clamp(0.0, 100.0)
        } else {
            percent
        };
        (*fixture_id, target / 100.0)
    });
    set_command_fixture_intensities(state, session, values, timing);
    Ok(fixture_ids.len())
}

fn apply_current_selection_value(
    state: &AppState,
    session: &Session,
    value: &[String],
    timing: CommandTiming,
) -> Result<usize, String> {
    let current = state
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?;
    if current.selected.is_empty() {
        return Err("AT requires a current selection".into());
    }
    if value.len() == 3 && value[1] == "." {
        apply_command_preset(
            state,
            session,
            &format!("{}.{}", value[0], value[2]),
            &current.selected,
        )?;
        return Ok(current.selected.len());
    }
    if value.iter().any(|token| token == "THRU") {
        let points = parse_spread_points(value)?;
        if let Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) =
            current.selection_expression.clone()
        {
            state.programmers.set_group_faded_with_timing(
                session.id,
                group_id,
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Spread(points),
                timing.fade_millis,
                timing.delay_millis,
            );
            return Ok(current.selected.len());
        }
        let count = current.selected.len();
        set_command_fixture_intensities(
            state,
            session,
            current
                .selected
                .iter()
                .enumerate()
                .map(|(index, fixture_id)| (*fixture_id, spread_position(&points, index, count))),
            timing,
        );
        return Ok(current.selected.len());
    }
    let relative = value.len() == 2 && matches!(value[0].as_str(), "+" | "-");
    if value.len() != if relative { 2 } else { 1 } {
        return Err("unexpected tokens after level".into());
    }
    let level_token = value
        .get(usize::from(relative))
        .ok_or("AT requires a level")?;
    let percent = if level_token == "FULL" && !relative {
        100.0
    } else {
        level_token
            .parse::<f32>()
            .map_err(|_| "level must be a percentage or FULL")?
    };
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err("level must be within 0-100".into());
    }
    if let Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) =
        current.selection_expression.clone()
    {
        if relative {
            return Err(
                "relative group values require GROUP GROUP so each fixture keeps its own offset"
                    .into(),
            );
        }
        state.programmers.set_group_faded_with_timing(
            session.id,
            group_id,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(percent / 100.0),
            timing.fade_millis,
            timing.delay_millis,
        );
        return Ok(current.selected.len());
    }
    let resolved = relative.then(|| state.engine.resolved_values());
    let values = current.selected.iter().map(|fixture_id| {
        let target = if let Some(resolved) = &resolved {
            let current = resolved
                .get(&(*fixture_id, light_core::AttributeKey::intensity()))
                .and_then(light_core::AttributeValue::normalized)
                .unwrap_or(0.0)
                * 100.0;
            (current + if value[0] == "+" { percent } else { -percent }).clamp(0.0, 100.0)
        } else {
            percent
        };
        (*fixture_id, target / 100.0)
    });
    set_command_fixture_intensities(state, session, values, timing);
    Ok(current.selected.len())
}
fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Session, ApiError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized("missing session token"))?;
    authenticate_token(state, token)
}
fn authenticate_token(state: &AppState, token: &str) -> Result<Session, ApiError> {
    let session = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("invalid session token"))?;
    attach_session_command_context(state, &session);
    Ok(session)
}

fn attach_session_command_context(state: &AppState, session: &Session) {
    state
        .programmers
        .attach_command_context(session.id, SessionId(session.desk.id));
}
fn parse_if_match(headers: &HeaderMap) -> Result<u64, ApiError> {
    let value = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("If-Match revision is required"))?
        .trim_matches('"');
    value
        .parse()
        .map_err(|_| ApiError::bad_request("If-Match must contain a numeric revision"))
}
fn backup_show(state: &AppState, entry: &ShowEntry) -> Result<PathBuf, ApiError> {
    let directory = state.data_dir.join("backups");
    std::fs::create_dir_all(&directory).map_err(ApiError::io)?;
    let destination = directory.join(format!(
        "{}-{}.show",
        entry.name,
        chrono::Utc::now().timestamp_millis()
    ));
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .backup_to(&destination)
        .map_err(ApiError::store)?;
    let prefix = format!("{}-", entry.name);
    let mut backups = std::fs::read_dir(&directory)
        .map_err(ApiError::io)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".show"))
        })
        .collect::<Vec<_>>();
    backups.sort();
    let retention = state.configuration.read().backup_retention;
    let remove_count = backups.len().saturating_sub(retention);
    for path in backups.into_iter().take(remove_count) {
        std::fs::remove_file(path).map_err(ApiError::io)?;
    }
    Ok(destination)
}
async fn activate_snapshot(
    state: &AppState,
    snapshot: EngineSnapshot,
    transition: &Transition,
    duration: Option<u64>,
) -> Result<(), ApiError> {
    // A remembered Highlight selection belongs only to the current live show context. Clear the
    // transient overlay before any transition so it cannot reappear in the newly loaded show.
    state.highlight.clear_all();
    state.patch_preview_highlights.lock().clear();
    state.engine.clear_highlighted_fixtures();
    let media_fixture_ids = snapshot
        .fixtures
        .iter()
        .filter(|fixture| fixture.direct_control.is_some())
        .map(|fixture| fixture.fixture_id)
        .collect::<std::collections::HashSet<_>>();
    state
        .media_status
        .write()
        .retain(|fixture, _| media_fixture_ids.contains(fixture));
    state.media_cache.lock().retain_fixtures(
        &media_fixture_ids
            .iter()
            .map(|fixture| fixture.0.to_string())
            .collect(),
    );
    let frame = Duration::from_millis(25);
    match transition {
        Transition::HoldCurrent => {
            state.output_control.lock().hold = true;
            state
                .engine
                .replace_snapshot_releasing_playback(snapshot)
                .map_err(|error| ApiError::internal(error.to_string()))?;
            tokio::time::sleep(frame).await;
            state.output_control.lock().hold = false;
        }
        Transition::SafeBlackout => {
            state.output_control.lock().options.blackout = true;
            tokio::time::sleep(frame * 2).await;
            state
                .engine
                .replace_snapshot_releasing_playback(snapshot)
                .map_err(|error| ApiError::internal(error.to_string()))?;
            tokio::time::sleep(frame).await;
            state.output_control.lock().options.blackout = false;
        }
        Transition::TimedFade => {
            let duration = duration.unwrap_or(1_000).clamp(100, 30_000);
            let steps = 20_u64;
            let sleep = Duration::from_millis((duration / (steps * 2)).max(1));
            for step in 1..=steps {
                state.output_control.lock().options.grand_master = 1.0 - step as f32 / steps as f32;
                tokio::time::sleep(sleep).await;
            }
            state
                .engine
                .replace_snapshot_releasing_playback(snapshot)
                .map_err(|error| ApiError::internal(error.to_string()))?;
            for step in 1..=steps {
                state.output_control.lock().options.grand_master = step as f32 / steps as f32;
                tokio::time::sleep(sleep).await;
            }
        }
    }
    Ok(())
}
fn spawn_control_inputs(
    state: &AppState,
    cancel: CancellationToken,
) -> Vec<tokio::task::JoinHandle<()>> {
    let configuration = state.configuration.read().clone();
    let mut tasks = Vec::new();
    if state.manual_clock.is_none() {
        let feedback_state = state.clone();
        let feedback_cancel = cancel.clone();
        tasks.push(tokio::spawn(async move{let mut interval=tokio::time::interval(Duration::from_millis(500));loop{tokio::select!{_=feedback_cancel.cancelled()=>break,_=interval.tick()=>send_osc_feedback(&feedback_state,false)}}}));
        let refresh_state = state.clone();
        let refresh_cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            loop {
                tokio::select! {
                    _ = refresh_cancel.cancelled() => break,
                    _ = interval.tick() => { refresh_speed_group_engine(&refresh_state); }
                }
            }
        }));
    }
    for (address, protocol) in [
        (configuration.osc_bind, UdpInputProtocol::Osc),
        (
            configuration.art_timecode_bind,
            UdpInputProtocol::ArtTimeCode,
        ),
    ] {
        let Some(address) = address else {
            continue;
        };
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match UdpControlInput::bind(address, protocol).await {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%address,%error,"control input could not bind"),
            }
        }));
    }
    for port in configuration.midi_inputs {
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match MidiControlInput::open(&port) {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%port,%error,"MIDI input could not open"),
            }
        }));
    }
    if let Some(address) = configuration.rtp_midi_bind {
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            match RtpMidiInput::bind(address, "Light").await {
                Ok(input) => drive_control_input(state, cancel, input).await,
                Err(error) => tracing::error!(%address,%error,"RTP-MIDI input could not bind"),
            }
        }));
    }
    tasks
}

async fn drive_control_input<I: ControlInput>(
    state: AppState,
    cancel: CancellationToken,
    mut input: I,
) {
    loop {
        tokio::select! { _=cancel.cancelled()=>break,event=input.next_event()=>match event { Some(event)=>handle_control_event(&state,event),None=>break } }
    }
}

fn handle_control_event(state: &AppState, event: ControlEvent) {
    if let ControlEvent::Timecode(timecode) = &event {
        ingest_timecode(state, timecode.clone());
    }
    let input_locked = if let ControlEvent::Osc { address, .. } = &event {
        let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
        parts
            .get(1)
            .and_then(|alias| osc_control_desk(state, alias))
            .is_some_and(|desk| read_desk_lock(state, desk.id).locked)
    } else {
        false
    };
    if let ControlEvent::Osc {
        address, arguments, ..
    } = &event
        && let Some(configuration) = &state.configuration.read().osc_timecode
        && &configuration.address == address
        && let [
            OscArgument::Int(hours),
            OscArgument::Int(minutes),
            OscArgument::Int(seconds),
            OscArgument::Int(frames),
            ..,
        ] = arguments.as_slice()
        && (0..24).contains(hours)
        && (0..60).contains(minutes)
        && (0..60).contains(seconds)
        && (0..i32::from(configuration.rate.nominal_frames())).contains(frames)
    {
        ingest_timecode(
            state,
            SmpteTimecode {
                hours: *hours as u8,
                minutes: *minutes as u8,
                seconds: *seconds as u8,
                frames: *frames as u8,
                rate: configuration.rate,
                source: format!("osc:{address}"),
                received_at: chrono::Utc::now(),
            },
        );
    }
    if let ControlEvent::Osc {
        address,
        arguments,
        source,
    } = &event
    {
        if !handle_subscription_osc(state, address, arguments, source.as_deref()) && !input_locked {
            handle_playback_osc(state, address, arguments, source.as_deref());
            handle_highlight_osc(state, address, arguments, source.as_deref());
            handle_programmer_osc(state, address, arguments, source.as_deref());
            handle_timing_osc(state, address, arguments);
            handle_encoder_osc(state, address, arguments);
        }
        send_osc_feedback(state, false);
    }
    if input_locked {
        return;
    }
    let mappings = state.engine.snapshot().control_mappings.clone();
    let mut mapping_applied = false;
    for mapping in mappings.iter().filter(|mapping| mapping.matches(&event)) {
        mapping_applied = true;
        match mapping.action {
            ControlAction::CueGo { cue_list_id } => {
                let _ = state.engine.playback().write().go(cue_list_id);
            }
            ControlAction::CueBack { cue_list_id } => {
                let _ = state.engine.playback().write().back(cue_list_id);
            }
            ControlAction::CuePause { cue_list_id } => {
                let _ = state.engine.playback().write().pause(cue_list_id);
            }
            ControlAction::CueRelease { cue_list_id } => {
                state.engine.playback().write().release(cue_list_id);
            }
            ControlAction::Blackout { enabled } => {
                state.output_control.lock().options.blackout = enabled
            }
            ControlAction::GrandMaster { level } => {
                state.output_control.lock().options.grand_master = level.clamp(0.0, 1.0)
            }
            ControlAction::DeskSet => {
                emit(state, "desk_action", serde_json::json!({"action":"set"}))
            }
        }
    }
    if mapping_applied {
        let _ = persist_active_playbacks(state);
        let _ = persist_output_runtime(state);
    }
    emit(
        state,
        "control_event",
        serde_json::to_value(event)
            .unwrap_or_else(|_| serde_json::json!({"error":"serialization failed"})),
    );
}

fn handle_subscription_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> bool {
    if address != "/light/subscribe" && address != "/light/unsubscribe" {
        return false;
    }
    let Some(client_id) = arguments.first().and_then(|v| match v {
        OscArgument::String(v) => Some(v.clone()),
        _ => None,
    }) else {
        return true;
    };
    if address == "/light/unsubscribe" {
        state.osc_subscribers.lock().remove(&client_id);
        emit(
            state,
            "hardware_connection_changed",
            serde_json::json!({"connected":!state.osc_subscribers.lock().is_empty()}),
        );
        return true;
    }
    let Some(desk_alias) = arguments.get(1).and_then(|v| match v {
        OscArgument::String(v) => Some(v.clone()),
        _ => None,
    }) else {
        return true;
    };
    let Some(port) = arguments.get(2).and_then(|v| match v {
        OscArgument::Int(v) => u16::try_from(*v).ok(),
        _ => None,
    }) else {
        return true;
    };
    let Some(command_source) = source.and_then(|v| v.parse::<SocketAddr>().ok()) else {
        return true;
    };
    let mut target = command_source;
    target.set_port(port);
    let Some(desk) = osc_control_desk(state, &desk_alias) else {
        return true;
    };
    let desk_alias = desk.osc_alias.clone();
    let existing = state.osc_subscribers.lock().get(&client_id).cloned();
    let attached_session = {
        let sessions = state.sessions.read();
        sessions
            .values()
            .find(|session| {
                session.connected
                    && session.desk.id == desk.id
                    && state.programmers.get(session.id).is_some()
            })
            .cloned()
    };
    if let Some(session) = &attached_session {
        attach_session_command_context(state, session);
    }
    let session_id = existing
        .filter(|subscriber| subscriber.desk_alias.eq_ignore_ascii_case(&desk_alias))
        .map(|subscriber| subscriber.session_id)
        .or_else(|| attached_session.map(|session| session.id))
        .unwrap_or_else(|| {
            let Some(user) = state
                .desk
                .lock()
                .users()
                .ok()
                .and_then(|u| u.into_iter().find(|u| u.enabled))
            else {
                return SessionId::new();
            };
            let id = SessionId::new();
            let session = Session {
                id,
                user: user.clone(),
                token: Uuid::new_v4().to_string(),
                connected: true,
                desk: desk.clone(),
            };
            state.programmers.start(id, user.id);
            attach_session_command_context(state, &session);
            state.sessions.write().insert(id, session);
            id
        });
    state.osc_subscribers.lock().insert(
        client_id,
        OscSubscriber {
            desk_alias,
            target,
            command_source,
            session_id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    emit(
        state,
        "hardware_connection_changed",
        serde_json::json!({"connected":true}),
    );
    send_osc_feedback(state, true);
    true
}

fn osc_pressed(arguments: &[OscArgument]) -> bool {
    arguments
        .first()
        .map(|v| match v {
            OscArgument::Bool(v) => *v,
            OscArgument::Int(v) => *v != 0,
            OscArgument::Float(v) => *v > 0.0,
            OscArgument::String(v) => v != "0" && v != "false",
        })
        .unwrap_or(true)
}

fn remove_command_token(value: &str) -> String {
    let trimmed = value.trim_end();
    let Some(last) = trimmed.chars().next_back() else {
        return String::new();
    };
    if last.is_ascii_digit() || matches!(last, '.' | '-' | '+') {
        let end = trimmed.len() - last.len_utf8();
        return trimmed[..end].trim_end().to_string();
    }
    let mut start = trimmed.len();
    for (index, character) in trimmed.char_indices().rev() {
        if character.is_ascii_alphabetic() {
            start = index;
        } else {
            break;
        }
    }
    trimmed[..start].trim_end().to_string()
}

fn edit_osc_programmer_command(command: &str, key: &str, target: &str) -> String {
    let trimmed = command.trim();
    let default_scope = if target == "GROUP" { 'G' } else { 'F' };
    let pending_file_action = match key {
        "set" => Some("SET"),
        "cpy" | "copy" => Some("COPY"),
        "mov" | "move" => Some("MOVE"),
        "del" | "delete" => Some("DELETE"),
        _ => None,
    };
    if let Some(action) = pending_file_action
        && (trimmed.is_empty() || matches!(trimmed, "FIXTURE" | "GROUP"))
    {
        return action.into();
    }
    if let Some(digit) = key.strip_prefix("digit-") {
        if trimmed.is_empty() {
            return format!("F{digit}");
        }
        if trimmed.eq_ignore_ascii_case("GROUP") {
            return format!("G{digit}");
        }
        if trimmed.ends_with(['+', '-']) {
            return format!("{trimmed} {default_scope}{digit}");
        }
        if trimmed.ends_with(['F', 'G', 'f', 'g']) {
            return format!("{trimmed}{digit}");
        }
        if trimmed
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphabetic())
        {
            return format!("{trimmed} {digit}");
        }
        return format!("{trimmed}{digit}");
    }
    if matches!(key, "grp" | "group") {
        if trimmed.is_empty() {
            return "GROUP".into();
        }
        if trimmed.ends_with(['+', '-']) {
            let scope = if default_scope == 'G' { 'F' } else { 'G' };
            return format!("{trimmed} {scope}");
        }
    }
    let token = match key {
        "grp" | "group" => "GROUP",
        "thru" => "THRU",
        "plus" | "add" => "+",
        "minus" | "subtract" => "-",
        "at" => "AT",
        "time" => "TIME",
        "delay" => "DELAY",
        "dot" => ".",
        "div" => "DIV",
        "set" => "SET",
        "cue" => "CUE",
        "rec" => "RECORD",
        value => value,
    };
    if matches!(
        token,
        "GROUP" | "THRU" | "+" | "-" | "AT" | "TIME" | "DELAY" | "DIV" | "SET" | "CUE" | "RECORD"
    ) {
        format!("{trimmed} {token}")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            + " "
    } else {
        format!("{trimmed}{token}")
    }
}

fn handle_highlight_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != "light" || parts[2] != "highlight" || !osc_pressed(arguments)
    {
        return;
    }
    let action = match parts[3] {
        "on" => HighlightAction::On,
        "off" => HighlightAction::Off,
        "toggle" => HighlightAction::Toggle,
        "next" => HighlightAction::Next,
        "previous" | "prev" => HighlightAction::Previous,
        "all" => HighlightAction::All,
        _ => return,
    };
    let Some(source) = source.and_then(|value| value.parse::<SocketAddr>().ok()) else {
        return;
    };
    let session_id = {
        let mut subscribers = state.osc_subscribers.lock();
        let Some(subscriber) = subscribers.values_mut().find(|subscriber| {
            subscriber.command_source == source && subscriber.desk_alias == parts[1]
        }) else {
            return;
        };
        let now = Instant::now();
        if is_duplicate_osc_action(
            subscriber
                .last_highlight_action
                .as_ref()
                .map(|(previous, received_at)| (previous.as_str(), *received_at)),
            action,
            now,
        ) {
            return;
        }
        subscriber.last_highlight_action = Some((action.osc_dedupe_key().to_owned(), now));
        subscriber.session_id
    };
    let Some(session) = state.sessions.read().get(&session_id).cloned() else {
        return;
    };
    attach_session_command_context(state, &session);
    let Some(programmer) = state.programmers.get(session.id) else {
        return;
    };
    let Some(selection) = state.programmers.selection(session.id) else {
        return;
    };
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    match state.highlight.action_guarded(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        action,
        &selection,
        &fixtures,
        &groups,
        programmer.blind || programmer.preview,
    ) {
        Ok(transition) => {
            let selection_changed = apply_highlight_selection_write(
                state,
                &session,
                transition.working_selection.as_ref(),
            )
            .unwrap_or(false);
            if selection_changed {
                emit(
                    state,
                    "programmer_changed",
                    serde_json::json!({"session_id":session.id,"source":"osc_highlight","action":action}),
                );
            }
            sync_highlight_output(state);
            emit(
                state,
                "highlight_changed",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "user_id":session.user.id,
                    "action":action,
                    "source":"osc",
                    "state":transition.state,
                }),
            );
        }
        Err(error) => emit(
            state,
            "highlight_rejected",
            serde_json::json!({
                "desk_id":session.desk.id,
                "user_id":session.user.id,
                "action":action,
                "source":"osc",
                "error":error.to_string(),
            }),
        ),
    }
}

fn handle_programmer_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() < 4 || parts[0] != "light" || parts[2] != "programmer" {
        return;
    }
    let pressed = osc_pressed(arguments);
    let source = source.and_then(|v| v.parse::<SocketAddr>().ok());
    let subscriber = state
        .osc_subscribers
        .lock()
        .values()
        .find(|s| Some(s.command_source) == source)
        .cloned();
    let Some(subscriber) = subscriber else {
        return;
    };
    let Some(session) = state.sessions.read().get(&subscriber.session_id).cloned() else {
        return;
    };
    let command_operation = state.command_http.operation_lock(session.desk.id);
    let _command_operation_guard = command_operation.lock();
    // The desk may have locked while this OSC action waited behind an HTTP/key operation. The
    // lock transition uses the same serializer, so this post-wait check closes that ordering race.
    if read_desk_lock(state, session.desk.id).locked {
        return;
    }
    let selection_revision_before = state
        .programmers
        .selection(session.id)
        .map(|selection| selection.revision);
    let action = parts[3];
    if action == "shift" {
        if let Some(source) = source
            && let Some(target) = state
                .osc_subscribers
                .lock()
                .values_mut()
                .find(|candidate| candidate.command_source == source)
        {
            if pressed {
                target.shifted = !target.shifted;
                target.shift_held = true;
            } else {
                target.shift_held = false;
                if target.update_first_release.is_some() {
                    target.shifted = false;
                }
            }
        }
        emit(
            state,
            "desk_action",
            serde_json::json!({
                "desk_alias":parts[1],
                "desk_id":session.desk.id,
                "session_id":session.id,
                "action":if pressed { "shift-down" } else { "shift-up" },
                "source":"osc"
            }),
        );
        return;
    }
    if action == "record" {
        #[derive(Clone, Copy)]
        enum Gesture {
            None,
            Arm,
            Targets,
            Settings,
        }
        let gesture = if let Some(source) = source {
            let mut subscribers = state.osc_subscribers.lock();
            let Some(target) = subscribers
                .values_mut()
                .find(|candidate| candidate.command_source == source)
            else {
                return;
            };
            if !target.shifted && !target.shift_held {
                Gesture::None
            } else if pressed && !target.shift_held {
                // Latched OSC Shift is often sent without a release. Attached controls holding
                // Shift use the release-duration path below for double/long discrimination.
                target.shifted = false;
                target.update_record_started = None;
                target.update_first_release = None;
                Gesture::Arm
            } else if pressed {
                target.update_record_started = Some(Instant::now());
                Gesture::None
            } else if let Some(started) = target.update_record_started.take() {
                let now = Instant::now();
                if now.saturating_duration_since(started) >= Duration::from_millis(650) {
                    target.update_first_release = None;
                    target.shifted = false;
                    Gesture::Settings
                } else if target.update_first_release.is_some_and(|first| {
                    now.saturating_duration_since(first) <= Duration::from_millis(600)
                }) {
                    target.update_first_release = None;
                    Gesture::Targets
                } else {
                    target.update_first_release = Some(now);
                    Gesture::Arm
                }
            } else {
                Gesture::None
            }
        } else {
            Gesture::None
        };
        match gesture {
            Gesture::Arm => {
                state
                    .programmers
                    .set_command_line(session.id, "UPDATE".into());
                let _ = persist_programmer(state, &session);
                emit(
                    state,
                    "update_armed",
                    serde_json::json!({"desk_id":session.desk.id,"session_id":session.id,"source":"osc"}),
                );
                emit(
                    state,
                    "programmer_changed",
                    serde_json::json!({"session_id":session.id}),
                );
            }
            Gesture::Targets => emit(
                state,
                "update_targets_requested",
                serde_json::json!({"desk_id":session.desk.id,"session_id":session.id,"source":"osc"}),
            ),
            Gesture::Settings => {
                state
                    .programmers
                    .set_command_line(session.id, String::new());
                let _ = persist_programmer(state, &session);
                emit(
                    state,
                    "update_settings_requested",
                    serde_json::json!({"desk_id":session.desk.id,"session_id":session.id,"source":"osc"}),
                );
                emit(
                    state,
                    "programmer_changed",
                    serde_json::json!({"session_id":session.id}),
                );
            }
            Gesture::None => {}
        }
        if !matches!(gesture, Gesture::None) || subscriber.shifted || subscriber.shift_held {
            return;
        }
    }
    if !pressed {
        return;
    }
    if subscriber.shifted
        && (action.starts_with("digit-") || matches!(action, "clear" | "delete" | "del"))
    {
        if let Some(source) = source
            && let Some(target) = state
                .osc_subscribers
                .lock()
                .values_mut()
                .find(|candidate| candidate.command_source == source)
        {
            target.shifted = false;
        }
        emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":parts[1],"session_id":session.id,"action":format!("shift-{}", action.strip_prefix("digit-").unwrap_or(action)),"source":"osc"}),
        );
        return;
    }
    if file_manager::route_osc_input(state, &session, action) {
        return;
    }
    match action {
        "set"
            if state.programmers.get(session.id).is_some_and(|programmer| {
                matches!(programmer.command_line.trim(), "" | "FIXTURE" | "GROUP")
            }) =>
        {
            emit(
                state,
                "desk_action",
                serde_json::json!({
                    "desk_alias":parts[1],
                    "session_id":session.id,
                    "action":"set",
                    "source":"osc"
                }),
            );
        }
        "enter" => {
            if let Some(programmer) = state.programmers.get(session.id) {
                let (retained_command, sensitive) =
                    command_audit_projection(&programmer.command_line);
                match command_http::execute_existing_command(
                    state,
                    &session,
                    &programmer.command_line,
                    "osc",
                    None,
                    command_http::ExistingCommandPolicy::Compatibility,
                ) {
                    command_http::ExistingCommandOutcome::Accepted { .. } => {
                        emit(
                            state,
                            "command_applied",
                            serde_json::json!({
                                "session_id":session.id,
                                "desk_id":session.desk.id,
                                "user_id":session.user.id,
                                "desk_alias":parts[1],
                                "command":retained_command,
                                "source":"osc"
                            }),
                        );
                        state
                            .programmers
                            .set_command_line(session.id, String::new());
                    }
                    command_http::ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
                        emit(
                            state,
                            "programmer_choice_requested",
                            serde_json::json!({
                                "session_id":session.id,
                                "desk_id":session.desk.id,
                                "user_id":session.user.id,
                                "pending_choice":pending_choice,
                                "source":"osc"
                            }),
                        );
                    }
                    command_http::ExistingCommandOutcome::Rejected { error } => {
                        let retained_error = if sensitive {
                            "Sensitive input omitted"
                        } else {
                            error.as_str()
                        };
                        emit(
                            state,
                            "programmer_command_rejected",
                            serde_json::json!({
                                "session_id":session.id,
                                "desk_id":session.desk.id,
                                "user_id":session.user.id,
                                "command":retained_command,
                                "error":retained_error,
                                "source":"osc"
                            }),
                        );
                    }
                }
            }
        }
        "clear" => {
            if let Some(p) = state.programmers.get(session.id) {
                if !p.command_line.is_empty() {
                    state
                        .programmers
                        .set_command_line(session.id, String::new());
                } else if !p.selected.is_empty() {
                    state.programmers.select(session.id, vec![]);
                } else {
                    state.programmers.clear_values(session.id);
                }
            }
        }
        "undo" => {
            state.programmers.undo(session.id);
        }
        "backspace" => {
            if let Some(p) = state.programmers.get(session.id) {
                state
                    .programmers
                    .set_command_line(session.id, remove_command_token(&p.command_line));
            }
        }
        "preload" => {
            if state
                .programmers
                .get(session.id)
                .is_some_and(|programmer| programmer.blind)
            {
                if let Err(error) = commit_preload(state, &session) {
                    emit(
                        state,
                        "desk_action",
                        serde_json::json!({"desk_alias":parts[1],"session_id":session.id,"action":"preload","source":"osc","error":error}),
                    );
                }
                return;
            }
            let capture_programmer = state.configuration.read().preload_programmer_changes;
            state
                .programmers
                .arm_preload(session.id, capture_programmer);
            reconcile_highlight_capture_mode(state, &session, "osc_preload");
        }
        "escape" | "menu" | "prog-playback" | "record" => emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":parts[1],"session_id":session.id,"action":action,"source":"osc"}),
        ),
        key => {
            if let Some(p) = state.programmers.get(session.id) {
                state.programmers.set_command_line(
                    session.id,
                    edit_osc_programmer_command(
                        &p.command_line,
                        key,
                        &state.programmers.command_target(session.id),
                    ),
                );
            }
        }
    }
    let _ = persist_programmer(state, &session);
    if state
        .programmers
        .selection(session.id)
        .map(|selection| selection.revision)
        != selection_revision_before
    {
        reconcile_highlight_selection(state, &session, "osc_programmer_selection");
    }
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id}),
    );
}

fn handle_timing_osc(state: &AppState, address: &str, arguments: &[OscArgument]) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let numeric = arguments.first().and_then(|v| match v {
        OscArgument::Float(v) => Some(*v),
        OscArgument::Int(v) => Some(*v as f32),
        _ => None,
    });
    if parts.len() == 4
        && parts[0] == "light"
        && parts[2] == "programmer"
        && matches!(parts[3], "prog-fade" | "cue-fade")
        && let Some(value) = numeric
    {
        let mut config = state.configuration.write();
        if parts[3] == "prog-fade" {
            config.programmer_fade_millis = (value.clamp(0.0, 1.0) * 20_000.0) as u64;
        } else {
            config.sequence_master_fade_millis = (value.clamp(0.0, 1.0) * 60_000.0) as u64;
        }
        drop(config);
        let _ = persist_server_configuration(state);
        refresh_speed_group_engine(state);
    }
    if parts.len() == 5
        && parts[0] == "light"
        && parts[2] == "speed-group"
        && parts[4] == "button"
        && let Ok(group) = parts[3].parse::<usize>()
        && group > 0
        && group <= 5
        && osc_pressed(arguments)
    {
        let index = group - 1;
        let mut controllers = state.speed_groups.lock();
        let now = application_millis(state);
        unlink_speed_group(&mut controllers, index, now);
        controllers[index].tap_learn(now);
        copy_speed_group_runtime_to_configuration(state, &controllers, &[index]);
        drop(controllers);
        state.sound_capture_owners.lock()[index] = None;
        let _ = persist_server_configuration(state);
        let snapshots = refresh_speed_group_engine(state);
        emit(
            state,
            "speed_group_action",
            serde_json::json!({"group":speed_group_name(index),"desk_alias":parts[1],"source":"osc","action":"learn","snapshot":snapshots[index]}),
        );
    }
    if parts.len() == 5
        && parts[0] == "light"
        && parts[2] == "speed-group"
        && parts[4] == "encoder"
        && let Ok(group) = parts[3].parse::<usize>()
        && let Some(value) = numeric
        && group > 0
        && group <= 5
    {
        let index = group - 1;
        let bpm = f64::from(value).clamp(0.1, 999.0);
        let now = application_millis(state);
        let mut controllers = state.speed_groups.lock();
        unlink_speed_group(&mut controllers, index, now);
        if controllers[index].set_manual_bpm(bpm).is_ok() {
            let _ = controllers[index].set_speed_master_scale(1.0);
            controllers[index].set_paused_at(false, now);
            copy_speed_group_runtime_to_configuration(state, &controllers, &[index]);
            drop(controllers);
            state.sound_capture_owners.lock()[index] = None;
            let _ = persist_server_configuration(state);
            refresh_speed_group_engine(state);
            emit(
                state,
                "speed_group_changed",
                serde_json::json!({"group":speed_group_name(index),"desk_alias":parts[1],"source":"osc","manual_bpm":bpm}),
            );
        } else {
            drop(controllers);
        }
    }
}

fn handle_encoder_osc(state: &AppState, address: &str, arguments: &[OscArgument]) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let value = arguments.first().and_then(|argument| match argument {
        OscArgument::String(value) => Some(value.as_str()),
        _ => None,
    });
    let valid =
        value.is_some_and(|value| matches!(value, "up" | "down" | "left" | "right" | "press"));
    if !valid || parts.first() != Some(&"light") {
        return;
    }
    let control = if parts.len() == 4
        && parts[2] == "encode"
        && parts[3]
            .parse::<u8>()
            .is_ok_and(|number| (1..=6).contains(&number))
    {
        format!("encode/{}", parts[3])
    } else if parts.len() == 3 && parts[2] == "nav" {
        "nav".into()
    } else {
        return;
    };
    emit(
        state,
        "desk_action",
        serde_json::json!({"desk_alias":parts[1],"control":control,"value":value,"source":"osc"}),
    );
}

fn send_osc(state: &AppState, target: SocketAddr, address: String, arguments: Vec<OscArgument>) {
    #[cfg(test)]
    state
        .osc_feedback_capture
        .lock()
        .push((target, address.clone(), arguments.clone()));
    if let (Some(socket), Ok(packet)) = (
        &state.osc_feedback,
        encode_osc_message(&address, &arguments),
    ) {
        let _ = socket.send_to(&packet, target);
    }
}

fn speed_group_osc_feedback(snapshot: SpeedSnapshot) -> Vec<OscArgument> {
    vec![
        OscArgument::Int(snapshot.effective_bpm.round().clamp(0.0, 999.0) as i32),
        OscArgument::Float(0.0),
        OscArgument::Float(0.75),
        OscArgument::Float(0.95),
        OscArgument::String(
            if snapshot.phase_advancing {
                "on"
            } else {
                "off"
            }
            .into(),
        ),
    ]
}

fn playback_color_rgb(color: &str, active: bool) -> (f32, f32, f32) {
    let component = |range: std::ops::Range<usize>| {
        u8::from_str_radix(color.get(range).unwrap_or_default(), 16).unwrap_or(0x20) as f32 / 255.0
    };
    let scale = if active { 1.0 } else { 0.35 };
    (
        component(1..3) * scale,
        component(3..5) * scale,
        component(5..7) * scale,
    )
}

fn osc_control_desk(state: &AppState, alias: &str) -> Option<ControlDesk> {
    let store = state.desk.lock();
    if alias.eq_ignore_ascii_case("main") || alias.is_empty() {
        store.desks().ok()?.into_iter().next()
    } else {
        store.control_desk_by_alias(alias).ok().flatten()
    }
}

fn send_osc_feedback(state: &AppState, _full: bool) {
    let now = Instant::now();
    let before = state.osc_subscribers.lock().len();
    state
        .osc_subscribers
        .lock()
        .retain(|_, s| now.duration_since(s.last_seen) < Duration::from_secs(20));
    let after = state.osc_subscribers.lock().len();
    if before != after {
        emit(
            state,
            "hardware_connection_changed",
            serde_json::json!({"connected":after>0}),
        );
    }
    let subscribers = state
        .osc_subscribers
        .lock()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let Some(show) = state.active_show.read().clone() else {
        return;
    };
    let snapshot = state.engine.snapshot();
    let runtime = state.engine.playback().read().runtime_status();
    let speed_groups: [SpeedSnapshot; 5] = {
        let now = application_millis(state);
        let controllers = state.speed_groups.lock();
        std::array::from_fn(|index| controllers[index].snapshot(now))
    };
    let highlight_fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let highlight_groups = highlight_groups(&snapshot);
    for subscriber in subscribers {
        let Ok(Some(desk)) = state
            .desk
            .lock()
            .control_desk_by_alias(&subscriber.desk_alias)
        else {
            continue;
        };
        send_osc(
            state,
            subscriber.target,
            format!("/light/{}/feedback/locked", subscriber.desk_alias),
            vec![OscArgument::Bool(read_desk_lock(state, desk.id).locked)],
        );
        let page = state.desk.lock().desk_page(desk.id, show.id).unwrap_or(1);
        let selected_playback = state
            .desk
            .lock()
            .selected_playback(desk.id, show.id)
            .ok()
            .flatten();
        send_osc(
            state,
            subscriber.target,
            format!("/light/{}/feedback/page", subscriber.desk_alias),
            vec![OscArgument::Int(i32::from(page))],
        );
        let programmer = state.programmers.get(subscriber.session_id);
        let command_line = programmer
            .as_ref()
            .map(|programmer| programmer.command_line.clone())
            .unwrap_or_default();
        send_osc(
            state,
            subscriber.target,
            format!("/light/{}/feedback/command-line", subscriber.desk_alias),
            vec![OscArgument::String(command_line.clone())],
        );
        send_osc(
            state,
            subscriber.target,
            format!("/light/{}/feedback/update/armed", subscriber.desk_alias),
            vec![OscArgument::Bool(command_line_arms_update(&command_line))],
        );
        for key in [
            "group", "at", "thru", "plus", "minus", "time", "delay", "cue", "record", "clear",
            "enter", "preload",
        ] {
            let token = match key {
                "group" => "GROUP".to_owned(),
                "thru" => "THRU".to_owned(),
                "plus" => "+".to_owned(),
                "minus" => "-".to_owned(),
                "record" => "RECORD".to_owned(),
                other => other.to_ascii_uppercase(),
            };
            send_osc(
                state,
                subscriber.target,
                format!("/light/{}/feedback/programmer/{key}", subscriber.desk_alias),
                vec![OscArgument::Bool(
                    command_line.split_whitespace().any(|part| part == token),
                )],
            );
        }
        if let Some(session) = state.sessions.read().get(&subscriber.session_id).cloned() {
            let capture_only = programmer
                .as_ref()
                .is_some_and(|programmer| programmer.blind || programmer.preview);
            let Some(selection) = state.programmers.selection(subscriber.session_id) else {
                continue;
            };
            let highlight = state.highlight.status(
                desk.id,
                session.user.id,
                Some(&session.user.name),
                &selection,
                &highlight_fixtures,
                &highlight_groups,
                capture_only,
            );
            let prefix = format!("/light/{}/feedback/highlight", subscriber.desk_alias);
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/active"),
                vec![OscArgument::Bool(highlight.state.active)],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/output"),
                vec![OscArgument::Bool(highlight.state.output_enabled)],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/mode"),
                vec![OscArgument::String(
                    match highlight.state.mode {
                        HighlightMode::Selection => "selection",
                        HighlightMode::Step => "step",
                    }
                    .into(),
                )],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/index"),
                vec![OscArgument::Int(
                    highlight
                        .state
                        .active_index
                        .map(|index| index.saturating_add(1) as i32)
                        .unwrap_or(0),
                )],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/total"),
                vec![OscArgument::Int(
                    highlight.state.remembered.len().min(i32::MAX as usize) as i32,
                )],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/can-next"),
                vec![OscArgument::Bool(highlight.state.can_next)],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/can-previous"),
                vec![OscArgument::Bool(highlight.state.can_previous)],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/fixture/id"),
                vec![OscArgument::String(
                    highlight
                        .state
                        .active_fixture
                        .as_ref()
                        .map(|fixture| fixture.fixture_id.0.to_string())
                        .unwrap_or_default(),
                )],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/fixture/number"),
                vec![OscArgument::Int(
                    highlight
                        .state
                        .active_fixture
                        .as_ref()
                        .and_then(|fixture| fixture.number)
                        .and_then(|number| i32::try_from(number).ok())
                        .unwrap_or(0),
                )],
            );
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/fixture/name"),
                vec![OscArgument::String(
                    highlight
                        .state
                        .active_fixture
                        .as_ref()
                        .and_then(|fixture| fixture.name.clone())
                        .unwrap_or_default(),
                )],
            );
        }
        let page_definition = snapshot.playback_pages.iter().find(|p| p.number == page);
        let playback_slots = desk.columns.saturating_mul(desk.rows).clamp(1, 96);
        for slot in 1u8..=playback_slots {
            let number = page_definition.and_then(|p| p.slots.get(&slot)).copied();
            let definition = number.and_then(|number| {
                snapshot
                    .playbacks
                    .iter()
                    .find(|definition| definition.number == number)
            });
            let running = number.and_then(|n| {
                runtime
                    .iter()
                    .find(|a| a.playback.playback_number == Some(n))
            });
            let level = definition
                .map(|definition| match &definition.target {
                    light_playback::PlaybackTarget::CueList { .. } => running
                        .map(|status| status.playback.fader_position)
                        .unwrap_or(0.0),
                    light_playback::PlaybackTarget::Group { group_id } => snapshot
                        .groups
                        .iter()
                        .find(|group| group.id == *group_id)
                        .map(|group| group.master)
                        .unwrap_or(0.0),
                    light_playback::PlaybackTarget::SpeedGroup { group } => {
                        let index = speed_group_index(group).unwrap_or(0);
                        match definition.fader {
                            light_playback::PlaybackFaderMode::DirectBpm => {
                                (speed_groups[index].effective_bpm / 300.0) as f32
                            }
                            light_playback::PlaybackFaderMode::CenteredRelative => {
                                ((speed_groups[index].speed_master_scale.log(4.0) / 2.0) + 0.5)
                                    as f32
                            }
                            _ => speed_groups[index].speed_master_scale as f32,
                        }
                    }
                    light_playback::PlaybackTarget::ProgrammerFade => {
                        state.configuration.read().programmer_fade_millis as f32 / 20_000.0
                    }
                    light_playback::PlaybackTarget::CueFade => {
                        state.configuration.read().sequence_master_fade_millis as f32 / 60_000.0
                    }
                    light_playback::PlaybackTarget::GrandMaster => {
                        state.output_control.lock().options.grand_master
                    }
                })
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            let name = "page-playback";
            send_osc(
                state,
                subscriber.target,
                format!(
                    "/light/{}/feedback/{name}/{slot}/fader",
                    subscriber.desk_alias
                ),
                vec![OscArgument::Float(level)],
            );
            {
                let prefix = format!("/light/{}/feedback/{name}/{slot}", subscriber.desk_alias);
                send_osc(
                    state,
                    subscriber.target,
                    format!("{prefix}/selected"),
                    vec![OscArgument::Bool(number == selected_playback)],
                );
                send_osc(
                    state,
                    subscriber.target,
                    format!("{prefix}/current-cue"),
                    vec![OscArgument::Float(
                        running
                            .and_then(|item| item.playback.current_cue_number)
                            .unwrap_or(-1.0) as f32,
                    )],
                );
                send_osc(
                    state,
                    subscriber.target,
                    format!("{prefix}/normal-next-cue"),
                    vec![OscArgument::Float(
                        running
                            .and_then(|item| item.normal_next_cue_number)
                            .unwrap_or(-1.0) as f32,
                    )],
                );
                send_osc(
                    state,
                    subscriber.target,
                    format!("{prefix}/effective-next-cue"),
                    vec![OscArgument::Float(
                        running
                            .and_then(|item| item.effective_next_cue_number)
                            .unwrap_or(-1.0) as f32,
                    )],
                );
                send_osc(
                    state,
                    subscriber.target,
                    format!("{prefix}/loaded-next"),
                    vec![OscArgument::Bool(
                        running.is_some_and(|item| item.effective_next_is_loaded),
                    )],
                );
            }
            for button in 1..=desk.buttons {
                let active = running.is_some_and(|item| {
                    item.playback.enabled || item.temporary_active || item.swap_active
                });
                let (r, g, b) = definition
                    .map(|definition| playback_color_rgb(&definition.color, active))
                    .unwrap_or((0.18, 0.20, 0.23));
                let state_name = if active { "on" } else { "off" };
                send_osc(
                    state,
                    subscriber.target,
                    format!(
                        "/light/{}/feedback/{name}/{slot}/button/{button}",
                        subscriber.desk_alias
                    ),
                    vec![
                        OscArgument::Float(r),
                        OscArgument::Float(g),
                        OscArgument::Float(b),
                        OscArgument::String(state_name.into()),
                    ],
                );
                if let Some(action) = definition
                    .and_then(|definition| definition.buttons.get(usize::from(button - 1)))
                    .and_then(|action| serde_json::to_value(action).ok())
                    .and_then(|action| action.as_str().map(str::to_owned))
                {
                    send_osc(
                        state,
                        subscriber.target,
                        format!(
                            "/light/{}/feedback/{name}/{slot}/button/{button}/action",
                            subscriber.desk_alias
                        ),
                        vec![OscArgument::String(action)],
                    );
                }
            }
            if let (Some(definition), Some(running)) = (definition, running) {
                let fader_mode = serde_json::to_value(definition.fader)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default();
                {
                    let prefix = format!("/light/{}/feedback/{name}/{slot}", subscriber.desk_alias);
                    send_osc(
                        state,
                        subscriber.target,
                        format!("{prefix}/fader-mode"),
                        vec![OscArgument::String(fader_mode.clone())],
                    );
                    send_osc(
                        state,
                        subscriber.target,
                        format!("{prefix}/fader-pickup"),
                        vec![OscArgument::Bool(running.playback.fader_pickup_required)],
                    );
                    send_osc(
                        state,
                        subscriber.target,
                        format!("{prefix}/temporary"),
                        vec![OscArgument::Bool(running.temporary_active)],
                    );
                    send_osc(
                        state,
                        subscriber.target,
                        format!("{prefix}/xfade-direction"),
                        vec![OscArgument::String(
                            serde_json::to_value(running.playback.manual_xfade_direction)
                                .ok()
                                .and_then(|value| value.as_str().map(str::to_owned))
                                .unwrap_or_default(),
                        )],
                    );
                }
            }
        }
        for (index, speed_group) in speed_groups.iter().copied().enumerate() {
            send_osc(
                state,
                subscriber.target,
                format!(
                    "/light/{}/feedback/speed-group/{}",
                    subscriber.desk_alias,
                    index + 1
                ),
                speed_group_osc_feedback(speed_group),
            );
        }
    }
    sync_highlight_output(state);
}

fn cuelist_for_page_playback(snapshot: &EngineSnapshot, page_number: u8, slot: u8) -> Option<u16> {
    let number = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == page_number)?
        .slots
        .get(&slot)
        .copied()?;
    snapshot
        .playbacks
        .iter()
        .any(|definition| definition.number == number)
        .then_some(number)
}

fn update_target_for_playback(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
) -> Result<UpdateApiTarget, String> {
    match &definition.target {
        light_playback::PlaybackTarget::CueList { cue_list_id } => {
            let context = active_update_cue_contexts(state)
                .into_iter()
                .find(|context| context.playback_number == definition.number);
            Ok(UpdateApiTarget {
                family: UpdateApiTargetFamily::Cue,
                object_id: Some(cue_list_id.0.to_string()),
                playback_number: Some(definition.number),
                cue_id: context.as_ref().map(|context| context.cue_id),
                cue_number: context.map(|context| context.cue_number),
                validate_active_context: true,
            })
        }
        light_playback::PlaybackTarget::Group { group_id } => Ok(UpdateApiTarget {
            family: UpdateApiTargetFamily::Group,
            object_id: Some(group_id.clone()),
            playback_number: Some(definition.number),
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        }),
        _ => Err(format!(
            "Playback {} is not assigned to a recordable Update target",
            definition.number
        )),
    }
}

fn intercept_update_playback_target(
    state: &AppState,
    session: &Session,
    definition: &light_playback::PlaybackDefinition,
    touched: bool,
) -> bool {
    if !touched
        || !state
            .programmers
            .get(session.id)
            .is_some_and(|programmer| command_line_arms_update(&programmer.command_line))
    {
        return false;
    }
    let target = match update_target_for_playback(state, definition) {
        Ok(target) => target,
        Err(error) => {
            emit(
                state,
                "update_target_rejected",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "session_id":session.id,
                    "playback_number":definition.number,
                    "source":"osc",
                    "error":error,
                }),
            );
            return true;
        }
    };
    state
        .programmers
        .set_command_line(session.id, String::new());
    let _ = persist_programmer(state, session);
    emit(
        state,
        "update_target_requested",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "source":"osc",
            "target":target,
        }),
    );
    emit_update_armed_transition(state, session, true, false, "osc_target");
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"desk_id":session.desk.id,"source":"osc_target"}),
    );
    true
}

fn handle_playback_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let pressed = arguments
        .first()
        .map(|argument| match argument {
            OscArgument::Bool(value) => *value,
            OscArgument::Int(value) => *value != 0,
            OscArgument::Float(value) => *value > 0.0,
            OscArgument::String(value) => value != "0" && value != "false",
        })
        .unwrap_or(true);
    let value = arguments.first().and_then(|argument| match argument {
        OscArgument::Float(value) => Some(*value),
        OscArgument::Int(value) => Some(*value as f32 / 127.0),
        _ => None,
    });
    if parts.len() == 3 && parts.first() == Some(&"light") && parts.get(2) == Some(&"page") {
        let Some(page) = arguments.first().and_then(|argument| match argument {
            OscArgument::Int(value) => u8::try_from(*value).ok(),
            OscArgument::Float(value) if value.is_finite() => Some(*value as u8),
            _ => None,
        }) else {
            return;
        };
        let Some(show) = state.active_show.read().clone() else {
            return;
        };
        let desk = osc_control_desk(state, parts[1]);
        if let Some(desk) = desk {
            if !ensure_playback_page_for_advance(state, &show, page).unwrap_or(false) {
                return;
            }
            let _ = state.desk.lock().set_desk_page(desk.id, show.id, page);
            emit(
                state,
                "playback_page_changed",
                serde_json::json!({"desk_id":desk.id,"page":page}),
            );
        }
        return;
    }
    let (number, action_index) =
        if parts.len() >= 5 && parts.first() == Some(&"light") && parts.get(1) == Some(&"playback")
        {
            let (Ok(page), Ok(slot)) = (parts[2].parse::<u8>(), parts[3].parse::<u8>()) else {
                return;
            };
            let snapshot = state.engine.snapshot();
            let Some(number) = cuelist_for_page_playback(&snapshot, page, slot) else {
                return;
            };
            (number, 4)
        } else if parts.len() >= 4
            && parts.first() == Some(&"light")
            && parts
                .get(1)
                .is_some_and(|name| *name == "cuelist" || *name == "qlist" || *name == "playback")
        {
            let Ok(number) = parts[2].parse::<u16>() else {
                return;
            };
            (number, 3)
        } else if parts.len() >= 5
            && parts.first() == Some(&"light")
            && parts
                .get(2)
                .is_some_and(|name| *name == "page-playback" || *name == "paged-playback")
        {
            let Ok(slot) = parts[3].parse::<u8>() else {
                return;
            };
            let Some(show) = state.active_show.read().clone() else {
                return;
            };
            let Some(desk) = osc_control_desk(state, parts[1]) else {
                return;
            };
            let page_number = state.desk.lock().desk_page(desk.id, show.id).unwrap_or(1);
            let snapshot = state.engine.snapshot();
            let Some(number) = cuelist_for_page_playback(&snapshot, page_number, slot) else {
                return;
            };
            (number, 4)
        } else {
            return;
        };
    let Some(definition) = state
        .engine
        .snapshot()
        .playbacks
        .iter()
        .find(|definition| definition.number == number)
        .cloned()
    else {
        return;
    };
    let button = (parts[action_index] == "button")
        .then(|| parts.get(action_index + 1)?.parse::<u8>().ok())
        .flatten();
    let input = PoolPlaybackInput {
        value: value.map(|value| value.clamp(0.0, 1.0)),
        pressed: Some(pressed),
        button,
        surface: Some("osc".into()),
        ..PoolPlaybackInput::default()
    };
    let source_address = source.and_then(|source| source.parse::<SocketAddr>().ok());
    let subscriber = state
        .osc_subscribers
        .lock()
        .values()
        .find(|subscriber| Some(subscriber.command_source) == source_address)
        .cloned();
    let action_alias = if parts
        .get(2)
        .is_some_and(|part| *part == "page-playback" || *part == "paged-playback")
    {
        parts[1]
    } else {
        "main"
    };
    let action_desk = osc_control_desk(state, action_alias);
    let session = subscriber
        .and_then(|subscriber| state.sessions.read().get(&subscriber.session_id).cloned())
        .or_else(|| {
            let desk = action_desk.as_ref()?;
            state
                .sessions
                .read()
                .values()
                .find(|session| session.connected && session.desk.id == desk.id)
                .cloned()
        });
    let action = if parts[action_index] == "fader" {
        "master"
    } else {
        parts[action_index]
    };
    let target_touched = if action == "master" {
        value.is_some()
    } else {
        pressed
    };
    if session.as_ref().is_some_and(|session| {
        intercept_update_playback_target(state, session, &definition, target_touched)
    }) {
        return;
    }
    if dispatch_playback_action(
        state,
        session.as_ref(),
        action_desk.as_ref(),
        &definition,
        action,
        &input,
        "osc",
    )
    .is_ok_and(|changed| changed)
    {
        emit(
            state,
            "playback_changed",
            serde_json::json!({"playback_number":number,"action":action,"source":"osc","session_id":session.map(|session|session.id)}),
        );
    }
}

fn ingest_timecode(state: &AppState, timecode: SmpteTimecode) {
    let current = state.timecode_router.lock().ingest(timecode).cloned();
    if let Some(timecode) = current {
        let fps = u64::from(timecode.rate.nominal_frames());
        let seconds = u64::from(timecode.hours) * 3600
            + u64::from(timecode.minutes) * 60
            + u64::from(timecode.seconds);
        state
            .engine
            .set_timecode_frame(Some(seconds * fps + u64::from(timecode.frames)));
    }
}
fn load_engine_snapshot(entry: &ShowEntry) -> Result<EngineSnapshot, String> {
    if entry.name == default_show::name() {
        default_show::upgrade(&entry.path).map_err(|error| error.to_string())?;
    }
    reconcile_show_schema_defaults(entry)?;
    reconcile_show_logical_heads(entry)?;
    reconcile_show_cue_identities(entry)?;
    load_engine_snapshot_with_override(entry, None)
}

fn reconcile_show_schema_defaults(entry: &ShowEntry) -> Result<(), String> {
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    let fixture_objects = store
        .objects("patched_fixture")
        .map_err(|error| error.to_string())?;
    let all_fixture_numbers_missing = !fixture_objects.is_empty()
        && fixture_objects.iter().all(|object| {
            object
                .body
                .get("fixture_number")
                .and_then(serde_json::Value::as_u64)
                .is_none()
                && object
                    .body
                    .get("virtual_fixture_number")
                    .and_then(serde_json::Value::as_u64)
                    .is_none()
        });
    let mut inferred_fixture_numbers = HashMap::new();
    if all_fixture_numbers_missing {
        let mut candidates = fixture_objects
            .iter()
            .map(|object| {
                (
                    object.id.clone(),
                    object
                        .body
                        .get("universe")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(u64::MAX),
                    object
                        .body
                        .get("address")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(u64::MAX),
                    object
                        .body
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                )
            })
            .collect::<Vec<_>>();
        candidates
            .sort_by(|left, right| (left.1, left.2, &left.0).cmp(&(right.1, right.2, &right.0)));
        let mut used = std::collections::BTreeSet::new();
        for (id, _, _, name) in &candidates {
            if let Some(number) = default_show::default_fixture_number(name)
                && used.insert(number)
            {
                inferred_fixture_numbers.insert(id.clone(), number);
            }
        }
        let mut next = 1_u32;
        for (id, _, _, _) in &candidates {
            if inferred_fixture_numbers.contains_key(id) {
                continue;
            }
            while used.contains(&next) {
                next += 1;
            }
            inferred_fixture_numbers.insert(id.clone(), next);
            used.insert(next);
            next += 1;
        }
    }

    let mut used_virtual_fixture_numbers = fixture_objects
        .iter()
        .filter_map(|object| {
            object
                .body
                .get("virtual_fixture_number")
                .and_then(serde_json::Value::as_u64)
                .and_then(|number| u32::try_from(number).ok())
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut next_virtual_fixture_number = 1_u32;
    let mut updates = Vec::<(String, String, serde_json::Value, u64)>::new();
    for object in fixture_objects {
        let original = object.body;
        let mut fixture = serde_json::from_value::<light_fixture::PatchedFixture>(original.clone())
            .map_err(|error| format!("invalid patched fixture: {error}"))?;
        if all_fixture_numbers_missing {
            fixture.fixture_number = inferred_fixture_numbers.get(&object.id).copied();
        }
        if !fixture.definition.is_dmx_patchable() {
            fixture.fixture_number = None;
            if fixture.virtual_fixture_number.is_none() {
                while used_virtual_fixture_numbers.contains(&next_virtual_fixture_number) {
                    next_virtual_fixture_number += 1;
                }
                fixture.virtual_fixture_number = Some(next_virtual_fixture_number);
                used_virtual_fixture_numbers.insert(next_virtual_fixture_number);
                next_virtual_fixture_number += 1;
            }
        }
        light_fixture::migrate_patched_fixture_to_v2(&mut fixture)
            .map_err(|error| format!("fixture schema-v1-to-v2 migration failed: {error}"))?;
        let normalized = serde_json::to_value(fixture).map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store.objects("group").map_err(|error| error.to_string())? {
        let original = object.body;
        let mut group =
            serde_json::from_value::<light_programmer::GroupDefinition>(original.clone())
                .map_err(|error| format!("invalid group: {error}"))?;
        group.id.clone_from(&object.id);
        let normalized = serde_json::to_value(group).map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store.objects("preset").map_err(|error| error.to_string())? {
        let original = object.body;
        let mut preset = serde_json::from_value::<light_programmer::Preset>(original.clone())
            .map_err(|error| format!("invalid preset: {error}"))?;
        preset.reconcile_address(&object.id)?;
        let normalized = serialize_preset_preserving_extensions(&original, &preset)
            .map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store
        .objects("cue_list")
        .map_err(|error| error.to_string())?
    {
        let original = object.body;
        let mut cue_list = serde_json::from_value::<light_playback::CueList>(original.clone())
            .map_err(|error| format!("invalid cue list: {error}"))?;
        cue_list.migrate_legacy_chaser_xfade(&default_speed_groups());
        let normalized = serde_json::to_value(cue_list).map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store
        .objects("playback")
        .map_err(|error| error.to_string())?
    {
        let original = object.body;
        let playback =
            serde_json::from_value::<light_playback::PlaybackDefinition>(original.clone())
                .map_err(|error| format!("invalid playback: {error}"))?;
        let normalized = serde_json::to_value(playback).map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store.objects("route").map_err(|error| error.to_string())? {
        let original = object.body;
        let destination_was_missing = original.get("destination").is_none();
        let delivery_mode_was_missing = original.get("delivery_mode").is_none();
        let mut route = serde_json::from_value::<light_output::OutputRoute>(original.clone())
            .map_err(|error| format!("invalid output route: {error}"))?;
        if destination_was_missing {
            route.destination = None;
        }
        if delivery_mode_was_missing {
            route.delivery_mode = Some(route.resolved_delivery_mode());
        }
        route
            .validate()
            .map_err(|error| format!("invalid output route: {error}"))?;
        let mut normalized = serde_json::to_value(&route).map_err(|error| error.to_string())?;
        // Preserve the supported historical default as explicit current-schema data. `None`
        // selects the protocol's standard destination, but an omitted legacy field must migrate
        // once to `null` rather than remaining indistinguishable from an unnormalised object.
        if destination_was_missing && let Some(body) = normalized.as_object_mut() {
            body.insert("destination".into(), serde_json::Value::Null);
        }
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    if updates.is_empty() {
        return Ok(());
    }
    let migration_probe = std::path::Path::new(&entry.path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(format!(".migration-probe-{}.show", Uuid::new_v4()));
    store
        .backup_to(&migration_probe)
        .map_err(|error| error.to_string())?;
    let probe_result = (|| {
        let probe_store = ShowStore::open(&migration_probe).map_err(|error| error.to_string())?;
        for (kind, id, body, revision) in &updates {
            probe_store
                .put_object(kind, id, body, *revision)
                .map_err(|error| error.to_string())?;
        }
        drop(probe_store);
        let probe = ShowEntry {
            path: migration_probe.display().to_string(),
            ..entry.clone()
        };
        load_engine_snapshot_with_override(&probe, None)?
            .validate()
            .map_err(|error| error.to_string())
    })();
    let _ = std::fs::remove_file(&migration_probe);
    let _ = std::fs::remove_file(format!("{}-wal", migration_probe.display()));
    let _ = std::fs::remove_file(format!("{}-shm", migration_probe.display()));
    probe_result?;
    for (kind, id, body, revision) in updates {
        store
            .put_object(&kind, &id, &body, revision)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn reconcile_show_logical_heads(entry: &ShowEntry) -> Result<(), String> {
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    for object in store
        .objects("patched_fixture")
        .map_err(|error| error.to_string())?
    {
        let mut fixture = serde_json::from_value::<light_fixture::PatchedFixture>(object.body)
            .map_err(|error| error.to_string())?;
        if light_fixture::reconcile_logical_heads(&mut fixture) {
            store
                .put_object(
                    "patched_fixture",
                    &object.id,
                    &serde_json::to_value(fixture).map_err(|error| error.to_string())?,
                    object.revision,
                )
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
fn reconcile_show_cue_identities(entry: &ShowEntry) -> Result<(), String> {
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    for object in store
        .objects("cue_list")
        .map_err(|error| error.to_string())?
    {
        let missing_identity = object
            .body
            .get("cues")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|cues| {
                cues.iter()
                    .any(|cue| cue.get("id").and_then(serde_json::Value::as_str).is_none())
            });
        if !missing_identity {
            continue;
        }
        let cue_list = serde_json::from_value::<light_playback::CueList>(object.body)
            .map_err(|error| error.to_string())?;
        store
            .put_object(
                "cue_list",
                &object.id,
                &serde_json::to_value(cue_list).map_err(|error| error.to_string())?,
                object.revision,
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
fn compile_active_show_for_startup(engine: &Engine, entry: &ShowEntry) -> Option<String> {
    load_engine_snapshot(entry)
        .and_then(|snapshot| engine.replace_snapshot(snapshot).map_err(|error| error.to_string()))
        .err()
        .map(|error| format!("The active show '{}' could not be loaded and might be corrupted or incompatible: {error}", entry.name))
}
fn load_engine_snapshot_with_override(
    entry: &ShowEntry,
    override_value: Option<(&str, &str, &serde_json::Value)>,
) -> Result<EngineSnapshot, String> {
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    let mut revision = entry.revision;
    let mut read_kind = |kind: &str| -> Result<Vec<light_show::VersionedObject>, String> {
        let mut objects = store.objects(kind).map_err(|error| error.to_string())?;
        if let Some((override_kind, override_id, body)) = override_value
            && override_kind == kind
        {
            if let Some(object) = objects.iter_mut().find(|object| object.id == override_id) {
                object.body = body.clone();
            } else {
                objects.push(light_show::VersionedObject {
                    kind: kind.into(),
                    id: override_id.into(),
                    body: body.clone(),
                    revision: 0,
                    updated_at: String::new(),
                });
            }
        }
        revision = revision.max(
            objects
                .iter()
                .map(|object| object.revision)
                .max()
                .unwrap_or(0),
        );
        Ok(objects)
    };
    let fixtures = read_kind("patched_fixture")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_fixture::PatchedFixture>, _>>()
        .map_err(|error| format!("invalid patched fixture: {error}"))?;
    let cue_lists = read_kind("cue_list")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_playback::CueList>, _>>()
        .map_err(|error| format!("invalid cue list: {error}"))?;
    let mut playbacks = read_kind("playback")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_playback::PlaybackDefinition>, _>>()
        .map_err(|error| format!("invalid playback: {error}"))?;
    let mut playback_pages = read_kind("playback_page")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_playback::PlaybackPage>, _>>()
        .map_err(|error| format!("invalid playback page: {error}"))?;
    let routes = read_kind("route")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_output::OutputRoute>, _>>()
        .map_err(|error| format!("invalid output route: {error}"))?;
    let control_mappings = read_kind("control_mapping")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_control::ControlMapping>, _>>()
        .map_err(|error| format!("invalid control mapping: {error}"))?;
    let groups = read_kind("group")?
        .into_iter()
        .map(|object| {
            serde_json::from_value::<light_programmer::GroupDefinition>(object.body).map(
                |mut group| {
                    group.id = object.id;
                    group
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid group: {error}"))?;
    if playbacks.is_empty() {
        playbacks = cue_lists
            .iter()
            .take(1_000)
            .enumerate()
            .map(|(index, cue)| light_playback::PlaybackDefinition {
                number: index as u16 + 1,
                name: cue.name.clone(),
                target: light_playback::PlaybackTarget::CueList {
                    cue_list_id: cue.id,
                },
                buttons: [
                    light_playback::PlaybackButtonAction::GoMinus,
                    light_playback::PlaybackButtonAction::Go,
                    light_playback::PlaybackButtonAction::Flash,
                ],
                button_count: 3,
                fader: light_playback::PlaybackFaderMode::Master,
                has_fader: true,
                go_activates: true,
                auto_off: true,
                xfade_millis: 0,
                color: "#20c997".into(),
                flash_release: light_playback::FlashReleaseMode::default(),
                protect_from_swap: false,
                presentation_icon: None,
                presentation_image: None,
            })
            .collect();
    }
    if playback_pages.is_empty() {
        playback_pages.push(light_playback::PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::new(),
        });
    }
    Ok(EngineSnapshot {
        fixtures,
        cue_lists,
        playbacks,
        playback_pages,
        routes,
        control_mappings,
        groups,
        revision,
    })
}
fn persist_programmer(state: &AppState, session: &Session) -> Result<(), ApiError> {
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    state
        .desk
        .lock()
        .save_session(&PersistedSession {
            id: session.id,
            user_id: session.user.id,
            token: session.token.clone(),
            programmer_json: serde_json::to_string(&programmer)
                .map_err(|error| ApiError::internal(error.to_string()))?,
            connected: programmer.connected,
            updated_at: chrono::Utc::now().to_rfc3339(),
        })
        .map_err(ApiError::store)
}

fn active_playbacks_setting(show_id: light_core::ShowId) -> String {
    format!("active_playbacks:{}", show_id.0)
}

fn output_runtime_setting(show_id: light_core::ShowId) -> String {
    format!("output_runtime:{}", show_id.0)
}

fn persist_output_runtime(state: &AppState) -> Result<(), ApiError> {
    let Some(show) = state.active_show.read().clone() else {
        return Ok(());
    };
    let (grand_master, blackout) = {
        let control = state.output_control.lock();
        (control.options.grand_master, control.options.blackout)
    };
    let runtime = PersistedOutputRuntime {
        grand_master,
        blackout,
        dynamics_paused_at: state.engine.playback().read().dynamics_paused_since(),
        group_masters: state
            .engine
            .snapshot()
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.master))
            .collect(),
    };
    let serialized =
        serde_json::to_string(&runtime).map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .desk
        .lock()
        .set_setting(&output_runtime_setting(show.id), &serialized)
        .map_err(ApiError::store)
}

fn persist_active_playbacks(state: &AppState) -> Result<(), ApiError> {
    let Some(show) = state.active_show.read().clone() else {
        return Ok(());
    };
    let runtime = state.engine.playback().read().runtime();
    let serialized =
        serde_json::to_string(&runtime).map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .desk
        .lock()
        .set_setting(&active_playbacks_setting(show.id), &serialized)
        .map_err(ApiError::store)
}
fn emit(state: &AppState, kind: &str, payload: serde_json::Value) {
    let event = Event {
        revision: state.event_revision.fetch_add(1, Ordering::Relaxed) + 1,
        kind: kind.into(),
        payload,
    };
    {
        let mut audit = state.audit_events.lock();
        if audit.len() == 2048 {
            audit.pop_front();
        }
        audit.push_back(event.clone());
    }
    let _ = state.events.send(event);
}

fn record_command_history(
    state: &AppState,
    session: &Session,
    command: &str,
    status: &str,
    feedback: &str,
    source: &str,
    request_id: Option<&str>,
) {
    let (retained_command, sensitive) = command_audit_projection(command);
    if retained_command.is_empty() {
        return;
    }
    let retained_feedback = if sensitive {
        "Sensitive input omitted".into()
    } else {
        feedback.chars().take(1_000).collect::<String>()
    };
    let entry = CommandHistoryEntry {
        id: Uuid::new_v4().to_string(),
        desk_id: session.desk.id,
        session_id: session.id,
        command: retained_command,
        status: status.into(),
        feedback: retained_feedback,
        source: source.into(),
        request_id: request_id.map(str::to_owned),
        at: chrono::Utc::now().to_rfc3339(),
    };
    {
        let mut histories = state.command_history.lock();
        let history = histories.entry(session.desk.id).or_default();
        history.push_front(entry.clone());
        history.truncate(COMMAND_HISTORY_LIMIT);
    }
    emit(
        state,
        "command_history",
        serde_json::to_value(entry).expect("command history entries serialize"),
    );
}

fn command_audit_projection(command: &str) -> (String, bool) {
    let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = normalized.to_ascii_uppercase();
    let sensitive = [
        "PASSWORD",
        "PASSCODE",
        "TOKEN",
        "SECRET",
        "AUTHORIZATION",
        "API_KEY",
    ]
    .iter()
    .any(|term| upper.split_whitespace().any(|token| token.contains(term)));
    if sensitive {
        ("[REDACTED SENSITIVE COMMAND]".into(), true)
    } else {
        (normalized.chars().take(512).collect(), false)
    }
}
fn validate_show_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 100 || name.contains(['/', '\\']) {
        Err(ApiError::bad_request(
            "show name must be a plain name up to 100 characters",
        ))
    } else {
        Ok(())
    }
}

fn available_show_name(state: &AppState, stem: &str) -> Result<String, ApiError> {
    let existing = state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .map(|show| show.name.to_lowercase())
        .collect::<HashSet<_>>();
    for number in 1..=10_000 {
        let candidate = if number == 1 {
            stem.to_owned()
        } else {
            format!("{stem} {number}")
        };
        let path = state
            .data_dir
            .join("shows")
            .join(format!("{candidate}.show"));
        if !existing.contains(&candidate.to_lowercase()) && !path.exists() {
            return Ok(candidate);
        }
    }
    Err(ApiError::conflict("no available show name remains"))
}

fn revision_copy_name(
    state: &AppState,
    source_name: &str,
    revision: u64,
    copied_on: chrono::NaiveDate,
) -> Result<String, ApiError> {
    let existing = state
        .desk
        .lock()
        .library()
        .map_err(ApiError::store)?
        .into_iter()
        .map(|show| show.name.to_lowercase())
        .collect::<HashSet<_>>();
    let stem_suffix = format!("-rev-{revision}-{copied_on}");
    for number in 1..=10_000 {
        let disambiguator = if number == 1 {
            String::new()
        } else {
            format!("-{number}")
        };
        let available = 100usize.saturating_sub(stem_suffix.len() + disambiguator.len());
        let mut boundary = source_name.len().min(available);
        while !source_name.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let candidate = format!(
            "{}{}{}",
            &source_name[..boundary],
            stem_suffix,
            disambiguator
        );
        let path = state
            .data_dir
            .join("shows")
            .join(format!("{candidate}.show"));
        if !existing.contains(&candidate.to_lowercase()) && !path.exists() {
            return Ok(candidate);
        }
    }
    Err(ApiError::conflict(
        "no unused name is available for the revision copy",
    ))
}
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}
impl ApiError {
    fn fixture(error: light_fixture::FixtureError) -> Self {
        match error {
            light_fixture::FixtureError::RevisionConflict { .. } => {
                Self::conflict(error.to_string())
            }
            _ => Self::bad_request(error.to_string()),
        }
    }
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
    fn not_found(what: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: format!("{} not found", what.into()),
        }
    }
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }
    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }
    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }
    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }
    fn store(error: light_show::StoreError) -> Self {
        match error {
            light_show::StoreError::RevisionConflict { .. } => Self::conflict(error.to_string()),
            _ => Self::bad_request(error.to_string()),
        }
    }
    fn io(error: std::io::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(light_wire::v2::command_line::CommandErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[path = "command_http_tests.rs"]
    mod command_http_tests;

    fn test_control_desk() -> ControlDesk {
        ControlDesk {
            id: Uuid::nil(),
            name: "Test desk".into(),
            osc_alias: "test-desk".into(),
            columns: 8,
            rows: 1,
            buttons: 3,
            playback_layout: None,
        }
    }

    fn preload_test_playback(
        buttons: [light_playback::PlaybackButtonAction; 3],
    ) -> light_playback::PlaybackDefinition {
        light_playback::PlaybackDefinition {
            number: 1,
            name: "Preload test".into(),
            target: light_playback::PlaybackTarget::CueList {
                cue_list_id: light_core::CueListId::new(),
            },
            buttons,
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader: true,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        }
    }

    fn preload_atomicity_test_snapshot() -> EngineSnapshot {
        let first_cue_list_id = light_core::CueListId::new();
        let second_cue_list_id = light_core::CueListId::new();
        let cue_list = |id, name: &str| light_playback::CueList {
            id,
            name: name.into(),
            priority: 0,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![light_playback::Cue::new(1.0)],
        };
        let playback = |number, target| light_playback::PlaybackDefinition {
            number,
            name: format!("Atomic Preload {number}"),
            target,
            buttons: [light_playback::PlaybackButtonAction::None; 3],
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader: true,
            go_activates: true,
            auto_off: false,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                ..Default::default()
            }],
            cue_lists: vec![
                cue_list(first_cue_list_id, "Atomic Preload A"),
                cue_list(second_cue_list_id, "Atomic Preload B"),
            ],
            playbacks: vec![
                playback(
                    1,
                    light_playback::PlaybackTarget::CueList {
                        cue_list_id: first_cue_list_id,
                    },
                ),
                playback(
                    2,
                    light_playback::PlaybackTarget::CueList {
                        cue_list_id: second_cue_list_id,
                    },
                ),
                playback(
                    3,
                    light_playback::PlaybackTarget::Group {
                        group_id: "front".into(),
                    },
                ),
            ],
            ..Default::default()
        }
    }

    fn matter_test_snapshot() -> EngineSnapshot {
        let cue_list_id = light_core::CueListId::new();
        let cue_list = light_playback::CueList {
            id: cue_list_id,
            name: "Matter look".into(),
            priority: 0,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![light_playback::Cue::new(1.0)],
        };
        let definition = |number, has_fader| light_playback::PlaybackDefinition {
            number,
            name: format!("Matter playback {number}"),
            target: light_playback::PlaybackTarget::CueList { cue_list_id },
            buttons: [light_playback::PlaybackButtonAction::None; 3],
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader,
            go_activates: true,
            auto_off: false,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        EngineSnapshot {
            cue_lists: vec![cue_list],
            playbacks: vec![definition(25, true), definition(26, false)],
            playback_pages: vec![
                light_playback::PlaybackPage {
                    number: 1,
                    name: "Main".into(),
                    slots: HashMap::from([(7, 26)]),
                },
                light_playback::PlaybackPage {
                    number: 4,
                    name: "Matter".into(),
                    slots: HashMap::from([(7, 25)]),
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn preload_capture_resolves_real_buttons_canonicalizes_temp_and_excludes_live_controls() {
        use light_playback::PlaybackButtonAction as Action;
        let playback = preload_test_playback([Action::Toggle, Action::Flash, Action::Go]);
        let button = |number, pressed| PoolPlaybackInput {
            button: Some(number),
            pressed: Some(pressed),
            ..PoolPlaybackInput::default()
        };
        assert_eq!(
            preload_capture_action(&playback, "button", &button(1, true)).unwrap(),
            Some("toggle")
        );
        assert_eq!(
            preload_capture_action(&playback, "button", &button(2, true)).unwrap(),
            None
        );
        assert_eq!(
            preload_capture_action(&playback, "button", &button(3, true)).unwrap(),
            Some("go")
        );
        assert_eq!(
            preload_capture_action(&playback, "button", &button(1, false)).unwrap(),
            None
        );
        let temp_playback = preload_test_playback([Action::Temp, Action::Flash, Action::Go]);
        assert_eq!(
            preload_capture_action_with_temp_state(
                &temp_playback,
                "button",
                &button(1, true),
                false,
            )
            .unwrap(),
            Some("temp-on")
        );
        assert_eq!(
            preload_capture_action_with_temp_state(
                &temp_playback,
                "button",
                &button(1, true),
                true,
            )
            .unwrap(),
            Some("temp-off")
        );
        assert_eq!(
            preload_capture_action(&playback, "temp-off", &button(1, false)).unwrap(),
            Some("temp-off")
        );
        assert_eq!(
            preload_capture_action(
                &playback,
                "master",
                &PoolPlaybackInput {
                    value: Some(0.5),
                    ..PoolPlaybackInput::default()
                }
            )
            .unwrap(),
            None
        );
        for (requested, retained) in [
            ("toggle", "toggle"),
            ("go", "go"),
            ("go-minus", "go-minus"),
            ("back", "go-minus"),
            ("off", "off"),
            ("on", "on"),
            ("temp-on", "temp-on"),
            ("temp-off", "temp-off"),
        ] {
            assert_eq!(
                preload_capture_action(&playback, requested, &PoolPlaybackInput::default())
                    .unwrap(),
                Some(retained)
            );
        }
    }

    #[test]
    fn preload_rejects_a_late_invalid_action_without_publishing_earlier_actions() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "atomic-preload".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        state
            .engine
            .replace_snapshot(preload_atomicity_test_snapshot())
            .unwrap();
        assert!(state.programmers.arm_preload(session.id, true));
        assert!(state.programmers.queue_preload_playback_action(
            session.id,
            1,
            "go".into(),
            "physical".into(),
        ));
        assert!(state.programmers.queue_preload_playback_action(
            session.id,
            3,
            "on".into(),
            "virtual".into(),
        ));
        let programmer_before = state.programmers.get(session.id).unwrap();

        let error = commit_preload(&state, &session).unwrap_err();

        assert!(error.contains("group playback"), "{error}");
        assert!(state.engine.playback().read().runtime().is_empty());
        let programmer_after = state.programmers.get(session.id).unwrap();
        assert_eq!(
            programmer_after.preload_playback_pending,
            programmer_before.preload_playback_pending
        );
        assert_eq!(programmer_after.blind, programmer_before.blind);
        assert!(
            state
                .audit_events
                .lock()
                .iter()
                .all(|event| event.kind != "preload_committed")
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn staged_preload_applies_exclusions_without_mutating_the_source_engine() {
        let (state, data_dir) = test_state();
        state
            .engine
            .replace_snapshot(preload_atomicity_test_snapshot())
            .unwrap();
        state.engine.playback().write().on(2).unwrap();
        let snapshot = state.engine.snapshot();
        let definition = snapshot
            .playbacks
            .iter()
            .find(|definition| definition.number == 1)
            .cloned()
            .unwrap();
        let pending = light_programmer::PreloadPlaybackAction {
            playback_number: 1,
            action: "on".into(),
            surface: "virtual".into(),
        };
        let current = state.engine.playback().read().clone();

        let (staged, actions) = stage_preload_playback_batch(
            &current,
            &[(pending, definition)],
            chrono::Utc::now(),
            0,
            &[vec![1, 2]],
        )
        .unwrap();

        let source = current.runtime();
        assert!(
            source
                .iter()
                .any(|runtime| { runtime.playback_number == Some(2) && runtime.enabled })
        );
        assert!(
            source
                .iter()
                .all(|runtime| runtime.playback_number != Some(1))
        );
        let result = staged.runtime();
        assert!(
            result
                .iter()
                .any(|runtime| { runtime.playback_number == Some(1) && runtime.enabled })
        );
        assert!(
            result
                .iter()
                .any(|runtime| { runtime.playback_number == Some(2) && !runtime.enabled })
        );
        assert_eq!(actions[0].released_playbacks, vec![2]);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn command_backspace_removes_words_as_tokens_and_numbers_as_characters() {
        let mut value = "GROUP 1 THRU 6 AT 88".to_string();
        for expected in [
            "GROUP 1 THRU 6 AT 8",
            "GROUP 1 THRU 6 AT",
            "GROUP 1 THRU 6",
            "GROUP 1 THRU",
            "GROUP 1",
            "GROUP",
            "",
        ] {
            value = remove_command_token(&value);
            assert_eq!(value, expected);
        }
    }

    #[test]
    fn osc_keypad_uses_the_same_scoped_selection_edits_as_the_ui() {
        let mut value = edit_osc_programmer_command("", "grp", "FIXTURE");
        value = edit_osc_programmer_command(&value, "digit-7", "FIXTURE");
        value = edit_osc_programmer_command(&value, "plus", "FIXTURE");
        value = edit_osc_programmer_command(&value, "digit-8", "FIXTURE");
        assert_eq!(value, "G7 + F8");

        let override_scope = edit_osc_programmer_command("G7 +", "grp", "FIXTURE");
        assert_eq!(
            edit_osc_programmer_command(&override_scope, "digit-8", "FIXTURE"),
            "G7 + G8"
        );
        assert_eq!(
            edit_osc_programmer_command("G7 +", "digit-8", "GROUP"),
            "G7 + G8"
        );
    }

    #[test]
    fn osc_and_ui_share_the_unlocked_desk_command_context_not_the_user_session() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let (front, wing) = {
            let store = state.desk.lock();
            (
                store.add_desk("Front", "front").unwrap(),
                store.add_desk("Wing", "wing").unwrap(),
            )
        };
        let ui = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "front-ui".into(),
            connected: true,
            desk: front.clone(),
        };
        let second_front = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "front-second".into(),
            connected: true,
            desk: front.clone(),
        };
        let wing_ui = Session {
            id: SessionId::new(),
            user,
            token: "wing-ui".into(),
            connected: true,
            desk: wing,
        };
        for session in [&ui, &second_front, &wing_ui] {
            state.programmers.start(session.id, session.user.id);
            attach_session_command_context(&state, session);
            state.sessions.write().insert(session.id, session.clone());
        }
        state.programmers.set_command_line(ui.id, "GROUP".into());
        state.programmers.set_command_target(ui.id, "GROUP".into());

        write_desk_lock(
            &state,
            front.id,
            &DeskLockConfiguration {
                locked: true,
                ..DeskLockConfiguration::default()
            },
        )
        .unwrap();
        let source = "127.0.0.1:19010";
        handle_control_event(
            &state,
            ControlEvent::Osc {
                address: "/light/subscribe".into(),
                arguments: vec![
                    OscArgument::String("front-hardware".into()),
                    OscArgument::String("front".into()),
                    OscArgument::Int(19011),
                ],
                source: Some(source.into()),
            },
        );
        handle_control_event(
            &state,
            ControlEvent::Osc {
                address: "/light/front/programmer/digit-7".into(),
                arguments: vec![OscArgument::Bool(true)],
                source: Some(source.into()),
            },
        );
        assert_eq!(state.programmers.get(ui.id).unwrap().command_line, "GROUP");

        write_desk_lock(&state, front.id, &DeskLockConfiguration::default()).unwrap();
        handle_control_event(
            &state,
            ControlEvent::Osc {
                address: "/light/front/programmer/digit-7".into(),
                arguments: vec![OscArgument::Bool(true)],
                source: Some(source.into()),
            },
        );
        assert_eq!(state.programmers.get(ui.id).unwrap().command_line, "G7");
        assert_eq!(
            state.programmers.get(second_front.id).unwrap().command_line,
            "G7"
        );
        assert!(
            state
                .programmers
                .get(wing_ui.id)
                .unwrap()
                .command_line
                .is_empty()
        );
        assert_eq!(state.programmers.command_target(wing_ui.id), "FIXTURE");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn file_input_context_follows_the_desk_not_the_shared_programmer_session() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let mut front = test_control_desk();
        front.id = Uuid::new_v4();
        front.osc_alias = "front".into();
        let mut wing = test_control_desk();
        wing.id = Uuid::new_v4();
        wing.osc_alias = "wing".into();
        let owner = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "owner".into(),
            connected: true,
            desk: front.clone(),
        };
        let same_desk_hardware = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "hardware".into(),
            connected: true,
            desk: front,
        };
        let different_desk = Session {
            id: SessionId::new(),
            user,
            token: "wing".into(),
            connected: true,
            desk: wing,
        };
        state.file_input_contexts.lock().insert(
            owner.desk.id,
            file_manager::FileInputContext {
                instance_id: "front-files".into(),
                action: file_manager::FileInputAction::Copy,
                session_id: owner.id,
                desk_id: owner.desk.id,
                expires_at: Instant::now() + Duration::from_secs(60),
            },
        );

        assert!(file_manager::route_osc_input(
            &state,
            &same_desk_hardware,
            "enter"
        ));
        assert!(!file_manager::route_osc_input(
            &state,
            &different_desk,
            "enter"
        ));
        assert!(
            state
                .file_input_contexts
                .lock()
                .contains_key(&owner.desk.id)
        );
        assert!(file_manager::route_osc_input(
            &state,
            &same_desk_hardware,
            "escape"
        ));
        assert!(state.file_input_contexts.lock().is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn competing_file_input_context_claims_are_atomic() {
        let (state, data_dir) = test_state();
        let desk_id = Uuid::new_v4();
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let results = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for instance_id in ["files-left", "files-right"] {
                let state = state.clone();
                let barrier = Arc::clone(&barrier);
                handles.push(scope.spawn(move || {
                    let context = file_manager::FileInputContext {
                        instance_id: instance_id.into(),
                        action: file_manager::FileInputAction::Copy,
                        session_id: SessionId::new(),
                        desk_id,
                        expires_at: Instant::now() + Duration::from_secs(60),
                    };
                    barrier.wait();
                    file_manager::try_claim_input_context(&state, context, || Ok(())).is_ok()
                }));
            }
            barrier.wait();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });

        assert_eq!(results.iter().filter(|claimed| **claimed).count(), 1);
        assert_eq!(state.file_input_contexts.lock().len(), 1);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn legacy_four_speed_group_configuration_gains_group_e() {
        let configuration: DeskConfiguration =
            serde_json::from_value(serde_json::json!({"speed_groups_bpm":[101,102,103,104]}))
                .unwrap();
        assert_eq!(
            configuration.speed_groups_bpm,
            [101.0, 102.0, 103.0, 104.0, 15.0]
        );
        assert_eq!(
            configuration.speed_group_sound_to_light,
            default_sound_to_light()
        );
        assert!(!configuration.matter_enabled);
        assert!(!configuration.patch_preview_highlight_dmx);
        assert!(!configuration.file_manager_system_picker_fallback);
        assert!(configuration.file_manager_roots.is_empty());
        let five: DeskConfiguration =
            serde_json::from_value(serde_json::json!({"speed_groups_bpm":[1,2,3,4,5]})).unwrap();
        assert_eq!(five.speed_groups_bpm, [1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn matter_bridge_writes_and_tracking_feedback_use_explicit_global_addresses() {
        let (state, data_dir) = test_state();
        state.configuration.write().matter_enabled = true;
        state
            .engine
            .replace_snapshot(matter_test_snapshot())
            .unwrap();

        let initial = refresh_matter_bridge(&state);
        assert_eq!(initial.lights.len(), 2);
        assert_eq!(
            initial
                .lights
                .iter()
                .map(|light| (light.page, light.playback, light.playback_number))
                .collect::<Vec<_>>(),
            vec![(1, 7, 26), (4, 7, 25)]
        );

        let status = apply_matter_playback_write(
            &state,
            matter::endpoint_id(4, 7).unwrap(),
            matter::MatterPlaybackWrite {
                on: None,
                level: Some(127),
            },
        )
        .unwrap();
        let runtime = state.engine.playback().read().runtime();
        let addressed = runtime
            .iter()
            .find(|playback| playback.playback_number == Some(25))
            .unwrap();
        assert!(addressed.enabled);
        assert!((addressed.master - 0.5).abs() < 0.001);
        assert!(
            runtime
                .iter()
                .all(|playback| playback.playback_number != Some(26)),
            "page 4/playback 7 must not inherit page 1/playback 7"
        );
        let light = status
            .lights
            .iter()
            .find(|light| light.page == 4 && light.playback == 7)
            .unwrap();
        assert!(light.on);
        assert_eq!(light.level, 127);

        // Automatic tracking/off behavior is mirrored back to the Matter attribute snapshot.
        state.engine.playback().write().off(25).unwrap();
        let tracked_off = refresh_matter_bridge(&state);
        let light = tracked_off
            .lights
            .iter()
            .find(|light| light.page == 4 && light.playback == 7)
            .unwrap();
        assert!(!light.on);
        assert_eq!(light.level, 0);
        assert_eq!(
            state.audit_events.lock().back().unwrap().payload["source"],
            "matter"
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn matter_virtual_master_controls_and_tracks_a_faderless_assignment() {
        let (state, data_dir) = test_state();
        state.configuration.write().matter_enabled = true;
        state
            .engine
            .replace_snapshot(matter_test_snapshot())
            .unwrap();
        let endpoint = matter::endpoint_id(1, 7).unwrap();
        let definition = state
            .engine
            .snapshot()
            .playbacks
            .iter()
            .find(|definition| definition.number == 26)
            .cloned()
            .unwrap();

        let rejected = dispatch_playback_action(
            &state,
            None,
            None,
            &definition,
            "fader",
            &PoolPlaybackInput {
                value: Some(0.5),
                ..PoolPlaybackInput::default()
            },
            "osc",
        )
        .unwrap_err();
        assert_eq!(rejected.message, "playback does not have a fader");

        let status = apply_matter_playback_write(
            &state,
            endpoint,
            matter::MatterPlaybackWrite {
                on: None,
                level: Some(127),
            },
        )
        .unwrap();
        let runtime = state.engine.playback().read().runtime();
        let active = runtime
            .iter()
            .find(|playback| playback.playback_number == Some(26))
            .unwrap();
        assert!(active.enabled);
        assert!((active.master - 0.5).abs() < 0.001);
        assert!((active.fader_position - 0.5).abs() < 0.001);
        let light = status
            .lights
            .iter()
            .find(|light| light.endpoint_id == endpoint)
            .unwrap();
        assert!(light.on);
        assert_eq!(light.level, 127);

        let off = apply_matter_playback_write(
            &state,
            endpoint,
            matter::MatterPlaybackWrite {
                on: Some(false),
                level: None,
            },
        )
        .unwrap();
        let light = off
            .lights
            .iter()
            .find(|light| light.endpoint_id == endpoint)
            .unwrap();
        assert!(!light.on);
        assert_eq!(light.level, 0);

        let on = apply_matter_playback_write(
            &state,
            endpoint,
            matter::MatterPlaybackWrite {
                on: Some(true),
                level: None,
            },
        )
        .unwrap();
        let light = on
            .lights
            .iter()
            .find(|light| light.endpoint_id == endpoint)
            .unwrap();
        assert!(light.on);
        assert_eq!(light.level, matter::MAX_MATTER_LEVEL);
        assert_eq!(
            state
                .engine
                .playback()
                .read()
                .runtime()
                .iter()
                .find(|playback| playback.playback_number == Some(26))
                .unwrap()
                .master,
            1.0
        );

        state.engine.playback().write().off(26).unwrap();
        let tracked_off = refresh_matter_bridge(&state);
        let light = tracked_off
            .lights
            .iter()
            .find(|light| light.endpoint_id == endpoint)
            .unwrap();
        assert!(!light.on);
        assert_eq!(light.level, 0);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn matter_writes_reach_every_assignable_faderless_target_family() {
        let (state, data_dir) = test_state();
        state.configuration.write().matter_enabled = true;
        let definition = |number, target, fader| light_playback::PlaybackDefinition {
            number,
            name: format!("Matter playback {number}"),
            target,
            buttons: [light_playback::PlaybackButtonAction::None; 3],
            button_count: 3,
            fader,
            has_fader: false,
            go_activates: true,
            auto_off: false,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                groups: vec![light_programmer::GroupDefinition {
                    id: "front".into(),
                    name: "Front".into(),
                    master: 1.0,
                    ..Default::default()
                }],
                playbacks: vec![
                    definition(
                        1,
                        light_playback::PlaybackTarget::Group {
                            group_id: "front".into(),
                        },
                        light_playback::PlaybackFaderMode::Master,
                    ),
                    definition(
                        2,
                        light_playback::PlaybackTarget::SpeedGroup { group: "A".into() },
                        light_playback::PlaybackFaderMode::DirectBpm,
                    ),
                    definition(
                        3,
                        light_playback::PlaybackTarget::ProgrammerFade,
                        light_playback::PlaybackFaderMode::Master,
                    ),
                    definition(
                        4,
                        light_playback::PlaybackTarget::CueFade,
                        light_playback::PlaybackFaderMode::Master,
                    ),
                    definition(
                        5,
                        light_playback::PlaybackTarget::GrandMaster,
                        light_playback::PlaybackFaderMode::Master,
                    ),
                ],
                playback_pages: vec![light_playback::PlaybackPage {
                    number: 1,
                    name: "Matter".into(),
                    slots: HashMap::from([(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]),
                }],
                ..Default::default()
            })
            .unwrap();

        for playback in 1..=5 {
            apply_matter_playback_write(
                &state,
                matter::endpoint_id(1, playback).unwrap(),
                matter::MatterPlaybackWrite {
                    on: None,
                    level: Some(127),
                },
            )
            .unwrap();
        }

        assert!(
            (state.engine.snapshot().groups[0].master - 0.5).abs() < 0.001,
            "Group Master uses the Matter level"
        );
        let speed = state.speed_groups.lock()[0].snapshot(application_millis(&state));
        assert!((speed.manual_bpm - 150.0).abs() < 0.001);
        assert!((speed.speed_master_scale - 1.0).abs() < 0.001);
        let configuration = state.configuration.read();
        assert_eq!(configuration.programmer_fade_millis, 10_000);
        assert_eq!(configuration.sequence_master_fade_millis, 30_000);
        assert!((state.output_control.lock().options.grand_master - 0.5).abs() < 0.001);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn matter_feedback_tracks_faderless_temp_and_manual_xfade_positions() {
        let (state, data_dir) = test_state();
        state.configuration.write().matter_enabled = true;
        let mut snapshot = matter_test_snapshot();
        let cue_list_id = snapshot.cue_lists[0].id;
        let definition = |number, fader, has_fader| light_playback::PlaybackDefinition {
            number,
            name: format!("Matter playback {number}"),
            target: light_playback::PlaybackTarget::CueList { cue_list_id },
            buttons: [light_playback::PlaybackButtonAction::None; 3],
            button_count: 3,
            fader,
            has_fader,
            go_activates: true,
            auto_off: false,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        snapshot.playbacks = vec![
            definition(27, light_playback::PlaybackFaderMode::Temp, false),
            definition(28, light_playback::PlaybackFaderMode::XFade, false),
        ];
        snapshot.playback_pages = vec![light_playback::PlaybackPage {
            number: 3,
            name: "Matter".into(),
            slots: HashMap::from([(1, 27), (2, 28)]),
        }];
        state.engine.replace_snapshot(snapshot).unwrap();

        let faderless_xfade = state
            .engine
            .snapshot()
            .playbacks
            .iter()
            .find(|definition| definition.number == 28)
            .cloned()
            .unwrap();
        let rejected = dispatch_playback_action(
            &state,
            None,
            None,
            &faderless_xfade,
            "fader",
            &PoolPlaybackInput {
                value: Some(0.5),
                ..PoolPlaybackInput::default()
            },
            "osc",
        )
        .unwrap_err();
        assert_eq!(rejected.message, "playback does not have a fader");

        for playback in 1..=2 {
            apply_matter_playback_write(
                &state,
                matter::endpoint_id(3, playback).unwrap(),
                matter::MatterPlaybackWrite {
                    on: None,
                    level: Some(127),
                },
            )
            .unwrap();
        }
        let status = refresh_matter_bridge(&state);
        assert_eq!(
            status
                .lights
                .iter()
                .map(|light| (light.playback_number, light.level, light.on))
                .collect::<Vec<_>>(),
            vec![(27, 127, true), (28, 127, true)]
        );

        apply_matter_playback_write(
            &state,
            matter::endpoint_id(3, 1).unwrap(),
            matter::MatterPlaybackWrite {
                on: Some(false),
                level: None,
            },
        )
        .unwrap();
        let status = refresh_matter_bridge(&state);
        assert_eq!(status.lights[0].level, 0);
        assert!(!status.lights[0].on);

        let xfade_endpoint = matter::endpoint_id(3, 2).unwrap();
        let off = apply_matter_playback_write(
            &state,
            xfade_endpoint,
            matter::MatterPlaybackWrite {
                on: Some(false),
                level: None,
            },
        )
        .unwrap();
        assert_eq!(off.lights[1].level, 0);
        assert!(!off.lights[1].on);
        let on = apply_matter_playback_write(
            &state,
            xfade_endpoint,
            matter::MatterPlaybackWrite {
                on: Some(true),
                level: None,
            },
        )
        .unwrap();
        assert_eq!(on.lights[1].level, matter::MAX_MATTER_LEVEL);
        assert!(on.lights[1].on);
        assert_eq!(
            state
                .engine
                .playback()
                .read()
                .runtime()
                .iter()
                .find(|playback| playback.playback_number == Some(28))
                .unwrap()
                .manual_xfade_position,
            1.0
        );

        state.engine.playback().write().off(28).unwrap();
        let tracked_off = refresh_matter_bridge(&state);
        assert_eq!(tracked_off.lights[1].level, 0);
        assert!(!tracked_off.lights[1].on);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn matter_enablement_is_desk_persistent_and_status_is_explicit() {
        let (state, data_dir) = test_state();
        state
            .engine
            .replace_snapshot(matter_test_snapshot())
            .unwrap();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let mut configuration = state.configuration.read().clone();
        configuration.matter_enabled = true;
        let response = app
            .clone()
            .oneshot(
                Request::put("/api/v1/configuration")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&configuration).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = json(response).await;
        assert_eq!(response["matter"]["enabled"], true);
        assert_eq!(response["matter"]["transport"], "adapter_ready");
        assert_eq!(response["matter"]["commissionable"], false);
        assert!(response["matter"]["limitation"].is_string());

        let persisted: DeskConfiguration = serde_json::from_str(
            &state
                .desk
                .lock()
                .setting("server_configuration")
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert!(persisted.matter_enabled);

        let status = app
            .oneshot(
                Request::get("/api/v1/matter/status")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status = json(status).await;
        assert_eq!(status["lights"].as_array().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn direct_bpm_fader_reports_zero_half_and_full_authoritative_rates() {
        let (state, data_dir) = test_state();
        state.speed_groups.lock()[0]
            .set_speed_master_scale(0.25)
            .unwrap();

        let set_fader = |value| {
            apply_speed_group_playback_action(
                &state,
                "A",
                "master",
                &PoolPlaybackInput {
                    value: Some(value),
                    ..PoolPlaybackInput::default()
                },
                light_playback::PlaybackFaderMode::DirectBpm,
            )
            .unwrap();
            state.speed_groups.lock()[0].snapshot(0)
        };

        let half = set_fader(0.5);
        assert_eq!(half.effective_bpm, 150.0);
        assert_eq!(half.speed_master_scale, 1.0);
        assert!(!half.paused);

        let zero = set_fader(0.0);
        assert_eq!(zero.effective_bpm, 0.0);
        assert_eq!(zero.speed_master_scale, 0.0);
        assert!(zero.paused);

        let full = set_fader(1.0);
        assert_eq!(full.effective_bpm, 300.0);
        assert_eq!(full.speed_master_scale, 1.0);
        assert!(!full.paused);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn opening_a_show_repairs_and_persists_missing_logical_heads() {
        let directory = std::env::temp_dir().join(format!("light-head-repair-{}", Uuid::new_v4()));
        let path = directory.join("repair.show");
        std::fs::create_dir_all(&directory).unwrap();
        let show_id = default_show::initialise(&path).unwrap();
        let store = ShowStore::open(&path).unwrap();
        let object = store
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .find(|object| object.body["fixture_number"] == 501)
            .unwrap();
        let mut fixture =
            serde_json::from_value::<light_fixture::PatchedFixture>(object.body.clone()).unwrap();
        fixture.logical_heads.clear();
        store
            .put_object(
                "patched_fixture",
                &object.id,
                &serde_json::to_value(&fixture).unwrap(),
                object.revision,
            )
            .unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Repair".into(),
            path: path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        reconcile_show_logical_heads(&entry).unwrap();
        let repaired = ShowStore::open(&path)
            .unwrap()
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == object.id)
            .unwrap();
        let repaired_fixture =
            serde_json::from_value::<light_fixture::PatchedFixture>(repaired.body).unwrap();
        assert_eq!(repaired_fixture.logical_heads.len(), 10);
        let ids = repaired_fixture
            .logical_heads
            .iter()
            .map(|head| head.fixture_id)
            .collect::<Vec<_>>();
        reconcile_show_logical_heads(&entry).unwrap();
        let stable = ShowStore::open(&path)
            .unwrap()
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == object.id)
            .unwrap();
        let stable_fixture =
            serde_json::from_value::<light_fixture::PatchedFixture>(stable.body).unwrap();
        assert_eq!(
            stable_fixture
                .logical_heads
                .iter()
                .map(|head| head.fixture_id)
                .collect::<Vec<_>>(),
            ids
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn opening_a_legacy_show_migrates_embedded_profiles_and_patch_assignments() {
        let directory =
            std::env::temp_dir().join(format!("light-fixture-v2-repair-{}", Uuid::new_v4()));
        let path = directory.join("repair.show");
        std::fs::create_dir_all(&directory).unwrap();
        let show_id = default_show::initialise(&path).unwrap();
        let store = ShowStore::open(&path).unwrap();
        let object = store.objects("patched_fixture").unwrap().remove(0);
        let mut fixture =
            serde_json::from_value::<light_fixture::PatchedFixture>(object.body).unwrap();
        assert_eq!(fixture.definition.schema_version, 1);
        fixture.split_patches.clear();
        fixture.multipatch.push(light_fixture::MultiPatchInstance {
            id: Uuid::new_v4(),
            name: "Legacy balcony".into(),
            universe: Some(99),
            address: Some(100),
            split_patches: vec![],
            location: Default::default(),
            rotation: Default::default(),
        });
        store
            .put_object(
                "patched_fixture",
                &object.id,
                &serde_json::to_value(&fixture).unwrap(),
                object.revision,
            )
            .unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Legacy fixture migration".into(),
            path: path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };

        let loaded = load_engine_snapshot(&entry).unwrap();
        let migrated = loaded
            .fixtures
            .iter()
            .find(|candidate| candidate.fixture_id == fixture.fixture_id)
            .unwrap();
        assert_eq!(
            migrated.definition.schema_version,
            light_fixture::FIXTURE_PROFILE_SCHEMA_VERSION
        );
        assert!(migrated.definition.profile_snapshot.is_some());
        assert_eq!(migrated.split_patches[0].universe, fixture.universe);
        assert_eq!(migrated.split_patches[0].address, fixture.address);
        assert_eq!(migrated.multipatch[0].split_patches[0].universe, Some(99));
        assert_eq!(migrated.multipatch[0].split_patches[0].address, Some(100));

        let persisted = ShowStore::open(&path)
            .unwrap()
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == object.id)
            .unwrap();
        assert_eq!(persisted.body["definition"]["schema_version"], 2);
        assert!(persisted.body["definition"]["profile_snapshot"].is_object());
        assert_eq!(
            persisted.body["split_patches"][0]["universe"],
            fixture.universe.unwrap()
        );
        assert_eq!(
            persisted.body["multipatch"][0]["split_patches"][0]["universe"],
            99
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn opening_a_legacy_show_persists_stable_cue_identities_once() {
        let directory =
            std::env::temp_dir().join(format!("light-cue-id-repair-{}", Uuid::new_v4()));
        let path = directory.join("repair.show");
        std::fs::create_dir_all(&directory).unwrap();
        let show_id = default_show::initialise(&path).unwrap();
        let store = ShowStore::open(&path).unwrap();
        let mut first_cue = light_playback::Cue::new(1.0);
        first_cue
            .group_changes
            .push(light_playback::GroupCueChange {
                group_id: "1".into(),
                attribute: light_core::AttributeKey::intensity(),
                value: Some(light_core::AttributeValue::Normalized(0.5)),
                automatic_restore: false,
                fade_millis: None,
                delay_millis: None,
            });
        let list = light_playback::CueList {
            id: light_core::CueListId::new(),
            name: "Legacy".into(),
            priority: 0,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![first_cue, light_playback::Cue::new(2.0)],
        };
        let mut legacy = serde_json::to_value(list).unwrap();
        for cue in legacy["cues"].as_array_mut().unwrap() {
            cue.as_object_mut().unwrap().remove("id");
            cue.as_object_mut().unwrap().remove("cue_only");
            for change in cue["group_changes"].as_array_mut().unwrap() {
                change.as_object_mut().unwrap().remove("automatic_restore");
            }
        }
        store.put_object("cue_list", "legacy", &legacy, 0).unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Repair".into(),
            path: path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        let loaded = load_engine_snapshot(&entry).unwrap();
        assert!(!loaded.cue_lists[0].cues[0].cue_only);
        assert!(!loaded.cue_lists[0].cues[0].group_changes[0].automatic_restore);
        reconcile_show_cue_identities(&entry).unwrap();
        let repaired = ShowStore::open(&path)
            .unwrap()
            .objects("cue_list")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let ids = repaired.body["cues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cue| cue["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        reconcile_show_cue_identities(&entry).unwrap();
        let stable = ShowStore::open(&path)
            .unwrap()
            .objects("cue_list")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(stable.revision, repaired.revision);
        assert_eq!(
            stable.body["cues"]
                .as_array()
                .unwrap()
                .iter()
                .map(|cue| cue["id"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>(),
            ids
        );
        let _ = std::fs::remove_dir_all(directory);
    }
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn citp_test_packet(content: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = 26 + payload.len();
        let mut bytes = Vec::with_capacity(size);
        bytes.extend_from_slice(b"CITP");
        bytes.extend_from_slice(&[1, 0]);
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&(size as u32).to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(b"MSEX");
        bytes.extend_from_slice(&[1, 2]);
        bytes.extend_from_slice(&content);
        bytes.extend_from_slice(payload);
        bytes
    }
    async fn read_citp_test_packet(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        use tokio::io::AsyncReadExt;
        let mut header = [0_u8; 20];
        stream.read_exact(&mut header).await.unwrap();
        let size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        let mut packet = header.to_vec();
        packet.resize(size, 0);
        stream.read_exact(&mut packet[20..]).await.unwrap();
        packet
    }

    fn schema_v1_dimmer_rows(
        manufacturer: &str,
        family: &str,
    ) -> Vec<(light_fixture::FixtureDefinition, Vec<u8>)> {
        [1_u16, 2_u16]
            .into_iter()
            .enumerate()
            .map(|(index, footprint)| {
                let definition = light_fixture::FixtureDefinition {
                    schema_version: 1,
                    id: light_core::FixtureId::new(),
                    revision: 1,
                    manufacturer: manufacturer.into(),
                    device_type: "dimmer".into(),
                    name: family.into(),
                    model: family.into(),
                    mode: if index == 0 { "Coarse" } else { "Fine" }.into(),
                    footprint,
                    heads: vec![light_fixture::LogicalHead {
                        index: 0,
                        name: "Main".into(),
                        shared: true,
                        parameters: vec![light_fixture::Parameter {
                            attribute: light_core::AttributeKey("intensity".into()),
                            components: (0..footprint)
                                .map(|offset| light_fixture::ChannelComponent {
                                    offset,
                                    byte_order: light_fixture::ByteOrder::MsbFirst,
                                })
                                .collect(),
                            default: 0.0,
                            virtual_dimmer: false,
                            metadata: light_fixture::ParameterMetadata::default(),
                            capabilities: Vec::new(),
                        }],
                    }],
                    color_calibration: None,
                    physical: light_fixture::FixturePhysicalProperties::default(),
                    model_asset: None,
                    icon_asset: None,
                    hazardous: false,
                    direct_control_protocols: Vec::new(),
                    signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
                    safe_values: BTreeMap::new(),
                    profile_id: None,
                    mode_id: None,
                    profile_snapshot: None,
                };
                (
                    definition,
                    format!("retained-startup-gdtf-{index}").into_bytes(),
                )
            })
            .collect()
    }

    fn seed_schema_v1_fixture_database(
        data_dir: &FsPath,
        rows: &[(light_fixture::FixtureDefinition, Vec<u8>)],
    ) {
        std::fs::create_dir_all(data_dir).unwrap();
        let connection = rusqlite::Connection::open(data_dir.join("fixtures.sqlite")).unwrap();
        connection.execute_batch("CREATE TABLE fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));").unwrap();
        for (definition, source) in rows {
            connection.execute(
                "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![
                    definition.id.0.to_string(),
                    definition.revision,
                    definition.manufacturer,
                    definition.model,
                    definition.mode,
                    serde_json::to_string(definition).unwrap(),
                    source,
                ],
            ).unwrap();
        }
    }

    #[test]
    fn startup_fixture_library_migrates_schema_v1_and_loads_transferable_packages_once() {
        let data_dir = std::env::temp_dir().join(format!(
            "light-startup-fixture-migration-{}",
            Uuid::new_v4()
        ));
        let family = format!("Startup family {}", Uuid::new_v4());
        let rows = schema_v1_dimmer_rows("Startup Legacy", &family);
        assert_eq!(rows.len(), 2);
        let expected_profile_id = rows[0].0.id;
        seed_schema_v1_fixture_database(&data_dir, &rows);

        let package_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixture-library");
        let package_count = std::fs::read_dir(&package_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "toskfixture")
            })
            .count();
        let library = open_fixture_library_for_startup(&data_dir, Some(&package_dir)).unwrap();
        let profiles = library.profiles().unwrap();
        let migrated = profiles
            .iter()
            .find(|profile| profile.id == expected_profile_id)
            .unwrap();
        assert_eq!(migrated.schema_version, 2);
        assert_eq!(migrated.revision, 1);
        assert_eq!(migrated.manufacturer, "Startup Legacy");
        assert_eq!(migrated.name, family);
        assert_eq!(
            migrated
                .modes
                .iter()
                .map(|mode| mode.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Coarse", "Fine"]
        );
        assert!(
            profiles
                .iter()
                .all(|profile| profile.reserved_source.is_none())
        );
        let vendor_profiles = profiles
            .iter()
            .filter(|profile| profile.manufacturer == "ROBE")
            .collect::<Vec<_>>();
        assert_eq!(profiles.len(), package_count + 1);
        assert_eq!(vendor_profiles.len(), 5);
        assert!(vendor_profiles.iter().any(|profile| {
            profile.name == "Robin 600X LEDWash"
                && profile
                    .modes
                    .iter()
                    .map(|mode| mode.splits[0].footprint)
                    .collect::<Vec<_>>()
                    == vec![37, 21, 15, 10, 37, 25]
        }));
        let legacy_sources = library
            .profile_legacy_sources(expected_profile_id, 1)
            .unwrap();
        assert_eq!(legacy_sources.len(), 2);
        for (definition, source) in &rows {
            let expected_json = serde_json::to_string(definition).unwrap();
            assert_eq!(
                library.export_json(definition.id, 1).unwrap().as_deref(),
                Some(expected_json.as_str())
            );
            assert_eq!(
                library.source_gdtf(definition.id, 1).unwrap().as_deref(),
                Some(source.as_slice())
            );
        }
        let initial = serde_json::to_value(&profiles).unwrap();
        drop(library);

        let reopened = open_fixture_library_for_startup(&data_dir, Some(&package_dir)).unwrap();
        assert_eq!(
            serde_json::to_value(reopened.profiles().unwrap()).unwrap(),
            initial
        );
        assert!(reopened.migration_warnings().unwrap().is_empty());
        drop(reopened);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn startup_fixture_library_keeps_malformed_and_conflicting_schema_v1_evidence() {
        let data_dir =
            std::env::temp_dir().join(format!("light-startup-fixture-recovery-{}", Uuid::new_v4()));
        let family = format!("Conflict family {}", Uuid::new_v4());
        let mut rows = schema_v1_dimmer_rows("Startup Recovery", &family);
        rows[1].0.physical.width_millimetres = Some(500.0);
        seed_schema_v1_fixture_database(&data_dir, &rows);
        let malformed_id = light_core::FixtureId::new();
        let malformed_source = b"retained-malformed-startup-gdtf";
        let connection = rusqlite::Connection::open(data_dir.join("fixtures.sqlite")).unwrap();
        connection.execute(
            "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,1,'Broken','Broken','Broken','{',?2)",
            rusqlite::params![malformed_id.0.to_string(), malformed_source.as_slice()],
        ).unwrap();
        drop(connection);

        let library = open_fixture_library_for_startup(&data_dir, None).unwrap();
        let warnings = library.migration_warnings().unwrap();
        assert!(warnings.iter().any(|warning| {
            warning.contains(&malformed_id.0.to_string())
                && warning.contains("could not be migrated")
                && warning.contains("original definition and GDTF source were retained")
        }));
        assert!(warnings.iter().any(|warning| {
            warning.contains("Startup Recovery")
                && warning.contains(&family)
                && warning.contains("conflicting fixture-level metadata")
                && warning.contains("retained as separate profiles")
        }));
        assert_eq!(
            library.export_json(malformed_id, 1).unwrap().as_deref(),
            Some("{")
        );
        assert_eq!(
            library.source_gdtf(malformed_id, 1).unwrap().as_deref(),
            Some(malformed_source.as_slice())
        );
        for (definition, source) in &rows {
            let expected_json = serde_json::to_string(definition).unwrap();
            assert_eq!(
                library.export_json(definition.id, 1).unwrap().as_deref(),
                Some(expected_json.as_str())
            );
            assert_eq!(
                library.source_gdtf(definition.id, 1).unwrap().as_deref(),
                Some(source.as_slice())
            );
        }
        let profiles = serde_json::to_value(library.profiles().unwrap()).unwrap();
        drop(library);

        let reopened = open_fixture_library_for_startup(&data_dir, None).unwrap();
        assert_eq!(reopened.migration_warnings().unwrap(), warnings);
        assert_eq!(
            serde_json::to_value(reopened.profiles().unwrap()).unwrap(),
            profiles
        );
        assert_eq!(
            reopened.export_json(malformed_id, 1).unwrap().as_deref(),
            Some("{")
        );
        assert_eq!(
            reopened.source_gdtf(malformed_id, 1).unwrap().as_deref(),
            Some(malformed_source.as_slice())
        );
        drop(reopened);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    fn test_state() -> (AppState, PathBuf) {
        let data_dir = std::env::temp_dir().join(format!("light-server-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(data_dir.join("shows")).unwrap();
        let desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
        let fixture_library =
            light_fixture::FixtureLibrary::open(data_dir.join("fixtures.sqlite")).unwrap();
        let programmers = ProgrammerRegistry::default();
        let engine = Arc::new(Engine::new(programmers.clone()));
        let (events, _) = broadcast::channel(32);
        (
            AppState {
                desk: Arc::new(Mutex::new(desk)),
                fixture_library: Arc::new(Mutex::new(fixture_library)),
                data_dir: data_dir.clone(),
                sessions: Arc::default(),
                session_clients: Arc::default(),
                ws_connections: Arc::new(Mutex::new(HashMap::new())),
                programmers,
                engine,
                highlight: Arc::new(HighlightRegistry::default()),
                patch_preview_highlights: Arc::default(),
                output_health: Arc::new(std::sync::Mutex::new(OutputHealth::default())),
                output_rate: Arc::new(AtomicU16::new(44)),
                configuration: Arc::new(RwLock::new(DeskConfiguration::default())),
                matter_bridge: Arc::new(matter::MatterBridgeAdapter::default()),
                matter_transport: None,
                output_control: Arc::new(Mutex::new(OutputControl::default())),
                activation_lock: Arc::new(tokio::sync::Mutex::new(())),
                playback_action_lock: Arc::new(Mutex::new(())),
                timecode_router: Arc::new(Mutex::new(TimecodeRouter::default())),
                active_show: Arc::default(),
                active_show_error: Arc::default(),
                events,
                application_events: EventBus::default(),
                audit_events: Arc::new(Mutex::new(VecDeque::with_capacity(2048))),
                command_history: Arc::new(Mutex::new(HashMap::new())),
                command_http: command_http::CommandHttpState::default(),
                event_revision: Arc::new(AtomicU64::new(0)),
                desk_token: None,
                shutdown: CancellationToken::new(),
                media_cache: Arc::new(Mutex::new(MediaCache::default())),
                media_status: Arc::new(RwLock::new(HashMap::new())),
                input_locks: Arc::new(Mutex::new(HashMap::new())),
                file_input_contexts: Arc::new(Mutex::new(HashMap::new())),
                osc_subscribers: Arc::new(Mutex::new(HashMap::new())),
                osc_feedback: None,
                osc_feedback_capture: Arc::new(Mutex::new(Vec::new())),
                mvr_imports: Arc::new(Mutex::new(HashMap::new())),
                network_output: None,
                output_sequences: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                manual_clock: None,
                speed_groups: Arc::new(Mutex::new(std::array::from_fn(|index| {
                    SpeedGroupController::new(
                        default_speed_groups()[index],
                        SoundToLightConfig::default(),
                    )
                    .unwrap()
                }))),
                sound_capture_owners: Arc::new(Mutex::new([None; 5])),
            },
            data_dir,
        )
    }

    #[test]
    fn startup_rebases_show_paths_after_the_desk_data_directory_moves() {
        let root = std::env::temp_dir().join(format!("light-show-rebase-{}", Uuid::new_v4()));
        let legacy = root.join("legacy");
        let current = root.join("current");
        std::fs::create_dir_all(legacy.join("shows")).unwrap();
        std::fs::create_dir_all(&current).unwrap();
        let old_path = legacy.join("shows").join("Default Stage Show.show");
        default_show::initialise(&old_path).unwrap();
        let desk = DeskStore::open(current.join("desk.sqlite")).unwrap();
        let entry = desk
            .upsert_show(default_show::name(), &old_path.display().to_string(), false)
            .unwrap();
        std::fs::rename(legacy.join("shows"), current.join("shows")).unwrap();

        rebase_desk_show_paths(&desk, &current).unwrap();

        let relocated = desk.show(entry.id).unwrap().unwrap();
        assert_eq!(
            FsPath::new(&relocated.path),
            current.join("shows").join("Default Stage Show.show")
        );
        validate_show_file(&relocated.path).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn clean_default_load_creates_a_pristine_copy_without_replacing_manual_changes() {
        let (state, data_dir) = test_state();
        let working = ensure_default_show_available(&state.desk.lock(), &data_dir).unwrap();
        let working_store = ShowStore::open(&working.path).unwrap();
        let hazer = working_store
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .find(|object| object.body["name"] == "Stage Hazer")
            .unwrap();
        assert!(
            working_store
                .delete_object("patched_fixture", &hazer.id)
                .unwrap()
        );
        state.desk.lock().set_active_show(Some(working.id)).unwrap();
        *state.active_show.write() = Some(working.clone());
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&working).unwrap())
            .unwrap();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;

        let response = app
            .oneshot(
                Request::post("/api/v1/shows/default/open")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let opened = json(response).await;
        assert_eq!(opened["name"], "Default Stage Show Clean Copy");
        let clean_store = ShowStore::open(opened["path"].as_str().unwrap()).unwrap();
        let clean_fixtures = clean_store.objects("patched_fixture").unwrap();
        assert_eq!(clean_fixtures.len(), 49);
        assert!(
            clean_fixtures
                .iter()
                .any(|object| object.body["name"] == "Stage Hazer")
        );
        assert_eq!(
            ShowStore::open(&working.path)
                .unwrap()
                .objects("patched_fixture")
                .unwrap()
                .len(),
            48
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn command_history_is_desk_scoped_bounded_newest_first_and_redacted() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "history-token".into(),
            connected: true,
            desk: test_control_desk(),
        };
        let other = Session {
            id: SessionId::new(),
            user,
            token: "other-history-token".into(),
            connected: true,
            desk: ControlDesk {
                id: Uuid::new_v4(),
                name: "Other desk".into(),
                osc_alias: "other-desk".into(),
                ..test_control_desk()
            },
        };
        state.sessions.write().insert(session.id, session.clone());
        state.sessions.write().insert(other.id, other.clone());
        for number in 0..54 {
            record_command_history(
                &state,
                &session,
                &format!("GROUP 1 AT {number}"),
                "accepted",
                "Accepted",
                "software",
                None,
            );
        }
        record_command_history(
            &state,
            &session,
            "LOGIN TOKEN super-secret-value",
            "rejected",
            "parser included super-secret-value",
            "software",
            None,
        );
        record_command_history(
            &state,
            &other,
            "GROUP 2 AT 50",
            "accepted",
            "Accepted",
            "osc",
            None,
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer history-token".parse().unwrap(),
        );
        let Json(entries) = command_history(State(state), headers).await.unwrap();
        assert_eq!(entries.len(), COMMAND_HISTORY_LIMIT);
        assert_eq!(entries[0].command, "[REDACTED SENSITIVE COMMAND]");
        assert_eq!(entries[0].feedback, "Sensitive input omitted");
        assert_eq!(entries[49].command, "GROUP 1 AT 5");
        assert!(entries.iter().all(|entry| entry.desk_id == session.desk.id));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn advancing_from_an_occupied_last_playback_page_creates_one_empty_page() {
        let (state, data_dir) = test_state();
        let show_path = data_dir.join("shows/page-advance.show");
        let show_id = initialise_show(&show_path, "Page advance").unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Page advance".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        let page = light_playback::PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::from([(1, 1)]),
        };
        let playback = light_playback::PlaybackDefinition {
            number: 1,
            name: "Grand Master".into(),
            target: light_playback::PlaybackTarget::GrandMaster,
            buttons: light_playback::PlaybackDefinition::default_buttons(
                &light_playback::PlaybackTarget::GrandMaster,
            ),
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader: true,
            go_activates: true,
            auto_off: false,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        let store = ShowStore::open(&entry.path).unwrap();
        store
            .put_object("playback", "1", &serde_json::to_value(playback).unwrap(), 0)
            .unwrap();
        store
            .put_object(
                "playback_page",
                "1",
                &serde_json::to_value(page).unwrap(),
                0,
            )
            .unwrap();
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();

        assert!(ensure_playback_page_for_advance(&state, &entry, 2).unwrap());
        assert!(!ensure_playback_page_for_advance(&state, &entry, 3).unwrap());
        let pages = ShowStore::open(&entry.path)
            .unwrap()
            .objects("playback_page")
            .unwrap();
        let created = pages.iter().find(|object| object.id == "2").unwrap();
        assert_eq!(
            serde_json::from_value::<light_playback::PlaybackPage>(created.body.clone())
                .unwrap()
                .name,
            "Page 2"
        );
        assert_eq!(pages.len(), 2);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn production_router_does_not_expose_test_clock_controls() {
        let (state, data_dir) = test_state();
        let response = router(state)
            .oneshot(
                Request::post("/api/v1/test/clock/advance")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"millis":0}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn alignment_uses_shortest_wrapped_path_and_deterministic_order() {
        assert!((aligned_normalized("left", 1, 3, 0.9, 0.1, false).unwrap() - 0.5).abs() < 0.001);
        let wrapped = aligned_normalized("left", 1, 3, 0.9, 0.1, true).unwrap();
        assert!(!(0.001..=0.999).contains(&wrapped));
        assert!((aligned_normalized("right", 0, 3, 0.2, 0.8, false).unwrap() - 0.8).abs() < 0.001);
    }

    fn schema_v2_direct_fixture() -> (light_fixture::PatchedFixture, Uuid, [Uuid; 2]) {
        let mut profile = light_fixture::FixtureProfile::blank();
        profile.revision = 1;
        profile.manufacturer = "Test".into();
        profile.name = "Semantic fixture".into();
        profile.short_name = "Semantic".into();
        let mode_id = profile.modes[0].id;
        let head_id = profile.modes[0].heads[0].id;
        let indexed_channel = Uuid::new_v4();
        let reset_channel = Uuid::new_v4();
        let action_id = Uuid::new_v4();
        profile.modes[0].splits[0].footprint = 2;
        profile.modes[0].channels = vec![
            light_fixture::FixtureChannel {
                id: indexed_channel,
                head_id,
                split: 1,
                attribute: light_core::AttributeKey("gobo.1".into()),
                resolution: light_fixture::ChannelResolution::U8,
                secondary_slots: vec![],
                default_raw: 0,
                highlight_raw: 255,
                physical_min: None,
                physical_max: None,
                unit: None,
                invert: false,
                snap: true,
                reacts_to_virtual_intensity: false,
                reacts_to_sequence_master: false,
                reacts_to_group_master: false,
                reacts_to_grand_master: false,
                behavior: light_fixture::ChannelBehavior::Controlled,
                functions: vec![light_fixture::ChannelFunction {
                    id: Uuid::new_v4(),
                    name: "Dots".into(),
                    dmx_from: 0,
                    dmx_to: 127,
                    attribute: light_core::AttributeKey("gobo.1".into()),
                    priority: 100,
                    behavior: light_fixture::ChannelFunctionBehavior::Indexed {
                        semantic_id: "gobo.dots".into(),
                        label: "Dots".into(),
                        raw_value: 93,
                    },
                }],
            },
            light_fixture::FixtureChannel {
                id: reset_channel,
                head_id,
                split: 1,
                attribute: light_core::AttributeKey("control.reset".into()),
                resolution: light_fixture::ChannelResolution::U8,
                secondary_slots: vec![],
                default_raw: 7,
                highlight_raw: 7,
                physical_min: None,
                physical_max: None,
                unit: None,
                invert: false,
                snap: true,
                reacts_to_virtual_intensity: false,
                reacts_to_sequence_master: false,
                reacts_to_group_master: false,
                reacts_to_grand_master: false,
                behavior: light_fixture::ChannelBehavior::Controlled,
                functions: vec![],
            },
        ];
        profile.modes[0].control_actions = vec![light_fixture::ControlAction {
            id: action_id,
            name: "Reset".into(),
            semantic: light_fixture::ControlActionSemantic::Reset,
            kind: light_fixture::ControlActionKind::Momentary,
            duration_millis: None,
            assignments: vec![
                light_fixture::ControlActionAssignment {
                    channel_id: indexed_channel,
                    active_raw: 201,
                    inactive_raw: 0,
                },
                light_fixture::ControlActionAssignment {
                    channel_id: reset_channel,
                    active_raw: 255,
                    inactive_raw: 7,
                },
            ],
        }];
        let definition = profile.resolved_definition(mode_id).unwrap();
        (
            light_fixture::PatchedFixture {
                fixture_id: light_core::FixtureId::new(),
                fixture_number: Some(1),
                virtual_fixture_number: None,
                name: "Semantic fixture".into(),
                definition,
                universe: Some(1),
                address: Some(1),
                split_patches: vec![],
                layer_id: "default".into(),
                direct_control: None,
                location: Default::default(),
                rotation: Default::default(),
                logical_heads: vec![],
                multipatch: vec![],
                move_in_black_enabled: true,
                move_in_black_delay_millis: 0,
                highlight_overrides: Default::default(),
            },
            action_id,
            [indexed_channel, reset_channel],
        )
    }

    fn highlight_test_fixtures() -> Vec<light_fixture::PatchedFixture> {
        let fixture = schema_v2_direct_fixture().0;
        (0..3)
            .map(|index| {
                let mut fixture = fixture.clone();
                fixture.fixture_id = light_core::FixtureId::new();
                fixture.fixture_number = Some(index + 1);
                fixture.name = format!("Highlight fixture {}", index + 1);
                fixture.address = Some(1 + index as u16 * 10);
                fixture
            })
            .collect()
    }

    #[tokio::test]
    async fn patch_preview_highlight_is_default_off_scoped_and_released() {
        let (state, data_dir) = test_state();
        let fixtures = highlight_test_fixtures();
        let fixture_id = fixtures[0].fixture_id;
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures,
                ..EngineSnapshot::default()
            })
            .unwrap();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let request = |active| {
            Request::put("/api/v1/patch-preview-highlight")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "active":active,
                        "fixture_ids":[fixture_id]
                    }))
                    .unwrap(),
                ))
                .unwrap()
        };

        let disabled = json(app.clone().oneshot(request(true)).await.unwrap()).await;
        assert_eq!(disabled["allowed"], false);
        assert!(state.engine.highlighted_fixtures().is_empty());

        state.configuration.write().patch_preview_highlight_dmx = true;
        let enabled = json(app.clone().oneshot(request(true)).await.unwrap()).await;
        assert_eq!(enabled["active"], true);
        assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_id]);

        let released = json(app.oneshot(request(false)).await.unwrap()).await;
        assert_eq!(released["active"], false);
        assert!(state.engine.highlighted_fixtures().is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    fn highlight_multi_head_fixture() -> (light_fixture::PatchedFixture, [light_core::FixtureId; 2])
    {
        let (mut fixture, _, _) = schema_v2_direct_fixture();
        fixture.fixture_number = Some(1);
        fixture.name = "Two-cell Highlight fixture".into();
        let mut profile = *fixture.definition.profile_snapshot.take().unwrap();
        let mode_id = profile.modes[0].id;
        profile.modes[0].heads.extend([
            light_fixture::FixtureHead {
                id: Uuid::new_v4(),
                name: "Cell 1".into(),
                master_shared: false,
            },
            light_fixture::FixtureHead {
                id: Uuid::new_v4(),
                name: "Cell 2".into(),
                master_shared: false,
            },
        ]);
        fixture.definition = profile.resolved_definition(mode_id).unwrap();
        fixture.logical_heads.clear();
        assert!(light_fixture::reconcile_logical_heads(&mut fixture));
        let children = ordered_child_ids(&fixture);
        (fixture, [children[0], children[1]])
    }

    #[test]
    fn highlight_participation_uses_logical_fixture_identities_independent_of_patch() {
        let mut fixture = schema_v2_direct_fixture().0;
        let parent = fixture.fixture_id;
        let head = light_core::FixtureId::new();
        fixture.universe = None;
        fixture.address = None;
        fixture.logical_heads = vec![light_fixture::PatchedHead {
            head_index: 1,
            fixture_id: head,
        }];
        fixture.multipatch = vec![
            light_fixture::MultiPatchInstance {
                id: Uuid::new_v4(),
                name: "First physical copy".into(),
                universe: Some(2),
                address: Some(1),
                split_patches: vec![],
                location: Default::default(),
                rotation: Default::default(),
            },
            light_fixture::MultiPatchInstance {
                id: Uuid::new_v4(),
                name: "Visualizer-only copy".into(),
                universe: None,
                address: None,
                split_patches: vec![],
                location: Default::default(),
                rotation: Default::default(),
            },
        ];

        let summaries = highlight_fixture_summaries(&[fixture.clone(), fixture]);
        assert_eq!(
            summaries
                .iter()
                .map(|summary| summary.fixture_id)
                .collect::<Vec<_>>(),
            vec![parent, head],
            "the unpatched parent participates once, multipatch copies add no step identities, and a logical head may participate independently"
        );

        let registry = HighlightRegistry::default();
        let selection = light_programmer::ProgrammerSelection {
            selected: vec![head, parent, head, parent],
            expression: Some(light_programmer::SelectionExpression::Static),
            revision: 1,
        };
        let stepped = registry
            .action(
                Uuid::new_v4(),
                light_core::UserId::new(),
                None,
                HighlightAction::Next,
                &selection,
                &summaries,
                &HashMap::new(),
                false,
            )
            .unwrap();
        assert_eq!(
            stepped
                .state
                .remembered
                .iter()
                .map(|summary| summary.fixture_id)
                .collect::<Vec<_>>(),
            vec![head, parent],
            "overlapping or duplicate selections de-duplicate without changing their first authoritative order"
        );
    }

    fn enable_highlight_test_feedback(state: &AppState) {
        *state.active_show.write() = Some(ShowEntry {
            id: light_core::ShowId::new(),
            name: "Highlight feedback test".into(),
            path: state
                .data_dir
                .join("shows/highlight-feedback-test.show")
                .display()
                .to_string(),
            revision: 0,
            updated_at: chrono::Utc::now().to_rfc3339(),
            revision_copy: None,
        });
    }

    #[test]
    fn schema_v2_direct_actions_are_channel_atomic_and_presets_are_opt_in_semantic_values() {
        let (fixture, action_id, channel_ids) = schema_v2_direct_fixture();
        let fixture_id = fixture.fixture_id;
        let snapshot = EngineSnapshot {
            fixtures: vec![fixture],
            ..EngineSnapshot::default()
        };
        let (assignments, duration, kind) =
            control_action_programmer_values(&snapshot, fixture_id, action_id, true).unwrap();
        assert_eq!(duration, None);
        assert_eq!(kind, light_fixture::ControlActionKind::Momentary);
        assert_eq!(assignments.len(), 2);
        assert_eq!(
            assignments
                .iter()
                .map(|(_, attribute, value)| (attribute.clone(), value.clone()))
                .collect::<HashMap<_, _>>(),
            HashMap::from([
                (
                    light_fixture::FixtureMode::control_action_attribute(channel_ids[0]),
                    light_core::AttributeValue::RawDmxExact(201),
                ),
                (
                    light_fixture::FixtureMode::control_action_attribute(channel_ids[1]),
                    light_core::AttributeValue::RawDmxExact(255),
                ),
            ])
        );

        let mut timed_snapshot = snapshot.clone();
        let timed_action = &mut timed_snapshot.fixtures[0]
            .definition
            .profile_snapshot
            .as_mut()
            .unwrap()
            .modes[0]
            .control_actions[0];
        timed_action.kind = light_fixture::ControlActionKind::TimedPulse;
        timed_action.duration_millis = Some(750);
        assert_eq!(
            control_action_programmer_values(&timed_snapshot, fixture_id, action_id, true)
                .unwrap()
                .1,
            Some(750)
        );

        let generated = generated_profile_presets(&snapshot, &HashSet::from([fixture_id])).unwrap();
        assert_eq!(generated.len(), 1);
        assert_eq!(generated[0].semantic_id, "gobo.dots");
        assert_eq!(generated[0].family, "Beam");
        assert_eq!(
            generated[0].values[&fixture_id][&light_core::AttributeKey("gobo.1".into())],
            light_core::AttributeValue::Discrete("gobo.dots".into())
        );
    }

    #[tokio::test(start_paused = true)]
    async fn timed_control_action_is_transient_and_reveals_latched_fan_value_at_deadline() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "timed-control-action".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        state.sessions.write().insert(session.id, session.clone());

        let (mut fixture, action_id, channel_ids) = schema_v2_direct_fixture();
        fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].control_actions[0].kind =
            light_fixture::ControlActionKind::TimedPulse;
        fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].control_actions[0]
            .duration_millis = Some(750);
        fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].control_actions[0]
            .semantic = light_fixture::ControlActionSemantic::LampOn;
        let fan_action_id = Uuid::new_v4();
        fixture.definition.profile_snapshot.as_mut().unwrap().modes[0]
            .control_actions
            .push(light_fixture::ControlAction {
                id: fan_action_id,
                name: "Fan Max".into(),
                semantic: light_fixture::ControlActionSemantic::FanMax,
                kind: light_fixture::ControlActionKind::Latched,
                duration_millis: None,
                assignments: vec![light_fixture::ControlActionAssignment {
                    channel_id: channel_ids[0],
                    active_raw: 180,
                    inactive_raw: 0,
                }],
            });
        let fixture_id = fixture.fixture_id;
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                ..EngineSnapshot::default()
            })
            .unwrap();

        let fan_response = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "fan-max".into(),
                session_id: session.id,
                expected_revision: None,
                command: "programmer.control_action".into(),
                payload: serde_json::json!({
                    "fixture_id":fixture_id,
                    "action_id":fan_action_id,
                    "active":true,
                }),
            },
        );
        assert!(fan_response.ok, "{:?}", fan_response.error);

        let response = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "timed-pulse".into(),
                session_id: session.id,
                expected_revision: None,
                command: "programmer.control_action".into(),
                payload: serde_json::json!({
                    "fixture_id":fixture_id,
                    "action_id":action_id,
                    "active":true,
                }),
            },
        );
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(
            response.payload.as_ref().unwrap()["pulse_duration_millis"],
            750
        );

        let action_attributes =
            channel_ids.map(light_fixture::FixtureMode::control_action_attribute);
        let persistent_raw_values = |programmer: &light_programmer::ProgrammerState| {
            programmer
                .values
                .iter()
                .filter_map(|value| match value.value {
                    light_core::AttributeValue::RawDmxExact(raw) => {
                        Some((value.attribute.clone(), raw))
                    }
                    _ => None,
                })
                .collect::<HashMap<_, _>>()
        };
        let transient_raw_values = |programmer: &light_programmer::ProgrammerState| {
            programmer
                .transient_values
                .iter()
                .flat_map(|action| &action.values)
                .filter_map(|value| match value.value {
                    light_core::AttributeValue::RawDmxExact(raw) => {
                        Some((value.attribute.clone(), raw))
                    }
                    _ => None,
                })
                .collect::<HashMap<_, _>>()
        };
        let persisted = || {
            let session = state
                .desk
                .lock()
                .persisted_sessions()
                .unwrap()
                .into_iter()
                .find(|persisted| persisted.id == session.id)
                .unwrap();
            serde_json::from_str::<light_programmer::ProgrammerState>(&session.programmer_json)
                .unwrap()
        };
        let expected_active = HashMap::from([
            (action_attributes[0].clone(), 201),
            (action_attributes[1].clone(), 255),
        ]);
        let expected_fan_max = HashMap::from([(action_attributes[0].clone(), 180)]);
        let programmer = state.programmers.get(session.id).unwrap();
        assert_eq!(transient_raw_values(&programmer), expected_active);
        assert_eq!(persistent_raw_values(&programmer), expected_fan_max);
        assert_eq!(persistent_raw_values(&persisted()), expected_fan_max);
        assert!(persisted().transient_values.is_empty());
        assert_eq!(
            state
                .audit_events
                .lock()
                .iter()
                .map(|event| event.kind.as_str())
                .collect::<Vec<_>>(),
            vec![
                "command_applied",
                "programmer_changed",
                "command_applied",
                "programmer_changed"
            ]
        );

        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(749)).await;
        tokio::task::yield_now().await;
        assert_eq!(persistent_raw_values(&persisted()), expected_fan_max);

        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        let programmer = state.programmers.get(session.id).unwrap();
        assert!(transient_raw_values(&programmer).is_empty());
        assert_eq!(persistent_raw_values(&programmer), expected_fan_max);
        assert_eq!(persistent_raw_values(&persisted()), expected_fan_max);
        let events = state.audit_events.lock();
        assert_eq!(events.len(), 5);
        assert_eq!(events[4].kind, "programmer_changed");
        assert_eq!(events[4].payload["action_id"], action_id.to_string());
        assert_eq!(events[4].payload["active"], false);
        assert_eq!(events[4].payload["timed_pulse_complete"], true);
        drop(events);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn explicit_profile_preset_generation_writes_portable_show_objects() {
        let (state, data_dir) = test_state();
        let (fixture, _, _) = schema_v2_direct_fixture();
        let fixture_id = fixture.fixture_id;
        let show_path = data_dir.join("shows/generated-presets.show");
        let show_id = initialise_show(&show_path, "Generated presets").unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Generated presets".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        let store = ShowStore::open(&show_path).unwrap();
        store
            .put_object(
                "patched_fixture",
                &fixture_id.0.to_string(),
                &serde_json::to_value(fixture).unwrap(),
                0,
            )
            .unwrap();
        *state.active_show.write() = Some(entry.clone());
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();
        assert!(store.objects("preset").unwrap().is_empty());
        store
            .put_object(
                "preset",
                "2.1",
                &serde_json::to_value(light_programmer::Preset {
                    name: "Red".into(),
                    family: light_programmer::PresetFamily::Color,
                    number: 1,
                    ..Default::default()
                })
                .unwrap(),
                0,
            )
            .unwrap();

        let response = generate_profile_presets(&state, vec![fixture_id]).unwrap();

        assert_eq!(response["created"][0]["name"], "Dots");
        assert_eq!(response["created"][0]["address"]["family"], "Beam");
        assert_eq!(response["created"][0]["address"]["number"], 1);
        let stored = ShowStore::open(&show_path)
            .unwrap()
            .objects("preset")
            .unwrap();
        assert_eq!(stored.len(), 2);
        assert!(stored.iter().any(|object| object.id == "2.1"
            && object.body["family"] == "Color"
            && object.body["number"] == 1));
        let generated = stored.iter().find(|object| object.id == "4.1").unwrap();
        assert_eq!(generated.body["family"], "Beam");
        assert_eq!(generated.body["number"], 1);
        assert_eq!(
            generated.body["generated_from_fixture_profile"]["semantic_id"],
            "gobo.dots"
        );
        let preset: light_programmer::Preset =
            serde_json::from_value(generated.body.clone()).unwrap();
        assert_eq!(
            preset.values[&fixture_id][&light_core::AttributeKey("gobo.1".into())],
            light_core::AttributeValue::Discrete("gobo.dots".into())
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn blind_and_preload_transitions_synchronously_suppress_live_highlight() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "highlight-safety".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        state.sessions.write().insert(session.id, session.clone());
        let fixture = schema_v2_direct_fixture().0;
        let fixture_id = fixture.fixture_id;
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                ..EngineSnapshot::default()
            })
            .unwrap();
        state.programmers.select(session.id, [fixture_id]);
        let fixtures = highlight_fixture_summaries(&state.engine.snapshot().fixtures);
        let groups = HashMap::new();
        let selection = state.programmers.selection(session.id).unwrap();
        state
            .highlight
            .action(
                session.desk.id,
                user.id,
                Some(&user.name),
                HighlightAction::On,
                &selection,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        sync_highlight_output(&state);
        assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_id]);

        let blind = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "blind".into(),
                session_id: session.id,
                expected_revision: None,
                command: "programmer.mode".into(),
                payload: serde_json::json!({"blind":true}),
            },
        );
        assert!(blind.ok, "{:?}", blind.error);
        assert!(state.engine.highlighted_fixtures().is_empty());

        state
            .programmers
            .set_modes(session.id, Some(false), None, None, None);
        state
            .highlight
            .action(
                session.desk.id,
                user.id,
                Some(&user.name),
                HighlightAction::On,
                &selection,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        sync_highlight_output(&state);
        assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_id]);

        let preview = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "preview".into(),
                session_id: session.id,
                expected_revision: None,
                command: "programmer.mode".into(),
                payload: serde_json::json!({"preview":true}),
            },
        );
        assert!(preview.ok, "{:?}", preview.error);
        assert!(state.engine.highlighted_fixtures().is_empty());
        let preview_state = current_highlight_transition(&state, &session).unwrap();
        assert!(preview_state.state.active);
        assert!(preview_state.state.capture_only);
        assert!(!preview_state.state.output_enabled);

        let leave_preview = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "leave-preview".into(),
                session_id: session.id,
                expected_revision: None,
                command: "programmer.mode".into(),
                payload: serde_json::json!({"preview":false}),
            },
        );
        assert!(leave_preview.ok, "{:?}", leave_preview.error);
        assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_id]);

        let preload = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "preload".into(),
                session_id: session.id,
                expected_revision: None,
                command: "preload.enter".into(),
                payload: serde_json::json!({}),
            },
        );
        assert!(preload.ok, "{:?}", preload.error);
        assert!(state.engine.highlighted_fixtures().is_empty());
        let state_after_preload = state.highlight.status(
            session.desk.id,
            user.id,
            Some(&user.name),
            &selection,
            &fixtures,
            &groups,
            true,
        );
        assert!(state_after_preload.state.active);
        assert!(state_after_preload.state.capture_only);
        assert!(!state_after_preload.state.output_enabled);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn authenticated_osc_highlight_adapter_feedback_dedupe_and_reconnect_are_authoritative() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (_token, session_id) = login(&app, "Operator").await;
        let session_id = SessionId(Uuid::parse_str(&session_id).unwrap());
        let session = state.sessions.read()[&session_id].clone();
        let fixtures = highlight_test_fixtures();
        let fixture_ids = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures,
                ..EngineSnapshot::default()
            })
            .unwrap();
        state.programmers.select(session.id, fixture_ids.clone());
        enable_highlight_test_feedback(&state);

        let client_id = "authenticated-highlight-hardware";
        let source = "127.0.0.1:19031";
        let subscribe = || ControlEvent::Osc {
            address: "/light/subscribe".into(),
            arguments: vec![
                OscArgument::String(client_id.into()),
                OscArgument::String(session.desk.osc_alias.clone()),
                OscArgument::Int(19032),
            ],
            source: Some(source.into()),
        };
        handle_control_event(&state, subscribe());
        assert_eq!(
            state.osc_subscribers.lock()[client_id].session_id,
            session.id,
            "OSC hardware must attach to the already authenticated desk session"
        );

        let send = |action: &str| {
            handle_control_event(
                &state,
                ControlEvent::Osc {
                    address: format!("/light/{}/highlight/{action}", session.desk.osc_alias),
                    arguments: vec![OscArgument::Bool(true)],
                    source: Some(source.into()),
                },
            );
        };
        send("on");
        assert_eq!(
            state
                .engine
                .highlighted_fixtures()
                .into_iter()
                .collect::<HashSet<_>>(),
            fixture_ids.iter().copied().collect::<HashSet<_>>()
        );

        // Model a simultaneous software press followed immediately by its attached-hardware echo.
        // The registry-level guard is the shared cross-surface authority, so only one step occurs.
        let highlight_snapshot = state.engine.snapshot();
        let fixture_summaries = highlight_fixture_summaries(&highlight_snapshot.fixtures);
        let groups = highlight_groups(&highlight_snapshot);
        let selection = state.programmers.selection(session.id).unwrap();
        let software = state
            .highlight
            .action_guarded(
                session.desk.id,
                session.user.id,
                Some(&session.user.name),
                HighlightAction::Next,
                &selection,
                &fixture_summaries,
                &groups,
                false,
            )
            .unwrap();
        apply_highlight_selection_write(&state, &session, software.working_selection.as_ref())
            .unwrap();
        assert_eq!(software.state.active_index, Some(0));
        send("next");
        let selection = state.programmers.selection(session.id).unwrap();
        let after_hardware_echo = state.highlight.status(
            session.desk.id,
            session.user.id,
            Some(&session.user.name),
            &selection,
            &fixture_summaries,
            &groups,
            false,
        );
        assert_eq!(after_hardware_echo.state.active_index, Some(0));

        // Seed item three without touching either adapter's repeat clock, then prove OSC aliases
        // share the subscriber guard: previous + prev is one physical step, not two.
        let selection = state.programmers.selection(session.id).unwrap();
        let second = state
            .highlight
            .action(
                session.desk.id,
                session.user.id,
                Some(&session.user.name),
                HighlightAction::Next,
                &selection,
                &fixture_summaries,
                &groups,
                false,
            )
            .unwrap();
        apply_highlight_selection_write(&state, &session, second.working_selection.as_ref())
            .unwrap();
        let selection = state.programmers.selection(session.id).unwrap();
        let third = state
            .highlight
            .action(
                session.desk.id,
                session.user.id,
                Some(&session.user.name),
                HighlightAction::Next,
                &selection,
                &fixture_summaries,
                &groups,
                false,
            )
            .unwrap();
        apply_highlight_selection_write(&state, &session, third.working_selection.as_ref())
            .unwrap();
        send("previous");
        send("prev");
        let selection = state.programmers.selection(session.id).unwrap();
        let after_aliases = state.highlight.status(
            session.desk.id,
            session.user.id,
            Some(&session.user.name),
            &selection,
            &fixture_summaries,
            &groups,
            false,
        );
        assert_eq!(after_aliases.state.active_index, Some(1));
        assert_eq!(after_aliases.output_fixtures, vec![fixture_ids[1]]);
        assert!(state.audit_events.lock().iter().any(|event| {
            event.kind == "highlight_changed"
                && event.payload["source"] == "osc"
                && event.payload["action"] == "previous"
        }));

        let feedback = state.osc_feedback_capture.lock();
        let prefix = format!("/light/{}/feedback/highlight", session.desk.osc_alias);
        for (suffix, arguments) in [
            ("active", vec![OscArgument::Bool(true)]),
            ("output", vec![OscArgument::Bool(true)]),
            ("mode", vec![OscArgument::String("step".into())]),
            ("index", vec![OscArgument::Int(2)]),
            ("total", vec![OscArgument::Int(3)]),
            ("can-previous", vec![OscArgument::Bool(true)]),
            ("can-next", vec![OscArgument::Bool(true)]),
        ] {
            assert!(
                feedback.iter().any(|(_, address, actual)| {
                    address == &format!("{prefix}/{suffix}") && actual == &arguments
                }),
                "missing Highlight OSC feedback for {suffix}"
            );
        }
        drop(feedback);

        handle_control_event(
            &state,
            ControlEvent::Osc {
                address: "/light/unsubscribe".into(),
                arguments: vec![OscArgument::String(client_id.into())],
                source: Some(source.into()),
            },
        );
        assert!(!state.osc_subscribers.lock().contains_key(client_id));
        handle_control_event(&state, subscribe());
        assert_eq!(
            state.osc_subscribers.lock()[client_id].session_id,
            session.id
        );
        let selection = state.programmers.selection(session.id).unwrap();
        let reconnected = state.highlight.status(
            session.desk.id,
            session.user.id,
            Some(&session.user.name),
            &selection,
            &fixture_summaries,
            &groups,
            false,
        );
        assert_eq!(reconnected.state.active_index, Some(1));
        assert_eq!(reconnected.state.remembered.len(), 3);
        assert!(reconnected.state.output_enabled);

        send("capture");
        send("reset");
        let selection = state.programmers.selection(session.id).unwrap();
        let removed_actions_ignored = state.highlight.status(
            session.desk.id,
            session.user.id,
            Some(&session.user.name),
            &selection,
            &fixture_summaries,
            &groups,
            false,
        );
        assert_eq!(removed_actions_ignored.state.active_index, Some(1));
        send("all");
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            fixture_ids
        );
        let selection = state.programmers.selection(session.id).unwrap();
        let restored = state.highlight.status(
            session.desk.id,
            session.user.id,
            Some(&session.user.name),
            &selection,
            &fixture_summaries,
            &groups,
            false,
        );
        assert_eq!(restored.state.mode, HighlightMode::Selection);
        assert!(restored.state.active);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn rest_prev_next_all_change_the_real_selection_while_high_remains_independent() {
        async fn post_highlight(app: &Router, token: &str, action: &str) -> serde_json::Value {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/api/v1/highlight/action")
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(format!(r#"{{"action":"{action}"}}"#)))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            json(response).await
        }

        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let session_id = SessionId(Uuid::parse_str(&session_id).unwrap());
        let session = state.sessions.read()[&session_id].clone();
        let fixtures = highlight_test_fixtures();
        let fixture_ids = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures,
                groups: vec![light_programmer::GroupDefinition {
                    id: "1".into(),
                    name: "Live step source".into(),
                    fixtures: fixture_ids.clone(),
                    ..Default::default()
                }],
                ..EngineSnapshot::default()
            })
            .unwrap();
        state.programmers.select(session.id, fixture_ids.clone());

        let next = post_highlight(&app, &token, "next").await;
        assert_eq!(next["active"], false);
        assert_eq!(next["mode"], "step");
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            fixture_ids[..1]
        );
        let bootstrap = app
            .clone()
            .oneshot(
                Request::get("/api/v1/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bootstrap.status(), StatusCode::OK);
        let bootstrap = json(bootstrap).await;
        let bootstrapped_highlight = bootstrap["highlight_states"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["session_id"] == session.id.0.to_string())
            .unwrap();
        assert_eq!(bootstrapped_highlight["state"]["active"], false);
        assert_eq!(bootstrapped_highlight["state"]["mode"], "step");
        assert_eq!(
            bootstrapped_highlight["state"]["remembered"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            bootstrapped_highlight["state"]["active_fixture"]["fixture_id"],
            fixture_ids[0].0.to_string()
        );
        let next_event = state
            .audit_events
            .lock()
            .iter()
            .find(|event| event.kind == "highlight_changed" && event.payload["action"] == "next")
            .cloned()
            .unwrap();
        assert_eq!(next_event.payload["state"]["active"], false);
        assert_eq!(next_event.payload["state"]["mode"], "step");
        assert_eq!(
            next_event.payload["state"]["remembered"]
                .as_array()
                .unwrap()
                .len(),
            3
        );

        let all = post_highlight(&app, &token, "all").await;
        assert_eq!(all["active"], false);
        assert_eq!(all["mode"], "selection");
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            fixture_ids
        );

        let previous = post_highlight(&app, &token, "previous").await;
        assert_eq!(previous["active"], false);
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            fixture_ids[2..]
        );
        let high = post_highlight(&app, &token, "on").await;
        assert_eq!(high["active"], true);
        assert_eq!(high["mode"], "step");
        assert_eq!(state.engine.highlighted_fixtures(), fixture_ids[2..]);

        // An external selection write resets the step basis without toggling HIGH, including when
        // the new source is live. Editing that Group before ALL is then re-resolved at action time.
        state.programmers.select_expression(
            session.id,
            fixture_ids.clone(),
            light_programmer::SelectionExpression::LiveGroup {
                group_id: "1".into(),
                rule: light_programmer::SelectionRule::All,
            },
        );
        reconcile_highlight_selection(&state, &session, "test_external_group_selection");
        assert_eq!(
            state
                .engine
                .highlighted_fixtures()
                .into_iter()
                .collect::<HashSet<_>>(),
            fixture_ids.iter().copied().collect::<HashSet<_>>()
        );
        let stepped = post_highlight(&app, &token, "next").await;
        assert_eq!(stepped["active"], true);
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            fixture_ids[..1]
        );

        let mut snapshot = (*state.engine.snapshot()).clone();
        snapshot.groups[0].fixtures = vec![fixture_ids[2], fixture_ids[1]];
        state.engine.replace_snapshot(snapshot).unwrap();
        let restored = post_highlight(&app, &token, "all").await;
        assert_eq!(restored["active"], true);
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            vec![fixture_ids[2], fixture_ids[1]]
        );
        assert!(matches!(
            state
                .programmers
                .get(session.id)
                .unwrap()
                .selection_expression,
            Some(light_programmer::SelectionExpression::LiveGroup { ref group_id, .. })
                if group_id == "1"
        ));

        // HIGH remains on with an empty actual selection, produces no output, and automatically
        // follows the next external selection without another toggle.
        state.programmers.select(session.id, []);
        reconcile_highlight_selection(&state, &session, "test_clear_selection");
        assert!(state.engine.highlighted_fixtures().is_empty());
        let status = current_highlight_transition(&state, &session).unwrap();
        assert!(status.state.active);
        state.programmers.select(session.id, [fixture_ids[1]]);
        reconcile_highlight_selection(&state, &session, "test_new_selection");
        assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_ids[1]]);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn same_user_same_desk_highlight_survives_one_session_close_and_clears_with_the_last() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (first_token, first_session_id) = login(&app, "Operator").await;
        let first_session_id = SessionId(Uuid::parse_str(&first_session_id).unwrap());
        let first_session = state.sessions.read()[&first_session_id].clone();
        let second_login = app
            .clone()
            .oneshot(
                Request::post("/api/v1/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "username":"Operator",
                            "desk_id":first_session.desk.id,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second_login.status(), StatusCode::OK);
        let second_login = json(second_login).await;
        let second_token = second_login["token"].as_str().unwrap().to_owned();
        let second_session_id =
            SessionId(Uuid::parse_str(second_login["session_id"].as_str().unwrap()).unwrap());
        let second_session = state.sessions.read()[&second_session_id].clone();
        assert_eq!(second_session.user.id, first_session.user.id);
        assert_eq!(second_session.desk.id, first_session.desk.id);

        let fixtures = highlight_test_fixtures();
        let fixture_ids = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures,
                ..EngineSnapshot::default()
            })
            .unwrap();
        state
            .programmers
            .select(first_session.id, fixture_ids.clone());
        let activated = app
            .clone()
            .oneshot(
                Request::post("/api/v1/highlight/action")
                    .header(header::AUTHORIZATION, format!("Bearer {first_token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"action":"on"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(activated.status(), StatusCode::OK);
        assert_eq!(
            state
                .engine
                .highlighted_fixtures()
                .into_iter()
                .collect::<HashSet<_>>(),
            fixture_ids.iter().copied().collect::<HashSet<_>>()
        );

        let shared = app
            .clone()
            .oneshot(
                Request::get("/api/v1/highlight")
                    .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(shared.status(), StatusCode::OK);
        let shared = json(shared).await;
        assert_eq!(shared["active"], true);
        assert_eq!(shared["remembered"].as_array().unwrap().len(), 3);

        let first_closed = app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/sessions/{}", first_session.id.0))
                    .header(header::AUTHORIZATION, format!("Bearer {first_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first_closed.status(), StatusCode::NO_CONTENT);
        let after_one_close = app
            .clone()
            .oneshot(
                Request::get("/api/v1/highlight")
                    .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let after_one_close = json(after_one_close).await;
        assert_eq!(after_one_close["active"], true);
        assert_eq!(
            state
                .engine
                .highlighted_fixtures()
                .into_iter()
                .collect::<HashSet<_>>(),
            fixture_ids.iter().copied().collect::<HashSet<_>>()
        );

        let final_closed = app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/sessions/{}", second_session.id.0))
                    .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(final_closed.status(), StatusCode::NO_CONTENT);
        let summaries = highlight_fixture_summaries(&state.engine.snapshot().fixtures);
        let selection = light_programmer::ProgrammerSelection::default();
        let cleared = state.highlight.status(
            first_session.desk.id,
            first_session.user.id,
            Some(&first_session.user.name),
            &selection,
            &summaries,
            &HashMap::new(),
            false,
        );
        assert!(!cleared.state.active);
        assert!(cleared.state.remembered.is_empty());
        assert!(cleared.output_fixtures.is_empty());
        assert!(state.engine.highlighted_fixtures().is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn invalid_active_show_enters_recovery_instead_of_aborting_startup() {
        let engine = Engine::new(ProgrammerRegistry::default());
        let entry = ShowEntry {
            id: light_core::ShowId::new(),
            name: "Damaged Show".into(),
            path: std::env::temp_dir()
                .join(format!("missing-{}.show", Uuid::new_v4()))
                .display()
                .to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        let error = compile_active_show_for_startup(&engine, &entry)
            .expect("invalid show should enter recovery mode");
        assert!(error.contains("might be corrupted or incompatible"));
        assert!(error.contains("Damaged Show"));
        assert_eq!(engine.snapshot().fixtures.len(), 0);
    }
    #[test]
    fn repeated_group_command_freezes_membership_while_live_reference_refreshes() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "test".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        let first = light_core::FixtureId::new();
        let second = light_core::FixtureId::new();
        let third = light_core::FixtureId::new();
        let snapshot = |members| EngineSnapshot {
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Group 1".into(),
                fixtures: members,
                ..Default::default()
            }],
            ..Default::default()
        };
        state
            .engine
            .replace_snapshot(snapshot(vec![first, second]))
            .unwrap();
        assert_eq!(
            execute_programmer_command(&state, &session, "GROUP GROUP 1").unwrap(),
            2
        );
        state
            .engine
            .replace_snapshot(snapshot(vec![first, second, third]))
            .unwrap();
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            vec![first, second]
        );
        assert!(execute_programmer_command(&state, &session, "GROUP GROUP 2").is_err());
        execute_programmer_command(&state, &session, "GROUP 1").unwrap();
        state
            .engine
            .replace_snapshot(snapshot(vec![third]))
            .unwrap();
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            vec![third]
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mixed_selection_sources_dereference_only_the_addressed_term_and_replay_left_to_right() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "test".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);

        let show_path = data_dir.join("shows/mixed-selection.show");
        let show_id = default_show::initialise(&show_path).unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Mixed selection".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        let mut snapshot = load_engine_snapshot(&entry).unwrap();
        let fixture = |number| {
            snapshot
                .fixtures
                .iter()
                .find(|fixture| fixture.fixture_number == Some(number))
                .unwrap()
                .fixture_id
        };
        let fixtures = [1, 2, 3, 4, 5, 6, 101, 102, 103]
            .into_iter()
            .map(fixture)
            .collect::<Vec<_>>();
        snapshot.groups = vec![
            light_programmer::GroupDefinition {
                id: "3".into(),
                name: "Front".into(),
                fixtures: fixtures[..4].to_vec(),
                ..Default::default()
            },
            light_programmer::GroupDefinition {
                id: "5".into(),
                name: "Back".into(),
                fixtures: fixtures[4..8].to_vec(),
                ..Default::default()
            },
        ];
        state.engine.replace_snapshot(snapshot.clone()).unwrap();

        assert_eq!(
            execute_programmer_command(&state, &session, "DEGRP 3 + G5").unwrap(),
            8
        );
        let mixed = state.programmers.get(session.id).unwrap();
        assert_eq!(mixed.selected, fixtures[..8]);
        let Some(light_programmer::SelectionExpression::Sources { items }) =
            mixed.selection_expression
        else {
            panic!("mixed command must retain ordered sources")
        };
        assert_eq!(items.len(), 5);
        assert!(
            items[..4]
                .iter()
                .all(|item| matches!(item, light_programmer::SelectionReference::Fixture { .. }))
        );
        assert_eq!(
            items[4],
            light_programmer::SelectionReference::LiveGroup {
                group_id: "5".into()
            }
        );

        snapshot.groups[1].fixtures = vec![fixtures[8], fixtures[4]];
        state.engine.replace_snapshot(snapshot).unwrap();
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            vec![
                fixtures[0],
                fixtures[1],
                fixtures[2],
                fixtures[3],
                fixtures[8],
                fixtures[4]
            ]
        );

        execute_programmer_command(&state, &session, "G3 - F2 + F2").unwrap();
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            vec![fixtures[0], fixtures[2], fixtures[3], fixtures[1]]
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn set_group_requests_properties_only_for_the_originating_desk() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "test".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                groups: vec![light_programmer::GroupDefinition {
                    id: "4".into(),
                    name: "Center Spot".into(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            execute_programmer_command(&state, &session, "SET GROUP 4").unwrap(),
            0
        );
        let event = state.audit_events.lock().back().cloned().unwrap();
        assert_eq!(event.kind, "group_configuration_requested");
        assert_eq!(event.payload["group_id"], "4");
        assert_eq!(event.payload["desk_id"], session.desk.id.to_string());
        assert!(execute_programmer_command(&state, &session, "SET GROUP 99").is_err());
        assert!(execute_programmer_command(&state, &session, "SET GROUP 4 EXTRA").is_err());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn record_group_supports_overwrite_merge_subtract_and_empty_source_delete() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "test".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        let show_path = data_dir.join("shows/record-group.show");
        let show_id = initialise_show(&show_path, "Record Group").unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Record Group".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        let fixtures = (0..4)
            .map(|_| light_core::FixtureId::new())
            .collect::<Vec<_>>();
        let store = ShowStore::open(&show_path).unwrap();
        store
            .put_object(
                "group",
                "3",
                &serde_json::to_value(light_programmer::GroupDefinition {
                    id: "3".into(),
                    name: "Kept name".into(),
                    fixtures: fixtures[..2].to_vec(),
                    master: 0.4,
                    ..Default::default()
                })
                .unwrap(),
                0,
            )
            .unwrap();
        *state.active_show.write() = Some(entry.clone());
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();

        state.programmers.select_expression(
            session.id,
            fixtures[..3].to_vec(),
            light_programmer::SelectionExpression::LiveGroup {
                group_id: "3".into(),
                rule: light_programmer::SelectionRule::All,
            },
        );
        execute_programmer_command(&state, &session, "RECORD GROUP 3").unwrap();
        let read_group = || {
            let object = ShowStore::open(&show_path)
                .unwrap()
                .objects("group")
                .unwrap()
                .into_iter()
                .find(|object| object.id == "3")
                .unwrap();
            serde_json::from_value::<light_programmer::GroupDefinition>(object.body).unwrap()
        };
        let overwritten = read_group();
        assert_eq!(overwritten.fixtures, fixtures[..3]);
        assert_eq!(overwritten.name, "Kept name");
        assert_eq!(overwritten.master, 0.4);
        assert!(overwritten.derived_from.is_none());

        let group_3_revision = ShowStore::open(&show_path)
            .unwrap()
            .objects("group")
            .unwrap()
            .into_iter()
            .find(|object| object.id == "3")
            .unwrap()
            .revision;
        ShowStore::open(&show_path)
            .unwrap()
            .put_object(
                "group",
                "4",
                &serde_json::to_value(light_programmer::GroupDefinition {
                    id: "4".into(),
                    name: "Derived from 3".into(),
                    derived_from: Some(light_programmer::DerivedGroup {
                        source_group_id: "3".into(),
                        rule: light_programmer::SelectionRule::All,
                    }),
                    ..Default::default()
                })
                .unwrap(),
                0,
            )
            .unwrap();
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();
        state.programmers.select_expression(
            session.id,
            fixtures[..3].to_vec(),
            light_programmer::SelectionExpression::LiveGroup {
                group_id: "4".into(),
                rule: light_programmer::SelectionRule::All,
            },
        );
        execute_programmer_command(&state, &session, "RECORD GROUP 3").unwrap();
        assert!(read_group().derived_from.is_none());
        assert!(
            ShowStore::open(&show_path)
                .unwrap()
                .objects("group")
                .unwrap()
                .into_iter()
                .find(|object| object.id == "3")
                .unwrap()
                .revision
                > group_3_revision
        );

        state.programmers.select(session.id, []);
        assert!(execute_programmer_command(&state, &session, "RECORD - GROUP 3").is_err());
        execute_programmer_command(&state, &session, "DELETE GROUP 4").unwrap();

        state
            .programmers
            .select(session.id, [fixtures[2], fixtures[3]]);
        execute_programmer_command(&state, &session, "RECORD + GROUP 3").unwrap();
        assert_eq!(read_group().fixtures, fixtures);

        state
            .programmers
            .select(session.id, [fixtures[1], fixtures[3]]);
        execute_programmer_command(&state, &session, "RECORD - GROUP 3").unwrap();
        assert_eq!(read_group().fixtures, vec![fixtures[0], fixtures[2]]);

        state.programmers.select(session.id, []);
        execute_programmer_command(&state, &session, "RECORD - GROUP 3").unwrap();
        assert!(
            ShowStore::open(&show_path)
                .unwrap()
                .objects("group")
                .unwrap()
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn command_line_contract_supports_subsets_preset_lifecycle_and_cue_list_creation() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let control_desk = state.desk.lock().add_desk("Commands", "commands").unwrap();
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "test".into(),
            connected: true,
            desk: control_desk,
        };
        state.programmers.start(session.id, user.id);
        let show_path = data_dir.join("shows/commands.show");
        let show_id = initialise_show(&show_path, "Commands").unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Commands".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        let store = ShowStore::open(&show_path).unwrap();
        let group = light_programmer::GroupDefinition {
            id: "1".into(),
            name: "Group 1".into(),
            ..Default::default()
        };
        store
            .put_object("group", "1", &serde_json::to_value(group).unwrap(), 0)
            .unwrap();
        *state.active_show.write() = Some(entry.clone());
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();
        execute_programmer_command(&state, &session, "GROUP 1 DIV 2 + 1").unwrap();
        execute_programmer_command(&state, &session, "GROUP 1 AT 50 DELAY 1 TIME 2").unwrap();
        let programmer = state.programmers.get(session.id).unwrap();
        let timed_group = &programmer.group_values["1"][&light_core::AttributeKey::intensity()];
        assert_eq!(timed_group.fade_millis, Some(2_000));
        assert_eq!(timed_group.delay_millis, Some(1_000));

        let preset_fixture = light_core::FixtureId::new();
        state.programmers.set(
            session.id,
            preset_fixture,
            light_core::AttributeKey("pan".into()),
            light_core::AttributeValue::Normalized(0.4),
        );
        execute_programmer_command(&state, &session, "RECORD 0.1").unwrap();
        execute_programmer_command(&state, &session, "RECORD 1.1").unwrap();
        let intensity_preset: light_programmer::Preset = serde_json::from_value(
            ShowStore::open(&show_path)
                .unwrap()
                .objects("preset")
                .unwrap()
                .into_iter()
                .find(|object| object.id == "1.1")
                .unwrap()
                .body,
        )
        .unwrap();
        assert_eq!(
            intensity_preset.family,
            light_programmer::PresetFamily::Intensity
        );
        assert!(intensity_preset.values.values().all(|attributes| {
            attributes
                .keys()
                .all(light_core::AttributeKey::is_intensity)
        }));
        execute_programmer_command(&state, &session, "DELETE 1.1").unwrap();
        execute_programmer_command(&state, &session, "COPY 0.1 AT 2").unwrap();
        execute_programmer_command(&state, &session, "MOVE 0.2 AT 3").unwrap();
        execute_programmer_command(&state, &session, "DELETE 0.1").unwrap();
        let preset_ids = ShowStore::open(&show_path)
            .unwrap()
            .objects("preset")
            .unwrap()
            .into_iter()
            .map(|object| object.id)
            .collect::<Vec<_>>();
        assert_eq!(preset_ids, vec!["0.3"]);

        execute_programmer_command(&state, &session, "RECORD SET 25 TIME 3 DELAY 1.5").unwrap();
        execute_programmer_command(&state, &session, "RECORD SET 25 CUE 2.5").unwrap();
        let snapshot = state.engine.snapshot();
        let (_, _, cue_list) =
            cue_list_for_playback(&ShowStore::open(&show_path).unwrap(), &snapshot, 25).unwrap();
        assert_eq!(
            cue_list
                .cues
                .iter()
                .map(|cue| cue.number)
                .collect::<Vec<_>>(),
            vec![1.0, 2.5]
        );
        assert_eq!(cue_list.cues[0].fade_millis, 3_000);
        assert_eq!(cue_list.cues[0].delay_millis, 0);
        assert!(matches!(
            cue_list.cues[0].trigger,
            light_playback::CueTrigger::Wait {
                delay_millis: 1_500
            }
        ));
        assert_eq!(cue_list.cues[0].group_changes[0].fade_millis, Some(2_000));
        assert_eq!(cue_list.cues[0].group_changes[0].delay_millis, Some(1_000));
        execute_programmer_command(&state, &session, "RECORD SET 25 CUE 2.5 DELAY 0").unwrap();
        let (_, _, cue_list) = cue_list_for_playback(
            &ShowStore::open(&show_path).unwrap(),
            &state.engine.snapshot(),
            25,
        )
        .unwrap();
        assert!(matches!(
            cue_list
                .cues
                .iter()
                .find(|cue| cue.number == 2.5)
                .unwrap()
                .trigger,
            light_playback::CueTrigger::Follow { delay_millis: 0 }
        ));

        state
            .desk
            .lock()
            .set_selected_playback(session.desk.id, show_id, Some(25))
            .unwrap();
        execute_programmer_command(&state, &session, "RECORD CUE 7").unwrap();
        let (_, _, selected_list) = cue_list_for_playback(
            &ShowStore::open(&show_path).unwrap(),
            &state.engine.snapshot(),
            25,
        )
        .unwrap();
        assert!(selected_list.cues.iter().any(|cue| cue.number == 7.0));

        let color = light_core::AttributeKey("color.emitter.red".into());
        let set_only_color = || {
            let mut programmer = state.programmers.get(session.id).unwrap();
            programmer.values.clear();
            programmer.group_values.clear();
            state.programmers.restore(programmer);
            assert!(state.programmers.set_group(
                session.id,
                "1".into(),
                color.clone(),
                light_core::AttributeValue::Normalized(0.5),
            ));
        };
        set_only_color();
        execute_programmer_command(&state, &session, "RECORD + SET 25 CUE 2.5").unwrap();
        let (_, _, cue_list) = cue_list_for_playback(
            &ShowStore::open(&show_path).unwrap(),
            &state.engine.snapshot(),
            25,
        )
        .unwrap();
        let merged = cue_list.cues.iter().find(|cue| cue.number == 2.5).unwrap();
        assert_eq!(merged.group_changes.len(), 2);

        execute_programmer_command(&state, &session, "RECORD - SET 25 CUE 2.5").unwrap();
        let (_, _, cue_list) = cue_list_for_playback(
            &ShowStore::open(&show_path).unwrap(),
            &state.engine.snapshot(),
            25,
        )
        .unwrap();
        let subtracted = cue_list.cues.iter().find(|cue| cue.number == 2.5).unwrap();
        assert_eq!(subtracted.group_changes.len(), 1);
        assert_eq!(
            subtracted.group_changes[0].attribute,
            light_core::AttributeKey::intensity()
        );

        set_only_color();
        execute_programmer_command(&state, &session, "RECORD SET 25 CUE 2.5").unwrap();
        let (_, _, cue_list) = cue_list_for_playback(
            &ShowStore::open(&show_path).unwrap(),
            &state.engine.snapshot(),
            25,
        )
        .unwrap();
        let overwritten = cue_list.cues.iter().find(|cue| cue.number == 2.5).unwrap();
        assert_eq!(overwritten.group_changes.len(), 1);
        assert_eq!(overwritten.group_changes[0].attribute, color);

        let mut programmer = state.programmers.get(session.id).unwrap();
        programmer.values.clear();
        programmer.group_values.clear();
        state.programmers.restore(programmer);
        execute_programmer_command(&state, &session, "RECORD - SET 25 CUE 2.5").unwrap();
        let (_, _, cue_list) = cue_list_for_playback(
            &ShowStore::open(&show_path).unwrap(),
            &state.engine.snapshot(),
            25,
        )
        .unwrap();
        assert_eq!(
            cue_list
                .cues
                .iter()
                .map(|cue| cue.number)
                .collect::<Vec<_>>(),
            vec![1.0, 7.0]
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn spd_grp_commands_preserve_precision_mapping_relative_changes_and_phase_links() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user,
            token: "speed-command".into(),
            connected: true,
            desk: test_control_desk(),
        };

        execute_programmer_command(&state, &session, "SPD GRP 1 AT 120").unwrap();
        execute_programmer_command(&state, &session, "SPD GRP 2 AT 127,5").unwrap();
        execute_programmer_command(&state, &session, "SPD GRP 3 AT 130").unwrap();
        execute_programmer_command(&state, &session, "SPD GRP 4 AT 140").unwrap();
        execute_programmer_command(&state, &session, "SPD GRP 5 AT 150").unwrap();
        assert_eq!(
            state.configuration.read().speed_groups_bpm,
            [120.0, 127.5, 130.0, 140.0, 150.0]
        );

        execute_programmer_command(&state, &session, "SPD GRP 1 AT + 5").unwrap();
        assert_eq!(state.configuration.read().speed_groups_bpm[0], 125.0);
        execute_programmer_command(&state, &session, "SPD GRP 1 AT - 5").unwrap();
        assert_eq!(state.configuration.read().speed_groups_bpm[0], 120.0);
        assert_eq!(state.configuration.read().speed_groups_bpm[1], 127.5);

        execute_programmer_command(&state, &session, "SPD GRP 1 AT SPD GRP 3").unwrap();
        {
            let controllers = state.speed_groups.lock();
            assert_eq!(controllers[0].manual_bpm(), 120.0);
            assert_eq!(controllers[2].manual_bpm(), 120.0);
            assert_eq!(controllers[0].synchronized_with(), Some(3));
            assert_eq!(controllers[2].synchronized_with(), Some(1));
            let now = application_millis(&state).saturating_add(18_750);
            let source = controllers[0].snapshot(now);
            let target = controllers[2].snapshot(now);
            assert_eq!(source.phase_origin_millis, target.phase_origin_millis);
            assert!((source.beat_phase - target.beat_phase).abs() < f64::EPSILON);
        }

        execute_programmer_command(&state, &session, "SPD GRP 3 AT 90").unwrap();
        {
            let controllers = state.speed_groups.lock();
            assert_eq!(controllers[0].manual_bpm(), 120.0);
            assert_eq!(controllers[2].manual_bpm(), 90.0);
            assert_eq!(controllers[0].synchronized_with(), None);
            assert_eq!(controllers[2].synchronized_with(), None);
        }

        execute_programmer_command(&state, &session, "SPD GRP 1 AT SPD GRP 3").unwrap();
        let tap_start = application_millis(&state).saturating_add(1_000);
        {
            let mut controllers = state.speed_groups.lock();
            let retained_peer_bpm = controllers[2].manual_bpm();
            unlink_speed_group(&mut controllers, 0, tap_start);
            assert!(matches!(
                controllers[0].tap_learn(tap_start),
                light_control::speed::LearnResult::Armed
            ));
            assert!(matches!(
                controllers[0].tap_learn(tap_start + 400),
                light_control::speed::LearnResult::Learned { .. }
            ));
            assert_eq!(controllers[0].manual_bpm(), 150.0);
            assert_eq!(controllers[2].manual_bpm(), retained_peer_bpm);
            assert_eq!(controllers[0].synchronized_with(), None);
            assert_eq!(controllers[2].synchronized_with(), None);
            copy_speed_group_runtime_to_configuration(&state, &controllers, &[0]);
        }
        assert_eq!(state.configuration.read().speed_groups_bpm[0], 150.0);
        assert_eq!(state.configuration.read().speed_groups_bpm[2], 120.0);
        assert!(execute_programmer_command(&state, &session, "SPD GRP 0 AT 120").is_err());
        assert!(execute_programmer_command(&state, &session, "SPD GRP 6 AT 120").is_err());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn cue_move_copy_requires_a_choice_and_preserves_plain_status_and_move_copy_axes() {
        let setup = || {
            let (state, data_dir) = test_state();
            let user = state.desk.lock().users().unwrap().remove(0);
            let desk = state
                .desk
                .lock()
                .add_desk("Cue transfer", "cue-transfer")
                .unwrap();
            let session = Session {
                id: SessionId::new(),
                user: user.clone(),
                token: "cue-transfer".into(),
                connected: true,
                desk,
            };
            state.programmers.start(session.id, user.id);

            let first = light_core::FixtureId::new();
            let second = light_core::FixtureId::new();
            let untouched = light_core::FixtureId::new();
            let intensity = light_core::AttributeKey::intensity();
            let mut source_one = light_playback::Cue::new(1.0);
            source_one.changes.push(light_playback::CueChange::set(
                first,
                intensity.clone(),
                light_core::AttributeValue::Normalized(1.0),
            ));
            source_one
                .group_changes
                .push(light_playback::GroupCueChange {
                    group_id: "1".into(),
                    attribute: intensity.clone(),
                    value: Some(light_core::AttributeValue::Normalized(1.0)),
                    automatic_restore: false,
                    fade_millis: None,
                    delay_millis: None,
                });
            let mut source_two = light_playback::Cue::new(2.0);
            source_two.changes.push(light_playback::CueChange::set(
                second,
                intensity.clone(),
                light_core::AttributeValue::Normalized(1.0),
            ));
            source_two
                .group_changes
                .push(light_playback::GroupCueChange {
                    group_id: "2".into(),
                    attribute: intensity.clone(),
                    value: Some(light_core::AttributeValue::Normalized(1.0)),
                    automatic_restore: false,
                    fade_millis: None,
                    delay_millis: None,
                });
            let source_two_id = source_two.id;
            let mut source_three = light_playback::Cue::new(3.0);
            source_three.changes.push(light_playback::CueChange::set(
                first,
                intensity.clone(),
                light_core::AttributeValue::Normalized(0.0),
            ));
            let mut destination_one = light_playback::Cue::new(1.0);
            destination_one.changes.push(light_playback::CueChange::set(
                first,
                intensity.clone(),
                light_core::AttributeValue::Normalized(0.0),
            ));
            destination_one.changes.push(light_playback::CueChange::set(
                untouched,
                intensity.clone(),
                light_core::AttributeValue::Normalized(1.0),
            ));
            destination_one.group_changes.extend([
                light_playback::GroupCueChange {
                    group_id: "1".into(),
                    attribute: intensity.clone(),
                    value: Some(light_core::AttributeValue::Normalized(0.0)),
                    automatic_restore: false,
                    fade_millis: None,
                    delay_millis: None,
                },
                light_playback::GroupCueChange {
                    group_id: "3".into(),
                    attribute: intensity.clone(),
                    value: Some(light_core::AttributeValue::Normalized(1.0)),
                    automatic_restore: false,
                    fade_millis: None,
                    delay_millis: None,
                },
            ]);

            let list = |id, name: &str, cues| light_playback::CueList {
                id,
                name: name.into(),
                priority: 0,
                mode: light_playback::CueListMode::Sequence,
                looped: false,
                chaser_step_millis: 1_000,
                speed_group: None,
                intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
                wrap_mode: Some(light_playback::WrapMode::Off),
                restart_mode: light_playback::RestartMode::FirstCue,
                force_cue_timing: false,
                disable_cue_timing: false,
                chaser_xfade_millis: 0,
                chaser_xfade_percent: Some(0),
                speed_multiplier: 1.0,
                cues,
            };
            let source_id = light_core::CueListId::new();
            let destination_id = light_core::CueListId::new();
            let source = list(
                source_id,
                "Source",
                vec![source_one, source_two, source_three],
            );
            let destination = list(destination_id, "Destination", vec![destination_one]);
            let playback = |number, cue_list_id| light_playback::PlaybackDefinition {
                number,
                name: format!("Cuelist {number}"),
                target: light_playback::PlaybackTarget::CueList { cue_list_id },
                buttons: [light_playback::PlaybackButtonAction::None; 3],
                button_count: 3,
                fader: light_playback::PlaybackFaderMode::Master,
                has_fader: true,
                go_activates: true,
                auto_off: false,
                xfade_millis: 0,
                color: "#20c997".into(),
                flash_release: light_playback::FlashReleaseMode::ReleaseAll,
                protect_from_swap: false,
                presentation_icon: None,
                presentation_image: None,
            };

            let show_path = data_dir.join("shows/cue-transfer.show");
            let show_id = initialise_show(&show_path, "Cue transfer").unwrap();
            let store = ShowStore::open(&show_path).unwrap();
            store
                .put_object(
                    "cue_list",
                    &source_id.0.to_string(),
                    &serde_json::to_value(source).unwrap(),
                    0,
                )
                .unwrap();
            store
                .put_object(
                    "cue_list",
                    &destination_id.0.to_string(),
                    &serde_json::to_value(destination).unwrap(),
                    0,
                )
                .unwrap();
            for definition in [playback(1, source_id), playback(2, destination_id)] {
                store
                    .put_object(
                        "playback",
                        &definition.number.to_string(),
                        &serde_json::to_value(&definition).unwrap(),
                        0,
                    )
                    .unwrap();
            }
            let entry = ShowEntry {
                id: show_id,
                name: "Cue transfer".into(),
                path: show_path.display().to_string(),
                revision: 0,
                updated_at: String::new(),
                revision_copy: None,
            };
            *state.active_show.write() = Some(entry.clone());
            state
                .engine
                .replace_snapshot(load_engine_snapshot(&entry).unwrap())
                .unwrap();
            (
                state,
                data_dir,
                session,
                show_path,
                [first, second, untouched],
                source_two_id,
            )
        };

        for (operation, mode, moves, status) in [
            ("COPY", "PLAIN", false, false),
            ("MOVE", "PLAIN", true, false),
            ("COPY", "STATUS", false, true),
            ("MOVE", "STATUS", true, true),
        ] {
            let (state, data_dir, session, show_path, fixtures, source_cue_id) = setup();
            let before_store = ShowStore::open(&show_path).unwrap();
            let (_, source_before, _) =
                cue_list_for_playback(&before_store, &state.engine.snapshot(), 1).unwrap();
            let (_, destination_before, _) =
                cue_list_for_playback(&before_store, &state.engine.snapshot(), 2).unwrap();

            if operation == "COPY" && mode == "PLAIN" {
                let response = dispatch_ws_command(
                    &state,
                    &session,
                    WsCommand {
                        protocol_version: 1,
                        request_id: "pending-copy".into(),
                        session_id: session.id,
                        expected_revision: None,
                        command: "programmer.execute".into(),
                        payload: serde_json::json!({
                            "value":"COPY SET 1 CUE 2 AT SET 2 CUE 2"
                        }),
                    },
                );
                assert!(response.ok);
                let pending = &response.payload.unwrap()["pending_choice"];
                assert_eq!(pending["type"], "cue_move_copy");
                assert_eq!(pending["options"][0]["label"], "Plain Copy");
                assert_eq!(pending["options"][1]["label"], "Status Copy");
                assert_eq!(pending["cancel_label"], "Cancel");
                assert!(
                    execute_programmer_command(&state, &session, "COPY SET 1 CUE 2 AT SET 2 CUE 2")
                        .is_err()
                );
                let unchanged = ShowStore::open(&show_path).unwrap();
                let (_, source, _) =
                    cue_list_for_playback(&unchanged, &state.engine.snapshot(), 1).unwrap();
                let (_, destination, _) =
                    cue_list_for_playback(&unchanged, &state.engine.snapshot(), 2).unwrap();
                assert_eq!(source.body, source_before.body);
                assert_eq!(destination.body, destination_before.body);
            }

            execute_programmer_command(
                &state,
                &session,
                &format!("{operation} {mode} SET 1 CUE 2 AT SET 2 CUE 2"),
            )
            .unwrap();
            let store = ShowStore::open(&show_path).unwrap();
            let (_, source_object, source) =
                cue_list_for_playback(&store, &state.engine.snapshot(), 1).unwrap();
            let (_, destination_object, destination) =
                cue_list_for_playback(&store, &state.engine.snapshot(), 2).unwrap();
            if moves {
                assert_eq!(source.cues.len(), 2);
                assert!(source.cues.iter().all(|cue| cue.number != 2.0));
                assert!(source_object.revision > source_before.revision);
                let remaining = source.state_at_number(3.0);
                assert_eq!(
                    remaining.get(&(fixtures[0], light_core::AttributeKey::intensity())),
                    Some(&light_core::AttributeValue::Normalized(0.0))
                );
                assert!(
                    !remaining.contains_key(&(fixtures[1], light_core::AttributeKey::intensity()))
                );
            } else {
                assert_eq!(source_object.body, source_before.body);
                assert_eq!(source_object.revision, source_before.revision);
            }
            assert!(destination_object.revision > destination_before.revision);
            assert_eq!(
                destination
                    .cues
                    .iter()
                    .map(|cue| cue.number)
                    .collect::<Vec<_>>(),
                vec![1.0, 2.0]
            );
            let transferred = destination
                .cues
                .iter()
                .find(|cue| cue.number == 2.0)
                .unwrap();
            assert_eq!(transferred.id == source_cue_id, moves);
            assert_eq!(transferred.changes.len(), if status { 2 } else { 1 });
            assert_eq!(transferred.group_changes.len(), if status { 2 } else { 1 });
            assert!(
                transferred
                    .changes
                    .iter()
                    .all(|change| change.fixture_id != fixtures[2])
            );
            assert!(
                transferred
                    .group_changes
                    .iter()
                    .all(|change| change.group_id != "3")
            );

            let replayed = destination.state_at_number(2.0);
            assert_eq!(
                replayed.get(&(fixtures[0], light_core::AttributeKey::intensity())),
                Some(&light_core::AttributeValue::Normalized(if status {
                    1.0
                } else {
                    0.0
                }))
            );
            assert_eq!(
                replayed.get(&(fixtures[1], light_core::AttributeKey::intensity())),
                Some(&light_core::AttributeValue::Normalized(1.0))
            );
            assert_eq!(
                replayed.get(&(fixtures[2], light_core::AttributeKey::intensity())),
                Some(&light_core::AttributeValue::Normalized(1.0))
            );
            let _ = std::fs::remove_dir_all(data_dir);
        }
    }

    #[test]
    fn cue_addresses_use_cue_for_pool_and_page_playbacks() {
        let snapshot = EngineSnapshot {
            playback_pages: vec![light_playback::PlaybackPage {
                number: 4,
                name: "Page 4".into(),
                slots: HashMap::from([(7, 25)]),
            }],
            ..Default::default()
        };
        let pool = ["SET", "25", "CUE", "2", ".", "5"].map(String::from);
        let (address, used) = parse_playback_address(&pool, true, &snapshot).unwrap();
        assert_eq!((address.playback, address.cue, used), (25, Some(2.5), 6));
        let pool_only = ["SET", "25"].map(String::from);
        let (address, used) = parse_playback_address(&pool_only, true, &snapshot).unwrap();
        assert_eq!((address.playback, address.cue, used), (25, None, 2));
        let page = ["SET", "4", ".", "7", "CUE", "12"].map(String::from);
        let (address, used) = parse_playback_address(&page, true, &snapshot).unwrap();
        assert_eq!((address.playback, address.cue, used), (25, Some(12.0), 6));
        let page_only = ["SET", "4", ".", "7"].map(String::from);
        let (address, used) = parse_playback_address(&page_only, true, &snapshot).unwrap();
        assert_eq!((address.playback, address.cue, used), (25, None, 4));
        let old_entangled = ["SET", "4", "SET", "7", ".", "12"].map(String::from);
        let (_, used) = parse_playback_address(&old_entangled, true, &snapshot).unwrap();
        assert_ne!(used, old_entangled.len());
    }

    #[test]
    fn update_addresses_keep_current_page_and_explicit_page_distinct() {
        let snapshot = EngineSnapshot {
            playback_pages: vec![
                light_playback::PlaybackPage {
                    number: 1,
                    name: "Page 1".into(),
                    slots: HashMap::from([(7, 11)]),
                },
                light_playback::PlaybackPage {
                    number: 4,
                    name: "Page 4".into(),
                    slots: HashMap::from([(7, 25)]),
                },
            ],
            ..Default::default()
        };
        let current = ["SET", "7", "CUE", "2", ".", "5"].map(String::from);
        let explicit = ["SET", "1", ".", "7", "CUE", "2", ".", "5"].map(String::from);

        let page_one = parse_update_playback_address(&current, 1, &snapshot).unwrap();
        let page_four = parse_update_playback_address(&current, 4, &snapshot).unwrap();
        let pinned = parse_update_playback_address(&explicit, 4, &snapshot).unwrap();

        assert_eq!((page_one.playback, page_one.cue), (11, Some(2.5)));
        assert_eq!((page_four.playback, page_four.cue), (25, Some(2.5)));
        assert_eq!((pinned.playback, pinned.cue), (11, Some(2.5)));
    }

    #[test]
    fn command_line_update_enter_applies_the_configured_group_default_directly() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "update-enter-default".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        state.sessions.write().insert(session.id, session.clone());

        let first = light_core::FixtureId::new();
        let added = light_core::FixtureId::new();
        let group = light_programmer::GroupDefinition {
            id: "981".into(),
            name: "Enter Update".into(),
            fixtures: vec![first],
            ..Default::default()
        };
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                groups: vec![group.clone()],
                ..Default::default()
            })
            .unwrap();
        state.programmers.select(session.id, [first, added]);
        state.configuration.write().update_settings_by_desk.insert(
            session.desk.id,
            update::UpdateSettings {
                group_mode: update::ExistingContentMode::AddNew,
                show_update_modal_on_touch: true,
                ..Default::default()
            },
        );

        let show_path = data_dir.join("shows/update-enter-default.show");
        let show_id = initialise_show(&show_path, "Update Enter default").unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Update Enter default".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        *state.active_show.write() = Some(entry);
        let store = ShowStore::open(&show_path).unwrap();
        store
            .put_object("group", "981", &serde_json::to_value(&group).unwrap(), 0)
            .unwrap();

        assert_eq!(
            execute_programmer_command(&state, &session, "UPDATE GROUP 981").unwrap(),
            1
        );
        let updated = serde_json::from_value::<light_programmer::GroupDefinition>(
            stored_update_object(&store, "group", "981").unwrap().body,
        )
        .unwrap();
        assert_eq!(updated.fixtures, vec![first, added]);
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            vec![first, added]
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn touched_update_target_rejects_a_changed_playback_context_but_explicit_cue_remains_pinned() {
        let cue_list_id = light_core::CueListId::new();
        let first_cue = Uuid::new_v4();
        let second_cue = Uuid::new_v4();
        let active = vec![update::ActiveCueContext {
            playback_number: 7,
            cue_list_id,
            cue_id: second_cue,
            cue_number: 2.0,
        }];
        let touched = UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(cue_list_id.0.to_string()),
            playback_number: Some(7),
            cue_id: Some(first_cue),
            cue_number: Some(1.0),
            validate_active_context: true,
        };
        let error = resolve_update_cue_target(&touched, &active).unwrap_err();
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert!(error.message.contains("context changed"));

        let explicit = UpdateApiTarget {
            validate_active_context: false,
            ..touched
        };
        assert_eq!(
            resolve_update_cue_target(&explicit, &active)
                .unwrap()
                .cue_id,
            first_cue
        );
    }

    #[test]
    fn confirmed_update_rejects_changed_programmer_and_is_one_step_undoable() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "update-confirmation".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        state.sessions.write().insert(session.id, session.clone());

        let fixture = light_core::FixtureId::new();
        let cue_list_id = light_core::CueListId::new();
        let mut first = light_playback::Cue::new(1.0);
        first.changes.push(light_playback::CueChange::set(
            fixture,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(0.2),
        ));
        let mut second = light_playback::Cue::new(2.0);
        second.changes.push(light_playback::CueChange::set(
            fixture,
            light_core::AttributeKey("color.red".into()),
            light_core::AttributeValue::Normalized(0.3),
        ));
        let third = light_playback::Cue::new(3.0);
        let cue_list = light_playback::CueList {
            id: cue_list_id,
            name: "Update undo".into(),
            priority: 0,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![first, second, third],
        };
        let playback = light_playback::PlaybackDefinition {
            number: 7,
            name: "Update playback".into(),
            target: light_playback::PlaybackTarget::CueList { cue_list_id },
            buttons: [light_playback::PlaybackButtonAction::None; 3],
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader: true,
            go_activates: true,
            auto_off: false,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                cue_lists: vec![cue_list.clone()],
                playbacks: vec![playback],
                playback_pages: vec![light_playback::PlaybackPage {
                    number: 1,
                    name: "Main".into(),
                    slots: HashMap::from([(7, 7)]),
                }],
                ..EngineSnapshot::default()
            })
            .unwrap();
        for _ in 0..3 {
            state.engine.playback().write().go_playback(7).unwrap();
        }

        let show_path = data_dir.join("shows/update-confirmation.show");
        let show_id = initialise_show(&show_path, "Update confirmation").unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: "Update confirmation".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        *state.active_show.write() = Some(entry.clone());
        let store = ShowStore::open(&show_path).unwrap();
        let cue_list_object_id = cue_list_id.0.to_string();
        let stored_revision = store
            .put_object(
                "cue_list",
                &cue_list_object_id,
                &serde_json::to_value(&cue_list).unwrap(),
                0,
            )
            .unwrap();
        let baseline = stored_update_object(&store, "cue_list", &cue_list_object_id)
            .unwrap()
            .body;

        state.programmers.set(
            session.id,
            fixture,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(0.8),
        );
        state.programmers.set(
            session.id,
            fixture,
            light_core::AttributeKey("color.red".into()),
            light_core::AttributeValue::Normalized(0.7),
        );
        let target = UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(cue_list_object_id.clone()),
            playback_number: Some(7),
            cue_id: Some(cue_list.cues[2].id),
            cue_number: Some(3.0),
            validate_active_context: true,
        };
        let preview_request = UpdateApiRequest {
            target: target.clone(),
            mode: update::UpdateMode::Cue(update::CueUpdateMode::ExistingOnly),
            expected_revision: None,
            expected_programmer_revision: None,
        };
        let preview = preview_update_request(&state, &session, &preview_request).unwrap();
        assert_eq!(preview.revision, stored_revision);
        assert_eq!(preview.preview.changed_count(), 2);

        state.programmers.set(
            session.id,
            fixture,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(0.9),
        );
        let stale = UpdateApiRequest {
            expected_revision: Some(preview.revision),
            expected_programmer_revision: Some(preview.programmer_revision),
            ..preview_request.clone()
        };
        let error = perform_update(&state, &session, &stale).unwrap_err();
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert!(error.message.contains("programmer content changed"));
        assert_eq!(
            stored_update_object(&store, "cue_list", &cue_list_object_id)
                .unwrap()
                .body,
            baseline
        );

        let preview = preview_update_request(&state, &session, &preview_request).unwrap();
        let confirmed = UpdateApiRequest {
            expected_revision: Some(preview.revision),
            expected_programmer_revision: Some(preview.programmer_revision),
            ..preview_request
        };
        let result = perform_update(&state, &session, &confirmed).unwrap();
        assert_eq!(result.changed_cues.len(), 2);
        assert_eq!(result.revision_after, stored_revision + 1);
        assert_eq!(
            store
                .undo_object("cue_list", &cue_list_object_id, result.revision_after)
                .unwrap(),
            result.revision_after + 1
        );
        assert_eq!(
            stored_update_object(&store, "cue_list", &cue_list_object_id)
                .unwrap()
                .body,
            baseline
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn cue_commands_use_the_desk_selected_concrete_playback() {
        let (state, data_dir) = test_state();
        let (user, first_desk, second_desk) = {
            let store = state.desk.lock();
            let user = store.users().unwrap().remove(0);
            let first = store.add_desk("Front", "front").unwrap();
            let second = store.add_desk("Wing", "wing").unwrap();
            (user, first, second)
        };
        let show_id = light_core::ShowId::new();
        *state.active_show.write() = Some(ShowEntry {
            id: show_id,
            name: "Selection".into(),
            path: data_dir.join("selection.show").display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        });
        let list_id = light_core::CueListId::new();
        let list = light_playback::CueList {
            id: list_id,
            name: "Shared".into(),
            priority: 0,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![
                light_playback::Cue::new(1.0),
                light_playback::Cue::new(2.0),
                light_playback::Cue::new(3.0),
            ],
        };
        let definition = |number| light_playback::PlaybackDefinition {
            number,
            name: format!("Playback {number}"),
            target: light_playback::PlaybackTarget::CueList {
                cue_list_id: list_id,
            },
            buttons: [
                light_playback::PlaybackButtonAction::GoMinus,
                light_playback::PlaybackButtonAction::Go,
                light_playback::PlaybackButtonAction::Flash,
            ],
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader: true,
            go_activates: true,
            auto_off: false,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        };
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                cue_lists: vec![list],
                playbacks: vec![definition(1), definition(2)],
                playback_pages: vec![light_playback::PlaybackPage {
                    number: 4,
                    name: "Page 4".into(),
                    slots: HashMap::from([(7, 2)]),
                }],
                ..Default::default()
            })
            .unwrap();
        state
            .desk
            .lock()
            .set_selected_playback(first_desk.id, show_id, Some(1))
            .unwrap();
        state
            .desk
            .lock()
            .set_selected_playback(second_desk.id, show_id, Some(2))
            .unwrap();
        state
            .desk
            .lock()
            .set_desk_page(first_desk.id, show_id, 4)
            .unwrap();
        handle_playback_osc(
            &state,
            "/light/front/page-playback/7/select",
            &[OscArgument::Bool(true)],
            None,
        );
        assert_eq!(
            state
                .desk
                .lock()
                .selected_playback(first_desk.id, show_id)
                .unwrap(),
            Some(2)
        );
        state
            .desk
            .lock()
            .set_selected_playback(first_desk.id, show_id, Some(1))
            .unwrap();
        let first = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "first".into(),
            connected: true,
            desk: first_desk,
        };
        let second = Session {
            id: SessionId::new(),
            user,
            token: "second".into(),
            connected: true,
            desk: second_desk,
        };
        execute_programmer_command(&state, &first, "CUE 2").unwrap();
        execute_programmer_command(&state, &second, "CUE 3").unwrap();
        execute_programmer_command(&state, &first, "CUE CUE 1").unwrap();
        let runtime = state.engine.playback().read().runtime();
        let first_runtime = runtime
            .iter()
            .find(|item| item.playback_number == Some(1))
            .unwrap();
        let second_runtime = runtime
            .iter()
            .find(|item| item.playback_number == Some(2))
            .unwrap();
        assert_eq!(
            (
                first_runtime.current_cue_number,
                first_runtime.loaded_cue_number
            ),
            (Some(2.0), Some(1.0))
        );
        assert_eq!(
            (
                second_runtime.current_cue_number,
                second_runtime.loaded_cue_number
            ),
            (Some(3.0), None)
        );
        execute_programmer_command(&state, &first, "CUE SET 2 CUE 1").unwrap();
        execute_programmer_command(&state, &first, "CUE CUE SET 4 . 7 CUE 2").unwrap();
        let second_runtime = state
            .engine
            .playback()
            .read()
            .runtime()
            .into_iter()
            .find(|item| item.playback_number == Some(2))
            .unwrap();
        assert_eq!(
            (
                second_runtime.current_cue_number,
                second_runtime.loaded_cue_number
            ),
            (Some(1.0), Some(2.0))
        );
        assert_eq!(
            state
                .desk
                .lock()
                .selected_playback(first.desk.id, show_id)
                .unwrap(),
            Some(1)
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn fixture_selection_accepts_minus_before_subsetting() {
        let tokens = ["1", "THRU", "10", "-", "5", "DIV", "2"].map(String::from);
        assert!(parse_fixture_selection(&[], &tokens).is_ok());
        let malformed = ["-", "5"].map(String::from);
        assert_eq!(
            parse_fixture_selection(&[], &malformed).unwrap_err(),
            "- requires fixture selections on both sides"
        );
    }

    #[test]
    fn bare_multi_head_selection_expands_to_children_and_steps_without_parent_identity() {
        let mut fixture = schema_v2_direct_fixture().0;
        fixture.fixture_number = Some(1);
        let parent = fixture.fixture_id;
        let first_head = light_core::FixtureId::new();
        let second_head = light_core::FixtureId::new();
        fixture.definition.heads = vec![
            light_fixture::LogicalHead {
                index: 0,
                name: "Master".into(),
                shared: true,
                parameters: Vec::new(),
            },
            light_fixture::LogicalHead {
                index: 1,
                name: "Cell 1".into(),
                shared: false,
                parameters: Vec::new(),
            },
            light_fixture::LogicalHead {
                index: 2,
                name: "Cell 2".into(),
                shared: false,
                parameters: Vec::new(),
            },
        ];
        fixture.logical_heads = vec![
            light_fixture::PatchedHead {
                head_index: 1,
                fixture_id: first_head,
            },
            light_fixture::PatchedHead {
                head_index: 2,
                fixture_id: second_head,
            },
        ];

        let expanded = parse_fixture_selection(&[fixture.clone()], &["1".into()]).unwrap();
        assert_eq!(expanded, vec![first_head, second_head]);
        assert_eq!(
            parse_fixture_selection(&[fixture.clone()], &["1".into(), ".".into(), "0".into()])
                .unwrap(),
            vec![parent],
            "only an explicit .0 address selects the master identity"
        );

        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = light_core::UserId::new();
        let fixtures = highlight_fixture_summaries(&[fixture]);
        let complete = light_programmer::ProgrammerSelection {
            selected: expanded,
            expression: Some(light_programmer::SelectionExpression::Static),
            revision: 1,
        };
        let first = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &complete,
                &fixtures,
                &HashMap::new(),
                false,
            )
            .unwrap();
        assert_eq!(
            first.working_selection.as_ref().unwrap().selected,
            vec![first_head]
        );
        let stepped = light_programmer::ProgrammerSelection {
            selected: vec![first_head],
            expression: Some(light_programmer::SelectionExpression::Static),
            revision: 2,
        };
        registry.acknowledge_internal_selection(desk, user, &stepped);
        let second = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &stepped,
                &fixtures,
                &HashMap::new(),
                false,
            )
            .unwrap();
        assert_eq!(
            second.working_selection.as_ref().unwrap().selected,
            vec![second_head]
        );
        assert!(
            !second
                .state
                .remembered
                .iter()
                .any(|item| item.fixture_id == parent)
        );
    }

    #[test]
    fn authoritative_selection_surfaces_expand_a_multi_head_parent_to_child_rows() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "multi-head-selection".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        state.sessions.write().insert(session.id, session.clone());
        let (fixture, children) = highlight_multi_head_fixture();
        let parent = fixture.fixture_id;
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                ..EngineSnapshot::default()
            })
            .unwrap();

        let set = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "multi-head-set".into(),
                session_id: session.id,
                expected_revision: None,
                command: "selection.set".into(),
                payload: serde_json::json!({"fixtures":[parent]}),
            },
        );
        assert!(set.ok, "{:?}", set.error);
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            children
        );

        state.programmers.select(session.id, []);
        let gesture = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "multi-head-gesture".into(),
                session_id: session.id,
                expected_revision: None,
                command: "selection.gesture".into(),
                payload: serde_json::json!({
                    "source":{"type":"fixture","fixture_id":parent}
                }),
            },
        );
        assert!(gesture.ok, "{:?}", gesture.error);
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            children
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn osc_exposes_time_minus_and_latched_shift_shortcuts() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "osc-test".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        state.sessions.write().insert(session.id, session.clone());
        let source: SocketAddr = "127.0.0.1:9010".parse().unwrap();
        state.osc_subscribers.lock().insert(
            "test".into(),
            OscSubscriber {
                desk_alias: "main".into(),
                target: source,
                command_source: source,
                session_id: session.id,
                last_seen: Instant::now(),
                shifted: false,
                shift_held: false,
                update_record_started: None,
                update_first_release: None,
                last_highlight_action: None,
            },
        );
        let pressed = [OscArgument::Bool(true)];
        handle_programmer_osc(
            &state,
            "/light/main/programmer/set",
            &pressed,
            Some("127.0.0.1:9010"),
        );
        assert_eq!(state.programmers.get(session.id).unwrap().command_line, "");
        assert!(state.audit_events.lock().iter().any(|event| {
            event.kind == "desk_action"
                && event.payload["action"] == "set"
                && event.payload["session_id"] == serde_json::json!(session.id)
        }));
        state
            .programmers
            .set_command_line(session.id, "COPY".into());
        handle_programmer_osc(
            &state,
            "/light/main/programmer/set",
            &pressed,
            Some("127.0.0.1:9010"),
        );
        assert_eq!(
            state.programmers.get(session.id).unwrap().command_line,
            "COPY SET "
        );
        state
            .programmers
            .set_command_line(session.id, String::new());
        handle_programmer_osc(
            &state,
            "/light/main/programmer/time",
            &pressed,
            Some("127.0.0.1:9010"),
        );
        handle_programmer_osc(
            &state,
            "/light/main/programmer/minus",
            &pressed,
            Some("127.0.0.1:9010"),
        );
        assert_eq!(
            state.programmers.get(session.id).unwrap().command_line,
            "TIME - "
        );
        handle_programmer_osc(
            &state,
            "/light/main/programmer/shift",
            &pressed,
            Some("127.0.0.1:9010"),
        );
        assert!(state.osc_subscribers.lock()["test"].shifted);
        handle_programmer_osc(
            &state,
            "/light/main/programmer/digit-1",
            &pressed,
            Some("127.0.0.1:9010"),
        );
        assert!(!state.osc_subscribers.lock()["test"].shifted);
        assert_eq!(
            state.programmers.get(session.id).unwrap().command_line,
            "TIME - "
        );
        handle_programmer_osc(
            &state,
            "/light/main/programmer/shift",
            &pressed,
            Some("127.0.0.1:9010"),
        );
        handle_programmer_osc(
            &state,
            "/light/main/programmer/clear",
            &pressed,
            Some("127.0.0.1:9010"),
        );
        assert!(!state.osc_subscribers.lock()["test"].shifted);
        assert_eq!(
            state.programmers.get(session.id).unwrap().command_line,
            "TIME - "
        );
        let events = state.audit_events.lock();
        let shifted_clear = events.back().unwrap();
        assert_eq!(shifted_clear.kind, "desk_action");
        assert_eq!(shifted_clear.payload["action"], "shift-clear");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn held_shift_record_short_double_and_long_gestures_are_mutually_distinct() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "osc-update-test".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        state.sessions.write().insert(session.id, session.clone());
        let source: SocketAddr = "127.0.0.1:9011".parse().unwrap();
        state.osc_subscribers.lock().insert(
            "update-test".into(),
            OscSubscriber {
                desk_alias: "main".into(),
                target: source,
                command_source: source,
                session_id: session.id,
                last_seen: Instant::now(),
                shifted: false,
                shift_held: false,
                update_record_started: None,
                update_first_release: None,
                last_highlight_action: None,
            },
        );
        let pressed = [OscArgument::Bool(true)];
        let released = [OscArgument::Bool(false)];
        let send = |action: &str, arguments: &[OscArgument]| {
            handle_programmer_osc(
                &state,
                &format!("/light/main/programmer/{action}"),
                arguments,
                Some("127.0.0.1:9011"),
            );
        };

        send("shift", &pressed);
        send("record", &pressed);
        send("record", &released);
        assert_eq!(
            state.programmers.get(session.id).unwrap().command_line,
            "UPDATE"
        );

        send("record", &pressed);
        send("record", &released);

        send("record", &pressed);
        state
            .osc_subscribers
            .lock()
            .get_mut("update-test")
            .unwrap()
            .update_record_started = Some(Instant::now() - Duration::from_millis(700));
        send("record", &released);
        assert_eq!(state.programmers.get(session.id).unwrap().command_line, "");

        let kinds = state
            .audit_events
            .lock()
            .iter()
            .map(|event| event.kind.clone())
            .filter(|kind| kind.starts_with("update_"))
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                "update_armed".to_string(),
                "update_targets_requested".to_string(),
                "update_settings_requested".to_string()
            ]
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn software_update_armed_state_is_shared_only_with_the_same_desk() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let front = test_control_desk();
        let mut wing = test_control_desk();
        wing.id = Uuid::new_v4();
        wing.osc_alias = "wing".into();
        let first = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "update-front-one".into(),
            connected: true,
            desk: front.clone(),
        };
        let second = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "update-front-two".into(),
            connected: true,
            desk: front,
        };
        let other = Session {
            id: SessionId::new(),
            user,
            token: "update-wing".into(),
            connected: true,
            desk: wing,
        };
        for session in [&first, &second, &other] {
            state.programmers.start(session.id, session.user.id);
            attach_session_command_context(&state, session);
            state.sessions.write().insert(session.id, session.clone());
        }

        let armed = dispatch_ws_command(
            &state,
            &first,
            WsCommand {
                protocol_version: 1,
                request_id: "arm-update".into(),
                session_id: first.id,
                expected_revision: None,
                command: "programmer.command_line".into(),
                payload: serde_json::json!({"value":"UPDATE "}),
            },
        );
        assert!(armed.ok);
        assert_eq!(
            state.programmers.get(second.id).unwrap().command_line,
            "UPDATE "
        );
        assert!(
            state
                .programmers
                .get(other.id)
                .unwrap()
                .command_line
                .is_empty()
        );
        let event = state
            .audit_events
            .lock()
            .iter()
            .rev()
            .find(|event| event.kind == "update_armed")
            .cloned()
            .unwrap();
        assert_eq!(event.payload["desk_id"], first.desk.id.to_string());
        assert_eq!(event.payload["armed"], true);

        let disarmed = dispatch_ws_command(
            &state,
            &second,
            WsCommand {
                protocol_version: 1,
                request_id: "disarm-update".into(),
                session_id: second.id,
                expected_revision: None,
                command: "programmer.command_line".into(),
                payload: serde_json::json!({"value":""}),
            },
        );
        assert!(disarmed.ok);
        assert!(
            state
                .programmers
                .get(first.id)
                .unwrap()
                .command_line
                .is_empty()
        );
        let events = state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.kind == "update_armed")
            .map(|event| event.payload["armed"].as_bool())
            .collect::<Vec<_>>();
        assert_eq!(events, vec![Some(true), Some(false)]);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn update_settings_endpoint_persists_and_reloads_per_desk() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let front = test_control_desk();
        let mut wing = test_control_desk();
        wing.id = Uuid::new_v4();
        wing.osc_alias = "wing".into();
        let writer = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "update-settings-writer".into(),
            connected: true,
            desk: front.clone(),
        };
        let reader = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "update-settings-reader".into(),
            connected: true,
            desk: front.clone(),
        };
        let other_desk = Session {
            id: SessionId::new(),
            user,
            token: "update-settings-other-desk".into(),
            connected: true,
            desk: wing.clone(),
        };
        for session in [&writer, &reader, &other_desk] {
            state.programmers.start(session.id, session.user.id);
            attach_session_command_context(&state, session);
            state.sessions.write().insert(session.id, session.clone());
        }
        let app = router(state.clone());
        let expected = update::UpdateSettings {
            cue_mode: update::CueUpdateMode::ExistingOnly,
            preset_mode: update::ExistingContentMode::AddNew,
            group_mode: update::ExistingContentMode::AddNew,
            other_target_modes: HashMap::from([(
                "macro".into(),
                update::ExistingContentMode::AddNew,
            )]),
            show_update_modal_on_touch: false,
        };

        let saved = app
            .clone()
            .oneshot(
                Request::put("/api/v1/update/settings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {}", writer.token))
                    .body(Body::from(serde_json::to_vec(&expected).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(saved.status(), StatusCode::OK);
        assert_eq!(
            serde_json::from_value::<update::UpdateSettings>(json(saved).await).unwrap(),
            expected
        );

        let persisted = state
            .desk
            .lock()
            .setting("server_configuration")
            .unwrap()
            .unwrap();
        let reloaded_configuration: DeskConfiguration = serde_json::from_str(&persisted).unwrap();
        assert_eq!(
            reloaded_configuration
                .update_settings_by_desk
                .get(&front.id),
            Some(&expected)
        );
        assert!(
            !reloaded_configuration
                .update_settings_by_desk
                .contains_key(&wing.id)
        );

        // Rebuild the HTTP surface around configuration decoded from the persisted desk setting,
        // matching the configuration boundary used by a process restart.
        let mut reloaded_state = state.clone();
        reloaded_state.configuration = Arc::new(RwLock::new(reloaded_configuration));
        let reloaded_app = router(reloaded_state);
        let same_desk = reloaded_app
            .clone()
            .oneshot(
                Request::get("/api/v1/update/settings")
                    .header(header::AUTHORIZATION, format!("Bearer {}", reader.token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(same_desk.status(), StatusCode::OK);
        assert_eq!(
            serde_json::from_value::<update::UpdateSettings>(json(same_desk).await).unwrap(),
            expected
        );
        let isolated = reloaded_app
            .oneshot(
                Request::get("/api/v1/update/settings")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", other_desk.token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(isolated.status(), StatusCode::OK);
        assert_eq!(
            serde_json::from_value::<update::UpdateSettings>(json(isolated).await).unwrap(),
            update::UpdateSettings::default()
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn armed_hardware_playback_touch_requests_update_without_operating_playback() {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "hardware-update-target".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        state.sessions.write().insert(session.id, session.clone());
        state
            .programmers
            .set_command_line(session.id, "UPDATE ".into());
        let mut snapshot = matter_test_snapshot();
        snapshot.playbacks[0].buttons[0] = light_playback::PlaybackButtonAction::Go;
        state.engine.replace_snapshot(snapshot).unwrap();
        let source: SocketAddr = "127.0.0.1:19021".parse().unwrap();
        state.osc_subscribers.lock().insert(
            "hardware-update".into(),
            OscSubscriber {
                desk_alias: session.desk.osc_alias.clone(),
                target: "127.0.0.1:19022".parse().unwrap(),
                command_source: source,
                session_id: session.id,
                last_seen: Instant::now(),
                shifted: false,
                shift_held: false,
                update_record_started: None,
                update_first_release: None,
                last_highlight_action: None,
            },
        );

        handle_playback_osc(
            &state,
            "/light/playback/4/7/button/1",
            &[OscArgument::Bool(true)],
            Some("127.0.0.1:19021"),
        );

        assert!(
            state
                .programmers
                .get(session.id)
                .unwrap()
                .command_line
                .is_empty()
        );
        let events = state.audit_events.lock();
        let requested = events
            .iter()
            .find(|event| event.kind == "update_target_requested")
            .unwrap();
        assert_eq!(requested.payload["desk_id"], session.desk.id.to_string());
        assert_eq!(requested.payload["target"]["family"]["type"], "cue");
        assert_eq!(requested.payload["target"]["playback_number"], 25);
        assert!(
            events
                .iter()
                .any(|event| { event.kind == "update_armed" && event.payload["armed"] == false })
        );
        assert!(!events.iter().any(|event| event.kind == "playback_changed"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    async fn json(response: Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn login(app: &Router, username: &str) -> (String, String) {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({"username":username}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value = json(response).await;
        (
            value["token"].as_str().unwrap().into(),
            value["session_id"].as_str().unwrap().into(),
        )
    }

    #[tokio::test]
    async fn fixture_profile_api_rejects_invalid_discrete_wheel_before_storing_revision() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let (fixture, _, channel_ids) = schema_v2_direct_fixture();
        let mut profile = *fixture.definition.profile_snapshot.unwrap();
        let profile_id = profile.id;
        let head_id = profile.modes[0].heads[0].id;
        profile.modes[0].color_systems = vec![light_fixture::HeadColorSystem {
            head_id,
            correction_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            system: light_fixture::ColorSystem::DiscreteWheel {
                channel_id: channel_ids[0],
                slots: vec![
                    light_fixture::ColorWheelSlot {
                        semantic_id: "red".into(),
                        label: "Red".into(),
                        dmx_from: 0,
                        dmx_to: 100,
                        measured_xyz: None,
                    },
                    light_fixture::ColorWheelSlot {
                        semantic_id: "blue".into(),
                        label: "Blue".into(),
                        dmx_from: 100,
                        dmx_to: 120,
                        measured_xyz: None,
                    },
                ],
            },
        }];

        let response = app
            .oneshot(
                Request::put("/api/v1/fixture-profiles")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::IF_MATCH, "0")
                    .body(Body::from(serde_json::to_vec(&profile).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            state
                .fixture_library
                .lock()
                .profile(profile_id, 1)
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn inactive_show_rejects_invalid_schema_v2_patch_before_persistence() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let show = create_show(&app, &token, "Inactive patch preflight").await;
        let show_id = light_core::ShowId(Uuid::parse_str(show["id"].as_str().unwrap()).unwrap());
        let entry = state.desk.lock().show(show_id).unwrap().unwrap();
        assert!(state.active_show.read().is_none());

        let (fixture, _, _) = schema_v2_direct_fixture();
        let object_id = fixture.fixture_id.0.to_string();
        let mut inconsistent_identity = fixture.clone();
        inconsistent_identity.definition.profile_id = Some(light_core::FixtureId::new());

        let mut unknown_split = fixture.clone();
        unknown_split.split_patches = vec![light_fixture::SplitPatch {
            split: 99,
            universe: Some(1),
            address: Some(1),
        }];

        let mut overlapping_multipatch = fixture;
        overlapping_multipatch.split_patches = vec![light_fixture::SplitPatch {
            split: 1,
            universe: Some(1),
            address: Some(1),
        }];
        overlapping_multipatch.multipatch = vec![light_fixture::MultiPatchInstance {
            id: Uuid::new_v4(),
            name: "Overlapping instance".into(),
            universe: None,
            address: None,
            split_patches: vec![light_fixture::SplitPatch {
                split: 1,
                universe: Some(1),
                address: Some(2),
            }],
            location: Default::default(),
            rotation: Default::default(),
        }];

        for invalid in [inconsistent_identity, unknown_split, overlapping_multipatch] {
            let response = put_show_object(
                &app,
                &token,
                &show_id.0.to_string(),
                "patched_fixture",
                &object_id,
                serde_json::to_value(invalid).unwrap(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert!(
                ShowStore::open(&entry.path)
                    .unwrap()
                    .objects("patched_fixture")
                    .unwrap()
                    .iter()
                    .all(|object| object.id != object_id)
            );
        }

        let (mut multi_split, _, _) = schema_v2_direct_fixture();
        let mut profile = *multi_split.definition.profile_snapshot.take().unwrap();
        let mode_id = profile.modes[0].id;
        profile.modes[0].splits.push(light_fixture::FixtureSplit {
            number: 2,
            footprint: 1,
        });
        profile.modes[0].heads.push(light_fixture::FixtureHead {
            id: Uuid::new_v4(),
            name: "Second".into(),
            master_shared: false,
        });
        multi_split.definition = profile.resolved_definition(mode_id).unwrap();
        multi_split.split_patches = vec![
            light_fixture::SplitPatch {
                split: 1,
                universe: Some(1),
                address: Some(1),
            },
            light_fixture::SplitPatch {
                split: 2,
                universe: None,
                address: None,
            },
        ];
        multi_split.multipatch = vec![light_fixture::MultiPatchInstance {
            id: Uuid::new_v4(),
            name: "Second body".into(),
            universe: None,
            address: None,
            split_patches: multi_split.split_patches.clone(),
            location: Default::default(),
            rotation: Default::default(),
        }];

        let mut missing_parent = multi_split.clone();
        missing_parent.split_patches.pop();
        let mut duplicate_parent = multi_split.clone();
        duplicate_parent.split_patches[1].split = 1;
        let mut partial_parent = multi_split.clone();
        partial_parent.split_patches[1].universe = Some(2);
        let mut missing_multipatch = multi_split;
        missing_multipatch.multipatch[0].split_patches.clear();

        for invalid in [
            missing_parent,
            duplicate_parent,
            partial_parent,
            missing_multipatch,
        ] {
            let response = put_show_object(
                &app,
                &token,
                &show_id.0.to_string(),
                "patched_fixture",
                &object_id,
                serde_json::to_value(invalid).unwrap(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert!(
                ShowStore::open(&entry.path)
                    .unwrap()
                    .objects("patched_fixture")
                    .unwrap()
                    .iter()
                    .all(|object| object.id != object_id)
            );
        }

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn fixture_profile_api_assigns_atomic_revisions_retains_gdtf_and_rejects_stale_edits() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let mut profile = light_fixture::FixtureProfile::blank();
        profile.manufacturer = "Acme".into();
        profile.name = "Orbit".into();
        profile.short_name = "Orbit".into();
        let profile_id = profile.id;

        let created = app
            .clone()
            .oneshot(
                Request::put("/api/v1/fixture-profiles")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::IF_MATCH, "0")
                    .body(Body::from(serde_json::to_vec(&profile).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        let created = json(created).await;
        assert_eq!(created["revision"], 1);

        let exported = app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/fixture-profiles/{}/1/package",
                    profile_id.0
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(exported.status(), StatusCode::OK);
        assert_eq!(
            exported.headers()[header::CONTENT_TYPE],
            light_fixture::FIXTURE_PACKAGE_MIME_TYPE
        );
        let package = exported.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            light_fixture::read_fixture_package(&package).unwrap().id,
            profile_id
        );
        let imported = app
            .clone()
            .oneshot(
                Request::post("/api/v1/fixture-packages/import")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(
                        header::CONTENT_TYPE,
                        light_fixture::FIXTURE_PACKAGE_MIME_TYPE,
                    )
                    .body(Body::from(package))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(imported.status(), StatusCode::OK);
        assert_eq!(json(imported).await["revision"], 1);

        let stale = app
            .clone()
            .oneshot(
                Request::put("/api/v1/fixture-profiles")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::IF_MATCH, "0")
                    .body(Body::from(created.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stale.status(), StatusCode::CONFLICT);

        let source = b"PK\x03\x04retained-gdtf";
        let retained = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/fixture-profiles/{}/1/source-gdtf",
                    profile_id.0
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(source.as_slice()))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(retained.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            state
                .fixture_library
                .lock()
                .profile_source_gdtf(profile_id, 1)
                .unwrap()
                .as_deref(),
            Some(source.as_slice())
        );

        let revisions = app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/fixture-profiles/{}/revisions",
                    profile_id.0
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(revisions.status(), StatusCode::OK);
        assert_eq!(json(revisions).await.as_array().unwrap().len(), 1);

        let warnings = app
            .oneshot(
                Request::get("/api/v1/fixture-profiles/warnings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(warnings.status(), StatusCode::OK);
        assert!(json(warnings).await.as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn event_socket_disconnect_keeps_file_input_owned_until_session_close() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let session = state
            .sessions
            .read()
            .values()
            .find(|session| session.token == token)
            .cloned()
            .unwrap();
        state
            .programmers
            .set_command_line(session.id, "COPY".into());
        state.ws_connections.lock().insert(session.id, 1);

        let claimed = app
            .clone()
            .oneshot(
                Request::post("/api/v1/files/input-context")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "instance_id":"acceptance-file-manager",
                            "action":"copy",
                            "origin":"pending"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(claimed.status(), StatusCode::OK);

        // ApiDriver commands use a short-lived event socket. Its asynchronous
        // close must not release a claim made immediately afterwards by the
        // still-authenticated Desk session.
        finish_event_socket(&state, &session);
        assert!(!state.ws_connections.lock().contains_key(&session.id));
        assert!(
            state
                .file_input_contexts
                .lock()
                .contains_key(&session.desk.id)
        );

        let competing = app
            .clone()
            .oneshot(
                Request::post("/api/v1/files/input-context")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "instance_id":"another-pane",
                            "action":"copy",
                            "origin":"toolbar"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(competing.status(), StatusCode::CONFLICT);

        let disconnected = app
            .oneshot(
                Request::delete(format!("/api/v1/sessions/{session_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(disconnected.status(), StatusCode::NO_CONTENT);
        assert!(state.file_input_contexts.lock().is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn losing_file_input_claim_does_not_consume_the_pending_command() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let session = state
            .sessions
            .read()
            .values()
            .find(|session| session.token == token)
            .cloned()
            .unwrap();
        state
            .programmers
            .set_command_line(session.id, "COPY".into());

        let winner = app
            .clone()
            .oneshot(
                Request::post("/api/v1/files/input-context")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "instance_id":"winning-toolbar",
                            "action":"copy",
                            "origin":"toolbar"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(winner.status(), StatusCode::OK);

        let loser = app
            .oneshot(
                Request::post("/api/v1/files/input-context")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "instance_id":"losing-pending-pane",
                            "action":"copy",
                            "origin":"pending"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(loser.status(), StatusCode::CONFLICT);
        assert_eq!(
            state.programmers.get(session.id).unwrap().command_line,
            "COPY"
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn sound_to_light_is_authoritative_per_speed_group_and_capture_is_desk_scoped() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let primary_desk = state
            .sessions
            .read()
            .values()
            .find(|session| session.token == token)
            .unwrap()
            .desk
            .id;

        let enabled = SoundToLightConfig {
            enabled: true,
            smoothing: 0.0,
            ..SoundToLightConfig::default()
        };
        let updated = app
            .clone()
            .oneshot(
                Request::put("/api/v1/speed-groups/A")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&enabled).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(updated.status(), StatusCode::OK);

        let observation = serde_json::json!({
            "captured_at_millis": 1,
            "source_available": true,
            "usable_signal": true,
            "level": 0.8,
            "selected_band_level": 0.7,
            "detected_bpm": 120.0,
            "confidence": 0.95
        });
        let observed = app
            .clone()
            .oneshot(
                Request::post("/api/v1/speed-groups/A/observation")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(observation.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(observed.status(), StatusCode::OK);
        let observed = json(observed).await;
        assert_eq!(observed["snapshot"]["source"], "sound");
        assert_eq!(observed["snapshot"]["effective_bpm"], 120.0);

        // Two browser sessions attached to one desk are alternate surfaces of that same desk and
        // may therefore feed the same analyzer lease.
        let same_desk_login = app
            .clone()
            .oneshot(
                Request::post("/api/v1/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({"username":"Operator","desk_id":primary_desk})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let same_desk_token = json(same_desk_login).await["token"]
            .as_str()
            .unwrap()
            .to_owned();
        let same_desk_observation = app
            .clone()
            .oneshot(
                Request::post("/api/v1/speed-groups/A/observation")
                    .header(header::AUTHORIZATION, format!("Bearer {same_desk_token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(observation.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(same_desk_observation.status(), StatusCode::OK);

        let other_desk = state.desk.lock().add_desk("Other", "other").unwrap();
        let other_login = app
            .clone()
            .oneshot(
                Request::post("/api/v1/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({"username":"Operator","desk_id":other_desk.id})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let other_token = json(other_login).await["token"]
            .as_str()
            .unwrap()
            .to_owned();
        let contested = app
            .clone()
            .oneshot(
                Request::post("/api/v1/speed-groups/A/observation")
                    .header(header::AUTHORIZATION, format!("Bearer {other_token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(observation.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(contested.status(), StatusCode::CONFLICT);

        // A direct/manual value from any attached surface takes ownership and remains the stable
        // fallback instead of silently retaining Sound mode.
        let mut direct = state.configuration.read().clone();
        direct.speed_groups_bpm[0] = 111.0;
        let direct_response = app
            .clone()
            .oneshot(
                Request::put("/api/v1/configuration")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&direct).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(direct_response.status(), StatusCode::OK);
        let current = app
            .oneshot(
                Request::get("/api/v1/speed-groups/A")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let current = json(current).await;
        assert_eq!(current["snapshot"]["source"], "manual");
        assert_eq!(current["snapshot"]["effective_bpm"], 111.0);
        assert_eq!(current["configuration"]["enabled"], false);
        assert!(state.sound_capture_owners.lock()[0].is_none());

        let persisted: DeskConfiguration = serde_json::from_str(
            &state
                .desk
                .lock()
                .setting("server_configuration")
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(persisted.speed_groups_bpm[0], 111.0);
        assert!(!persisted.speed_group_sound_to_light[0].enabled);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn osc_speed_group_feedback_uses_effective_sound_rate_and_pause_state() {
        let mut controller = SpeedGroupController::new(
            96.0,
            SoundToLightConfig {
                enabled: true,
                smoothing: 0.0,
                multiplier: 2.0,
                ..SoundToLightConfig::default()
            },
        )
        .unwrap();
        controller.observe_sound(SoundObservation::tempo(1_000, 120.0, 0.95));

        let running = speed_group_osc_feedback(controller.snapshot(1_000));
        assert_eq!(running[0], OscArgument::Int(240));
        assert_eq!(running[4], OscArgument::String("on".into()));

        controller.set_paused(true);
        let paused = speed_group_osc_feedback(controller.snapshot(1_001));
        assert_eq!(paused[0], OscArgument::Int(240));
        assert_eq!(paused[4], OscArgument::String("off".into()));
    }

    #[test]
    fn osc_speed_group_button_performs_the_authoritative_learn_action() {
        let (state, data_dir) = test_state();
        let enabled = SoundToLightConfig {
            enabled: true,
            ..SoundToLightConfig::default()
        };
        state.speed_groups.lock()[0]
            .set_sound_config(enabled.clone())
            .unwrap();
        state.configuration.write().speed_group_sound_to_light[0] = enabled;
        state.sound_capture_owners.lock()[0] = Some(SoundCaptureOwner {
            desk_id: Uuid::new_v4(),
            last_seen_millis: 1,
        });

        handle_timing_osc(
            &state,
            "/light/main/speed-group/1/button",
            &[OscArgument::Bool(true)],
        );

        assert!(!state.speed_groups.lock()[0].sound_config().enabled);
        assert!(!state.configuration.read().speed_group_sound_to_light[0].enabled);
        assert!(state.sound_capture_owners.lock()[0].is_none());
        let event = state.audit_events.lock().back().cloned().unwrap();
        assert_eq!(event.kind, "speed_group_action");
        assert_eq!(event.payload["source"], "osc");
        assert_eq!(event.payload["action"], "learn");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn desk_lock_is_persisted_scoped_and_enforced_by_the_server() {
        let (state, data_dir) = test_state();
        let second = state.desk.lock().add_desk("Second", "second").unwrap();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let configure = app.clone().oneshot(Request::put("/api/v1/desk-lock").header(header::AUTHORIZATION, format!("Bearer {token}")).header(header::CONTENT_TYPE,"application/json").body(Body::from(r#"{"message":"Call the operator","wallpaper":null,"unlock_mode":"pin","pin":"1234"}"#)).unwrap()).await.unwrap();
        assert_eq!(configure.status(), StatusCode::OK);
        let lock = app
            .clone()
            .oneshot(
                Request::post("/api/v1/desk-lock/lock")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(lock.status(), StatusCode::OK);
        assert!(
            read_desk_lock(
                &state,
                state
                    .sessions
                    .read()
                    .values()
                    .find(|session| session.token == token)
                    .unwrap()
                    .desk
                    .id
            )
            .locked
        );
        let desk_id = state
            .sessions
            .read()
            .values()
            .find(|session| session.token == token)
            .unwrap()
            .desk
            .id;
        let reopened = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
        let persisted: DeskLockConfiguration =
            serde_json::from_str(&reopened.setting(&desk_lock_key(desk_id)).unwrap().unwrap())
                .unwrap();
        assert!(
            persisted.locked,
            "a server restart must reopen the desk as locked"
        );
        let blocked = app
            .clone()
            .oneshot(
                Request::put("/api/v1/master")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grand_master":0.5}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(blocked.status(), StatusCode::CONFLICT);
        let wrong = app
            .clone()
            .oneshot(
                Request::post("/api/v1/desk-lock/unlock")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"pin":"9999"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

        let second_login = app
            .clone()
            .oneshot(
                Request::post("/api/v1/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({"username":"Operator","desk_id":second.id}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let second_token = json(second_login).await["token"]
            .as_str()
            .unwrap()
            .to_owned();
        let unaffected = app
            .clone()
            .oneshot(
                Request::put("/api/v1/master")
                    .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grand_master":0.5}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unaffected.status(), StatusCode::OK);

        let unlock = app
            .oneshot(
                Request::post("/api/v1/desk-lock/unlock")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"pin":"1234"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unlock.status(), StatusCode::OK);
        let stored = state
            .desk
            .lock()
            .setting(&desk_lock_key(
                state
                    .sessions
                    .read()
                    .values()
                    .find(|session| session.token == token)
                    .unwrap()
                    .desk
                    .id,
            ))
            .unwrap()
            .unwrap();
        assert!(!stored.contains("1234"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn login_reuses_client_desk_when_remembered_desk_is_stale() {
        let (state, data_dir) = test_state();
        let app = router(state);
        let client_id = Uuid::new_v4();
        let login = |desk_id: Option<Uuid>| {
            let app = app.clone();
            async move {
                app.oneshot(
                    Request::post("/api/v1/sessions")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            serde_json::json!({"username":"Operator","client_id":client_id,"desk_id":desk_id}).to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap()
            }
        };
        let first = login(None).await;
        assert_eq!(first.status(), StatusCode::OK);
        let first_desk = json(first).await["desk"]["id"].clone();
        let second = login(Some(Uuid::new_v4())).await;
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(json(second).await["desk"]["id"], first_desk);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn citp_thumbnail_api_uses_patched_parent_endpoint_and_cache() {
        use tokio::io::AsyncWriteExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (state, data_dir) = test_state();
        let fixture_id = light_core::FixtureId::new();
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![light_fixture::PatchedFixture {
                    name: "Media Server".into(),
                    layer_id: "default".into(),
                    fixture_id,
                    fixture_number: None,
                    virtual_fixture_number: None,
                    definition: light_fixture::FixtureDefinition {
                        schema_version: 1,
                        id: light_core::FixtureId::new(),
                        revision: 1,
                        manufacturer: "Test".into(),
                        device_type: "media server".into(),
                        name: "Media Server".into(),
                        model: "Media Server".into(),
                        mode: "2 layers".into(),
                        footprint: 1,
                        heads: vec![light_fixture::LogicalHead {
                            index: 0,
                            name: "Master".into(),
                            shared: true,
                            parameters: vec![],
                        }],
                        color_calibration: None,
                        physical: Default::default(),
                        model_asset: None,
                        icon_asset: None,
                        hazardous: false,
                        direct_control_protocols: vec![light_fixture::DirectControlProtocol::Citp],
                        signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
                        safe_values: std::collections::BTreeMap::new(),
                        profile_id: None,
                        mode_id: None,
                        profile_snapshot: None,
                    },
                    universe: Some(1),
                    address: Some(1),
                    split_patches: Vec::new(),
                    direct_control: Some(light_fixture::DirectControlEndpoint {
                        protocol: light_fixture::DirectControlProtocol::Citp,
                        ip_address: address.ip(),
                        port: address.port(),
                    }),
                    location: Default::default(),
                    rotation: Default::default(),
                    logical_heads: vec![light_fixture::PatchedHead {
                        head_index: 1,
                        fixture_id: light_core::FixtureId::new(),
                    }],
                    move_in_black_enabled: true,
                    move_in_black_delay_millis: 0,
                    multipatch: vec![],
                    highlight_overrides: Default::default(),
                }],
                revision: 1,
                ..EngineSnapshot::default()
            })
            .unwrap();
        let mock = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let cinf = read_citp_test_packet(&mut stream).await;
            assert_eq!(&cinf[22..26], b"CInf");
            let mut info = citp_test_packet(*b"SInf", &[]);
            info[6..8].copy_from_slice(&1_u16.to_le_bytes());
            stream.write_all(&info).await.unwrap();
            let request = read_citp_test_packet(&mut stream).await;
            assert_eq!(&request[22..26], b"GETh");
            let mut payload = vec![1, 0, 0, 0, 0, 7];
            payload.extend_from_slice(b"JPEG");
            payload.extend_from_slice(&2_u16.to_le_bytes());
            payload.extend_from_slice(&1_u16.to_le_bytes());
            payload.extend_from_slice(&3_u16.to_le_bytes());
            payload.extend_from_slice(&[1, 2, 3]);
            stream
                .write_all(&citp_test_packet(*b"EThn", &payload))
                .await
                .unwrap();
        });
        let app = router(state);
        let (token, _) = login(&app, "Operator").await;
        let refreshed = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/media/{}/thumbnails/refresh", fixture_id.0))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"elements":[7],"width":64,"height":64}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(refreshed.status(), StatusCode::OK);
        let image = app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/media/{}/thumbnail?element=7",
                    fixture_id.0
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(image.status(), StatusCode::OK);
        assert_eq!(image.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(
            image
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .as_ref(),
            &[1, 2, 3]
        );
        let status = app
            .oneshot(
                Request::get("/api/v1/media")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = json(status).await;
        assert_eq!(status["fixtures"][0]["status"]["online"], true);
        assert_eq!(status["fixtures"][0]["layers"].as_array().unwrap().len(), 1);
        mock.await.unwrap();
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn programmer_set_many_validates_then_applies_one_faded_undo_step() {
        let (state, data_dir) = test_state();
        let fixture = schema_v2_direct_fixture().0;
        let fixture_id = fixture.fixture_id;
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                revision: 1,
                ..EngineSnapshot::default()
            })
            .unwrap();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let session = authenticate_token(&state, &token).unwrap();

        let response = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "home".into(),
                session_id: session.id,
                expected_revision: None,
                command: "programmer.set_many".into(),
                payload: serde_json::json!({"assignments":[
                    {"fixture_id":fixture_id,"attribute":"pan","value":0.25},
                    {"fixture_id":fixture_id,"attribute":"tilt","value":0.75}
                ]}),
            },
        );
        assert!(response.ok, "{:?}", response.error);
        let values = state.programmers.get(session.id).unwrap().values;
        assert_eq!(values.len(), 2);
        assert!(values.iter().all(|value| value.fade));
        assert_eq!(values[0].changed_at, values[1].changed_at);

        let rejected = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "invalid-home".into(),
                session_id: session.id,
                expected_revision: None,
                command: "programmer.set_many".into(),
                payload: serde_json::json!({"assignments":[
                    {"fixture_id":fixture_id,"attribute":"pan","value":0.5},
                    {"fixture_id":light_core::FixtureId::new(),"attribute":"tilt","value":0.5}
                ]}),
            },
        );
        assert!(!rejected.ok);
        assert_eq!(
            serde_json::to_value(state.programmers.get(session.id).unwrap().values).unwrap(),
            serde_json::to_value(values).unwrap()
        );
        assert!(state.programmers.undo(session.id));
        assert!(state.programmers.get(session.id).unwrap().values.is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn websocket_commands_are_typed_owned_and_revision_checked() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let session = authenticate_token(&state, &token).unwrap();
        let fixture = light_core::FixtureId::new();
        let response = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "set-1".into(),
                session_id: SessionId(Uuid::parse_str(&session_id).unwrap()),
                expected_revision: Some(0),
                command: "programmer.set".into(),
                payload: serde_json::json!({"fixture_id":fixture,"attribute":"intensity","value":0.75}),
            },
        );
        assert!(response.ok);
        assert_eq!(state.programmers.get(session.id).unwrap().values.len(), 1);
        let same_user_session = Session {
            id: SessionId::new(),
            user: session.user.clone(),
            token: "same-user".into(),
            connected: true,
            desk: session.desk.clone(),
        };
        state
            .programmers
            .start(same_user_session.id, same_user_session.user.id);
        let same_user_update = dispatch_ws_command(
            &state,
            &same_user_session,
            WsCommand {
                protocol_version: 1,
                request_id: "same-user".into(),
                session_id: same_user_session.id,
                expected_revision: Some(999),
                command: "programmer.set".into(),
                payload: serde_json::json!({"fixture_id":fixture,"attribute":"intensity","value":0.5}),
            },
        );
        assert!(
            same_user_update.ok,
            "one user owns the lock across all of their sessions"
        );
        let other_user = state.desk.lock().add_user("Other operator").unwrap();
        let other_session = Session {
            id: SessionId::new(),
            user: other_user,
            token: "other-user".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state
            .programmers
            .start(other_session.id, other_session.user.id);
        let competing_update = dispatch_ws_command(
            &state,
            &other_session,
            WsCommand {
                protocol_version: 1,
                request_id: "other-user".into(),
                session_id: other_session.id,
                expected_revision: None,
                command: "programmer.set".into(),
                payload: serde_json::json!({"fixture_id":fixture,"attribute":"intensity","value":0.2}),
            },
        );
        assert!(
            competing_update.ok,
            "different users own independent programmers that arbitrate in the engine"
        );
        let primary_programmer = state.programmers.get(session.id).unwrap();
        let competing_programmer = state.programmers.get(other_session.id).unwrap();
        assert_ne!(primary_programmer.id, competing_programmer.id);
        assert_eq!(primary_programmer.values.len(), 1);
        assert_eq!(competing_programmer.values.len(), 1);
        let foreign = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "foreign".into(),
                session_id: SessionId::new(),
                expected_revision: None,
                command: "programmer.clear".into(),
                payload: serde_json::Value::Null,
            },
        );
        assert!(!foreign.ok);
        assert!(foreign.error.unwrap().contains("does not own"));
        let stale = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "stale".into(),
                session_id: session.id,
                expected_revision: Some(99),
                command: "programmer.clear".into(),
                payload: serde_json::Value::Null,
            },
        );
        assert!(
            stale.ok,
            "live absolute commands ignore unrelated show revisions"
        );
        let revisioned = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "revisioned".into(),
                session_id: session.id,
                expected_revision: Some(99),
                command: "show.activate".into(),
                payload: serde_json::Value::Null,
            },
        );
        assert!(!revisioned.ok);
        assert!(revisioned.error.unwrap().contains("revision conflict"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn direct_programmer_writes_resolve_configured_fade_for_recording() {
        let (state, data_dir) = test_state();
        state
            .engine
            .replace_snapshot(EngineSnapshot {
                groups: vec![light_programmer::GroupDefinition {
                    id: "1".into(),
                    name: "Front".into(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let session = authenticate_token(&state, &token).unwrap();
        let fixture = light_core::FixtureId::new();

        let fixture_response = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "fixture-fade".into(),
                session_id: SessionId(Uuid::parse_str(&session_id).unwrap()),
                expected_revision: None,
                command: "programmer.set".into(),
                payload: serde_json::json!({
                    "fixture_id": fixture,
                    "attribute": "intensity",
                    "value": 0.75
                }),
            },
        );
        assert!(fixture_response.ok);

        let group_response = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "group-fade".into(),
                session_id: session.id,
                expected_revision: None,
                command: "programmer.group.set".into(),
                payload: serde_json::json!({
                    "group_id": "1",
                    "attribute": "intensity",
                    "value": 0.5
                }),
            },
        );
        assert!(group_response.ok);

        let direct = state.programmers.get(session.id).unwrap();
        assert_eq!(direct.values[0].fade_millis, Some(3_000));
        assert_eq!(
            direct.group_values["1"][&light_core::AttributeKey::intensity()].fade_millis,
            Some(3_000)
        );

        execute_programmer_command(&state, &session, "GROUP 1 AT 25").unwrap();
        let command = state.programmers.get(session.id).unwrap();
        assert_eq!(
            command.group_values["1"][&light_core::AttributeKey::intensity()].fade_millis,
            Some(3_000),
            "commands without TIME resolve Programmer Fade when the value is written"
        );
        let recorded = programmer_cue(&command, 1.0, CommandTiming::default());
        assert_eq!(recorded.changes[0].fade_millis, Some(3_000));
        assert_eq!(recorded.group_changes[0].fade_millis, Some(3_000));
        assert_eq!(
            recorded.fade_millis, 0,
            "Programmer Fade is per change, not Cue TIME"
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn bootstrap_does_not_relock_the_desk_store() {
        let (state, data_dir) = test_state();
        let response = tokio::time::timeout(
            Duration::from_secs(1),
            router(state).oneshot(
                Request::get("/api/v1/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            ),
        )
        .await
        .expect("bootstrap must not deadlock")
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json(response).await;
        let attributes = body["attribute_registry"].as_array().unwrap();
        assert_eq!(attributes.len(), ATTRIBUTE_REGISTRY.len());
        assert!(attributes.iter().any(|attribute| {
            attribute
                == &serde_json::json!({
                    "id": "zoom",
                    "label": "Zoom",
                    "family": "beam",
                    "value_type": "continuous",
                    "default_unit": "deg"
                })
        }));
        assert!(
            !attributes
                .iter()
                .any(|attribute| attribute["id"] == "beam.zoom")
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn optional_desk_token_guards_the_api_boundary() {
        let (mut state, data_dir) = test_state();
        state.desk_token = Some(Arc::from("shared-secret"));
        let app = router(state);
        let denied = app
            .clone()
            .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        let allowed = app
            .clone()
            .oneshot(
                Request::get("/api/v1/health")
                    .header("x-light-desk-token", "shared-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK);
        let allowed_ws_boundary = app
            .clone()
            .oneshot(
                Request::get("/api/v1/health")
                    .header(
                        header::SEC_WEBSOCKET_PROTOCOL,
                        "light.v1, light.desk.b64.c2hhcmVkLXNlY3JldA",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed_ws_boundary.status(), StatusCode::OK);
        let static_asset = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(static_asset.status(), StatusCode::OK);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn authenticated_shutdown_requests_orderly_server_cancellation() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let unauthorized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/shutdown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let (token, _) = login(&app, "Operator").await;
        let response = app
            .oneshot(
                Request::post("/api/v1/shutdown")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(state.shutdown.is_cancelled());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn emitted_events_have_strictly_sequential_revisions() {
        let (state, data_dir) = test_state();
        let mut receiver = state.events.subscribe();
        emit(&state, "first", serde_json::Value::Null);
        emit(&state, "second", serde_json::Value::Null);
        let first = receiver.try_recv().unwrap();
        let second = receiver.try_recv().unwrap();
        assert_eq!(first.revision + 1, second.revision);
        let audit = state.audit_events.lock();
        assert_eq!(audit.len(), 2);
        assert_eq!(audit[0].kind, "first");
        assert_eq!(audit[1].revision, second.revision);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn preset_store_endpoint_merges_with_revision_control() {
        let (state, data_dir) = test_state();
        let app = router(state);
        let (token, _) = login(&app, "Operator").await;
        let created = app
            .clone()
            .oneshot(
                Request::post("/api/v1/shows")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(
                        r#"{"name":"Preset Test","data_base64":null,"overwrite":false}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let show_id = json(created).await["id"].as_str().unwrap().to_owned();
        let fixture = light_core::FixtureId::new();
        let first = light_programmer::Preset {
            name: "Look".into(),
            family: light_programmer::PresetFamily::Intensity,
            number: 1,
            values: HashMap::from([(
                fixture,
                HashMap::from([
                    (
                        light_core::AttributeKey::intensity(),
                        light_core::AttributeValue::Normalized(0.5),
                    ),
                    (
                        light_core::AttributeKey("pan".into()),
                        light_core::AttributeValue::Normalized(0.25),
                    ),
                ]),
            )]),
            group_values: HashMap::new(),
        };
        let stored = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/presets/1.1/store"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::IF_MATCH, "0")
                    .body(Body::from(
                        serde_json::json!({"mode":"overwrite","preset":first}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stored.status(), StatusCode::OK);
        let stored = json(stored).await;
        assert_eq!(stored["revision"], 1);
        assert_eq!(stored["preset"]["family"], "Intensity");
        assert_eq!(
            stored["preset"]["values"][fixture.0.to_string()]
                .as_object()
                .unwrap()
                .len(),
            1
        );
        let stale = app
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/presets/1.1/store"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::IF_MATCH, "0")
                    .body(Body::from(
                        serde_json::json!({"mode":"merge","preset":{"name":"","values":{}}})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stale.status(), StatusCode::CONFLICT);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn preset_object_api_uses_family_scoped_numbers() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let created = create_show(&app, &token, "Typed preset addresses").await;
        let show_id = created["id"].as_str().unwrap();

        for (storage_key, family) in [("2.1", "Color"), ("3.1", "Position")] {
            let response = put_show_object(
                &app,
                &token,
                show_id,
                "preset",
                storage_key,
                serde_json::json!({
                    "name": format!("{family} one"),
                    "family": family,
                    "number": 1,
                    "values": {},
                    "group_values": {},
                }),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        let entry = state
            .desk
            .lock()
            .show(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
            .unwrap()
            .unwrap();
        ShowStore::open(&entry.path)
            .unwrap()
            .put_object(
                "preset",
                "7",
                &serde_json::json!({
                    "name": "Legacy Color seven",
                    "family": "Color",
                    "values": {},
                    "group_values": {},
                }),
                0,
            )
            .unwrap();

        let listed = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{show_id}/objects/preset"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::OK);
        let listed = json(listed).await;
        assert_eq!(listed.as_array().unwrap().len(), 3);
        assert!(
            listed
                .as_array()
                .unwrap()
                .iter()
                .any(|object| object["id"] == "2.1"
                    && object["body"]["family"] == "Color"
                    && object["body"]["number"] == 1)
        );
        assert!(
            listed
                .as_array()
                .unwrap()
                .iter()
                .any(|object| object["id"] == "7"
                    && object["body"]["family"] == "Color"
                    && object["body"]["number"] == 7)
        );
        assert!(
            listed
                .as_array()
                .unwrap()
                .iter()
                .any(|object| object["id"] == "3.1"
                    && object["body"]["family"] == "Position"
                    && object["body"]["number"] == 1)
        );

        let global_plain_id = put_show_object(
            &app,
            &token,
            show_id,
            "preset",
            "1",
            serde_json::json!({
                "name": "Ambiguous",
                "family": "Color",
                "number": 1,
                "values": {},
                "group_values": {},
            }),
        )
        .await;
        assert_eq!(global_plain_id.status(), StatusCode::BAD_REQUEST);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    async fn create_show(app: &Router, token: &str, name: &str) -> serde_json::Value {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/shows")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(
                        serde_json::json!({"name":name,"data_base64":null,"overwrite":false})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        json(response).await
    }

    #[tokio::test]
    async fn active_empty_show_rename_preserves_identity_content_and_revisions() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let created = create_show(&app, &token, "New Empty Show").await;
        let show_id = created["id"].as_str().unwrap();
        let original_path = created["path"].as_str().unwrap().to_owned();
        let opened = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/open"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        let reopened_desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
        let reopened_empty = reopened_desk.active_show().unwrap().unwrap();
        assert_eq!(reopened_empty.id.0.to_string(), show_id);
        assert_eq!(reopened_empty.name, "New Empty Show");
        assert!(FsPath::new(&reopened_empty.path).exists());
        drop(reopened_desk);
        let stored = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/shows/{show_id}/objects/user_layout/operator"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from(r#"{"marker":"before naming"}"#))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stored.status(), StatusCode::OK);
        let revision = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/revisions"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"name":"Before naming"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(revision.status(), StatusCode::CREATED);

        let renamed = app
            .clone()
            .oneshot(
                Request::put(format!("/api/v1/shows/{show_id}/rename"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"name":"Opening Night"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(renamed.status(), StatusCode::OK);
        let renamed = json(renamed).await;
        assert_eq!(renamed["id"], show_id);
        assert_eq!(renamed["name"], "Opening Night");
        let renamed_path = renamed["path"].as_str().unwrap();
        assert!(renamed_path.ends_with("Opening Night.show"));
        assert!(!FsPath::new(&original_path).exists());
        let portable = ShowStore::open(renamed_path).unwrap();
        assert_eq!(portable.id().unwrap().0.to_string(), show_id);
        assert_eq!(portable.name().unwrap(), "Opening Night");

        let objects = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{show_id}/objects/user_layout"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(objects.status(), StatusCode::OK);
        assert_eq!(json(objects).await[0]["body"]["marker"], "before naming");
        let revisions = state
            .desk
            .lock()
            .show_revisions(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
            .unwrap();
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].name, "Before naming");
        let active = state.desk.lock().active_show().unwrap().unwrap();
        assert_eq!(active.id.0.to_string(), show_id);
        assert_eq!(active.name, "Opening Night");

        let _occupied = create_show(&app, &token, "Occupied").await;
        let collision = app
            .clone()
            .oneshot(
                Request::put(format!("/api/v1/shows/{show_id}/rename"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"name":"occupied"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(collision.status(), StatusCode::CONFLICT);
        let still_active = state.desk.lock().active_show().unwrap().unwrap();
        assert_eq!(still_active.id.0.to_string(), show_id);
        assert_eq!(still_active.name, "Opening Night");
        assert!(FsPath::new(&still_active.path).exists());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn named_revision_load_creates_an_independent_provenanced_copy() {
        let (state, data_dir) = test_state();
        let app = router(state);
        let (token, _) = login(&app, "Operator").await;
        let show = create_show(&app, &token, "Revision source").await;
        let show_id = show["id"].as_str().unwrap();
        let first = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/shows/{show_id}/objects/user_layout/operator"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from(r#"{"marker":"manual"}"#))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        let saved = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/revisions"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"name":"Before experiment"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(saved.status(), StatusCode::CREATED);
        let saved = json(saved).await;
        assert_eq!(saved["revision"], 1);
        assert_eq!(saved["name"], "Before experiment");
        assert!(saved.get("path").is_none());
        let autosaved = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/shows/{show_id}/objects/user_layout/operator"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(r#"{"marker":"autosave"}"#))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(autosaved.status(), StatusCode::OK);
        let opened = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/revisions/1/open"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        let copy = json(opened).await;
        let copy_id = copy["id"].as_str().unwrap();
        assert_ne!(copy_id, show_id);
        assert!(
            copy["name"]
                .as_str()
                .unwrap()
                .starts_with("Revision source-rev-1-")
        );
        assert_eq!(copy["revision_copy"]["show_id"], show_id);
        assert_eq!(copy["revision_copy"]["show_name"], "Revision source");
        assert_eq!(copy["revision_copy"]["revision"], 1);
        assert_eq!(copy["revision_copy"]["revision_name"], "Before experiment");
        assert!(copy["revision_copy"]["copied_at"].as_str().is_some());

        let original_objects = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{show_id}/objects/user_layout"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(original_objects.status(), StatusCode::OK);
        let original_objects = json(original_objects).await;
        assert_eq!(original_objects[0]["body"]["marker"], "autosave");
        let copy_objects = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{copy_id}/objects/user_layout"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(copy_objects.status(), StatusCode::OK);
        let copy_objects = json(copy_objects).await;
        assert_eq!(copy_objects[0]["body"]["marker"], "manual");

        let copy_edit = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/shows/{copy_id}/objects/user_layout/operator"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(r#"{"marker":"copy edit"}"#))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(copy_edit.status(), StatusCode::OK);
        let original_after_copy_edit = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{show_id}/objects/user_layout"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            json(original_after_copy_edit).await[0]["body"]["marker"],
            "autosave"
        );

        let opened_again = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/revisions/1/open"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(opened_again.status(), StatusCode::OK);
        let second_copy = json(opened_again).await;
        assert_ne!(second_copy["id"], copy["id"]);
        assert_ne!(second_copy["name"], copy["name"]);
        assert!(second_copy["name"].as_str().unwrap().ends_with("-2"));

        let revisions = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{show_id}/revisions"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let revisions = json(revisions).await;
        assert_eq!(revisions.as_array().unwrap().len(), 1);
        assert_eq!(revisions[0]["name"], "Before experiment");
        let _ = std::fs::remove_dir_all(data_dir);
    }
    async fn put_show_object(
        app: &Router,
        token: &str,
        show: &str,
        kind: &str,
        id: &str,
        body: serde_json::Value,
    ) -> Response {
        app.clone()
            .oneshot(
                Request::put(format!("/api/v1/shows/{show}/objects/{kind}/{id}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::IF_MATCH, "0")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn confirmed_overwrite_preserves_destination_identity_revisions_and_revision_copy() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let source = create_show(&app, &token, "Overwrite source").await;
        let source_id = source["id"].as_str().unwrap();
        assert_eq!(
            put_show_object(
                &app,
                &token,
                source_id,
                "user_layout",
                "operator",
                serde_json::json!({"marker":"named snapshot"}),
            )
            .await
            .status(),
            StatusCode::OK
        );
        let saved = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{source_id}/revisions"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"name":"Source Revision"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(saved.status(), StatusCode::CREATED);
        let source_revision_path = state
            .desk
            .lock()
            .show_revision(light_core::ShowId(Uuid::parse_str(source_id).unwrap()), 1)
            .unwrap()
            .unwrap()
            .path;
        let revision_body = |path: &str| {
            ShowStore::open(path)
                .unwrap()
                .objects("user_layout")
                .unwrap()
                .into_iter()
                .find(|object| object.id == "operator")
                .unwrap()
                .body
        };
        let source_revision_body_before = revision_body(&source_revision_path);
        assert_eq!(source_revision_body_before["marker"], "named snapshot");
        let opened = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{source_id}/revisions/1/open"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let copy = json(opened).await;
        let copy_id = copy["id"].as_str().unwrap();
        let copy_edit = app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/shows/{copy_id}/objects/user_layout/operator"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(r#"{"marker":"copy edit"}"#))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(copy_edit.status(), StatusCode::OK);
        let copy_revision = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{copy_id}/revisions"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"name":"Copy private checkpoint"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(copy_revision.status(), StatusCode::CREATED);

        let destination = create_show(&app, &token, "Destination").await;
        let destination_id = destination["id"].as_str().unwrap();
        assert_eq!(
            put_show_object(
                &app,
                &token,
                destination_id,
                "user_layout",
                "operator",
                serde_json::json!({"marker":"destination old state"}),
            )
            .await
            .status(),
            StatusCode::OK
        );
        let destination_revision = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{destination_id}/revisions"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"name":"Destination baseline"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(destination_revision.status(), StatusCode::CREATED);

        let overwritten = app
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v1/shows/{copy_id}/overwrite/{destination_id}"
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(overwritten.status(), StatusCode::OK);
        let overwritten = json(overwritten).await;
        assert_eq!(overwritten["id"], destination_id);
        assert_eq!(overwritten["name"], "Destination");
        assert!(overwritten.get("revision_copy").is_none());

        let destination_objects = app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/shows/{destination_id}/objects/user_layout"
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            json(destination_objects).await[0]["body"]["marker"],
            "copy edit"
        );
        let revisions = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{destination_id}/revisions"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let revisions = json(revisions).await;
        assert_eq!(revisions.as_array().unwrap().len(), 1);
        assert_eq!(revisions[0]["name"], "Destination baseline");
        let copy_revisions = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{copy_id}/revisions"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let copy_revisions = json(copy_revisions).await;
        assert_eq!(copy_revisions.as_array().unwrap().len(), 1);
        assert_eq!(copy_revisions[0]["name"], "Copy private checkpoint");
        assert_eq!(
            revision_body(&source_revision_path),
            source_revision_body_before,
            "overwriting another Latest Autosave must not mutate the immutable source revision"
        );

        let shows = app
            .clone()
            .oneshot(Request::get("/api/v1/shows").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let shows = json(shows).await;
        assert!(
            shows
                .as_array()
                .unwrap()
                .iter()
                .any(|show| show["id"] == copy_id)
        );
        let deleted_source = app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/shows/{source_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(deleted_source.status(), StatusCode::NO_CONTENT);
        let missing_original = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{copy_id}/overwrite/{source_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_original.status(), StatusCode::NOT_FOUND);
        let copy_after_source_deletion = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{copy_id}/objects/user_layout"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            json(copy_after_source_deletion).await[0]["body"]["marker"],
            "copy edit"
        );
        let bootstrap = app
            .oneshot(
                Request::get("/api/v1/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bootstrap = json(bootstrap).await;
        assert_eq!(bootstrap["active_show"]["id"], copy_id);
        assert_eq!(
            bootstrap["active_show"]["revision_copy"]["show_id"],
            source_id
        );
        assert!(
            std::fs::read_dir(data_dir.join("backups"))
                .unwrap()
                .flatten()
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("Destination-"))
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn rest_session_show_and_revision_flow() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"username":"Operator"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let session = json(response).await;
        let token = session["token"].as_str().unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/shows")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(
                        r#"{"name":"Tour","data_base64":null,"overwrite":false}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let show = json(response).await;
        let show_id = show["id"].as_str().unwrap();
        let uri = format!("/api/v1/shows/{show_id}/objects/group/front");
        let response = app
            .clone()
            .oneshot(
                Request::put(&uri)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::IF_MATCH, "0")
                    .body(Body::from(r#"{"name":"Front","fixtures":[]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::ETAG], "\"1\"");
        let conflict = app
            .clone()
            .oneshot(
                Request::put(&uri)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::IF_MATCH, "0")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let objects = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{show_id}/objects/group"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(json(objects).await.as_array().unwrap().len(), 1);
        assert!(
            std::fs::read_dir(data_dir.join("backups"))
                .unwrap()
                .next()
                .is_some()
        );
        let configuration=app.clone().oneshot(Request::put("/api/v1/configuration").header(header::CONTENT_TYPE,"application/json").header(header::AUTHORIZATION,format!("Bearer {token}")).body(Body::from(r#"{"frame_rate_hz":40,"output_bind_ip":"0.0.0.0","osc_bind":null,"art_timecode_bind":null,"backup_retention":5,"speed_groups_bpm":[101,102,103,104],"programmer_fade_millis":1250,"sequence_master_fade_millis":2500}"#)).unwrap()).await.unwrap();
        assert_eq!(configuration.status(), StatusCode::OK);
        assert_eq!(state.output_rate.load(Ordering::Relaxed), 40);
        assert_eq!(
            state.configuration.read().speed_groups_bpm,
            [101.0, 102.0, 103.0, 104.0, 15.0]
        );
        assert_eq!(state.configuration.read().programmer_fade_millis, 1_250);
        assert_eq!(
            state.configuration.read().sequence_master_fade_millis,
            2_500
        );
        let user = app
            .clone()
            .oneshot(
                Request::post("/api/v1/users")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"name":"Video","enabled":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(user.status(), StatusCode::CREATED);
        assert!(authenticate_token(&state, "not-a-session-token").is_err());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn operational_show_programmer_playback_and_rollback_flow() {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let first = create_show(&app, &token, "Programmed").await;
        let first_id = first["id"].as_str().unwrap();
        let physical = light_core::FixtureId::new();
        let fixture = light_fixture::PatchedFixture {
            name: "Media Server".into(),
            layer_id: "default".into(),
            fixture_id: physical,
            fixture_number: None,
            virtual_fixture_number: None,
            definition: light_fixture::FixtureDefinition {
                schema_version: 1,
                id: light_core::FixtureId::new(),
                revision: 1,
                manufacturer: "Test".into(),
                device_type: "dimmer".into(),
                name: "Dimmer".into(),
                model: "Dimmer".into(),
                mode: "1ch".into(),
                footprint: 1,
                heads: vec![light_fixture::LogicalHead {
                    index: 0,
                    name: "Main".into(),
                    shared: true,
                    parameters: vec![light_fixture::Parameter {
                        attribute: light_core::AttributeKey::intensity(),
                        components: vec![light_fixture::ChannelComponent {
                            offset: 0,
                            byte_order: light_fixture::ByteOrder::MsbFirst,
                        }],
                        default: 0.0,
                        virtual_dimmer: false,
                        metadata: light_fixture::ParameterMetadata::default(),
                        capabilities: vec![],
                    }],
                }],
                color_calibration: None,
                physical: Default::default(),
                model_asset: None,
                icon_asset: None,
                hazardous: false,
                direct_control_protocols: Vec::new(),
                signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
                safe_values: std::collections::BTreeMap::new(),
                profile_id: None,
                mode_id: None,
                profile_snapshot: None,
            },
            universe: Some(1),
            address: Some(1),
            split_patches: Vec::new(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            move_in_black_enabled: true,
            highlight_overrides: Default::default(),
            move_in_black_delay_millis: 0,
            multipatch: vec![],
        };
        let cue_list_id = light_core::CueListId::new();
        let mut cue = light_playback::Cue::new(1.0);
        cue.changes.push(light_playback::CueChange::set(
            physical,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(1.0),
        ));
        let cue_list = light_playback::CueList {
            id: cue_list_id,
            name: "Main".into(),
            priority: 10,
            mode: light_playback::CueListMode::Sequence,
            looped: false,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_step_millis: 1_000,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_group: None,
            speed_multiplier: 1.0,
            cues: vec![cue],
        };
        let route = light_output::OutputRoute {
            protocol: light_output::Protocol::Sacn,
            logical_universe: 1,
            destination_universe: 1,
            delivery_mode: Some(light_output::DeliveryMode::Multicast),
            destination: None,
            enabled: true,
            minimum_slots: 512,
        };
        assert_eq!(
            put_show_object(
                &app,
                &token,
                first_id,
                "patched_fixture",
                "dimmer",
                serde_json::to_value(fixture).unwrap()
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            put_show_object(
                &app,
                &token,
                first_id,
                "cue_list",
                "main",
                serde_json::to_value(cue_list).unwrap()
            )
            .await
            .status(),
            StatusCode::OK
        );
        assert_eq!(
            put_show_object(
                &app,
                &token,
                first_id,
                "route",
                "sacn",
                serde_json::to_value(route).unwrap()
            )
            .await
            .status(),
            StatusCode::OK
        );
        let opened = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{first_id}/open"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        assert_eq!(state.engine.snapshot().fixtures.len(), 1);
        let patch = app
            .clone()
            .oneshot(Request::get("/api/v1/patch").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(json(patch).await["fixtures"].as_array().unwrap().len(), 1);
        let override_response = app
            .clone()
            .oneshot(
                Request::put("/api/v1/dmx/override")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"universe":1,"address":1,"value":200}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(override_response.status(), StatusCode::OK);
        assert_eq!(
            state.output_control.lock().raw_overrides.get(&(1, 1)),
            Some(&200)
        );
        let dmx = app
            .clone()
            .oneshot(Request::get("/api/v1/dmx").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(json(dmx).await["overrides"].as_array().unwrap().len(), 1);
        let set=app.clone().oneshot(Request::post("/api/v1/programmer/set").header(header::CONTENT_TYPE,"application/json").header(header::AUTHORIZATION,format!("Bearer {token}")).body(Body::from(serde_json::json!({"fixture_id":physical,"attribute":"intensity","value":0.5}).to_string())).unwrap()).await.unwrap();
        assert_eq!(set.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            state
                .engine
                .render(RenderOptions::default())
                .unwrap()
                .universes[&1][0],
            128
        );
        let session = authenticate_token(&state, &token).unwrap();
        assert_eq!(
            execute_programmer_command(&state, &session, "FIXTURE 1").unwrap(),
            1
        );
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            vec![physical]
        );
        assert_eq!(
            execute_programmer_command(&state, &session, "FIXTURE 1 AT 25 TIME 0").unwrap(),
            1
        );
        assert_eq!(
            state
                .engine
                .render(RenderOptions::default())
                .unwrap()
                .universes[&1][0],
            64
        );
        assert_eq!(
            state.programmers.get(session.id).unwrap().selected,
            vec![physical]
        );
        let preset = light_programmer::Preset {
            name: "Three quarter".into(),
            family: light_programmer::PresetFamily::Intensity,
            number: 1,
            values: std::collections::HashMap::from([(
                physical,
                std::collections::HashMap::from([(
                    light_core::AttributeKey::intensity(),
                    light_core::AttributeValue::Normalized(0.75),
                )]),
            )]),
            group_values: std::collections::HashMap::new(),
        };
        assert_eq!(
            put_show_object(
                &app,
                &token,
                first_id,
                "preset",
                "1.1",
                serde_json::to_value(preset).unwrap()
            )
            .await
            .status(),
            StatusCode::OK
        );
        let applied = dispatch_ws_command(
            &state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "preset-test".into(),
                session_id: session.id,
                expected_revision: None,
                command: "preset.apply".into(),
                payload: serde_json::json!({"family":"Intensity","number":1}),
            },
        );
        assert!(applied.ok);
        assert_eq!(
            state.programmers.get(session.id).unwrap().values[0].fade_millis,
            Some(3_000),
            "preset.apply resolves Programmer Fade when recalling the value"
        );
        assert_eq!(
            state.programmers.get(session.id).unwrap().values[0]
                .value
                .normalized(),
            Some(0.75),
            "preset.apply exposes the recalled target before the resolved fade finishes"
        );
        apply_command_preset(&state, &session, "1.1", &[physical]).unwrap();
        assert_eq!(
            state.programmers.get(session.id).unwrap().values[0].fade_millis,
            Some(3_000),
            "command-line preset recall resolves Programmer Fade when recalling the value"
        );
        let go = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/playbacks/{}/go", cue_list_id.0))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(go.status(), StatusCode::OK);
        let playback = app
            .clone()
            .oneshot(
                Request::get("/api/v1/playbacks")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(json(playback).await["active"].as_array().unwrap().len(), 1);
        let diagnostics = app
            .clone()
            .oneshot(
                Request::get("/api/v1/diagnostics")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(diagnostics.status(), StatusCode::OK);
        let ready = app
            .clone()
            .oneshot(
                Request::get("/api/v1/readiness")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ready.status(), StatusCode::OK);
        let download = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{first_id}/download"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(download.status(), StatusCode::OK);
        assert_eq!(
            download.headers()[header::CONTENT_TYPE],
            "application/vnd.light.show"
        );
        let second = create_show(&app, &token, "Second").await;
        let second_id = second["id"].as_str().unwrap();
        let opened = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{second_id}/open"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(
                        r#"{"transition":"timed_fade","transition_millis":100}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        let rolled_back = app
            .clone()
            .oneshot(
                Request::post("/api/v1/shows/rollback")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rolled_back.status(), StatusCode::OK);
        assert_eq!(
            state.active_show.read().as_ref().unwrap().id.0.to_string(),
            first_id
        );
        let deleted = app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/shows/{second_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
        let active_delete = app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/shows/{first_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(active_delete.status(), StatusCode::CONFLICT);
        let disconnected = app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/sessions/{session_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(disconnected.status(), StatusCode::NO_CONTENT);
        assert!(
            !state
                .programmers
                .get(SessionId(Uuid::parse_str(&session_id).unwrap()))
                .unwrap()
                .connected
        );
        let (second_token, _) = login(&app, "Operator").await;
        let cleared = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/programmers/{session_id}/clear"))
                    .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cleared.status(), StatusCode::NO_CONTENT);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn malformed_show_upload_is_rejected_before_library_insert() {
        let (state, data_dir) = test_state();
        let app = router(state);
        let login = app
            .clone()
            .oneshot(
                Request::post("/api/v1/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"username":"Operator"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let token = json(login).await["token"].as_str().unwrap().to_owned();
        let encoded=STANDARD.encode(b"not sqlite but made long enough to pass an old superficial size check; this payload is deliberately invalid and should never enter the library........................................");
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/shows")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(
                        serde_json::json!({"name":"Bad","data_base64":encoded,"overwrite":false})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let shows = app
            .oneshot(Request::get("/api/v1/shows").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(json(shows).await.as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn template_groups_preload_store_reload_and_late_patch_render_end_to_end() {
        use light_core::{AttributeKey, AttributeValue, FixtureId};
        use light_fixture::{
            ByteOrder, ChannelComponent, FixtureDefinition, LogicalHead, Parameter,
            ParameterMetadata, PatchedFixture, SignalLossPolicy,
        };
        use light_playback::{Cue, CueList, CueListMode, CueTrigger, GroupCueChange};
        use light_programmer::{GroupDefinition, Preset};
        use std::collections::{BTreeMap, HashMap};

        fn parameter(
            attribute: &str,
            offsets: &[u16],
            default: f32,
            virtual_dimmer: bool,
        ) -> Parameter {
            Parameter {
                attribute: AttributeKey(attribute.into()),
                components: offsets
                    .iter()
                    .map(|offset| ChannelComponent {
                        offset: *offset,
                        byte_order: ByteOrder::MsbFirst,
                    })
                    .collect(),
                default,
                virtual_dimmer,
                metadata: ParameterMetadata::default(),
                capabilities: vec![],
            }
        }
        fn patched(
            name: String,
            address: u16,
            parameters: Vec<Parameter>,
            footprint: u16,
        ) -> PatchedFixture {
            PatchedFixture {
                name: name.clone(),
                layer_id: "default".into(),
                fixture_id: FixtureId::new(),
                fixture_number: None,
                virtual_fixture_number: None,
                definition: FixtureDefinition {
                    schema_version: 1,
                    id: FixtureId::new(),
                    revision: 1,
                    manufacturer: "Scenario Test".into(),
                    device_type: "other".into(),
                    name: name.clone(),
                    model: name,
                    mode: format!("{footprint} channel"),
                    footprint,
                    heads: vec![LogicalHead {
                        index: 0,
                        name: "Main".into(),
                        shared: true,
                        parameters,
                    }],
                    color_calibration: None,
                    physical: Default::default(),
                    model_asset: None,
                    icon_asset: None,
                    hazardous: false,
                    direct_control_protocols: vec![],
                    signal_loss_policy: SignalLossPolicy::HoldLast,
                    safe_values: BTreeMap::new(),
                    profile_id: None,
                    mode_id: None,
                    profile_snapshot: None,
                },
                universe: Some(1),
                address: Some(address),
                split_patches: Vec::new(),
                direct_control: None,
                location: Default::default(),
                rotation: Default::default(),
                logical_heads: vec![],
                move_in_black_enabled: true,
                move_in_black_delay_millis: 0,
                highlight_overrides: BTreeMap::new(),
                multipatch: vec![],
            }
        }
        fn dimmer(name: &str, address: u16) -> PatchedFixture {
            patched(
                name.into(),
                address,
                vec![parameter("intensity", &[0], 0.0, false)],
                1,
            )
        }
        fn profile(number: usize, address: u16) -> PatchedFixture {
            patched(
                format!("Profile {number}"),
                address,
                vec![
                    parameter("pan", &[0, 1], 0.5, false),
                    parameter("tilt", &[2, 3], 0.5, false),
                    parameter("intensity", &[4], 0.0, false),
                    parameter("shutter", &[5], 1.0, false),
                    parameter("color.emitter.red", &[6], 0.0, false),
                    parameter("color.emitter.green", &[7], 0.0, false),
                    parameter("color.emitter.blue", &[8], 0.0, false),
                    parameter("gobo", &[9], 0.0, false),
                    parameter("zoom", &[10], 0.0, false),
                    parameter("focus", &[11], 0.0, false),
                ],
                12,
            )
        }
        fn led(number: usize, address: u16) -> PatchedFixture {
            patched(
                format!("RGBW LED PAR {number}"),
                address,
                vec![
                    parameter("color.emitter.red", &[0], 0.0, true),
                    parameter("color.emitter.green", &[1], 0.0, true),
                    parameter("color.emitter.blue", &[2], 0.0, true),
                    parameter("color.emitter.white", &[3], 0.0, true),
                ],
                4,
            )
        }
        fn word(frame: &[u8; 512], address: u16) -> u16 {
            let offset = usize::from(address - 1);
            u16::from_be_bytes([frame[offset], frame[offset + 1]])
        }

        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let session_id = SessionId(Uuid::parse_str(&session_id).unwrap());
        let created = create_show(&app, &token, "Template group scenario").await;
        let show_id = light_core::ShowId(Uuid::parse_str(created["id"].as_str().unwrap()).unwrap());
        let entry = state.desk.lock().show(show_id).unwrap().unwrap();
        let store = ShowStore::open(&entry.path).unwrap();

        let dimmers = [
            "Front Left",
            "Front Mid Left",
            "Front Mid Right",
            "Front Right",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, name)| dimmer(name, index as u16 + 1))
        .collect::<Vec<_>>();
        let profiles = (0..6)
            .map(|index| profile(index + 1, 5 + index as u16 * 12))
            .collect::<Vec<_>>();
        let leds = (0..16)
            .map(|index| led(index + 1, 77 + index as u16 * 4))
            .collect::<Vec<_>>();
        let initial_fixtures = dimmers
            .iter()
            .chain(&profiles)
            .chain(&leds)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(initial_fixtures.len(), 26);
        for fixture in &initial_fixtures {
            store
                .put_object(
                    "patched_fixture",
                    &fixture.fixture_id.0.to_string(),
                    &serde_json::to_value(fixture).unwrap(),
                    0,
                )
                .unwrap();
        }

        let empty_groups = [
            ("front", "Front Light", 1),
            ("leds", "LEDs", 2),
            ("profile", "Profile", 3),
        ]
        .map(|(id, name, fader)| GroupDefinition {
            id: id.into(),
            name: name.into(),
            fixtures: vec![],
            master: 0.0,
            playback_fader: Some(fader),
            ..Default::default()
        });
        for group in &empty_groups {
            store
                .put_object("group", &group.id, &serde_json::to_value(group).unwrap(), 0)
                .unwrap();
        }

        let white = [
            ("intensity", AttributeValue::Normalized(1.0)),
            ("color.emitter.red", AttributeValue::Normalized(1.0)),
            ("color.emitter.green", AttributeValue::Normalized(1.0)),
            ("color.emitter.blue", AttributeValue::Normalized(1.0)),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let mut led_white = white.clone();
        led_white.insert("color.emitter.white", AttributeValue::Normalized(1.0));
        let preset = Preset {
            name: "All white at full".into(),
            family: light_programmer::PresetFamily::Mixed,
            number: 1,
            values: HashMap::new(),
            group_values: HashMap::from([
                (
                    "front".into(),
                    HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(1.0))]),
                ),
                (
                    "profile".into(),
                    white
                        .iter()
                        .map(|(key, value)| (AttributeKey((*key).into()), value.clone()))
                        .collect(),
                ),
                (
                    "leds".into(),
                    led_white
                        .iter()
                        .map(|(key, value)| (AttributeKey((*key).into()), value.clone()))
                        .collect(),
                ),
            ]),
        };
        store
            .put_object("preset", "0.1", &serde_json::to_value(&preset).unwrap(), 0)
            .unwrap();
        let cue_list_id = light_core::CueListId::new();
        let mut cue = Cue::new(1.0);
        cue.name = "All groups white".into();
        cue.trigger = CueTrigger::Manual;
        cue.group_changes = preset
            .group_values
            .iter()
            .flat_map(|(group_id, values)| {
                values.iter().map(move |(attribute, value)| GroupCueChange {
                    group_id: group_id.clone(),
                    attribute: attribute.clone(),
                    value: Some(value.clone()),
                    automatic_restore: false,
                    fade_millis: None,
                    delay_millis: None,
                })
            })
            .collect();
        let cue_list = CueList {
            id: cue_list_id,
            name: "Main".into(),
            priority: 0,
            mode: CueListMode::Sequence,
            looped: false,
            intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
            wrap_mode: Some(light_playback::WrapMode::Off),
            restart_mode: light_playback::RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_step_millis: 1_000,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_group: None,
            speed_multiplier: 1.0,
            cues: vec![cue],
        };
        let cue_object_id = cue_list_id.0.to_string();
        store
            .put_object(
                "cue_list",
                &cue_object_id,
                &serde_json::to_value(&cue_list).unwrap(),
                0,
            )
            .unwrap();

        *state.active_show.write() = Some(entry.clone());
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();
        state.engine.playback().write().go(cue_list_id).unwrap();
        let dark = state.engine.render(RenderOptions::default()).unwrap();
        let dark = dark.universes[&1];
        for fixture in &dimmers {
            assert_eq!(dark[usize::from(fixture.address.unwrap() - 1)], 0);
        }
        for fixture in &profiles {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(dark[offset + 4], 0);
            assert_eq!(&dark[offset + 6..=offset + 8], &[0; 3]);
        }
        for fixture in &leds {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(&dark[offset..offset + 4], &[0; 4]);
        }

        let populated_groups = [
            GroupDefinition {
                fixtures: dimmers.iter().map(|fixture| fixture.fixture_id).collect(),
                master: 1.0,
                ..empty_groups[0].clone()
            },
            GroupDefinition {
                fixtures: leds.iter().map(|fixture| fixture.fixture_id).collect(),
                master: 1.0,
                ..empty_groups[1].clone()
            },
            GroupDefinition {
                fixtures: profiles.iter().map(|fixture| fixture.fixture_id).collect(),
                master: 1.0,
                ..empty_groups[2].clone()
            },
        ];
        for group in &populated_groups {
            store
                .put_object("group", &group.id, &serde_json::to_value(group).unwrap(), 1)
                .unwrap();
        }
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();
        let white_frame = state
            .engine
            .render(RenderOptions::default())
            .unwrap()
            .universes[&1];
        for fixture in &dimmers {
            assert_eq!(white_frame[usize::from(fixture.address.unwrap() - 1)], 255);
        }
        for fixture in &profiles {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(&white_frame[offset + 4..=offset + 8], &[255; 5]);
        }
        for fixture in &leds {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(&white_frame[offset..offset + 4], &[255; 4]);
        }

        state
            .programmers
            .set_modes(session_id, Some(true), None, None, None);
        state.programmers.set_preload_group(
            session_id,
            "profile".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.25),
        );
        state.programmers.set_preload_group(
            session_id,
            "profile".into(),
            AttributeKey("tilt".into()),
            AttributeValue::Normalized(0.75),
        );
        let before_go = state
            .engine
            .render(RenderOptions::default())
            .unwrap()
            .universes[&1];
        assert_eq!(before_go, white_frame);
        state.programmers.activate_preload(session_id);
        let after_go = state
            .engine
            .render(RenderOptions::default())
            .unwrap()
            .universes[&1];
        assert_eq!(word(&after_go, profiles[0].address.unwrap()), 16_384);
        assert_eq!(word(&after_go, profiles[0].address.unwrap() + 2), 49_151);

        let stored = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{}/preload/store", show_id.0))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::IF_MATCH, "1")
                    .body(Body::from(
                        serde_json::json!({"target":"cue","target_id":cue_object_id,"cue_number":2.0,"name":"Preloaded position"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stored.status(), StatusCode::OK);
        assert_eq!(json(stored).await["revision"], 2);
        let persisted_after_store: CueList = serde_json::from_value(
            ShowStore::open(&entry.path)
                .unwrap()
                .objects("cue_list")
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
                .body,
        )
        .unwrap();
        assert_eq!(persisted_after_store.cues.len(), 2);
        assert!(
            persisted_after_store.cues[1]
                .group_changes
                .iter()
                .any(|change| {
                    change.group_id == "profile"
                        && change.attribute.0 == "pan"
                        && change.value.as_ref().and_then(AttributeValue::normalized) == Some(0.25)
                })
        );
        let programmer = state.programmers.get(session_id).unwrap();
        assert!(programmer.preload_active.is_empty());
        assert!(programmer.preload_group_active.is_empty());

        state
            .programmers
            .set_modes(session_id, Some(true), None, None, None);
        state.programmers.set_preload_group(
            session_id,
            "profile".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.8),
        );
        state.programmers.set_preload_group(
            session_id,
            "profile".into(),
            AttributeKey("tilt".into()),
            AttributeValue::Normalized(0.2),
        );
        state
            .engine
            .playback()
            .write()
            .jump(cue_list_id, 2.0)
            .unwrap();
        let recalled = state
            .engine
            .render(RenderOptions::default())
            .unwrap()
            .universes[&1];
        assert_eq!(word(&recalled, profiles[0].address.unwrap()), 16_384);
        assert_eq!(word(&recalled, profiles[0].address.unwrap() + 2), 49_151);
        state.programmers.activate_preload(session_id);
        let second_go = state
            .engine
            .render(RenderOptions::default())
            .unwrap()
            .universes[&1];
        assert_eq!(word(&second_go, profiles[0].address.unwrap()), 52_428);
        assert_eq!(word(&second_go, profiles[0].address.unwrap() + 2), 13_107);

        let export = data_dir.join("template-group-scenario.show");
        store.backup_to(&export).unwrap();
        validate_show_file(&export).unwrap();
        let reopened = ShowEntry {
            path: export.to_string_lossy().into_owned(),
            ..entry.clone()
        };
        let reopened_snapshot = load_engine_snapshot(&reopened).unwrap();
        assert_eq!(reopened_snapshot.fixtures.len(), 26);
        assert_eq!(reopened_snapshot.groups.len(), 3);
        assert_eq!(reopened_snapshot.cue_lists[0].cues.len(), 2);

        let extra_profiles = vec![profile(7, 141), profile(8, 153)];
        for fixture in &extra_profiles {
            store
                .put_object(
                    "patched_fixture",
                    &fixture.fixture_id.0.to_string(),
                    &serde_json::to_value(fixture).unwrap(),
                    0,
                )
                .unwrap();
        }
        let expanded_profile = GroupDefinition {
            fixtures: populated_groups[2]
                .fixtures
                .iter()
                .copied()
                .chain(extra_profiles.iter().map(|fixture| fixture.fixture_id))
                .collect(),
            ..populated_groups[2].clone()
        };
        store
            .put_object(
                "group",
                "profile",
                &serde_json::to_value(expanded_profile).unwrap(),
                2,
            )
            .unwrap();
        state.programmers.release_preload(session_id);
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();
        state
            .engine
            .playback()
            .write()
            .jump(cue_list_id, 2.0)
            .unwrap();
        let expanded = state
            .engine
            .render(RenderOptions::default())
            .unwrap()
            .universes[&1];
        for fixture in &extra_profiles {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(expanded[offset + 4], 255);
            assert_eq!(&expanded[offset + 6..=offset + 8], &[255; 3]);
            assert_eq!(word(&expanded, fixture.address.unwrap()), 16_384);
            assert_eq!(word(&expanded, fixture.address.unwrap() + 2), 49_151);
        }

        let stored_cue_list: CueList = serde_json::from_value(
            store
                .objects("cue_list")
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
                .body,
        )
        .unwrap();
        assert!(
            stored_cue_list.cues[1]
                .group_changes
                .iter()
                .any(|change| change.group_id == "profile" && change.attribute.0 == "pan")
        );
        assert_eq!(
            stored_cue_list.cues[1]
                .group_changes
                .iter()
                .filter(|change| change.group_id == "profile")
                .count(),
            2
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
