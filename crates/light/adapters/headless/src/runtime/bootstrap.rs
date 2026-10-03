//! Process bootstrap and ownership of server background tasks.

use super::attribute_configuration::InstalledAttributeConfiguration;
use super::capabilities::runtime::supervisor::CapabilitySupervisors;
use super::capability_resources::*;
use super::discovery_http;
use super::dynamic_snapshot_publication::DynamicSnapshotPublication;
use super::dynamic_source_origins::{DynamicSourceOrigins, SharedDynamicSourceOrigins};
use super::{
    ActionTimingResource, AppState, HighlightRegistry, matter, output_scheduler,
    playback_telemetry, refresh_matter_bridge, refresh_speed_group_engine, router, startup_options,
    startup_state::StartupState,
};
use axum::Router;
use light_application::{
    ActiveShowService, EventBus, OutputRuntimeService, PlaybackService, PlaybackTopologyService,
    ProgrammingService, SelectiveShowImportService, ShowPatchService, SpeedGroupService,
};
use light_control::TimecodeRouter;
use light_media::MediaCache;
use light_output::OutputHealth;
use light_show::ShowEntry;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::HashMap,
    env,
    net::{SocketAddr, UdpSocket},
    sync::{Arc, atomic::AtomicU16},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) async fn run() -> anyhow::Result<()> {
    if super::timecode_audio_output::run_output_device_probe_from_process()? {
        return Ok(());
    }
    initialize_tracing();
    configure_output_workers();
    let Some(options) = process_options()? else {
        return Ok(());
    };
    RunningServer::start(StartupState::load(options)?)
        .await?
        .serve()
        .await
}

/// The desk's default log level, and how to raise it.
///
/// `RUST_LOG` wins when it is set. The level was fixed at compile time before, so answering "what
/// actually failed?" on a desk that is showing errors meant building a new binary — which is not
/// something an operator can do, and not something anyone should have to do to read a log.
const DEFAULT_LOG_FILTER: &str = "light_headless_runtime=info,tower_http=info";

fn initialize_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)),
        )
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
    pub(super) action_timing: ActionTimingResource,
    pub(super) output_health: Arc<std::sync::Mutex<OutputHealth>>,
    pub(super) output_rate: Arc<AtomicU16>,
    pub(super) playback_telemetry: Arc<playback_telemetry::PlaybackTelemetrySampler>,
    pub(super) timecode_router: Arc<Mutex<TimecodeRouter>>,
    pub(super) timecodes: light_application::timeline::TimecodeRuntimeService,
    pub(super) managed_assets: Arc<dyn light_application::ManagedAssetStore>,
    pub(super) matter_bridge: Arc<matter::MatterBridgeAdapter>,
    pub(super) cancellation: CancellationToken,
    pub(super) output_cancellation: CancellationToken,
    pub(super) scheduler: output_scheduler::OutputScheduler,
    pub(super) events: EventBus,
    pub(super) playback_service: PlaybackService,
    pub(super) active_show: Arc<RwLock<Option<ShowEntry>>>,
    pub(super) activation: ActiveShowCoordinator,
    pub(super) dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    pub(super) dynamic_snapshot: Arc<DynamicSnapshotPublication>,
    pub(super) dynamic_source_origins: SharedDynamicSourceOrigins,
    pub(super) dynamic_auto_offs: Arc<Mutex<Vec<light_playback::PlaybackIdentity>>>,
    pub(super) visualization_frames: Arc<super::visualization_frame::VisualizationFrameHub>,
    pub(super) internal_audio: Arc<Mutex<super::internal_audio::InternalAudioRuntime>>,
}

