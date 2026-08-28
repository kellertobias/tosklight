//! Capability-owned runtime resources.
//!
//! Runtime state is grouped by capability so the Axum composition root owns narrow resources
//! instead of a mutable state bag.

use super::attribute_configuration::{
    AttributeConfigurationReplayCache, InstalledAttributeConfiguration, ReplayKey,
};
use super::*;

#[cfg(test)]
type InstallationDeskStore = DeskStore;

/// Durable output-loop state behind a capability boundary.
///
/// The scheduler owns timing, not SQLite. Keeping the store here prevents the output loop from
/// becoming a second installation-persistence owner while still allowing automatic transitions
/// to checkpoint before the next process start.
#[derive(Clone)]
pub(in crate::runtime) struct OutputPersistenceResource {
    desk: Arc<Mutex<DeskStore>>,
}

impl OutputPersistenceResource {
    pub(in crate::runtime) fn open(data_dir: &std::path::Path) -> anyhow::Result<Self> {
        Ok(Self {
            desk: Arc::new(Mutex::new(DeskStore::open(data_dir.join("desk.sqlite"))?)),
        })
    }

    pub(in crate::runtime) fn setting(
        &self,
        key: &str,
    ) -> Result<Option<String>, light_show::StoreError> {
        self.desk.lock().setting(key)
    }

