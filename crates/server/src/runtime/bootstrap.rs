//! Process bootstrap and ownership of server background tasks.

use super::{
    AppState, HighlightRegistry, matter, normalize_restored_virtual_playback_exclusions,
    output_scheduler, refresh_matter_bridge, refresh_speed_group_engine, router,
    spawn_control_inputs, spawn_matter_bridge_sync, startup_options, startup_state::StartupState,
};
use axum::Router;
use light_application::{
    ActiveShowService, EventBus, OutputRuntimeService, PlaybackService, ProgrammingService,
    SelectiveShowImportService, ShowPatchService,
};
use light_control::TimecodeRouter;
use light_media::MediaCache;
use light_output::OutputHealth;
use light_show::ShowEntry;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::{HashMap, VecDeque},
    env,
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicU16, AtomicU64},
    },
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(super) async fn run() -> anyhow::Result<()> {
    initialize_tracing();
    let Some(options) = process_options()? else {
        return Ok(());
    };
    RunningServer::start(StartupState::load(options)?)
        .await?
        .serve()
        .await
}

fn initialize_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter("light_server=info,tower_http=info")
        .init();
}

fn process_options() -> anyhow::Result<Option<startup_options::StartupOptions>> {
    match startup_options::from_process()? {
        startup_options::StartupAction::Run(options) => Ok(Some(options)),
        startup_options::StartupAction::ShowHelp => {
            println!("{}", startup_options::HELP);
            Ok(None)
        }
    }
}

struct RuntimeResources {
    pub(super) output_health: Arc<std::sync::Mutex<OutputHealth>>,
    pub(super) output_rate: Arc<AtomicU16>,
    pub(super) timecode_router: Arc<Mutex<TimecodeRouter>>,
    pub(super) matter_bridge: Arc<matter::MatterBridgeAdapter>,
    pub(super) cancellation: CancellationToken,
    pub(super) scheduler: output_scheduler::OutputScheduler,
    pub(super) events: EventBus,
    pub(super) playback_service: PlaybackService,
    pub(super) active_show: Arc<RwLock<Option<ShowEntry>>>,
    pub(super) activation_lock: Arc<tokio::sync::Mutex<()>>,
}

impl RuntimeResources {
    async fn start(startup: &mut StartupState) -> anyhow::Result<Self> {
        let persisted_runtime = std::mem::take(&mut startup.output_runtime);
        let configuration = &startup.persistent.configuration;
        let output_health = Arc::new(std::sync::Mutex::new(OutputHealth::default()));
        let timecode_router = Arc::new(Mutex::new(TimecodeRouter::default()));
        timecode_router
            .lock()
            .configure(configuration.timecode_sources.clone());
        let output_rate = Arc::new(AtomicU16::new(configuration.frame_rate_hz));
        let matter_bridge = Arc::new(matter::MatterBridgeAdapter::default());
        let cancellation = CancellationToken::new();
        let events = EventBus::default();
        let playback_service = PlaybackService::new(events.clone());
        let active_show = Arc::new(RwLock::new(startup.persistent.active_show.clone()));
        let activation_lock = Arc::new(tokio::sync::Mutex::new(()));
        let scheduler = output_scheduler::start(output_scheduler::Config {
            bind_ip: configuration.output_bind_ip,
            engine: Arc::clone(&startup.engine),
            health: Arc::clone(&output_health),
            rate: Arc::clone(&output_rate),
            timecode: Arc::clone(&timecode_router),
            cancellation: cancellation.clone(),
            persisted_runtime,
            playback_service: playback_service.clone(),
            active_show: Arc::clone(&active_show),
            activation_lock: Arc::clone(&activation_lock),
            test_bench: startup.persistent.test_bench,
        })
        .await?;
        Ok(Self {
            output_health,
            output_rate,
            timecode_router,
            matter_bridge,
            cancellation,
            scheduler,
            events,
            playback_service,
            active_show,
            activation_lock,
        })
    }
}

struct RunningServer {
    pub(super) bind: SocketAddr,
    pub(super) app: Router,
    pub(super) cancellation: CancellationToken,
    pub(super) scheduler: output_scheduler::OutputScheduler,
    pub(super) input_tasks: Vec<JoinHandle<()>>,
    pub(super) matter_sync: JoinHandle<()>,
}

impl RunningServer {
    async fn start(mut startup: StartupState) -> anyhow::Result<Self> {
        let resources = RuntimeResources::start(&mut startup).await?;
        let bind = startup.persistent.bind;
        let state = build_app_state(startup, &resources)?;
        normalize_restored_virtual_playback_exclusions(&state);
        refresh_matter_bridge(&state);
        let matter_sync = spawn_matter_bridge_sync(state.clone(), resources.cancellation.clone());
        refresh_speed_group_engine(&state);
        let input_tasks = spawn_control_inputs(&state, resources.cancellation.clone());
        let app = router(state);
        Ok(Self {
            bind,
            app,
            cancellation: resources.cancellation,
            scheduler: resources.scheduler,
            input_tasks,
            matter_sync,
        })
    }