impl RuntimeResources {
    async fn start(startup: &mut StartupState) -> anyhow::Result<Self> {
        let persisted_runtime = std::mem::take(&mut startup.output_runtime);
        let configuration = &startup.persistent.configuration;
        let action_timing = ActionTimingResource::default();
        let output_health = Arc::new(std::sync::Mutex::new(OutputHealth::default()));
        let timecode_router = Arc::new(Mutex::new(TimecodeRouter::default()));
        let events = EventBus::default();
        let managed_assets: Arc<dyn light_application::ManagedAssetStore> = Arc::new(
            light_application::FilesystemManagedAssetStore::open(
                startup.persistent.data_dir.join("managed-assets"),
            )
            .map_err(|error| anyhow::anyhow!(error.message))?,
        );
        let (internal_audio, timecodes) = start_audio_runtime(startup, &managed_assets, &events);
        timecode_router
            .lock()
            .configure(configuration.timecode_router_config());
        let output_rate = Arc::new(AtomicU16::new(configuration.frame_rate_hz));
        let playback_telemetry = Arc::new(playback_telemetry::PlaybackTelemetrySampler::new(
            Arc::clone(&output_rate),
        ));
        let matter_bridge = Arc::new(matter::MatterBridgeAdapter::default());
        let cancellation = CancellationToken::new();
        let output_cancellation = cancellation.child_token();
        let playback_service = PlaybackService::new(events.clone());
        let active_show = Arc::new(RwLock::new(startup.persistent.active_show.clone()));
        let activation = ActiveShowCoordinator::new();
        let dynamics = Arc::clone(&startup.dynamics);
        let dynamic_snapshot = Arc::new(DynamicSnapshotPublication::new(startup.engine.snapshot()));
        let dynamic_source_origins: SharedDynamicSourceOrigins = Arc::new(
            arc_swap::ArcSwap::from_pointee(DynamicSourceOrigins::default()),
        );
        let dynamic_auto_offs = Arc::new(Mutex::new(Vec::new()));
        let visualization_frames =
            Arc::new(super::visualization_frame::VisualizationFrameHub::default());
        restore_dynamic_runtime(
            startup,
            &persisted_runtime,
            &dynamics,
            &dynamic_source_origins,
            &dynamic_snapshot,
        );
        let scheduler = output_scheduler::start(output_scheduler::Config {
            bind_ip: configuration.output_bind_ip,
            engine: Arc::clone(&startup.engine),
            health: Arc::clone(&output_health),
            rate: Arc::clone(&output_rate),
            timecode: Arc::clone(&timecode_router),
            timecodes: timecodes.clone(),
            cancellation: output_cancellation.clone(),
            persisted_runtime,
            playback: PlaybackRenderCapability::new(
                playback_service.clone(),
                Arc::clone(&playback_telemetry),
            ),
            active_show: ActiveShowProjection::new(Arc::clone(&active_show)),
            activation: activation.clone(),
            dynamics: Arc::clone(&dynamics),
            dynamic_snapshot: Arc::clone(&dynamic_snapshot),
            dynamic_source_origins: Arc::clone(&dynamic_source_origins),
            speed_groups: Arc::clone(&startup.speed_groups),
            dynamic_auto_offs: Arc::clone(&dynamic_auto_offs),
            visualization_frames: Arc::clone(&visualization_frames),
            action_timing: action_timing.clone(),
            test_bench: startup.persistent.test_bench,
            data_dir: startup.persistent.data_dir.clone(),
            internal_audio: Arc::clone(&internal_audio),
        })
        .await?;
        Ok(Self {
            action_timing,
            output_health,
            output_rate,
            playback_telemetry,
            timecode_router,
            timecodes,
            managed_assets,
            matter_bridge,
            cancellation,
            output_cancellation,
            scheduler,
            events,
            playback_service,
            active_show,
            activation,
            dynamics,
            dynamic_snapshot,
            dynamic_source_origins,
            dynamic_auto_offs,
            visualization_frames,
            internal_audio,
        })
    }
}

