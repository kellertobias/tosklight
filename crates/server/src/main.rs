#![forbid(unsafe_code)]

mod default_show;
mod help;

use anyhow::Context;
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
use light_control::{
    ControlAction, ControlEvent, ControlInput, FrameRate, MidiControlInput, OscArgument,
    RtpMidiInput, SmpteTimecode, TimecodeRouter, TimecodeSourceConfig, UdpControlInput,
    UdpInputProtocol, encode_osc_message,
};
use light_core::{ApplicationClock, ManualClock, SessionId, SharedClock, SystemClock};
use light_engine::{Engine, EngineSnapshot, RenderOptions};
use light_media::{CitpClient, LibraryId, MediaCache, PreviewKey, ThumbnailKey};
use light_output::{NetworkOutput, OutputHealth, run_scheduler_dynamic};
use light_programmer::ProgrammerRegistry;
use light_show::{
    ControlDesk, DeskStore, DeskUser, PersistedSession, ScreenConfiguration, ShowEntry,
    ShowRevision, ShowStore, initialise_show, validate_show_file,
};
use parking_lot::{Mutex, RwLock};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    env, io,
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

#[derive(Clone)]
struct AppState {
    desk: Arc<Mutex<DeskStore>>,
    fixture_library: Arc<Mutex<light_fixture::FixtureLibrary>>,
    data_dir: PathBuf,
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    ws_connections: Arc<Mutex<HashMap<SessionId, u32>>>,
    programmers: ProgrammerRegistry,
    engine: Arc<Engine>,
    output_health: Arc<std::sync::Mutex<OutputHealth>>,
    output_rate: Arc<AtomicU16>,
    configuration: Arc<RwLock<DeskConfiguration>>,
    output_control: Arc<Mutex<OutputControl>>,
    activation_lock: Arc<tokio::sync::Mutex<()>>,
    timecode_router: Arc<Mutex<TimecodeRouter>>,
    active_show: Arc<RwLock<Option<ShowEntry>>>,
    active_show_error: Arc<RwLock<Option<String>>>,
    events: broadcast::Sender<Event>,
    audit_events: Arc<Mutex<VecDeque<Event>>>,
    event_revision: Arc<AtomicU64>,
    desk_token: Option<Arc<str>>,
    shutdown: CancellationToken,
    media_cache: Arc<Mutex<MediaCache>>,
    media_status: Arc<RwLock<HashMap<light_core::FixtureId, MediaServerStatus>>>,
    input_locks: Arc<Mutex<HashMap<String, (light_core::UserId, Instant)>>>,
    osc_subscribers: Arc<Mutex<HashMap<String, OscSubscriber>>>,
    osc_feedback: Option<Arc<std::net::UdpSocket>>,
    mvr_imports: Arc<Mutex<HashMap<Uuid, StagedMvrImport>>>,
    network_output: Option<Arc<NetworkOutput>>,
    output_sequences: Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
    manual_clock: Option<Arc<ManualClock>>,
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
}

#[derive(RustEmbed)]
#[folder = "../../apps/control-ui/dist"]
struct ControlUiAssets;
#[derive(Default)]
struct OutputControl {
    options: RenderOptions,
    hold: bool,
    last_frames: HashMap<light_core::Universe, light_output::DmxFrame>,
    raw_overrides: HashMap<(light_core::Universe, light_core::DmxAddress), u8>,
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
    token: String,
    user: DeskUser,
    desk: ControlDesk,
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
    preset: light_programmer::Preset,
}
#[derive(Deserialize)]
struct PreloadStoreInput {
    target: String,
    target_id: String,
    cue_number: Option<f64>,
    name: Option<String>,
    mode: Option<light_programmer::PresetStoreMode>,
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
struct Bootstrap {
    api_version: &'static str,
    users: Vec<DeskUser>,
    desks: Vec<ControlDesk>,
    active_show: Option<ShowEntry>,
    active_programmers: Vec<light_programmer::ProgrammerState>,
    frame_rate_hz: u16,
    output_health: OutputHealth,
    active_timecode_source: Option<String>,
    active_timecode: Option<String>,
    active_show_error: Option<String>,
    hardware_connected: bool,
}

fn default_speed_groups() -> [u16; 5] {
    [120, 90, 60, 30, 15]
}
fn deserialize_speed_groups<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<[u16; 5], D::Error> {
    let values = Vec::<u16>::deserialize(deserializer)?;
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
    speed_groups_bpm: [u16; 5],
    programmer_fade_millis: u64,
    sequence_master_fade_millis: u64,
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
            programmer_fade_millis: 3_000,
            sequence_master_fade_millis: 3_000,
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
            .any(|bpm| !(1..=999).contains(bpm))
        {
            return Err(ApiError::bad_request(
                "speed_groups_bpm values must be 1-999",
            ));
        }
        if self.programmer_fade_millis > 60_000 || self.sequence_master_fade_millis > 60_000 {
            return Err(ApiError::bad_request(
                "fade times must be 0-60000 milliseconds",
            ));
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("light_server=info,tower_http=info")
        .init();
    let mut data_dir = PathBuf::from("light-data");
    let mut bind = "127.0.0.1:5000".parse::<SocketAddr>()?;
    let mut test_bench = false;
    let mut osc_bind_override = None;
    let mut output_bind_override = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => data_dir = args.next().context("--data-dir requires a path")?.into(),
            "--bind" => bind = args.next().context("--bind requires an address")?.parse()?,
            "--test-bench" => test_bench = true,
            "--osc-bind" => {
                osc_bind_override = Some(
                    args.next()
                        .context("--osc-bind requires an address")?
                        .parse()?,
                )
            }
            "--output-bind-ip" => {
                output_bind_override = Some(
                    args.next()
                        .context("--output-bind-ip requires an address")?
                        .parse()?,
                )
            }
            "--help" => {
                println!(
                    "light-server [--data-dir PATH] [--bind ADDRESS] [--test-bench] [--osc-bind ADDRESS] [--output-bind-ip ADDRESS]"
                );
                return Ok(());
            }
            _ => anyhow::bail!("unknown option: {arg}"),
        }
    }
    if test_bench && !bind.ip().is_loopback() {
        anyhow::bail!("--test-bench requires a loopback HTTP bind");
    }
    std::fs::create_dir_all(data_dir.join("shows"))?;
    tracing::info!(path=%data_dir.display(), "opening desk data");
    let desk = DeskStore::open(data_dir.join("desk.sqlite"))?;
    tracing::info!("opening fixture library");
    let mut fixture_library =
        light_fixture::FixtureLibrary::open(data_dir.join("fixtures.sqlite"))?;
    let seeded = fixture_library.ensure_builtin_generics()?;
    if seeded > 0 {
        tracing::info!(
            profiles = seeded,
            "installed built-in Generic fixture modes"
        );
    }
    tracing::info!("fixture library ready");
    let mut configuration: DeskConfiguration = desk
        .setting("server_configuration")?
        .map(|json| serde_json::from_str(&json))
        .transpose()?
        .unwrap_or_default();
    if configuration.osc_bind.is_none() {
        configuration.osc_bind = Some(SocketAddr::from(([127, 0, 0, 1], 9000)));
    }
    if let Some(osc_bind) = osc_bind_override {
        configuration.osc_bind = Some(osc_bind);
    }
    if let Some(output_bind_ip) = output_bind_override {
        configuration.output_bind_ip = output_bind_ip;
    }
    configuration
        .validate()
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let active_show = desk.active_show()?;
    tracing::info!(active_show=?active_show.as_ref().map(|show| &show.name), "desk state loaded");
    let manual_clock = test_bench.then(|| Arc::new(ManualClock::new(fixed_test_time())));
    let application_clock: SharedClock = manual_clock
        .as_ref()
        .map(|clock| Arc::clone(clock) as SharedClock)
        .unwrap_or_else(|| Arc::new(SystemClock));
    let programmers = ProgrammerRegistry::with_clock(application_clock);
    let users = desk.users()?;
    for persisted in desk.persisted_sessions()? {
        if !users.iter().any(|user| user.id == persisted.user_id) {
            continue;
        }
        match serde_json::from_str::<light_programmer::ProgrammerState>(&persisted.programmer_json)
        {
            Ok(mut programmer) => {
                programmer.connected = false;
                programmers.restore(programmer);
            }
            Err(error) => {
                tracing::warn!(session_id=%persisted.id.0, %error, "ignoring invalid persisted programmer")
            }
        }
    }
    tracing::info!("persisted programmers restored");
    let (events, _) = broadcast::channel(256);
    let engine = Arc::new(Engine::new(programmers.clone()));
    let mut active_show_error = None;
    if let Some(active) = &active_show {
        tracing::info!(show=%active.name, "compiling active show");
        if let Some(message) = compile_active_show_for_startup(&engine, active) {
            let error = &message;
            tracing::error!(show=%active.name, %error, "starting in show recovery mode");
            active_show_error = Some(message);
        }
    }
    tracing::info!("engine snapshot ready");
    engine.set_control_timing(
        configuration.speed_groups_bpm,
        configuration.programmer_fade_millis,
        configuration.sequence_master_fade_millis,
    );
    let output_health = Arc::new(std::sync::Mutex::new(OutputHealth::default()));
    let timecode_router = Arc::new(Mutex::new(TimecodeRouter::default()));
    timecode_router
        .lock()
        .configure(configuration.timecode_sources.clone());
    let output_rate = Arc::new(AtomicU16::new(configuration.frame_rate_hz));
    let output = Arc::new(
        NetworkOutput::bind(
            configuration.output_bind_ip,
            *Uuid::new_v4().as_bytes(),
            "Light",
        )
        .await?,
    );
    let output_cancel = CancellationToken::new();
    let scheduler_cancel = output_cancel.clone();
    let scheduler_engine = Arc::clone(&engine);
    let scheduler_output = Arc::clone(&output);
    let scheduler_health = Arc::clone(&output_health);
    let scheduler_rate = Arc::clone(&output_rate);
    let sequences = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let scheduler_sequences = Arc::clone(&sequences);
    let output_control = Arc::new(Mutex::new(OutputControl::default()));
    let scheduler_control = Arc::clone(&output_control);
    let scheduler_timecode = Arc::clone(&timecode_router);
    let scheduler = tokio::spawn(async move {
        if test_bench {
            scheduler_cancel.cancelled().await;
        } else {
            run_scheduler_dynamic(
                scheduler_rate,
                scheduler_cancel.clone(),
                scheduler_health,
                || {
                    let engine = Arc::clone(&scheduler_engine);
                    let output = Arc::clone(&scheduler_output);
                    let sequences = Arc::clone(&scheduler_sequences);
                    let control = Arc::clone(&scheduler_control);
                    let timecode = Arc::clone(&scheduler_timecode);
                    async move {
                        let current = { timecode.lock().poll_loss().cloned() };
                        engine.set_timecode_frame(current.map(|timecode| {
                            let fps = u64::from(timecode.rate.nominal_frames());
                            (u64::from(timecode.hours) * 3600
                                + u64::from(timecode.minutes) * 60
                                + u64::from(timecode.seconds))
                                * fps
                                + u64::from(timecode.frames)
                        }));
                        let options = control.lock().options;
                        let rendered = engine.render(options).map_err(io::Error::other)?;
                        let snapshot = engine.snapshot();
                        let frames = {
                            let mut control = control.lock();
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
                        output
                            .send_routes(&snapshot.routes, &frames, &mut *sequences.lock().await)
                            .await
                    }
                },
            )
            .await;
        }
        let snapshot = scheduler_engine.snapshot();
        let mut shutdown_options = scheduler_control.lock().options;
        shutdown_options.control_loss_progress = Some(1.0);
        if let Ok(safe) = scheduler_engine.render(shutdown_options) {
            let _ = scheduler_output
                .send_routes(
                    &snapshot.routes,
                    &safe.universes,
                    &mut *scheduler_sequences.lock().await,
                )
                .await;
        }
        let _ = scheduler_output
            .terminate_routes(&snapshot.routes, &mut *scheduler_sequences.lock().await)
            .await;
    });
    let state = AppState {
        desk: Arc::new(Mutex::new(desk)),
        fixture_library: Arc::new(Mutex::new(fixture_library)),
        data_dir,
        sessions: Arc::default(),
        ws_connections: Arc::new(Mutex::new(HashMap::new())),
        programmers,
        engine,
        output_health,
        output_rate,
        configuration: Arc::new(RwLock::new(configuration)),
        output_control,
        activation_lock: Arc::new(tokio::sync::Mutex::new(())),
        timecode_router,
        active_show: Arc::new(RwLock::new(active_show)),
        active_show_error: Arc::new(RwLock::new(active_show_error)),
        events,
        audit_events: Arc::new(Mutex::new(VecDeque::with_capacity(2048))),
        event_revision: Arc::new(AtomicU64::new(0)),
        desk_token: env::var("LIGHT_DESK_TOKEN")
            .ok()
            .filter(|token| !token.is_empty())
            .map(Arc::from),
        shutdown: output_cancel.clone(),
        media_cache: Arc::new(Mutex::new(MediaCache::default())),
        media_status: Arc::new(RwLock::new(HashMap::new())),
        input_locks: Arc::new(Mutex::new(HashMap::new())),
        osc_subscribers: Arc::new(Mutex::new(HashMap::new())),
        osc_feedback: Some(Arc::new(std::net::UdpSocket::bind("0.0.0.0:0")?)),
        mvr_imports: Arc::new(Mutex::new(HashMap::new())),
        network_output: Some(Arc::clone(&output)),
        output_sequences: Arc::clone(&sequences),
        manual_clock,
    };
    let input_tasks = spawn_control_inputs(&state, output_cancel.clone());
    let app = router(state);
    tracing::info!(%bind, "starting light control server");
    axum::serve(tokio::net::TcpListener::bind(bind).await?, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = output_cancel.cancelled() => {},
            }
            output_cancel.cancel();
        })
        .await?;
    let _ = scheduler.await;
    for task in input_tasks {
        let _ = task.await;
    }
    Ok(())
}