    pub(in crate::runtime) fn checkpoint_active_playbacks(
        &self,
        show_id: light_core::ShowId,
        serialized: &str,
    ) -> Result<(), light_show::StoreError> {
        self.desk
            .lock()
            .set_setting(&active_playbacks_setting(show_id), serialized)
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct InstallationResource {
    desk: Arc<Mutex<DeskStore>>,
    #[cfg(not(test))]
    session_persistence: Arc<SessionPersistenceQueue>,
    fixture_library: Arc<Mutex<light_fixture::FixtureLibrary>>,
    data_dir: PathBuf,
    configuration: Arc<RwLock<DeskConfiguration>>,
    desk_token: Option<Arc<str>>,
}

#[cfg(not(test))]
struct SessionPersistenceQueue {
    sender: Mutex<Option<std::sync::mpsc::Sender<DeferredProgrammerPersistence>>>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

pub(in crate::runtime) struct DeferredProgrammerPersistence {
    pub(in crate::runtime) id: SessionId,
    pub(in crate::runtime) token: String,
    pub(in crate::runtime) programmer: light_programmer::ProgrammerState,
    pub(in crate::runtime) connected: bool,
    pub(in crate::runtime) updated_at: String,
}

#[cfg(not(test))]
impl SessionPersistenceQueue {
    fn new(desk: Arc<Mutex<DeskStore>>) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<DeferredProgrammerPersistence>();
        let worker = std::thread::Builder::new()
            .name("light-programmer-persistence".into())
            .spawn(move || {
                while let Ok(first) = receiver.recv() {
                    let mut pending = HashMap::from([(first.id, first)]);
                    // Programmer actions can arrive at encoder/key-repeat cadence. Wait for one
                    // short quiet window and retain only the newest state per session so JSON
                    // serialization never competes with the acknowledgement or first output tick.
                    loop {
                        match receiver.recv_timeout(std::time::Duration::from_secs(1)) {
                            Ok(next) => {
                                pending.insert(next.id, next);
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    for session in pending.into_values() {
                        let programmer_json = match serde_json::to_string(&session.programmer) {
                            Ok(programmer_json) => programmer_json,
                            Err(error) => {
                                tracing::error!(
                                    session_id = ?session.id,
                                    %error,
                                    "deferred Programmer serialization failed"
                                );
                                continue;
                            }
                        };
                        let persisted = PersistedSession {
                            id: session.id,
                            token: session.token,
                            programmer_json,
                            connected: session.connected,
                            updated_at: session.updated_at,
                        };
                        if let Err(error) = desk.lock().save_session(&persisted) {
                            tracing::error!(
                                session_id = ?session.id,
                                %error,
                                "deferred Programmer persistence failed"
                            );
                        }
                    }
                }
            })
            .expect("Programmer persistence worker must start");
        Self {
            sender: Mutex::new(Some(sender)),
            worker: Mutex::new(Some(worker)),
        }
    }

    fn schedule(&self, session: DeferredProgrammerPersistence) -> Result<(), String> {
        self.sender
            .lock()
            .as_ref()
            .ok_or_else(|| "Programmer persistence worker is unavailable".to_owned())?
            .send(session)
            .map_err(|_| "Programmer persistence worker is unavailable".to_owned())
    }
}

#[cfg(not(test))]
impl Drop for SessionPersistenceQueue {
    fn drop(&mut self) {
        // Disconnect first so recv_timeout wakes immediately, persists the newest queued state,
        // and lets graceful desk shutdown wait for the durable write instead of losing the last
        // second of Programmer interaction.
        self.sender.get_mut().take();
        if let Some(worker) = self.worker.get_mut().take() {
            let _ = worker.join();
        }
    }
}

impl InstallationResource {
    pub(in crate::runtime) fn open_fixture_library_for_startup(
        data_dir: &std::path::Path,
        fixture_package_dir: Option<&std::path::Path>,
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

    pub(in crate::runtime) fn new(
        desk: DeskStore,
        fixture_library: light_fixture::FixtureLibrary,
        data_dir: PathBuf,
        configuration: DeskConfiguration,
        desk_token: Option<Arc<str>>,
    ) -> Self {
        let desk = Arc::new(Mutex::new(desk));
        Self {
            #[cfg(not(test))]
            session_persistence: Arc::new(SessionPersistenceQueue::new(Arc::clone(&desk))),
            desk,
            fixture_library: Arc::new(Mutex::new(fixture_library)),
            data_dir,
            configuration: Arc::new(RwLock::new(configuration)),
            desk_token,
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn open_test_installation(data_dir: PathBuf) -> anyhow::Result<Self> {
        let desk = DeskStore::open(data_dir.join("desk.sqlite"))?;
        let fixture_library =
            light_fixture::FixtureLibrary::open(data_dir.join("fixtures.sqlite"))?;
        Ok(Self::new(
            desk,
            fixture_library,
            data_dir,
            DeskConfiguration::default(),
            None,
        ))
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct ProgrammingResource {
    programmers: ProgrammerRegistry,
    service: ProgrammingService,
    command_history: Arc<Mutex<HashMap<Uuid, VecDeque<CommandHistoryEntry>>>>,
}

#[derive(Clone)]
pub(in crate::runtime) struct AttributeConfigurationResource {
    installed: Arc<RwLock<InstalledAttributeConfiguration>>,
    replay: Arc<tokio::sync::Mutex<AttributeConfigurationReplayCache>>,
}

impl AttributeConfigurationResource {
    pub(in crate::runtime) fn new(installed: InstalledAttributeConfiguration) -> Self {
        Self {
            installed: Arc::new(RwLock::new(installed)),
            replay: Arc::default(),
        }
    }

    pub(super) fn snapshot(&self) -> InstalledAttributeConfiguration {
        self.installed.read().clone()
    }

    pub(super) fn install_entry(&self, entry: Option<&ShowEntry>) {
        *self.installed.write() = InstalledAttributeConfiguration::for_entry(entry);
    }

    pub(super) fn install_document(&self, document: &light_show::PortableShowDocument) {
        *self.installed.write() = InstalledAttributeConfiguration::for_document(document);
    }

    #[cfg(test)]
    pub(super) fn replace_installed(&self, installed: InstalledAttributeConfiguration) {
        *self.installed.write() = installed;
    }

    pub(super) fn activation_links(
        &self,
    ) -> HashMap<light_core::AttributeKey, Vec<light_core::AttributeKey>> {
        self.installed.read().configuration.activation_links()
    }

    pub(super) async fn replay(
        &self,
        key: &ReplayKey,
        request: &light_wire::v2::attribute_configuration::AttributeConfigurationUpdateRequest,
    ) -> Result<
        Option<light_wire::v2::attribute_configuration::AttributeConfigurationUpdateOutcome>,
        ApiError,
    > {
        self.replay.lock().await.get(key, request)
    }

    pub(super) async fn remember(
        &self,
        key: ReplayKey,
        request: light_wire::v2::attribute_configuration::AttributeConfigurationUpdateRequest,
        outcome: light_wire::v2::attribute_configuration::AttributeConfigurationUpdateOutcome,
    ) {
        self.replay.lock().await.insert(key, request, outcome);
    }
}

impl ProgrammingResource {
    pub(in crate::runtime) fn new(
        programmers: ProgrammerRegistry,
        service: ProgrammingService,
    ) -> Self {
        Self {
            programmers,
            service,
            command_history: Arc::default(),
        }
    }

    pub(in crate::runtime) fn programmers(&self) -> ProgrammerRegistry {
        self.programmers.clone()
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct HighlightResource {
    registry: Arc<HighlightRegistry>,
    service: light_application::HighlightService,
    patch_preview: Arc<Mutex<HashMap<SessionId, HashSet<light_core::FixtureId>>>>,
}

impl HighlightResource {
    pub(in crate::runtime) fn new(registry: Arc<HighlightRegistry>) -> Self {
        Self {
            service: light_application::HighlightService::new(Arc::clone(&registry)),
            registry,
            patch_preview: Arc::default(),
        }
    }

    /// Creates isolated Highlight ownership for a detached Programmer transaction.
    /// Raw registry construction stays inside the capability resource boundary.
    pub(in crate::runtime) fn detached_registry() -> Arc<HighlightRegistry> {
        Arc::new(HighlightRegistry::default())
    }
}

/// Owns the native Timecode audio worker lifecycle and its request channel.
///
/// The CPAL stream must remain on its creating thread on CoreAudio. Keeping the join handle in a
/// capability resource makes shutdown an explicit part of server-owned audio rather than adapter
/// task ownership.
#[cfg(feature = "native-audio-output")]
pub(in crate::runtime) struct TimecodeAudioWorkerResource {
    sender: std::sync::mpsc::Sender<TimecodeAudioWorkerRequest>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[cfg(feature = "native-audio-output")]
pub(in crate::runtime) struct TimecodeAudioWorkerRequest {
    pub(in crate::runtime) command: super::timecode_audio_output::NativeCommand,
    pub(in crate::runtime) reply: std::sync::mpsc::Sender<Result<(), String>>,
}

#[cfg(feature = "native-audio-output")]
impl TimecodeAudioWorkerResource {
    pub(in crate::runtime) fn new(
        sender: std::sync::mpsc::Sender<TimecodeAudioWorkerRequest>,
        worker: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            sender,
            worker: Mutex::new(Some(worker)),
        }
    }

    pub(in crate::runtime) fn request(
        &self,
        command: super::timecode_audio_output::NativeCommand,
    ) -> Result<(), String> {
        let (reply, response) = std::sync::mpsc::channel();
        self.sender
            .send(TimecodeAudioWorkerRequest { command, reply })
            .map_err(|_| "Timecode audio output worker is unavailable".to_owned())?;
        response.recv().map_err(|_| {
            "Timecode audio output worker stopped before applying a command".to_owned()
        })?
    }
}

#[cfg(feature = "native-audio-output")]
impl Drop for TimecodeAudioWorkerResource {
    fn drop(&mut self) {
        let (reply, _response) = std::sync::mpsc::channel();
        let _ = self.sender.send(TimecodeAudioWorkerRequest {
            command: super::timecode_audio_output::NativeCommand::Shutdown,
            reply,
        });
        if let Some(worker) = self.worker.get_mut().take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct ActiveShowResource {
    activation: ActiveShowCoordinator,
    show_change: Arc<tokio::sync::Mutex<()>>,
    active: Arc<RwLock<Option<ShowEntry>>>,
    document: Arc<Mutex<Option<light_show::PortableShowDocument>>>,
    backup_checkpoint: Arc<Mutex<Option<(light_core::ShowId, u64)>>>,
    error: Arc<RwLock<Option<String>>>,
    service: ActiveShowService,
    patch: ShowPatchService,
    selective_import: SelectiveShowImportService,
    mvr_imports: Arc<Mutex<HashMap<Uuid, StagedMvrImport>>>,
    #[cfg(test)]
    patch_profile_resolution: Arc<PatchProfileResolutionPause>,
    #[cfg(test)]
    http_lifecycle: Arc<ActiveShowLifecyclePause>,
    #[cfg(test)]
    preload_store_release_lifecycle: Arc<ActiveShowLifecyclePause>,
    #[cfg(test)]
    patch_lifecycle: Arc<ActiveShowLifecyclePause>,
}

impl ActiveShowResource {
    pub(in crate::runtime) fn new(
        activation: ActiveShowCoordinator,
        active: Arc<RwLock<Option<ShowEntry>>>,
        error: Option<String>,
        service: ActiveShowService,
        patch: ShowPatchService,
        selective_import: SelectiveShowImportService,
    ) -> Self {
        Self {
            activation,
            show_change: Arc::default(),
            active,
            document: Arc::default(),
            backup_checkpoint: Arc::default(),
            error: Arc::new(RwLock::new(error)),
            service,
            patch,
            selective_import,
            mvr_imports: Arc::default(),
            #[cfg(test)]
            patch_profile_resolution: Arc::default(),
            #[cfg(test)]
            http_lifecycle: Arc::default(),
            #[cfg(test)]
            preload_store_release_lifecycle: Arc::default(),
            #[cfg(test)]
            patch_lifecycle: Arc::default(),
        }
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct EventResource {
    application: EventBus,
    audit: Arc<Mutex<VecDeque<Event>>>,
    revision: Arc<AtomicU64>,
}

impl EventResource {
    const AUDIT_CAPACITY: usize = 2_048;

    pub(in crate::runtime) fn new(application: EventBus) -> Self {
        Self {
            application,
            audit: Arc::new(Mutex::new(VecDeque::with_capacity(Self::AUDIT_CAPACITY))),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct ReplayResource {
    show_library: Arc<tokio::sync::Mutex<show_library_v2::ShowLibraryReplayCache>>,
    fixture_library: Arc<tokio::sync::Mutex<fixture_api_replay::FixtureLibraryReplayCache>>,
    show_object: Arc<tokio::sync::Mutex<show_objects_v2::ShowObjectReplayCache>>,
    show_object_intent:
        Arc<tokio::sync::Mutex<show_object_intents_v2::ShowObjectIntentReplayCache>>,
    schedules: Arc<tokio::sync::Mutex<schedules_v2::ScheduleReplayCache>>,
    preset_generation: Arc<tokio::sync::Mutex<live_action_http::PresetGenerationReplayCache>>,
    screen_configuration:
        Arc<tokio::sync::Mutex<screen_configuration_v2::ScreenConfigurationReplayCache>>,
    control_desk_configuration:
        Arc<tokio::sync::Mutex<control_desk_configuration_v2::ControlDeskConfigurationReplayCache>>,
    desk_management: Arc<tokio::sync::Mutex<desk_management_v2::DeskManagementReplayCache>>,
    cue_thumbnails: Arc<Mutex<cue_thumbnails_http::CueThumbnailReplayCache>>,
    psn: Arc<Mutex<psn_http::PsnReplayCache>>,
    stage_layout: Arc<Mutex<stage_layout_http::StageLayoutReplayCache>>,
    virtual_playback_zones:
        Arc<Mutex<virtual_playback_zones_http::VirtualPlaybackZonesReplayCache>>,
    visualizer_view: Arc<Mutex<visualizer_view_http::VisualizerViewReplayCache>>,
}

impl Default for ReplayResource {
    fn default() -> Self {
        Self {
            show_library: Arc::default(),
            fixture_library: Arc::default(),
            show_object: Arc::default(),
            show_object_intent: Arc::default(),
            schedules: Arc::default(),
            preset_generation: Arc::default(),
            screen_configuration: Arc::default(),
            control_desk_configuration: Arc::default(),
            desk_management: Arc::default(),
            cue_thumbnails: Arc::default(),
            psn: Arc::default(),
            stage_layout: Arc::default(),
            virtual_playback_zones: Arc::default(),
            visualizer_view: Arc::default(),
        }
    }
}

mod active_show;
mod events;
mod installation;
mod integrations;
mod lifecycle;
mod media_replay;
mod output;
mod playback_highlight;
mod programming;
mod session;

pub(super) use active_show::*;
pub(super) use integrations::*;
pub(super) use lifecycle::*;
pub(super) use media_replay::*;
pub(super) use output::*;
pub(super) use playback_highlight::*;
pub(super) use session::*;

#[cfg(test)]
mod tests;