fn start_audio_runtime(
    startup: &StartupState,
    managed_assets: &Arc<dyn light_application::ManagedAssetStore>,
    events: &EventBus,
) -> (
    Arc<Mutex<super::internal_audio::InternalAudioRuntime>>,
    light_application::timeline::TimecodeRuntimeService,
) {
    let configuration = &startup.persistent.configuration;
    let timecode_clock = super::timecode_clock::runtime_clock(startup.manual_clock.as_ref());
    let audio_device = configuration.timecode_audio_output_device.as_ref().map_or(
        super::timecode_audio_output::OutputDeviceSelector::SystemDefault,
        |device| super::timecode_audio_output::OutputDeviceSelector::Name(device.clone()),
    );
    let trim_key = configuration
        .timecode_audio_output_device
        .as_deref()
        .unwrap_or("$system_default");
    let audio_configuration = super::timecode_audio_output::NativeTimecodeAudioConfig {
        device: audio_device,
        latency_trim_micros: configuration
            .timecode_audio_latency_trim_micros_by_output
            .get(trim_key)
            .copied()
            .unwrap_or(0),
    };
    let native_audio_output =
        super::timecode_audio_output::NativeTimecodeAudioOutput::open_with_timeout(
            Arc::clone(managed_assets),
            Arc::clone(&timecode_clock),
            &audio_configuration,
        )
        .map_err(|error| tracing::warn!(%error, "native Timecode audio is unavailable"))
        .ok();
    let mut audio_outputs_by_device = HashMap::new();
    if let Some(output) = &native_audio_output {
        tracing::info!(device = ?audio_configuration.device, "native Timecode audio output initialized");
        audio_outputs_by_device.insert(trim_key.to_owned(), output.internal_output());
    }
    let mut internal_outputs = std::collections::BTreeMap::new();
    if let Some(output) = &native_audio_output {
        internal_outputs.insert("default".to_owned(), output.internal_output());
    }
    for (binding, device_name) in &configuration.internal_audio_output_devices {
        let output = if let Some(output) = audio_outputs_by_device.get(device_name) {
            Some(output.clone())
        } else {
            let device = if device_name == "$system_default" {
                super::timecode_audio_output::OutputDeviceSelector::SystemDefault
            } else {
                super::timecode_audio_output::OutputDeviceSelector::Name(device_name.clone())
            };
            let config = super::timecode_audio_output::NativeTimecodeAudioConfig {
                device,
                latency_trim_micros: configuration
                    .timecode_audio_latency_trim_micros_by_output
                    .get(device_name)
                    .copied()
                    .unwrap_or(0),
            };
            match super::timecode_audio_output::NativeTimecodeAudioOutput::open_with_timeout(
                Arc::clone(managed_assets),
                Arc::clone(&timecode_clock),
                &config,
            ) {
                Ok(output) => {
                    let internal = output.internal_output();
                    audio_outputs_by_device.insert(device_name.clone(), internal.clone());
                    Some(internal)
                }
                Err(error) => {
                    tracing::warn!(binding, device = device_name, %error, "Internal audio output is unavailable");
                    None
                }
            }
        };
        if let Some(output) = output {
            internal_outputs.insert(binding.clone(), output);
        }
    }
    let internal_audio = Arc::new(Mutex::new(
        super::internal_audio::InternalAudioRuntime::new(
            &configuration.internal_audio_library_roots,
            internal_outputs,
        ),
    ));
    let audio_output = native_audio_output
        .map(|output| Arc::new(output) as Arc<dyn light_application::TimecodeAudioOutput>);
    let timecodes =
        super::timecode_v2::new_service_with_clock(timecode_clock, audio_output, events.clone());
    (internal_audio, timecodes)
}

/// Restores the persisted Dynamic source checkpoint, or else rebuilds programmer Dynamics.
fn restore_dynamic_runtime(
    startup: &StartupState,
    persisted_runtime: &super::PersistedOutputRuntime,
    dynamics: &Arc<Mutex<light_dynamics::DynamicRuntime>>,
    dynamic_source_origins: &SharedDynamicSourceOrigins,
    dynamic_snapshot: &Arc<DynamicSnapshotPublication>,
) {
    let restored_dynamic_runtime = match persisted_runtime.dynamic_source_checkpoint() {
        Ok(Some(mut checkpoint)) => {
            let restore = (|| -> anyhow::Result<()> {
                super::normalize_programmer_dynamic_checkpoint(
                    &startup.programmers,
                    &mut checkpoint.runtime,
                )?;
                OutputResource::restore_dynamic_source_state(
                    &startup.engine,
                    dynamics,
                    dynamic_source_origins,
                    dynamic_snapshot,
                    checkpoint,
                )?;
                Ok(())
            })();
            match restore {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(%error, "ignoring invalid persisted Dynamic runtime");
                    false
                }
            }
        }
        Ok(None) => false,
        Err(error) => {
            tracing::warn!(%error, "ignoring invalid persisted Dynamic source checkpoint");
            false
        }
    };
    if !restored_dynamic_runtime {
        restore_programmer_dynamics(
            dynamics,
            &startup.programmers,
            startup.engine.snapshot().as_ref(),
        );
    }
}