fn router(state: AppState) -> Router {
    let test_bench = state.manual_clock.is_some();
    let mut router = Router::new()
        .merge(help::router::<AppState>())
        .route("/", get(operator_ui))
        .route("/assets/{*path}", get(operator_asset))
        .route("/api/v1/health", get(health))
        .route("/api/v1/readiness", get(readiness))
        .route("/api/v1/version", get(version))
        .route("/api/v1/diagnostics", get(diagnostics))
        .route("/api/v1/bootstrap", get(bootstrap))
        .route("/api/v1/patch", get(patch_snapshot))
        .route(
            "/api/v1/fixture-library",
            get(list_fixture_library).put(put_fixture_library),
        )
        .route(
            "/api/v1/fixture-library/{id}/{revision}",
            delete(delete_fixture_library),
        )
        .route("/api/v1/visualization", get(visualization_snapshot))
        .route("/api/v1/media", get(media_servers))
        .route(
            "/api/v1/media/{fixture_id}/thumbnails/refresh",
            post(refresh_media_thumbnails),
        )
        .route("/api/v1/media/{fixture_id}/thumbnail", get(media_thumbnail))
        .route(
            "/api/v1/media/{fixture_id}/preview/refresh",
            post(refresh_media_preview),
        )
        .route(
            "/api/v1/media/{fixture_id}/preview/{source}",
            get(media_preview),
        )
        .route("/api/v1/dmx", get(dmx_snapshot))
        .route("/api/v1/dmx/override", put(update_dmx_override))
        .route("/api/v1/shutdown", post(shutdown_server))
        .route(
            "/api/v1/configuration",
            get(configuration).put(update_configuration),
        )
        .route("/api/v1/sessions", post(create_session))
        .route("/api/v1/sessions/{id}", delete(close_session))
        .route("/api/v1/users", post(create_user))
        .route("/api/v1/users/{id}", put(update_user).delete(delete_user))
        .route("/api/v1/shows", get(list_shows).post(upload_show))
        .route("/api/v1/shows/rollback", post(rollback_show))
        .route("/api/v1/shows/{id}", delete(delete_show))
        .route("/api/v1/shows/{id}/open", post(open_show))
        .route("/api/v1/shows/{id}/download", get(download_show))
        .route(
            "/api/v1/shows/{id}/revisions",
            get(list_show_revisions).post(save_show_revision),
        )
        .route(
            "/api/v1/shows/{id}/revisions/{revision}/open",
            post(open_show_revision),
        )
        .route("/api/v1/mvr/imports/preview", post(preview_mvr_import))
        .route("/api/v1/mvr/imports/{token}/apply", post(apply_mvr_import))
        .route("/api/v1/shows/{id}/mvr/preview", get(preview_mvr_export))
        .route("/api/v1/shows/{id}/mvr", get(export_mvr))
        .route("/api/v1/shows/{id}/objects/{kind}", get(list_objects))
        .route(
            "/api/v1/shows/{id}/objects/{kind}/{object_id}",
            put(put_object),
        )
        .route(
            "/api/v1/shows/{id}/objects/{kind}/{object_id}/undo",
            post(undo_object),
        )
        .route(
            "/api/v1/shows/{id}/presets/{preset_id}/store",
            post(store_preset),
        )
        .route("/api/v1/shows/{id}/preload/store", post(store_preload))
        .route("/api/v1/playbacks/{id}/{action}", post(playback_action))
        .route("/api/v1/cuelists/{number}", get(pool_playback_state))
        .route(
            "/api/v1/cuelists/{number}/{action}",
            post(pool_playback_action).put(pool_playback_action),
        )
        .route("/api/v1/qlists/{number}", get(pool_playback_state))
        .route(
            "/api/v1/qlists/{number}/{action}",
            post(pool_playback_action).put(pool_playback_action),
        )
        .route("/api/v1/playback-pool/{number}", get(pool_playback_state))
        .route(
            "/api/v1/playback-pool/{number}/{action}",
            post(pool_playback_action).put(pool_playback_action),
        )
        .route("/api/v1/control-desks/{id}/page", put(update_desk_page))
        .route("/api/v1/control-desks/{id}", put(update_control_desk))
        .route(
            "/api/v1/control-desks/{id}/page-playbacks/{slot}/{action}",
            post(paged_playback_action).put(paged_playback_action),
        )
        .route(
            "/api/v1/control-desks/{id}/paged-playbacks/{slot}/{action}",
            post(paged_playback_action).put(paged_playback_action),
        )
        .route("/api/v1/screens", get(list_screens))
        .route(
            "/api/v1/screens/{id}",
            put(put_screen).delete(delete_screen),
        )
        .route("/api/v1/screens/{id}/page", put(update_screen_page))
        .route("/api/v1/playbacks", get(playbacks))
        .route("/api/v1/programmers", get(list_programmers))
        .route("/api/v1/programmers/{id}/clear", post(clear_programmer))
        .route("/api/v1/programmer/set", post(set_programmer))
        .route("/api/v1/master", put(update_master))
        .route("/api/v1/midi/inputs", get(midi_inputs))
        .route("/api/v1/events", get(ws_events))
        .route("/api/v1/audit", get(audit_events));
    if test_bench {
        router = router
            .route("/api/v1/test/clock/reset", post(reset_test_clock))
            .route("/api/v1/test/clock/advance", post(advance_test_clock));
    }
    router
        .layer(middleware::from_fn_with_state(state.clone(), desk_boundary))
        .with_state(state)
        .layer(DefaultBodyLimit::max(256 * 1024 * 1024))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                .allow_headers([
                    header::AUTHORIZATION,
                    header::CONTENT_TYPE,
                    header::IF_MATCH,
                    header::HeaderName::from_static("x-light-desk-token"),
                ])
                .expose_headers([header::ETAG]),
        )
        .layer(TraceLayer::new_for_http())
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
    let rendered = state
        .engine
        .render(state.output_control.lock().options)
        .map_err(|error| ApiError::internal(error.to_string()))?;
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
    let packets = state
        .network_output
        .as_ref()
        .ok_or_else(|| ApiError::unavailable("network output is unavailable"))?
        .send_routes(
            &snapshot.routes,
            &frames,
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
    }
    send_osc_feedback(&state, true);
    Ok(Json(serde_json::json!({
        "now": now,
        "revision": rendered.revision,
        "packets_sent": packets,
        "universes": frames.into_iter().map(|(universe, slots)| serde_json::json!({"universe":universe,"slots":slots.to_vec()})).collect::<Vec<_>>(),
    })))
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

