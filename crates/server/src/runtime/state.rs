use super::*;

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) desk: Arc<Mutex<DeskStore>>,
    pub(super) fixture_library: Arc<Mutex<light_fixture::FixtureLibrary>>,
    pub(super) data_dir: PathBuf,
    pub(super) sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    pub(super) session_clients: Arc<RwLock<HashMap<SessionId, Uuid>>>,
    pub(super) ws_connections: Arc<Mutex<HashMap<SessionId, u32>>>,
    pub(super) programmers: ProgrammerRegistry,
    pub(super) programming: ProgrammingService,
    pub(super) playback_service: PlaybackService,
    pub(super) engine: Arc<Engine>,
    pub(super) highlight: Arc<HighlightRegistry>,
    pub(super) patch_preview_highlights:
        Arc<Mutex<HashMap<SessionId, HashSet<light_core::FixtureId>>>>,
    pub(super) output_health: Arc<std::sync::Mutex<OutputHealth>>,
    pub(super) output_rate: Arc<AtomicU16>,
    pub(super) configuration: Arc<RwLock<DeskConfiguration>>,
    pub(super) matter_bridge: Arc<matter::MatterBridgeAdapter>,
    pub(super) matter_transport: Option<Arc<matter::MatterTransport>>,
    pub(super) output_control: Arc<Mutex<OutputControl>>,
    pub(super) activation_lock: Arc<tokio::sync::Mutex<()>>,
    pub(super) playback_action_lock: Arc<Mutex<()>>,
    pub(super) timecode_router: Arc<Mutex<TimecodeRouter>>,
    pub(super) active_show: Arc<RwLock<Option<ShowEntry>>>,
    pub(super) active_show_error: Arc<RwLock<Option<String>>>,
    pub(super) events: broadcast::Sender<Event>,
    pub(super) application_events: EventBus,
    pub(super) show_patch: ShowPatchService,
    pub(super) audit_events: Arc<Mutex<VecDeque<Event>>>,
    pub(super) command_history: Arc<Mutex<HashMap<Uuid, VecDeque<CommandHistoryEntry>>>>,
    pub(super) event_revision: Arc<AtomicU64>,
    pub(super) desk_token: Option<Arc<str>>,
    pub(super) shutdown: CancellationToken,
    pub(super) media_cache: Arc<Mutex<MediaCache>>,
    pub(super) media_status: Arc<RwLock<HashMap<light_core::FixtureId, MediaServerStatus>>>,
    pub(super) input_locks: Arc<Mutex<HashMap<String, (light_core::UserId, Instant)>>>,
    pub(super) file_input_contexts: Arc<Mutex<HashMap<Uuid, file_manager::FileInputContext>>>,
    pub(super) osc_subscribers: Arc<Mutex<HashMap<String, OscSubscriber>>>,
    pub(super) osc_feedback: Option<Arc<std::net::UdpSocket>>,
    #[cfg(test)]
    pub(super) osc_feedback_capture: Arc<Mutex<Vec<CapturedOscMessage>>>,
    pub(super) mvr_imports: Arc<Mutex<HashMap<Uuid, StagedMvrImport>>>,
    pub(super) network_output: Option<Arc<NetworkOutput>>,
    pub(super) output_sequences:
        Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
    pub(super) manual_clock: Option<Arc<ManualClock>>,
    pub(super) speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
    pub(super) sound_capture_owners: Arc<Mutex<[Option<SoundCaptureOwner>; 5]>>,
}

#[cfg(test)]
pub(super) type CapturedOscMessage = (SocketAddr, String, Vec<OscArgument>);

#[derive(Clone, Copy)]
pub(super) struct SoundCaptureOwner {
    pub(super) desk_id: Uuid,
    pub(super) last_seen_millis: u64,
}

#[derive(Serialize)]
pub(super) struct SpeedGroupResponse {
    pub(super) group: String,
    pub(super) configuration: SoundToLightConfig,
    pub(super) snapshot: SpeedSnapshot,
}

#[derive(Deserialize)]
pub(super) struct SpeedGroupActionInput {
    pub(super) action: String,
    pub(super) captured_at_millis: Option<u64>,
}

#[derive(Clone)]
pub(super) struct StagedMvrImport {
    pub(super) document: light_mvr::MvrDocument,
    pub(super) created: Instant,
}

#[derive(Deserialize, Default)]
pub(super) struct MvrPreviewQuery {
    pub(super) show_id: Option<Uuid>,
}