struct RestoredController<'a> {
    authored: &'a light_dynamics::DynamicAddressValue,
    definition: light_dynamics::DynamicDefinition,
    reference: light_dynamics::DynamicReference,
    lane_rows: Vec<(light_core::FixtureId, Uuid)>,
    overrides: light_dynamics::DynamicInstanceOverrides,
    targets: Vec<light_core::FixtureId>,
    activated_at_millis: u64,
}

/// Snapshot lookups shared by every restored programmer Dynamic controller.
struct RestoreContext<'a> {
    snapshot: &'a light_engine::EngineSnapshot,
    groups: HashMap<String, light_programmer::GroupDefinition>,
    stage_positions: HashMap<light_core::FixtureId, light_dynamics::Position3d>,
}

fn restore_programmer_dynamics(
    runtime: &Mutex<light_dynamics::DynamicRuntime>,
    programmers: &light_programmer::ProgrammerRegistry,
    snapshot: &light_engine::EngineSnapshot,
) {
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let stage_positions = snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(fixture_id, position)| {
            (
                *fixture_id,
                light_dynamics::Position3d {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                    z: f64::from(position.z),
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let context = RestoreContext {
        snapshot,
        groups,
        stage_positions,
    };
    let mut runtime = runtime.lock();
    for programmer in programmers.active_for_sessions() {
        let controllers = collect_restored_controllers(&programmer, snapshot);
        for (instance_link, restored) in controllers {
            start_restored_controller(&mut runtime, &context, &programmer, instance_link, restored);
        }
    }
}

fn collect_restored_controllers<'a>(
    programmer: &'a light_programmer::ProgrammerState,
    snapshot: &light_engine::EngineSnapshot,
) -> HashMap<Uuid, RestoredController<'a>> {
    let mut controllers = HashMap::<Uuid, RestoredController>::new();
    for stored in light_dynamics::merge_dynamic_address_values(
        programmer
            .dynamic_values
            .iter()
            .chain(programmer.preload_dynamic_active.iter()),
    ) {
        let light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic,
            overrides,
            lane_id,
            ..
        } = &stored.value
        else {
            continue;
        };
        let definition = dynamic
            .dynamic_id
            .and_then(|id| {
                snapshot
                    .dynamics
                    .iter()
                    .find(|definition| definition.id == id)
            })
            .cloned()
            .unwrap_or_else(|| dynamic.embedded_fallback.definition.as_ref().clone());
        let entry = controllers
            .entry(*instance_link)
            .or_insert_with(|| RestoredController {
                authored: stored,
                definition: definition.clone(),
                reference: dynamic.clone(),
                lane_rows: Vec::new(),
                overrides: overrides.clone(),
                targets: Vec::new(),
                activated_at_millis: stored.changed_at_millis,
            });
        if light_dynamics::dynamic_address_edit_is_later(stored, entry.authored) {
            entry.authored = stored;
            entry.definition = definition;
            entry.reference = dynamic.clone();
            entry.overrides = overrides.clone();
            entry.activated_at_millis = stored.changed_at_millis;
        }
        entry.lane_rows.push((stored.fixture_id, *lane_id));
        if !entry.targets.contains(&stored.fixture_id) {
            entry.targets.push(stored.fixture_id);
        }
    }
    controllers
}