async fn desk_boundary(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(required) = &state.desk_token else {
        return next.run(request).await;
    };
    if request.uri().path() == "/"
        || request.uri().path().starts_with("/assets/")
        || request.uri().path().starts_with("/api/v1/help/assets/")
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
        serde_json::json!({"service":"light-server","version":env!("CARGO_PKG_VERSION"),"api_version":"v1","show_schema":3,"desk_schema":4}),
    )
}
async fn readiness(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    if let Some(show) = state.active_show.read().as_ref() {
        validate_show_file(&show.path).map_err(|error| ApiError::unavailable(error.to_string()))?;
    }
    Ok(Json(
        serde_json::json!({"status":"ready","active_show":state.active_show.read().as_ref().map(|show|show.id),"snapshot_revision":state.engine.snapshot().revision}),
    ))
}
async fn diagnostics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(
        serde_json::json!({"output":state.output_health.lock().expect("output health mutex poisoned").clone(),"event_queue_pressure":state.events.len(),"active_programmers":state.programmers.active(),"active_playbacks":state.engine.playback().read().active(),"timecode_source":state.timecode_router.lock().active_source(),"media_servers":state.media_status.read().clone(),"snapshot_revision":state.engine.snapshot().revision}),
    ))
}
async fn bootstrap(State(state): State<AppState>) -> Json<Bootstrap> {
    let (users, desks) = {
        let desk = state.desk.lock();
        (
            desk.users().unwrap_or_default(),
            desk.desks().unwrap_or_default(),
        )
    };
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
    Json(Bootstrap {
        api_version: "v1",
        users,
        desks,
        active_show: state.active_show.read().clone(),
        active_programmers: state.programmers.active(),
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
    let options = state.output_control.lock().options;
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
    Json(
        serde_json::json!({"configuration":state.configuration.read().clone(),"output_health":state.output_health.lock().expect("output health mutex poisoned").clone()}),
    )
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
    Json(configuration): Json<DeskConfiguration>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    configuration.validate()?;
    let previous = state.configuration.read().clone();
    state
        .desk
        .lock()
        .set_setting(
            "server_configuration",
            &serde_json::to_string(&configuration)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        )
        .map_err(ApiError::store)?;
    state
        .output_rate
        .store(configuration.frame_rate_hz, Ordering::Relaxed);
    state
        .timecode_router
        .lock()
        .configure(configuration.timecode_sources.clone());
    state.engine.set_control_timing(
        configuration.speed_groups_bpm,
        configuration.programmer_fade_millis,
        configuration.sequence_master_fade_millis,
    );
    let requires_restart = configuration.output_bind_ip != previous.output_bind_ip
        || configuration.osc_bind != previous.osc_bind
        || configuration.art_timecode_bind != previous.art_timecode_bind
        || configuration.midi_inputs != previous.midi_inputs
        || configuration.rtp_midi_bind != previous.rtp_midi_bind;
    *state.configuration.write() = configuration.clone();
    emit(
        &state,
        "server_configuration_changed",
        serde_json::json!({"configuration":configuration,"requires_restart":requires_restart}),
    );
    Ok(Json(
        serde_json::json!({"configuration":configuration,"requires_restart":requires_restart}),
    ))
}
async fn create_session(
    State(state): State<AppState>,
    Json(input): Json<CreateSession>,
) -> Result<Json<SessionResponse>, ApiError> {
    let user = state
        .desk
        .lock()
        .find_user(&input.username)
        .map_err(ApiError::store)?
        .filter(|u| u.enabled)
        .ok_or_else(|| ApiError::not_found("enabled user"))?;
    let desk = {
        let store = state.desk.lock();
        let remembered = match input.desk_id {
            Some(id) => store.control_desk(id).map_err(ApiError::store)?,
            None => None,
        };
        if let Some(desk) = remembered {
            desk
        } else {
            let client = input.client_id.unwrap_or_else(Uuid::new_v4);
            let suffix = client.simple().to_string();
            let alias = format!("desk-{}", &suffix[..8]);
            match store
                .control_desk_by_alias(&alias)
                .map_err(ApiError::store)?
            {
                Some(desk) => desk,
                None => store
                    .add_desk(&format!("Client {}", &suffix[..6]), &alias)
                    .map_err(ApiError::store)?,
            }
        }
    };
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: Uuid::new_v4().to_string(),
        connected: true,
        desk: desk.clone(),
    };
    state.programmers.start(session.id, user.id);
    state.sessions.write().insert(session.id, session.clone());
    persist_programmer(&state, &session)?;
    emit(
        &state,
        "session_started",
        serde_json::json!({"session_id":session.id,"user":user.name}),
    );
    Ok(Json(SessionResponse {
        session_id: session.id,
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
    state.programmers.disconnect(id);
    persist_programmer(&state, &session)?;
    emit(
        &state,
        "session_disconnected",
        serde_json::json!({"session_id":id}),
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
    let _activation = state.activation_lock.lock().await;
    backup_show(&state, &entry)?;
    ShowStore::open(&saved_revision.path)
        .map_err(ApiError::store)?
        .backup_to(&entry.path)
        .map_err(ApiError::store)?;
    let previous = state.active_show.read().clone();
    let transition = input.transition.unwrap_or(Transition::SafeBlackout);
    activate_snapshot(&state, compiled, &transition, input.transition_millis).await?;
    state
        .desk
        .lock()
        .set_active_show(Some(entry.id))
        .map_err(ApiError::store)?;
    if let Some(previous) = &previous
        && previous.id != entry.id
    {
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
        serde_json::json!({"show":entry,"saved_revision":saved_revision,"transition":transition}),
    );
    Ok(Json(entry))
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
        .upsert_show(&input.name, &path.display().to_string(), input.overwrite)
        .map_err(ApiError::store)?;
    emit(&state, "show_uploaded", serde_json::json!({"show":entry}));
    Ok((StatusCode::CREATED, Json(entry)))
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
    if let Some(id) = query.show_id {
        if let Some(show) = state
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
                .filter_map(|o| {
                    serde_json::from_value::<light_fixture::PatchedFixture>(o.body).ok()
                })
                .collect();
        }
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

fn mvr_definitions(
    state: &AppState,
    document: &light_mvr::MvrDocument,
) -> Result<
    (
        Vec<light_fixture::FixtureDefinition>,
        Vec<(light_fixture::FixtureDefinition, Vec<u8>)>,
    ),
    ApiError,
> {
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
                        .replace(' ', ".")
                        .replace('_', ".")
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
            .unwrap_or_else(light_core::FixtureId::new);
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
        let patched = light_fixture::PatchedFixture {
            fixture_id,
            fixture_number: source
                .fixture_id
                .as_deref()
                .and_then(|value| value.parse().ok()),
            name: source.name.clone(),
            definition: definition.clone(),
            universe,
            address,
            layer_id: source.layer.clone().unwrap_or_else(|| "default".into()),
            direct_control: None,
            location,
            rotation,
            logical_heads: heads,
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
    let mut assets = Vec::new();
    for geometry in &document.geometry {
        if let Some(data) = document.files.get(&geometry.file_name.to_ascii_lowercase()) {
            let encoded = STANDARD.encode(data);
            assets.push(serde_json::json!({"id":geometry.uuid,"mvrUuid":geometry.uuid,"name":geometry.name,"format":"glb","dataUrl":format!("data:model/gltf-binary;base64,{encoded}"),"position":{"x":geometry.matrix[9]/1000.0,"y":geometry.matrix[10]/1000.0,"z":geometry.matrix[11]/1000.0,"rotationX":0,"rotationY":0,"rotationZ":0},"scale":1}));
        }
    }
    if !assets.is_empty() {
        let layouts = store.objects("stage_layout").map_err(ApiError::store)?;
        let existing = layouts.iter().find(|o| o.id == "main");
        let mut body = existing.map(|o| o.body.clone()).unwrap_or_else(
            || serde_json::json!({"version":2,"positions":{},"positions3d":{},"assets":[]}),
        );
        let list = body
            .get_mut("assets")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| ApiError::bad_request("stage layout assets are invalid"))?;
        for asset in assets {
            let uuid = asset["mvrUuid"].clone();
            if let Some(slot) = list.iter_mut().find(|a| a.get("mvrUuid") == Some(&uuid)) {
                *slot = asset
            } else {
                list.push(asset)
            }
        }
        store
            .put_object(
                "stage_layout",
                "main",
                &body,
                existing.map(|o| o.revision).unwrap_or(0),
            )
            .map_err(ApiError::store)?;
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
        serde_json::json!({"show":entry,"fixtures":imported,"unresolved":unresolved,"scenery":staged.document.geometry.len()}),
    );
    Ok(Json(ApplyMvrResult {
        show: entry,
        imported_fixtures: imported,
        unresolved_fixtures: unresolved,
        imported_scenery: staged.document.geometry.len(),
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
    if let Some(layout) = store
        .objects("stage_layout")
        .map_err(ApiError::store)?
        .into_iter()
        .find(|o| o.id == "main")
    {
        if let Some(assets) = layout.body.get("assets").and_then(|v| v.as_array()) {
            for asset in assets {
                let Some(url) = asset.get("dataUrl").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(data) = url
                    .split_once(',')
                    .and_then(|(_, v)| STANDARD.decode(v).ok())
                else {
                    continue;
                };
                let uuid = asset
                    .get("mvrUuid")
                    .or_else(|| asset.get("id"))
                    .and_then(|v| v.as_str())
                    .and_then(|v| Uuid::parse_str(v).ok())
                    .unwrap_or_else(Uuid::new_v4);
                let file = format!("{}.glb", uuid);
                let p = &asset["position"];
                doc.geometry.push(light_mvr::MvrGeometry {
                    uuid,
                    name: asset
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Geometry")
                        .into(),
                    file_name: file.clone(),
                    matrix: [
                        1.,
                        0.,
                        0.,
                        0.,
                        1.,
                        0.,
                        0.,
                        0.,
                        1.,
                        p["x"].as_f64().unwrap_or(0.) * 1000.,
                        p["y"].as_f64().unwrap_or(0.) * 1000.,
                        p["z"].as_f64().unwrap_or(0.) * 1000.,
                    ],
                    layer: None,
                    class: None,
                });
                doc.files.insert(file.to_ascii_lowercase(), data);
            }
        }
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
    Ok(Json(
        ShowStore::open(entry.path)
            .map_err(ApiError::store)?
            .objects(&kind)
            .map_err(ApiError::store)?,
    ))
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
        light_fixture::reconcile_logical_heads(&mut fixture);
        body =
            serde_json::to_value(fixture).map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id)
    {
        let candidate =
            load_engine_snapshot_with_override(&entry, Some((&kind, &object_id, &body)))
                .map_err(ApiError::internal)?;
        candidate
            .validate()
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    backup_show(&state, &entry)?;
    let revision = store
        .put_object(&kind, &object_id, &body, expected)
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
    let existing = store
        .objects("preset")
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| object.id == preset_id);
    let mut preset: light_programmer::Preset = existing
        .map(|object| serde_json::from_value(object.body))
        .transpose()
        .map_err(|error| ApiError::bad_request(format!("invalid stored preset: {error}")))?
        .unwrap_or_default();
    preset.store(input.preset, input.mode);
    backup_show(&state, &entry)?;
    let revision = store
        .put_object(
            "preset",
            &preset_id,
            &serde_json::to_value(&preset)
                .map_err(|error| ApiError::internal(error.to_string()))?,
            expected,
        )
        .map_err(ApiError::store)?;
    emit(
        &state,
        "preset_stored",
        serde_json::json!({"show_id":show_id,"preset_id":preset_id,"revision":revision,"source_session":session.id}),
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
            let mut preset = light_programmer::Preset {
                name: input
                    .name
                    .unwrap_or_else(|| format!("Preset {}", input.target_id)),
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
            let existing = store
                .objects("preset")
                .map_err(ApiError::store)?
                .into_iter()
                .find(|object| object.id == input.target_id);
            let mut merged = existing
                .map(|object| serde_json::from_value::<light_programmer::Preset>(object.body))
                .transpose()
                .map_err(|error| ApiError::bad_request(error.to_string()))?
                .unwrap_or_default();
            merged.store(
                preset,
                input
                    .mode
                    .unwrap_or(light_programmer::PresetStoreMode::Merge),
            );
            store
                .put_object(
                    "preset",
                    &input.target_id,
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
    Ok(Json(
        serde_json::json!({"cue_lists":snapshot.cue_lists,"pool":snapshot.playbacks,"pages":snapshot.playback_pages,"active":state.engine.playback().read().active(),"desk":session.desk,"active_page":active_page}),
    ))
}

#[derive(Default, Deserialize)]
struct PoolPlaybackInput {
    value: Option<f32>,
    pressed: Option<bool>,
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
    if !state
        .engine
        .snapshot()
        .playback_pages
        .iter()
        .any(|page| page.number == input.page)
    {
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
    Ok(Json(serde_json::json!({"desk_id":id,"page":input.page})))
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
    if let light_playback::PlaybackTarget::Group { group_id } = &definition.target {
        let value = match action.as_str() {
            "on" => 1.0,
            "off" => 0.0,
            "master" => input
                .value
                .ok_or_else(|| ApiError::bad_request("master value is required"))?,
            "flash" => {
                if input.pressed.unwrap_or(true) {
                    1.0
                } else {
                    0.0
                }
            }
            _ => {
                return Err(ApiError::bad_request(
                    "action is not available for a group playback",
                ));
            }
        };
        if action == "flash" {
            state.engine.set_group_master_flash(group_id.clone(), value);
        } else {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(ApiError::bad_request("playback master must be within 0-1"));
            }
            let snapshot = state.engine.snapshot();
            let mut next = (*snapshot).clone();
            let group = next
                .groups
                .iter_mut()
                .find(|group| group.id == *group_id)
                .ok_or_else(|| ApiError::bad_request("group does not exist"))?;
            group.master = value;
            state
                .engine
                .replace_snapshot(next)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
    } else {
        let mut engine = state.engine.playback().write();
        match action.as_str() {
            "go" => {
                engine.go_playback(number).map_err(ApiError::bad_request)?;
            }
            "go-minus" | "back" => {
                engine
                    .back_playback(number)
                    .map_err(ApiError::bad_request)?;
            }
            "on" => engine.on(number).map_err(ApiError::bad_request)?,
            "off" => {
                engine.off(number).map_err(ApiError::bad_request)?;
            }
            "toggle" => {
                engine.toggle(number).map_err(ApiError::bad_request)?;
            }
            "master" => engine
                .set_master(
                    number,
                    input
                        .value
                        .ok_or_else(|| ApiError::bad_request("master value is required"))?,
                )
                .map_err(ApiError::bad_request)?,
            "flash" => engine
                .set_flash(number, input.pressed.unwrap_or(true))
                .map_err(ApiError::bad_request)?,
            "xfade-on" => engine.xfade(number, true).map_err(ApiError::bad_request)?,
            "xfade-off" => engine.xfade(number, false).map_err(ApiError::bad_request)?,
            _ => return Err(ApiError::not_found("playback action")),
        }
    }
    emit(
        &state,
        "playback_changed",
        serde_json::json!({"playback_number":number,"action":action,"session_id":session.id}),
    );
    let snapshot = state.engine.snapshot();
    Ok(Json(
        serde_json::json!({"playback":definition,"active":state.engine.playback().read().active(),"groups":snapshot.groups}),
    ))
}
async fn list_programmers(
    State(state): State<AppState>,
) -> Json<Vec<light_programmer::ProgrammerState>> {
    Json(state.programmers.active())
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
    // A programmer belongs to a user. Recreate one empty programmer and bind
    // every currently connected session for that user to it.
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
        state.programmers.disconnect(session.id);
        let _ = persist_programmer(&state, &session);
        emit(
            &state,
            "session_disconnected",
            serde_json::json!({"session_id":session.id}),
        );
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
    if command.protocol_version != 1 {
        return fail("unsupported protocol_version".into());
    }
    if command.session_id != session.id {
        return fail("session_id does not own this connection".into());
    }
    let live_absolute = matches!(
        command.command.as_str(),
        "selection.set"
            | "selection.macro"
            | "group.select"
            | "programmer.set"
            | "programmer.group.set"
            | "programmer.align"
            | "programmer.command_line"
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
            state.programmers.select(session.id, input.fixtures);
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
                value: f32,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            lock_live_input(
                state,
                session,
                format!("group:{}:{}", input.group_id, input.attribute),
            )?;
            if !input.value.is_finite() || !(0.0..=1.0).contains(&input.value) {
                return Err("value must be within 0-1".into());
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
            state.programmers.set_group(
                session.id,
                input.group_id,
                light_core::AttributeKey(input.attribute),
                light_core::AttributeValue::Normalized(input.value),
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "programmer.set" => {
            let input: ProgrammerSet =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            lock_live_input(
                state,
                session,
                format!("fixture:{}:{}", input.fixture_id.0, input.attribute),
            )?;
            if !input.value.is_finite() || !(0.0..=1.0).contains(&input.value) {
                return Err("value must be within 0-1".into());
            }
            state.programmers.set(
                session.id,
                input.fixture_id,
                light_core::AttributeKey(input.attribute),
                light_core::AttributeValue::Normalized(input.value),
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
        }
        "programmer.clear" => {
            state.programmers.clear_values(session.id);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"cleared":true}))
        }
        "preload.enter" => {
            state
                .programmers
                .set_modes(session.id, Some(true), None, None, None);
            persist_programmer(state, session).map_err(|e| e.message)?;
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
            state.programmers.set_preload_group(
                session.id,
                input.group_id,
                light_core::AttributeKey(input.attribute),
                light_core::AttributeValue::Normalized(input.value),
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"pending":true}))
        }
        "preload.go" => {
            state.programmers.activate_preload(session.id);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"active":true,"programmer":state.programmers.get(session.id)}))
        }
        "preload.clear" => {
            state.programmers.clear_preload_pending(session.id);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"pending_cleared":true,"active_unchanged":true}))
        }
        "preload.release" => {
            state.programmers.release_preload(session.id);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"released":true}))
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
            state.programmers.set_command_line(session.id, input.value);
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"updated":true}))
        }
        "programmer.execute" => {
            #[derive(Deserialize)]
            struct Input {
                value: String,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
            let applied = execute_programmer_command(state, session, &input.value)?;
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(
                serde_json::json!({"applied":applied,"programmer":state.programmers.get(session.id)}),
            )
        }
        "preset.apply" => {
            #[derive(Deserialize)]
            struct Input {
                preset_id: String,
            }
            let input: Input =
                serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
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
                .find(|object| object.id == input.preset_id)
                .ok_or("preset does not exist")?;
            let preset: light_programmer::Preset =
                serde_json::from_value(object.body).map_err(|e| e.to_string())?;
            let group_map = state
                .engine
                .snapshot()
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.clone()))
                .collect::<HashMap<_, _>>();
            let mut fixture_ids = preset.values.keys().copied().collect::<Vec<_>>();
            for group_id in preset.group_values.keys() {
                if let Ok(members) = light_programmer::resolve_group(group_id, &group_map) {
                    for fixture in members {
                        if !fixture_ids.contains(&fixture) {
                            fixture_ids.push(fixture);
                        }
                    }
                }
            }
            state.programmers.select(session.id, fixture_ids.clone());
            for (fixture_id, attributes) in preset.values {
                for (attribute, value) in attributes {
                    state
                        .programmers
                        .set_faded(session.id, fixture_id, attribute, value);
                }
            }
            for (group_id, attributes) in preset.group_values {
                if let Ok(members) = light_programmer::resolve_group(&group_id, &group_map) {
                    for (attribute, value) in attributes {
                        state.programmers.set_group_faded(
                            session.id,
                            group_id.clone(),
                            attribute,
                            value,
                        );
                    }
                    state.programmers.select_expression(
                        session.id,
                        members,
                        light_programmer::SelectionExpression::LiveGroup {
                            group_id,
                            rule: light_programmer::SelectionRule::All,
                        },
                    );
                }
            }
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(
                serde_json::json!({"applied":fixture_ids.len(),"programmer":state.programmers.get(session.id)}),
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
                input.highlight,
                input.active_context,
            );
            persist_programmer(state, session).map_err(|e| e.message)?;
            Ok(serde_json::json!({"updated":true}))
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
            Ok(
                serde_json::json!({"grand_master":control.options.grand_master,"blackout":control.options.blackout}),
            )
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
            emit(
                state,
                "command_applied",
                serde_json::json!({"request_id":command.request_id,"session_id":session.id,"command":command.command}),
            );
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
                        push_unique(&mut selected, fixture.fixture_id);
                        for child in ordered_child_ids(fixture) {
                            push_unique(&mut selected, child);
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

fn command_time_millis(token: &str) -> Result<u64, String> {
    let seconds = token
        .parse::<f64>()
        .map_err(|_| "TIME and DELAY require seconds")?;
    if !seconds.is_finite() || seconds < 0.0 || seconds > 86_400.0 {
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
    state
        .engine
        .replace_snapshot(load_engine_snapshot(entry)?)
        .map_err(|error| error.to_string())
}

fn apply_command_preset(
    state: &AppState,
    session: &Session,
    id: &str,
    selected: &[light_core::FixtureId],
) -> Result<(), String> {
    let (_, store) = active_show_store(state)?;
    let object = store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == id)
        .ok_or_else(|| format!("preset {id} does not exist"))?;
    let preset: light_programmer::Preset =
        serde_json::from_value(object.body).map_err(|error| error.to_string())?;
    let groups = state
        .engine
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    for fixture in selected {
        if let Some(attributes) = preset.values.get(fixture) {
            for (attribute, value) in attributes {
                state
                    .programmers
                    .set_faded(session.id, *fixture, attribute.clone(), value.clone());
            }
        }
        for (group_id, attributes) in &preset.group_values {
            if light_programmer::resolve_group(group_id, &groups)
                .is_ok_and(|members| members.contains(fixture))
            {
                for (attribute, value) in attributes {
                    state.programmers.set_faded(
                        session.id,
                        *fixture,
                        attribute.clone(),
                        value.clone(),
                    );
                }
            }
        }
    }
    state.programmers.select(session.id, selected.to_vec());
    Ok(())
}

fn command_preset_id(tokens: &[String]) -> Result<String, String> {
    if tokens.len() != 3 || tokens[1] != "." {
        return Err("expected <preset-type> . <preset-number>".into());
    }
    let kind = tokens[0]
        .parse::<u8>()
        .map_err(|_| "preset type is invalid")?;
    if kind > 4 {
        return Err("preset type must be within 0-4".into());
    }
    let number = tokens[2]
        .parse::<u32>()
        .map_err(|_| "preset number is invalid")?;
    if number == 0 {
        return Err("preset numbers start at 1".into());
    }
    Ok(format!("{kind}.{number}"))
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

fn programmer_preset(
    programmer: &light_programmer::ProgrammerState,
    name: String,
) -> light_programmer::Preset {
    let mut preset = light_programmer::Preset {
        name,
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
    preset
}

fn programmer_cue(
    programmer: &light_programmer::ProgrammerState,
    number: f64,
    timing: CommandTiming,
) -> light_playback::Cue {
    let mut cue = light_playback::Cue::new(number);
    cue.fade_millis = timing.fade_millis.unwrap_or(0);
    cue.delay_millis = timing.delay_millis.unwrap_or(0);
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
        store
            .put_object(
                "cue_list",
                &object.id,
                &serde_json::to_value(list).map_err(|error| error.to_string())?,
                object.revision,
            )
            .map_err(|error| error.to_string())?;
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
            cues: vec![programmer_cue(&programmer, number, timing)],
        };
        let definition = light_playback::PlaybackDefinition {
            number: playback,
            name: list.name.clone(),
            target: light_playback::PlaybackTarget::CueList { cue_list_id },
            buttons: [
                light_playback::PlaybackButtonAction::Go,
                light_playback::PlaybackButtonAction::GoMinus,
                light_playback::PlaybackButtonAction::Flash,
            ],
            fader: light_playback::PlaybackFaderMode::Master,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
        };
        store
            .put_object(
                "cue_list",
                &cue_list_id.0.to_string(),
                &serde_json::to_value(list).map_err(|error| error.to_string())?,
                0,
            )
            .map_err(|error| error.to_string())?;
        store
            .put_object(
                "playback",
                &playback.to_string(),
                &serde_json::to_value(definition).map_err(|error| error.to_string())?,
                0,
            )
            .map_err(|error| error.to_string())?;
    }
    refresh_command_show(state, &entry)
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
    let snapshot = state.engine.snapshot();
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
            return Ok(programmer.selected.len());
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
        let id = command_preset_id(body)?;
        let programmer = state
            .programmers
            .get(session.id)
            .ok_or("programmer does not exist")?;
        let preset = programmer_preset(&programmer, format!("Preset {id}"));
        if preset.values.is_empty() && preset.group_values.is_empty() {
            return Err("the programmer has no values to record".into());
        }
        let (entry, store) = active_show_store(state)?;
        let existing = store
            .objects("preset")
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|object| object.id == id);
        store
            .put_object(
                "preset",
                &id,
                &serde_json::to_value(preset).map_err(|error| error.to_string())?,
                existing.map_or(0, |object| object.revision),
            )
            .map_err(|error| error.to_string())?;
        refresh_command_show(state, &entry)?;
        return Ok(1);
    }
    if operation == "SET" {
        return execute_set_command(state, body);
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
            store
                .put_object(
                    "cue_list",
                    &source_object.id,
                    &serde_json::to_value(source_list).map_err(|error| error.to_string())?,
                    source_object.revision,
                )
                .map_err(|error| error.to_string())?;
        } else {
            let at = at.ok_or("MOVE and COPY require AT and a destination")?;
            let (destination, used) = parse_playback_address(&body[at + 1..], true, &snapshot)?;
            if used != body.len() - at - 1 {
                return Err("unexpected cue destination tokens".into());
            }
            let destination_cue = destination
                .cue
                .ok_or("cue destination requires CUE <cue-number>")?;
            let mut cue = source_list.cues[position].clone();
            cue.number = destination_cue;
            if destination.playback == source.playback {
                if source_list
                    .cues
                    .iter()
                    .any(|item| item.number == destination_cue)
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
                    .any(|item| item.number == destination_cue)
                {
                    return Err("destination cue already exists".into());
                }
                destination_list.cues.push(cue);
                destination_list
                    .cues
                    .sort_by(|a, b| a.number.total_cmp(&b.number));
                store
                    .put_object(
                        "cue_list",
                        &destination_object.id,
                        &serde_json::to_value(destination_list)
                            .map_err(|error| error.to_string())?,
                        destination_object.revision,
                    )
                    .map_err(|error| error.to_string())?;
                if operation == "MOVE" {
                    source_list.cues.remove(position);
                    store
                        .put_object(
                            "cue_list",
                            &source_object.id,
                            &serde_json::to_value(source_list)
                                .map_err(|error| error.to_string())?,
                            source_object.revision,
                        )
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        refresh_command_show(state, &entry)?;
        return Ok(1);
    }
    let at = body.iter().position(|token| token == "AT");
    let source_id = command_preset_id(at.map_or(body, |index| &body[..index]))?;
    let source = store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == source_id)
        .ok_or_else(|| format!("preset {source_id} does not exist"))?;
    if operation == "DELETE" {
        store
            .delete_object("preset", &source_id)
            .map_err(|error| error.to_string())?;
    } else {
        let at = at.ok_or("MOVE and COPY require AT and a destination number")?;
        if body.len() != at + 2 {
            return Err("preset destination must contain only its new number".into());
        }
        let kind = source_id.split('.').next().unwrap_or("0");
        let destination = format!(
            "{kind}.{}",
            body[at + 1]
                .parse::<u32>()
                .map_err(|_| "preset destination is invalid")?
        );
        if store
            .objects("preset")
            .map_err(|error| error.to_string())?
            .iter()
            .any(|object| object.id == destination)
        {
            return Err(format!("preset {destination} already exists"));
        }
        store
            .put_object("preset", &destination, &source.body, 0)
            .map_err(|error| error.to_string())?;
        if operation == "MOVE" {
            store
                .delete_object("preset", &source_id)
                .map_err(|error| error.to_string())?;
        }
    }
    refresh_command_show(state, &entry)?;
    Ok(1)
}

fn execute_set_command(state: &AppState, tokens: &[String]) -> Result<usize, String> {
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

fn parse_group_mixed_selection(
    snapshot: &EngineSnapshot,
    tokens: &[String],
) -> Result<Vec<light_core::FixtureId>, String> {
    fn push_unique(target: &mut Vec<light_core::FixtureId>, fixture: light_core::FixtureId) {
        if !target.contains(&fixture) {
            target.push(fixture);
        }
    }
    fn remove_fixture(target: &mut Vec<light_core::FixtureId>, fixture: light_core::FixtureId) {
        target.retain(|candidate| *candidate != fixture);
    }
    fn fixture_by_number(
        snapshot: &EngineSnapshot,
        token: &str,
    ) -> Result<light_core::FixtureId, String> {
        let number = token
            .parse::<u32>()
            .map_err(|_| "fixture number is invalid")?;
        snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_number == Some(number))
            .map(|fixture| fixture.fixture_id)
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
            .and_then(|members| {
                if members.is_empty() && skip_missing && !groups.contains_key(id) {
                    Ok(Vec::new())
                } else {
                    let valid = snapshot
                        .fixtures
                        .iter()
                        .map(|fixture| fixture.fixture_id)
                        .collect::<Vec<_>>();
                    Ok(members
                        .into_iter()
                        .filter(|fixture| valid.contains(fixture))
                        .collect())
                }
            })
    }

    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let mut selected = Vec::new();
    let mut index = 0;
    let mut operation = "+";
    while index < tokens.len() {
        match tokens[index].as_str() {
            "+" | "-" => {
                operation = tokens[index].as_str();
                index += 1;
            }
            token => {
                if tokens
                    .get(index + 1)
                    .is_some_and(|candidate| candidate == "THRU")
                {
                    let end = tokens
                        .get(index + 2)
                        .ok_or("THRU requires an end group")?
                        .parse::<i32>()
                        .map_err(|_| "group number is invalid")?;
                    let start = token
                        .parse::<i32>()
                        .map_err(|_| "group number is invalid")?;
                    let step = if start <= end { 1 } else { -1 };
                    let mut current = start;
                    loop {
                        for fixture in group_members(snapshot, &groups, &current.to_string(), true)?
                        {
                            if operation == "-" {
                                remove_fixture(&mut selected, fixture);
                            } else {
                                push_unique(&mut selected, fixture);
                            }
                        }
                        if current == end {
                            break;
                        }
                        current += step;
                    }
                    index += 3;
                } else if operation == "-" {
                    remove_fixture(&mut selected, fixture_by_number(snapshot, token)?);
                    index += 1;
                } else if selected.is_empty() {
                    for fixture in group_members(snapshot, &groups, token, false)? {
                        push_unique(&mut selected, fixture);
                    }
                    index += 1;
                } else {
                    push_unique(&mut selected, fixture_by_number(snapshot, token)?);
                    index += 1;
                }
            }
        }
    }
    Ok(selected)
}

fn execute_programmer_command(
    state: &AppState,
    session: &Session,
    command_line: &str,
) -> Result<usize, String> {
    let spaced = command_line
        .replace('.', " . ")
        .replace('+', " + ")
        .replace('-', " - ");
    let raw_tokens = spaced
        .split_whitespace()
        .map(|token| token.to_ascii_uppercase())
        .collect::<Vec<_>>();
    let (tokens, timing) = extract_command_timing(&raw_tokens)?;
    if tokens.is_empty() {
        return Err("the command line is empty".into());
    }
    if tokens.first().is_some_and(|token| token == "AT") {
        return apply_current_selection_value(state, session, &tokens[1..], timing);
    }
    if matches!(
        tokens[0].as_str(),
        "RECORD" | "REC" | "DELETE" | "DEL" | "MOVE" | "MOV" | "COPY" | "CPY" | "SET"
    ) {
        return execute_show_command(state, session, &tokens, timing);
    }
    if tokens.first().is_some_and(|token| token == "GROUP") {
        let frozen = tokens.get(1).is_some_and(|token| token == "GROUP");
        let id_index = if frozen { 2 } else { 1 };
        let at_index = tokens
            .iter()
            .position(|token| token == "AT")
            .unwrap_or(tokens.len());
        if !frozen
            && at_index == tokens.len()
            && tokens[id_index..at_index]
                .iter()
                .any(|token| matches!(token.as_str(), "THRU" | "+" | "-"))
            && !tokens[id_index..at_index]
                .iter()
                .any(|token| token == "DIV")
        {
            let snapshot = state.engine.snapshot();
            let fixtures = parse_group_mixed_selection(&snapshot, &tokens[id_index..at_index])?;
            state.programmers.select_expression(
                session.id,
                fixtures.clone(),
                light_programmer::SelectionExpression::Static,
            );
            state
                .programmers
                .set_command_line(session.id, command_line.to_owned());
            return Ok(fixtures.len());
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
                    for fixture in &fixtures {
                        let target = if relative {
                            let current = resolved
                                .get(&(*fixture, light_core::AttributeKey::intensity()))
                                .and_then(light_core::AttributeValue::normalized)
                                .unwrap_or(0.0)
                                * 100.0;
                            (current + if value[0] == "+" { percent } else { -percent })
                                .clamp(0.0, 100.0)
                        } else {
                            percent
                        };
                        state.programmers.set_faded_with_timing(
                            session.id,
                            *fixture,
                            light_core::AttributeKey::intensity(),
                            light_core::AttributeValue::Normalized(target / 100.0),
                            timing.fade_millis,
                            timing.delay_millis,
                        );
                    }
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
    let start = usize::from(matches!(
        tokens[0].as_str(),
        "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
    ));
    if tokens.len() <= start {
        return Err("expected a fixture number".into());
    }
    let snapshot = state.engine.snapshot();
    let at_index = tokens
        .iter()
        .position(|token| token == "AT")
        .unwrap_or(tokens.len());
    let fixture_ids = parse_fixture_selection(&snapshot.fixtures, &tokens[start..at_index])?;
    if at_index == tokens.len() {
        state.programmers.select(session.id, fixture_ids.clone());
        state
            .programmers
            .set_command_line(session.id, command_line.to_owned());
        return Ok(fixture_ids.len());
    }
    let value = &tokens[at_index + 1..];
    if value.len() == 3 && value[1] == "." {
        apply_command_preset(
            state,
            session,
            &format!("{}.{}", value[0], value[2]),
            &fixture_ids,
        )?;
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
    state.programmers.select(session.id, fixture_ids.clone());
    state
        .programmers
        .set_command_line(session.id, command_line.to_owned());
    let resolved = relative.then(|| state.engine.resolved_values());
    for fixture_id in &fixture_ids {
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
        state.programmers.set_faded_with_timing(
            session.id,
            *fixture_id,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(target / 100.0),
            timing.fade_millis,
            timing.delay_millis,
        );
    }
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
    for fixture_id in &current.selected {
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
        state.programmers.set_faded_with_timing(
            session.id,
            *fixture_id,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(target / 100.0),
            timing.fade_millis,
            timing.delay_millis,
        );
    }
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
    state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("invalid session token"))
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
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move{let mut interval=tokio::time::interval(Duration::from_millis(500));loop{tokio::select!{_=cancel.cancelled()=>break,_=interval.tick()=>send_osc_feedback(&state,false)}}}));
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
        if !handle_subscription_osc(state, address, arguments, source.as_deref()) {
            handle_playback_osc(state, address, arguments);
            handle_programmer_osc(state, address, arguments, source.as_deref());
            handle_timing_osc(state, address, arguments);
            handle_encoder_osc(state, address, arguments);
        }
        send_osc_feedback(state, false);
    }
    let mappings = state.engine.snapshot().control_mappings.clone();
    for mapping in mappings.iter().filter(|mapping| mapping.matches(&event)) {
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
            .find(|session| session.connected && session.desk.id == desk.id)
            .map(|session| session.id)
    };
    let session_id = existing
        .map(|s| s.session_id)
        .or(attached_session)
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

fn handle_programmer_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() < 4 || parts[0] != "light" || parts[2] != "programmer" || !osc_pressed(arguments)
    {
        return;
    }
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
    let action = parts[3];
    if action == "shift" {
        if let Some(source) = source {
            if let Some(target) = state
                .osc_subscribers
                .lock()
                .values_mut()
                .find(|candidate| candidate.command_source == source)
            {
                target.shifted = !target.shifted;
            }
        }
        return;
    }
    if subscriber.shifted && action.starts_with("digit-") {
        if let Some(source) = source {
            if let Some(target) = state
                .osc_subscribers
                .lock()
                .values_mut()
                .find(|candidate| candidate.command_source == source)
            {
                target.shifted = false;
            }
        }
        emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":parts[1],"session_id":session.id,"action":format!("shift-{}", &action[6..]),"source":"osc"}),
        );
        return;
    }
    match action {
        "enter" => {
            if let Some(programmer) = state.programmers.get(session.id) {
                let _ = execute_programmer_command(state, &session, &programmer.command_line);
                state
                    .programmers
                    .set_command_line(session.id, String::new());
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
            state.programmers.activate_preload(session.id);
        }
        "escape" | "menu" | "prog-playback" | "record" => emit(
            state,
            "desk_action",
            serde_json::json!({"desk_alias":parts[1],"session_id":session.id,"action":action,"source":"osc"}),
        ),
        key => {
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
                "rec" => "RECORD",
                v if v.starts_with("digit-") => &v[6..],
                v => v,
            };
            if let Some(p) = state.programmers.get(session.id) {
                let separator = matches!(
                    token,
                    "GROUP"
                        | "THRU"
                        | "+"
                        | "-"
                        | "AT"
                        | "TIME"
                        | "DELAY"
                        | "DIV"
                        | "SET"
                        | "RECORD"
                );
                state.programmers.set_command_line(
                    session.id,
                    if separator {
                        format!("{} {token} ", p.command_line)
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            + " "
                    } else {
                        format!("{}{token}", p.command_line)
                    },
                );
            }
        }
    }
    let _ = persist_programmer(state, &session);
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
        state.engine.set_control_timing(
            config.speed_groups_bpm,
            config.programmer_fade_millis,
            config.sequence_master_fade_millis,
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
        let mut config = state.configuration.write();
        config.speed_groups_bpm[group - 1] = (value.round() as u16).clamp(1, 999);
        state.engine.set_control_timing(
            config.speed_groups_bpm,
            config.programmer_fade_millis,
            config.sequence_master_fade_millis,
        );
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
    if let (Some(socket), Ok(packet)) = (
        &state.osc_feedback,
        encode_osc_message(&address, &arguments),
    ) {
        let _ = socket.send_to(&packet, target);
    }
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
    let active = state.engine.playback().read().active();
    for subscriber in subscribers {
        let Ok(Some(desk)) = state
            .desk
            .lock()
            .control_desk_by_alias(&subscriber.desk_alias)
        else {
            continue;
        };
        let page = state.desk.lock().desk_page(desk.id, show.id).unwrap_or(1);
        send_osc(
            state,
            subscriber.target,
            format!("/light/{}/feedback/page", subscriber.desk_alias),
            vec![OscArgument::Int(i32::from(page))],
        );
        let page_definition = snapshot.playback_pages.iter().find(|p| p.number == page);
        for slot in 1u8..=96 {
            let number = page_definition.and_then(|p| p.slots.get(&slot)).copied();
            let running = number.and_then(|n| active.iter().find(|a| a.playback_number == Some(n)));
            let level = running.map(|a| a.master).unwrap_or(0.0);
            for name in ["page-playback", "paged-playback"] {
                send_osc(
                    state,
                    subscriber.target,
                    format!(
                        "/light/{}/feedback/{name}/{slot}/fader",
                        subscriber.desk_alias
                    ),
                    vec![OscArgument::Float(level)],
                );
            }
            let button_count = if slot <= 20 { 3 } else { 1 };
            for button in 1..=button_count {
                let (r, g, b, state_name) = if running.is_some() {
                    (0.10, 0.85, 0.35, "on")
                } else if number.is_some() {
                    (0.12, 0.42, 0.95, "off")
                } else {
                    (0.18, 0.20, 0.23, "off")
                };
                for name in ["page-playback", "paged-playback"] {
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
                }
            }
        }
        for (index, bpm) in state
            .configuration
            .read()
            .speed_groups_bpm
            .iter()
            .enumerate()
        {
            send_osc(
                state,
                subscriber.target,
                format!(
                    "/light/{}/feedback/speed-group/{}",
                    subscriber.desk_alias,
                    index + 1
                ),
                vec![
                    OscArgument::Int(i32::from(*bpm)),
                    OscArgument::Float(0.0),
                    OscArgument::Float(0.75),
                    OscArgument::Float(0.95),
                    OscArgument::String("on".into()),
                ],
            );
        }
    }
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
        .any(|definition| {
            definition.number == number
                && matches!(
                    definition.target,
                    light_playback::PlaybackTarget::CueList { .. }
                )
        })
        .then_some(number)
}