#[derive(Clone, Serialize)]
pub(super) struct MvrImportPreview {
    pub(super) token: Uuid,
    pub(super) fixtures: Vec<MvrPreviewFixture>,
    pub(super) scenery: usize,
    pub(super) missing_profiles: Vec<String>,
    pub(super) warnings: Vec<String>,
    pub(super) address_conflicts: Vec<String>,
}
#[derive(Clone, Serialize)]
pub(super) struct MvrPreviewFixture {
    pub(super) uuid: Uuid,
    pub(super) name: String,
    pub(super) gdtf_spec: String,
    pub(super) gdtf_mode: String,
    pub(super) universe: Option<u16>,
    pub(super) address: Option<u16>,
    pub(super) matched: bool,
}

#[derive(Deserialize)]
pub(super) struct ApplyMvrImport {
    pub(super) new_show: Option<NewMvrShow>,
    pub(super) existing_show_id: Option<Uuid>,
    #[serde(default)]
    pub(super) resolutions: HashMap<Uuid, MvrResolution>,
}
#[derive(Deserialize)]
pub(super) struct NewMvrShow {
    pub(super) name: String,
    #[serde(default = "default_true")]
    pub(super) open_after_import: bool,
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(super) enum MvrResolution {
    Import,
    Skip,
    ImportUnpatched,
    Replace,
    Address { universe: u16, address: u16 },
}

#[derive(Serialize)]
pub(super) struct ApplyMvrResult {
    pub(super) show: ShowEntry,
    pub(super) imported_fixtures: usize,
    pub(super) unresolved_fixtures: usize,
    pub(super) imported_scenery: usize,
    pub(super) opened: bool,
    pub(super) warnings: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct MvrExportPreview {
    pub(super) fixtures: usize,
    pub(super) scenery: usize,
    pub(super) embedded_profiles: usize,
    pub(super) missing_profiles: Vec<String>,
    pub(super) omitted: Vec<String>,
    pub(super) warnings: Vec<String>,
}

#[derive(Clone)]
pub(super) struct OscSubscriber {
    pub(super) desk_alias: String,
    pub(super) target: SocketAddr,
    pub(super) command_source: SocketAddr,
    pub(super) session_id: SessionId,
    pub(super) last_seen: Instant,
    pub(super) shifted: bool,
    pub(super) shift_held: bool,
    pub(super) update_record_started: Option<Instant>,
    pub(super) update_first_release: Option<Instant>,
    pub(super) last_highlight_action: Option<(String, Instant)>,
}

#[derive(RustEmbed)]
#[folder = "$LIGHT_CONTROL_FRONTEND_DIR"]
pub(super) struct ControlUiAssets;
#[derive(Default)]
pub(super) struct OutputControl {
    pub(super) options: RenderOptions,
    pub(super) grand_master_flash: bool,
    pub(super) hold: bool,
    pub(super) last_frames: HashMap<light_core::Universe, light_output::DmxFrame>,
    pub(super) raw_overrides: HashMap<(light_core::Universe, light_core::DmxAddress), u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct PersistedOutputRuntime {
    pub(super) grand_master: f32,
    pub(super) blackout: bool,
    pub(super) dynamics_paused_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) group_masters: HashMap<String, f32>,
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
    pub(super) fn is_valid(&self) -> bool {
        self.grand_master.is_finite()
            && (0.0..=1.0).contains(&self.grand_master)
            && self
                .group_masters
                .values()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    }
}
impl OutputControl {
    pub(super) fn render_options(&self) -> RenderOptions {
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
pub(super) struct Session {
    pub(super) id: SessionId,
    pub(super) user: DeskUser,
    pub(super) token: String,
    pub(super) connected: bool,
    pub(super) desk: ControlDesk,
}
#[derive(Clone, Serialize)]
pub(super) struct Event {
    pub(super) revision: u64,
    pub(super) kind: String,
    pub(super) payload: serde_json::Value,
}
#[derive(Clone, Serialize)]
pub(super) struct CommandHistoryEntry {
    pub(super) id: String,
    pub(super) desk_id: Uuid,
    pub(super) session_id: SessionId,
    pub(super) command: String,
    pub(super) status: String,
    pub(super) feedback: String,
    pub(super) source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) request_id: Option<String>,
    pub(super) at: String,
}
#[derive(Deserialize)]
pub(super) struct AuditQuery {
    #[serde(default)]
    pub(super) after: u64,
}
#[derive(Clone, Default, Serialize)]
pub(super) struct MediaServerStatus {
    pub(super) online: bool,
    pub(super) last_success: Option<String>,
    pub(super) last_error: Option<String>,
}