fn start_restored_controller(
    runtime: &mut light_dynamics::DynamicRuntime,
    context: &RestoreContext<'_>,
    programmer: &light_programmer::ProgrammerState,
    instance_link: Uuid,
    restored: RestoredController<'_>,
) {
    let snapshot = context.snapshot;
    let controller_id =
        light_dynamics::programmer_dynamic_controller_id(programmer.id, instance_link);
    if restored.targets.is_empty() {
        return;
    }
    let lane_selection = light_dynamics::DynamicLaneSelection::for_recorded_values(
        &restored.reference,
        &restored.definition,
        &restored.lane_rows,
    );
    let live_group = match &restored.definition.target_binding {
        light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
            light_programmer::resolve_group_spatial(
                group_id,
                &context.groups,
                &context.stage_positions,
            )
            .ok()
        }
        light_dynamics::DynamicTargetBinding::FrozenTargets { .. }
        | light_dynamics::DynamicTargetBinding::Targetless => None,
    };
    let targets = live_group
        .as_ref()
        .map_or(restored.targets, |resolved| resolved.source_order.clone());
    let inherited_spatial_mapping = live_group.and_then(|resolved| resolved.effective_mapping);
    if let Err(error) = runtime.install_fallback_definition(restored.definition.clone()) {
        tracing::warn!(
            %controller_id,
            %error,
            "ignoring invalid persisted Dynamic fallback"
        );
        return;
    }
    let result = runtime.start(light_dynamics::DynamicStartRequest {
        definition_id: restored.definition.id,
        controller: light_dynamics::DynamicController {
            id: controller_id,
            source: light_dynamics::DynamicControllerSource::Programmer {
                programmer_id: programmer.id.0,
                instance_link: Some(instance_link),
            },
            priority: programmer.priority,
            activated_at_millis: restored.activated_at_millis,
            size: restored.overrides.size,
            speed_multiplier: restored.overrides.speed_multiplier.factor() as f32,
            phase_offset_degrees: restored.overrides.phase_offset_degrees,
            paused: false,
        },
        target_scope: light_dynamics::DynamicTargetScope {
            ordered_targets: targets,
        },
        stage_positions: snapshot.dynamic_stage_positions.as_ref().clone(),
        inherited_spatial_mapping,
        now_millis: restored.activated_at_millis,
        activation_delay_millis: 0,
        activation_duration_millis: 0,
        activation_policy_override: None,
        reuse_matching_targetless: true,
    });
    let result = result.and_then(|instance_id| {
        runtime.set_controller_lane_selection(instance_id, controller_id, lane_selection)
    });
    if let Err(error) = result {
        tracing::warn!(
            %controller_id,
            %error,
            "ignoring invalid persisted Dynamic controller"
        );
    }
}

struct RunningServer {
    pub(super) bind: SocketAddr,
    pub(super) app: Router,
    pub(super) supervisors: CapabilitySupervisors,
}

/// No supervisor owns queued tasks yet. Drop their receiver to break captures of AppState
/// (notably extension feedback) before returning an early startup error.
async fn discard_unstarted_runtime(state: &AppState, resources: RuntimeResources) {
    state.lifecycle.request_shutdown();
    drop(state.lifecycle.take_task_receiver());
    resources.cancellation.cancel();
    resources.output_cancellation.cancel();
    // Cancellation and dropping the start sender finish this unstarted scheduler and
    // release its future's Engine/Dynamics/resource ownership.
    if let Err(error) = resources.scheduler.into_task().await {
        tracing::warn!(%error, "failed startup scheduler cleanup");
    }
}

impl RunningServer {
    async fn start(mut startup: StartupState) -> anyhow::Result<Self> {
        let originals = super::startup_state::OriginalStartupOwners::capture(&startup.persistent)?;
        let mut resources = RuntimeResources::start(&mut startup).await?;
        let bind = startup.persistent.bind;
        let state = build_app_state(startup, &resources)?;
        if let Err(error) =
            super::startup_state::finalize_restored_owners_for_startup(&state, originals)
        {
            discard_unstarted_runtime(&state, resources).await;
            return Err(error);
        }
        refresh_matter_bridge(&state);
        refresh_speed_group_engine(&state);
        if let Err(error) = resources.scheduler.start_rendering() {
            discard_unstarted_runtime(&state, resources).await;
            return Err(error);
        }
        let supervisors = CapabilitySupervisors::start(
            resources.cancellation,
            resources.output_cancellation,
            resources.scheduler,
            &state,
        );
        let app = router(state);
        Ok(Self {
            bind,
            app,
            supervisors,
        })
    }

    async fn serve(self) -> anyhow::Result<()> {
        let Self {
            bind,
            app,
            supervisors,
        } = self;
        tracing::info!(%bind, "starting light control server");
        let cancellation = supervisors.cancellation();
        let server = async move {
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app)
                .with_graceful_shutdown(wait_for_shutdown(cancellation))
                .await
                .map_err(anyhow::Error::from)
        }
        .await;
        let shutdown = supervisors.shutdown().await;
        server?;
        shutdown
    }
}