    async fn serve(self) -> anyhow::Result<()> {
        tracing::info!(bind=%self.bind, "starting light control server");
        let listener = tokio::net::TcpListener::bind(self.bind).await?;
        axum::serve(listener, self.app)
            .with_graceful_shutdown(wait_for_shutdown(self.cancellation.clone()))
            .await?;
        self.scheduler.wait().await;
        join_tasks(self.input_tasks).await;
        let _ = self.matter_sync.await;
        Ok(())
    }
}

async fn wait_for_shutdown(cancellation: CancellationToken) {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = cancellation.cancelled() => {},
    }
    cancellation.cancel();
}

async fn join_tasks(tasks: Vec<JoinHandle<()>>) {
    for task in tasks {
        let _ = task.await;
    }
}

fn build_app_state(
    startup: StartupState,
    resources: &RuntimeResources,
) -> anyhow::Result<AppState> {
    let output = resources.scheduler.network_output();
    let output_sequences = resources.scheduler.sequences();
    let output_control = resources.scheduler.control();
    let matter_transport = Arc::new(matter::MatterTransport::new(&startup.persistent.data_dir));
    let osc_feedback = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
    let application_events = resources.events.clone();
    let active_show_service = ActiveShowService::new(application_events.clone());
    Ok(AppState {
        desk: Arc::new(Mutex::new(startup.persistent.desk)),
        fixture_library: Arc::new(Mutex::new(startup.persistent.fixture_library)),
        data_dir: startup.persistent.data_dir,
        sessions: Arc::default(),
        session_clients: Arc::default(),
        ws_connections: Arc::new(Mutex::new(HashMap::new())),
        programmers: startup.programmers.clone(),
        programming: ProgrammingService::new(startup.programmers),
        playback_service: resources.playback_service.clone(),
        output_runtime_service: OutputRuntimeService::new(application_events.clone()),
        engine: startup.engine,
        highlight: Arc::new(HighlightRegistry::default()),
        patch_preview_highlights: Arc::default(),
        output_health: Arc::clone(&resources.output_health),
        output_rate: Arc::clone(&resources.output_rate),
        configuration: Arc::new(RwLock::new(startup.persistent.configuration)),
        matter_bridge: Arc::clone(&resources.matter_bridge),
        matter_transport: Some(matter_transport),
        output_control,
        activation_lock: Arc::clone(&resources.activation_lock),
        timecode_router: Arc::clone(&resources.timecode_router),
        active_show: Arc::clone(&resources.active_show),
        active_show_error: Arc::new(RwLock::new(startup.active_show_error)),
        events: startup.events,
        application_events: application_events.clone(),
        active_show_service: active_show_service.clone(),
        show_patch: ShowPatchService::new(active_show_service.clone()),
        selective_show_import: SelectiveShowImportService::new(active_show_service),
        #[cfg(test)]
        patch_profile_resolution: Arc::default(),
        #[cfg(test)]
        active_show_http_lifecycle: Arc::default(),
        #[cfg(test)]
        patch_lifecycle: Arc::default(),
        audit_events: Arc::new(Mutex::new(VecDeque::with_capacity(2048))),
        command_history: Arc::new(Mutex::new(HashMap::new())),
        event_revision: Arc::new(AtomicU64::new(0)),
        desk_token: desk_token(),
        shutdown: resources.cancellation.clone(),
        media_cache: Arc::new(Mutex::new(MediaCache::default())),
        media_status: Arc::new(RwLock::new(HashMap::new())),
        input_locks: Arc::new(Mutex::new(HashMap::new())),
        file_input_contexts: Arc::new(Mutex::new(HashMap::new())),
        osc_subscribers: Arc::new(Mutex::new(HashMap::new())),
        osc_feedback: Some(osc_feedback),
        #[cfg(test)]
        osc_feedback_capture: Arc::new(Mutex::new(Vec::new())),
        mvr_imports: Arc::new(Mutex::new(HashMap::new())),
        network_output: Some(output),
        output_sequences,
        manual_clock: startup.manual_clock,
        speed_groups: startup.speed_groups,
        sound_capture_owners: Arc::new(Mutex::new([None; 5])),
    })
}

fn desk_token() -> Option<Arc<str>> {
    env::var("LIGHT_DESK_TOKEN")
        .ok()
        .filter(|token| !token.is_empty())
        .map(Arc::from)
}
