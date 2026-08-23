use super::*;

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) action_timing: ActionTimingResource,
    pub(super) attributes: AttributeConfigurationResource,
    pub(super) installation: InstallationResource,
    pub(super) sessions: SessionResource,
    pub(super) programming: ProgrammingResource,
    pub(super) fixture_freeze_history: super::fixture_freeze::FixtureFreezeHistory,
    pub(super) dynamics: light_application::DynamicsService,
    pub(super) macros: light_application::CommandMacroExecutionService,
    pub(super) timecodes: light_application::timeline::TimecodeRuntimeService,
    pub(super) managed_assets: Arc<dyn light_application::ManagedAssetStore>,
    pub(super) playback: PlaybackResource,
    pub(super) highlight: HighlightResource,
    pub(super) output: OutputResource,
    pub(super) active_show: ActiveShowResource,
    pub(super) events: EventResource,
    pub(super) extensions: extensions_runtime::ExtensionResource,
    pub(super) integrations: IntegrationResource,
    pub(super) media: MediaResource,
    pub(super) internal_audio: InternalAudioResource,
    pub(super) replay: ReplayResource,
    pub(super) lifecycle: LifecycleResource,
    /// The other ToskLights on the network, and this desk's own announcement to them.
    pub(super) discovery: discovery_http::DiscoveryResource,
}

#[cfg(test)]
pub(super) type CapturedOscMessage = (SocketAddr, String, Vec<OscArgument>);

#[derive(Serialize)]
pub(super) struct SpeedGroupResponse {
    pub(super) group: String,
    pub(super) source: light_wire::v2::desk_management::SpeedGroupSource,
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
    /// What this surface may do to the desk, decided by the path it connected on.
    pub(super) capability: light_core::SurfaceCapability,
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
    pub(super) revision: u64,
    pub(super) grand_master_flash: bool,
    pub(super) hold: bool,
    pub(super) last_frames: HashMap<light_core::Universe, light_output::DmxFrame>,
    pub(super) last_routes: Arc<[light_output::OutputRoute]>,
    pub(super) last_patched_slots: HashMap<light_core::Universe, u16>,
    pub(super) raw_overrides: HashMap<(light_core::Universe, light_core::DmxAddress), u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct PersistedOutputRuntime {
    #[serde(default)]
    pub(super) revision: u64,
    pub(super) grand_master: f32,
    pub(super) blackout: bool,
    pub(super) dynamics_paused_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) dynamic_playbacks: Vec<light_playback::ActiveDynamicPlayback>,
    pub(super) dynamic_runtime: Option<light_dynamics::DynamicRuntimeSnapshot>,
    pub(super) group_masters: HashMap<String, f32>,
}

impl Default for PersistedOutputRuntime {
    fn default() -> Self {
        Self {
            revision: 0,
            grand_master: 1.0,
            blackout: false,
            dynamics_paused_at: None,
            dynamic_playbacks: Vec::new(),
            dynamic_runtime: None,
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
            && self.dynamic_playbacks.iter().all(|playback| {
                playback.fader_value.is_finite()
                    && playback.size.is_finite()
                    && playback.master.is_finite()
                    && playback.local_speed_multiplier.denominator != 0
            })
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
    /// What this request's surface may do to the desk.
    ///
    /// Not stored with the session: it is resolved per request from the screen the request says
    /// it comes from, because one Tauri process drives the main window and every optional screen
    /// on the same session. A screen can only ever *reduce* what a request may do.
    #[serde(skip)]
    pub(super) capability: light_core::SurfaceCapability,
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