fn handle_playback_osc(state: &AppState, address: &str, arguments: &[OscArgument]) {
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
    if !state.engine.snapshot().playbacks.iter().any(|definition| {
        definition.number == number
            && matches!(
                definition.target,
                light_playback::PlaybackTarget::CueList { .. }
            )
    }) {
        return;
    }
    let mut playback = state.engine.playback().write();
    let _ = match parts[action_index] {
        "go" if pressed => playback.go_playback(number).map(|_| ()),
        "go-minus" if pressed => playback.back_playback(number).map(|_| ()),
        "on" if pressed => playback.on(number),
        "off" if pressed => playback.off(number).map(|_| ()),
        "toggle" if pressed => playback.toggle(number).map(|_| ()),
        "flash" => playback.set_flash(number, pressed),
        "master" | "fader" => value
            .ok_or_else(|| "OSC fader requires a numeric value".to_owned())
            .and_then(|value| playback.set_master(number, value.clamp(0.0, 1.0))),
        "xfade-on" if pressed => playback.xfade(number, true),
        "xfade-off" if pressed => playback.xfade(number, false),
        "button" if parts.len() == action_index + 2 => parts[action_index + 1]
            .parse::<u8>()
            .map_err(|_| "invalid playback button".to_owned())
            .and_then(|button| playback.button(number, button, pressed)),
        _ => Ok(()),
    };
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
    reconcile_show_logical_heads(entry)?;
    load_engine_snapshot_with_override(entry, None)
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
    let legacy_group_playbacks = store
        .objects("playback")
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter_map(|object| {
            serde_json::from_value::<light_playback::PlaybackDefinition>(object.body)
                .ok()
                .filter(|definition| {
                    matches!(
                        definition.target,
                        light_playback::PlaybackTarget::Group { .. }
                    )
                })
                .map(|definition| (object.id, definition.number))
        })
        .collect::<Vec<_>>();
    if !legacy_group_playbacks.is_empty() {
        let legacy_numbers = legacy_group_playbacks
            .iter()
            .map(|(_, number)| *number)
            .collect::<std::collections::HashSet<_>>();
        for object in store
            .objects("playback_page")
            .map_err(|error| error.to_string())?
        {
            let mut page = serde_json::from_value::<light_playback::PlaybackPage>(object.body)
                .map_err(|error| error.to_string())?;
            let previous = page.slots.len();
            page.slots
                .retain(|_, number| !legacy_numbers.contains(number));
            if page.slots.len() != previous {
                store
                    .put_object(
                        "playback_page",
                        &object.id,
                        &serde_json::to_value(page).map_err(|error| error.to_string())?,
                        object.revision,
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
        for (id, _) in legacy_group_playbacks {
            store
                .delete_object("playback", &id)
                .map_err(|error| error.to_string())?;
        }
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
                    light_playback::PlaybackButtonAction::Go,
                    light_playback::PlaybackButtonAction::GoMinus,
                    light_playback::PlaybackButtonAction::Flash,
                ],
                fader: light_playback::PlaybackFaderMode::Master,
                go_activates: true,
                auto_off: true,
                xfade_millis: 0,
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
fn validate_show_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 100 || name.contains(['/', '\\']) {
        Err(ApiError::bad_request(
            "show name must be a plain name up to 100 characters",
        ))
    } else {
        Ok(())
    }
}
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}
impl ApiError {
    fn fixture(error: light_fixture::FixtureError) -> Self {
        Self::bad_request(error.to_string())
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
        (self.status, Json(serde_json::json!({"error":self.message}))).into_response()
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    fn test_control_desk() -> ControlDesk {
        ControlDesk {
            id: Uuid::nil(),
            name: "Test desk".into(),
            osc_alias: "test-desk".into(),
            columns: 8,
            rows: 1,
            buttons: 3,
        }
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
    fn legacy_four_speed_group_configuration_gains_group_e() {
        let configuration: DeskConfiguration =
            serde_json::from_value(serde_json::json!({"speed_groups_bpm":[101,102,103,104]}))
                .unwrap();
        assert_eq!(configuration.speed_groups_bpm, [101, 102, 103, 104, 15]);
        let five: DeskConfiguration =
            serde_json::from_value(serde_json::json!({"speed_groups_bpm":[1,2,3,4,5]})).unwrap();
        assert_eq!(five.speed_groups_bpm, [1, 2, 3, 4, 5]);
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
            serde_json::from_value::<light_fixture::PatchedFixture>(object.body).unwrap();
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
                ws_connections: Arc::new(Mutex::new(HashMap::new())),
                programmers,
                engine,
                output_health: Arc::new(std::sync::Mutex::new(OutputHealth::default())),
                output_rate: Arc::new(AtomicU16::new(44)),
                configuration: Arc::new(RwLock::new(DeskConfiguration::default())),
                output_control: Arc::new(Mutex::new(OutputControl::default())),
                activation_lock: Arc::new(tokio::sync::Mutex::new(())),
                timecode_router: Arc::new(Mutex::new(TimecodeRouter::default())),
                active_show: Arc::default(),
                active_show_error: Arc::default(),
                events,
                audit_events: Arc::new(Mutex::new(VecDeque::with_capacity(2048))),
                event_revision: Arc::new(AtomicU64::new(0)),
                desk_token: None,
                shutdown: CancellationToken::new(),
                media_cache: Arc::new(Mutex::new(MediaCache::default())),
                media_status: Arc::new(RwLock::new(HashMap::new())),
                input_locks: Arc::new(Mutex::new(HashMap::new())),
                osc_subscribers: Arc::new(Mutex::new(HashMap::new())),
                osc_feedback: None,
                mvr_imports: Arc::new(Mutex::new(HashMap::new())),
                network_output: None,
                output_sequences: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                manual_clock: None,
            },
            data_dir,
        )
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
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "test".into(),
            connected: true,
            desk: test_control_desk(),
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

        execute_programmer_command(&state, &session, "RECORD 0.1").unwrap();
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
        assert_eq!(cue_list.cues[0].delay_millis, 1_500);
        assert_eq!(cue_list.cues[0].group_changes[0].fade_millis, Some(2_000));
        assert_eq!(cue_list.cues[0].group_changes[0].delay_millis, Some(1_000));

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
            vec![1.0]
        );
        let _ = std::fs::remove_dir_all(data_dir);
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
    fn osc_exposes_time_minus_and_latched_shift_keys() {
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
            },
        );
        let pressed = [OscArgument::Bool(true)];
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
                    },
                    universe: Some(1),
                    address: Some(1),
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
                    multipatch: vec![],
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
        assert!(!competing_update.ok);
        assert!(
            competing_update
                .error
                .unwrap()
                .contains("controlled by another user")
        );
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
            values: HashMap::from([(
                fixture,
                HashMap::from([(
                    light_core::AttributeKey::intensity(),
                    light_core::AttributeValue::Normalized(0.5),
                )]),
            )]),
            group_values: HashMap::new(),
        };
        let stored = app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/presets/look/store"))
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
        assert_eq!(json(stored).await["revision"], 1);
        let stale = app
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/presets/look/store"))
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
    async fn named_revision_restores_a_manual_snapshot_over_newer_autosaves() {
        let (state, data_dir) = test_state();
        let app = router(state);
        let (token, _) = login(&app, "Operator").await;
        let show = create_show(&app, &token, "Revision restore").await;
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
        let restored = app
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
        assert_eq!(restored.status(), StatusCode::OK);
        let objects = app
            .oneshot(
                Request::get(format!("/api/v1/shows/{show_id}/objects/user_layout"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(objects.status(), StatusCode::OK);
        let objects = json(objects).await;
        assert_eq!(objects[0]["body"]["marker"], "manual");
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
                    .body(Body::from(r#"{"fixtures":[1,2,3]}"#))
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
            [101, 102, 103, 104, 15]
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
            },
            universe: Some(1),
            address: Some(1),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
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
            chaser_step_millis: 1_000,
            speed_group: None,
            cues: vec![cue],
        };
        let route = light_output::OutputRoute {
            protocol: light_output::Protocol::Sacn,
            logical_universe: 1,
            destination_universe: 1,
            destination: None,
            enabled: true,
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
            execute_programmer_command(&state, &session, "FIXTURE 1 AT 25").unwrap(),
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
                "1",
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
                payload: serde_json::json!({"preset_id":"1"}),
            },
        );
        assert!(applied.ok);
        assert_eq!(
            state
                .engine
                .render(RenderOptions::default())
                .unwrap()
                .universes[&1][0],
            191
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
                },
                universe: Some(1),
                address: Some(address),
                direct_control: None,
                location: Default::default(),
                rotation: Default::default(),
                logical_heads: vec![],
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
            .put_object(
                "preset",
                "all-white",
                &serde_json::to_value(&preset).unwrap(),
                0,
            )
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
            chaser_step_millis: 1_000,
            speed_group: None,
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