async fn wait_for_shutdown(cancellation: CancellationToken) {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = cancellation.cancelled() => {},
    }
    cancellation.cancel();
}

fn build_app_state(
    startup: StartupState,
    resources: &RuntimeResources,
) -> anyhow::Result<AppState> {
    let extensions = super::extensions_runtime::ExtensionResource::start(
        startup.persistent.extensions_dir.clone(),
        startup.persistent.data_dir.join("extensions.json"),
    );
    let installed_attributes =
        InstalledAttributeConfiguration::for_entry(startup.persistent.active_show.as_ref());
    let discovery = start_discovery(&startup);
    let output = resources.scheduler.network_output();
    let output_sequences = resources.scheduler.sequences();
    let output_control = resources.scheduler.control_capability();
    let usb_output = resources.scheduler.usb_output();
    let matter_transport = Arc::new(matter::MatterTransport::new(&startup.persistent.data_dir));
    let osc_feedback = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
    let application_events = resources.events.clone();
    let active_show_service = ActiveShowService::new(application_events.clone());
    let highlight = Arc::new(HighlightRegistry::default());
    let programming = ProgrammingService::new(
        startup.programmers.clone(),
        application_events.clone(),
        Arc::clone(&highlight),
    );
    let playback_topology = PlaybackTopologyService::new(active_show_service.clone());
    let selective_import = SelectiveShowImportService::new(active_show_service.clone());
    let state = AppState {
        action_timing: resources.action_timing.clone(),
        attributes: AttributeConfigurationResource::new(installed_attributes),
        installation: InstallationResource::new(
            startup.persistent.desk,
            startup.persistent.fixture_library,
            startup.persistent.data_dir,
            startup.persistent.configuration,
            desk_token(),
        ),
        sessions: SessionResource::new(),
        dynamics: light_application::DynamicsService::new(startup.programmers.clone()),
        macros: light_application::CommandMacroExecutionService::default(),
        timecodes: resources.timecodes.clone(),
        managed_assets: Arc::clone(&resources.managed_assets),
        programming: ProgrammingResource::new(startup.programmers, programming),
        fixture_freeze_history: Default::default(),
        playback: PlaybackResource::new(
            resources.playback_service.clone(),
            playback_topology,
            Arc::clone(&resources.playback_telemetry),
        ),
        highlight: HighlightResource::new(highlight),
        psn: crate::runtime::psn::service::PsnResource::new(),
        output: OutputResource::new(
            OutputRuntimeService::new(application_events.clone()),
            SpeedGroupService::new(application_events.clone()),
            startup.engine,
            Arc::clone(&resources.output_health),
            Arc::clone(&resources.output_rate),
            output_control,
            Arc::clone(&resources.timecode_router),
            Some(output),
            usb_output,
            output_sequences,
            startup.manual_clock,
            startup.speed_groups,
            Arc::clone(&resources.dynamics),
            Arc::clone(&resources.dynamic_snapshot),
            Arc::clone(&resources.dynamic_source_origins),
            Arc::clone(&resources.dynamic_auto_offs),
            Arc::clone(&resources.visualization_frames),
        )
        .with_live_family_adapters(resources.scheduler.family_adapters()),
        active_show: ActiveShowResource::new(
            resources.activation.clone(),
            Arc::clone(&resources.active_show),
            startup.active_show_error,
            active_show_service.clone(),
            ShowPatchService::new(active_show_service),
            selective_import,
        ),
        events: EventResource::new(application_events),
        extensions,
        integrations: IntegrationResource::new(
            Arc::clone(&resources.matter_bridge),
            Some(matter_transport),
            Some(osc_feedback),
        ),
        media: MediaResource::new(MediaCache::default()),
        internal_audio: InternalAudioResource::new(Arc::clone(&resources.internal_audio)),
        replay: ReplayResource::default(),
        lifecycle: LifecycleResource::new(resources.cancellation.clone()),
        discovery,
    };
    // TL-560: a runtime that started at contract ≥ 1 rejects legacy-programming shows on
    // activation as well as at startup. Dormant at contract 0 (production today).
    state
        .active_show
        .engage_legacy_programming_gate(state.output.supported_programming_contract());
    state
        .output
        .engine()
        .set_color_model(state.attributes.color_model());
    state.extensions.attach_state(state.clone());
    super::psn_http::install_current_show(&state);
    // TL-548 C4: the Pending episode worker starts only behind the family-adapter gate.
    state.programming.pending_episodes().start_if_gated(&state);
    Ok(state)
}

/// Announce this desk to the network, and start listening for the other ToskLights.
///
/// `LIGHT_DISCOVERY=off` is for the installation that does not want its desk answering on mDNS at
/// all; everything else about the desk behaves the same either way.
fn start_discovery(startup: &StartupState) -> discovery_http::DiscoveryResource {
    if env::var("LIGHT_DISCOVERY").is_ok_and(|value| value.eq_ignore_ascii_case("off")) {
        return discovery_http::DiscoveryResource::default();
    }
    discovery_http::DiscoveryResource::start(
        startup.persistent.bind.port(),
        startup
            .persistent
            .active_show
            .as_ref()
            .map(|entry| entry.name.clone()),
    )
}

/// `LIGHT_OUTPUT_WORKERS` (a count, or `max`) sets how many threads an output frame may split its
/// independent per-fixture work over. Unset, the engine uses the available parallelism capped at
/// eight; `1` keeps every frame on the output thread. Outputs are identical for every count.
/// `LIGHT_VERIFY_PLANS=1` checks every planned Position forest against the general walk.
fn configure_output_workers() {
    if env::var("LIGHT_VERIFY_PLANS").is_ok_and(|value| value == "1") {
        light_dynamics::set_plan_verification(true);
    }
    if let Ok(value) = env::var("LIGHT_OUTPUT_WORKERS") {
        match light_engine::parallel::parse_output_workers(&value) {
            Some(workers) => light_engine::parallel::set_default_output_workers(workers),
            None => tracing::warn!(value, "ignoring an invalid LIGHT_OUTPUT_WORKERS"),
        }
    }
}

fn desk_token() -> Option<Arc<str>> {
    env::var("LIGHT_DESK_TOKEN")
        .ok()
        .filter(|token| !token.is_empty())
        .map(Arc::from)
}

#[cfg(test)]
mod log_filter_tests {
    use super::DEFAULT_LOG_FILTER;

    #[test]
    fn the_default_filter_is_a_filter_the_subscriber_accepts() {
        // A malformed default would leave a desk with no log at all, which is the one situation
        // where a log matters most.
        assert!(
            tracing_subscriber::EnvFilter::try_new(DEFAULT_LOG_FILTER).is_ok(),
            "the default log filter has to parse"
        );
    }

    #[test]
    fn the_default_keeps_the_desk_and_its_requests_at_info() {
        // Quiet enough to live with, loud enough that a failing request is recorded at all.
        assert!(DEFAULT_LOG_FILTER.contains("light_headless_runtime=info"));
        assert!(DEFAULT_LOG_FILTER.contains("tower_http=info"));
    }
}

#[cfg(test)]
#[path = "startup_owner_recovery_tests.rs"]
mod startup_owner_recovery_tests;

/// TL-560 test-only: a real `StartupState` turned into the served `AppState` (the same
/// `RuntimeResources::start` + `build_app_state` steps as `RunningServer::start`, without
/// supervisors or rendering), so startup-harness tests can drive HTTP routes after a
/// contract-specific startup. Release it with `close`.
#[cfg(test)]
pub(in crate::runtime) struct ServedStartupForTests {
    pub(in crate::runtime) state: AppState,
    resources: RuntimeResources,
}

#[cfg(test)]
impl ServedStartupForTests {
    pub(in crate::runtime) async fn start(mut startup: StartupState) -> Self {
        let resources = RuntimeResources::start(&mut startup).await.unwrap();
        let state = build_app_state(startup, &resources).unwrap();
        Self { state, resources }
    }

    pub(in crate::runtime) async fn close(self) {
        discard_unstarted_runtime(&self.state, self.resources).await;
    }
}
