//! Persistent state loading and engine restoration for process startup.

use super::{
    DeskConfiguration, Event, PersistedOutputRuntime, active_playbacks_setting,
    compile_active_show_for_startup, ensure_default_show_available, fixed_test_time,
    open_fixture_library_for_startup, output_runtime_setting, rebase_desk_show_paths,
    sibling_fixture_package_dir, startup_options,
};
use light_control::speed::SpeedGroupController;
use light_core::{ManualClock, SharedClock, SystemClock};
use light_engine::Engine;
use light_fixture::FixtureLibrary;
use light_programmer::ProgrammerRegistry;
use light_show::{DeskStore, ShowEntry};
use parking_lot::Mutex;
use std::{
    env,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::broadcast;

pub(super) struct PersistentState {
    pub(super) data_dir: PathBuf,
    pub(super) bind: SocketAddr,
    pub(super) test_bench: bool,
    pub(super) desk: DeskStore,
    pub(super) fixture_library: FixtureLibrary,
    pub(super) configuration: DeskConfiguration,
    pub(super) active_show: Option<ShowEntry>,
}

impl PersistentState {
    fn open(options: startup_options::StartupOptions) -> anyhow::Result<Self> {
        let startup_options::StartupOptions {
            data_dir,
            fixture_package_dir,
            bind,
            test_bench,
            osc_bind_override,
            output_bind_override,
        } = options;
        let fixture_package_dir = fixture_package_directory(fixture_package_dir);
        std::fs::create_dir_all(data_dir.join("shows"))?;
        tracing::info!(path=%data_dir.display(), "opening desk data");
        let desk = DeskStore::open(data_dir.join("desk.sqlite"))?;
        rebase_desk_show_paths(&desk, &data_dir)?;
        let default_show = ensure_default_show_available(&desk, &data_dir)?;
        let fixture_library =
            open_fixture_library_for_startup(&data_dir, fixture_package_dir.as_deref())?;
        let configuration = load_configuration(&desk, osc_bind_override, output_bind_override)?;
        let active_show = load_active_show(&desk, default_show)?;
        tracing::info!(active_show=?active_show.as_ref().map(|show| &show.name), "desk state loaded");
        Ok(Self {
            data_dir,
            bind,
            test_bench,
            desk,
            fixture_library,
            configuration,
            active_show,
        })
    }
}

fn fixture_package_directory(configured: Option<PathBuf>) -> Option<PathBuf> {
    configured.or_else(|| {
        env::current_exe()
            .ok()
            .as_deref()
            .and_then(sibling_fixture_package_dir)
    })
}

fn load_configuration(
    desk: &DeskStore,
    osc_bind_override: Option<SocketAddr>,
    output_bind_override: Option<IpAddr>,
) -> anyhow::Result<DeskConfiguration> {
    let mut configuration: DeskConfiguration = desk
        .setting("server_configuration")?
        .map(|json| serde_json::from_str(&json))
        .transpose()?
        .unwrap_or_default();
    configuration.osc_bind = osc_bind_override
        .or(configuration.osc_bind)
        .or(Some(SocketAddr::from(([127, 0, 0, 1], 9000))));
    if let Some(output_bind_ip) = output_bind_override {
        configuration.output_bind_ip = output_bind_ip;
    }
    configuration
        .validate()
        .map_err(|error| anyhow::anyhow!(error.message))?;
    Ok(configuration)
}

fn load_active_show(
    desk: &DeskStore,
    default_show: ShowEntry,
) -> anyhow::Result<Option<ShowEntry>> {
    if let Some(active) = desk.active_show()? {
        return Ok(Some(active));
    }
    desk.set_active_show(Some(default_show.id))?;
    Ok(Some(default_show))
}

pub(super) struct StartupState {
    pub(super) persistent: PersistentState,
    pub(super) programmers: ProgrammerRegistry,
    pub(super) engine: Arc<Engine>,
    pub(super) active_show_error: Option<String>,
    pub(super) output_runtime: PersistedOutputRuntime,
    pub(super) manual_clock: Option<Arc<ManualClock>>,
    pub(super) speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
    pub(super) events: broadcast::Sender<Event>,
}

impl StartupState {
    pub(super) fn load(options: startup_options::StartupOptions) -> anyhow::Result<Self> {
        let persistent = PersistentState::open(options)?;
        let (manual_clock, programmers) = restore_programmers(&persistent)?;
        let (events, _) = broadcast::channel(256);
        let (engine, active_show_error) = load_engine(&persistent, &programmers)?;
        let output_runtime = load_output_runtime(&persistent, active_show_error.as_deref())?;
        apply_output_runtime(&engine, &output_runtime);
        let speed_groups = create_speed_groups(&persistent.configuration);
        Ok(Self {
            persistent,
            programmers,
            engine,
            active_show_error,
            output_runtime,
            manual_clock,
            speed_groups,
            events,
        })
    }
}

fn restore_programmers(
    persistent: &PersistentState,
) -> anyhow::Result<(Option<Arc<ManualClock>>, ProgrammerRegistry)> {
    let manual_clock = persistent
        .test_bench
        .then(|| Arc::new(ManualClock::new(fixed_test_time())));
    let programmers = ProgrammerRegistry::with_clock(application_clock(manual_clock.as_ref()));
    let users = persistent.desk.users()?;
    for session in persistent.desk.persisted_sessions()? {
        if users.iter().any(|user| user.id == session.user_id) {
            restore_programmer(&programmers, session);
        }
    }
    tracing::info!("persisted programmers restored");
    Ok((manual_clock, programmers))
}

fn application_clock(manual_clock: Option<&Arc<ManualClock>>) -> SharedClock {
    manual_clock
        .map(|clock| Arc::clone(clock) as SharedClock)
        .unwrap_or_else(|| Arc::new(SystemClock))
}

fn restore_programmer(programmers: &ProgrammerRegistry, session: light_show::PersistedSession) {
    match serde_json::from_str::<light_programmer::ProgrammerState>(&session.programmer_json) {
        Ok(mut programmer) => {
            programmer.connected = false;
            programmers.restore(programmer);
        }
        Err(error) => {
            tracing::warn!(session_id=%session.id.0, %error, "ignoring invalid persisted programmer")
        }
    }
}

fn load_engine(
    persistent: &PersistentState,
    programmers: &ProgrammerRegistry,
) -> anyhow::Result<(Arc<Engine>, Option<String>)> {
    let engine = Arc::new(Engine::new(programmers.clone()));
    let active_show_error = compile_active_show(&engine, persistent.active_show.as_ref());
    tracing::info!("engine snapshot ready");
    configure_engine(&engine, &persistent.configuration);
    restore_active_playbacks(persistent, &engine, active_show_error.as_deref())?;
    Ok((engine, active_show_error))
}

fn compile_active_show(engine: &Engine, active_show: Option<&ShowEntry>) -> Option<String> {
    let active = active_show?;
    tracing::info!(show=%active.name, "compiling active show");
    let message = compile_active_show_for_startup(engine, active)?;
    tracing::error!(show=%active.name, error=%message, "starting in show recovery mode");
    Some(message)
}

fn configure_engine(engine: &Engine, configuration: &DeskConfiguration) {
    engine.set_control_timing(
        configuration.speed_groups_bpm,
        configuration.programmer_fade_millis,
        configuration.sequence_master_fade_millis,
    );
}

fn restore_active_playbacks(
    persistent: &PersistentState,
    engine: &Engine,
    recovery_error: Option<&str>,
) -> anyhow::Result<()> {
    let Some(show) = available_show(persistent, recovery_error) else {
        return Ok(());
    };
    let Some(serialized) = persistent
        .desk
        .setting(&active_playbacks_setting(show.id))?
    else {
        return Ok(());
    };
    match serde_json::from_str::<Vec<light_playback::ActivePlayback>>(&serialized) {
        Ok(playbacks) => engine.playback().write().restore_active(playbacks),
        Err(error) => {
            tracing::warn!(show_id=?show.id, %error, "ignoring invalid persisted playback runtime")
        }
    }
    Ok(())
}

fn available_show<'a>(
    persistent: &'a PersistentState,
    recovery_error: Option<&str>,
) -> Option<&'a ShowEntry> {
    persistent
        .active_show
        .as_ref()
        .filter(|_| recovery_error.is_none())
}

fn load_output_runtime(
    persistent: &PersistentState,
    recovery_error: Option<&str>,
) -> anyhow::Result<PersistedOutputRuntime> {
    let Some(show) = available_show(persistent, recovery_error) else {
        return Ok(PersistedOutputRuntime::default());
    };
    let Some(serialized) = persistent.desk.setting(&output_runtime_setting(show.id))? else {
        return Ok(PersistedOutputRuntime::default());
    };
    Ok(parse_output_runtime(show, &serialized))
}

fn parse_output_runtime(show: &ShowEntry, serialized: &str) -> PersistedOutputRuntime {
    match serde_json::from_str::<PersistedOutputRuntime>(serialized) {
        Ok(runtime) if runtime.is_valid() => runtime,
        Ok(_) => {
            tracing::warn!(show_id=?show.id, "ignoring invalid persisted output runtime");
            PersistedOutputRuntime::default()
        }
        Err(error) => {
            tracing::warn!(show_id=?show.id, %error, "ignoring invalid persisted output runtime");
            PersistedOutputRuntime::default()
        }
    }
}

fn apply_output_runtime(engine: &Engine, runtime: &PersistedOutputRuntime) {
    if !runtime.group_masters.is_empty() {
        apply_group_masters(engine, runtime);
    }
    engine
        .playback()
        .write()
        .restore_dynamics_paused_since(runtime.dynamics_paused_at);
}

fn apply_group_masters(engine: &Engine, runtime: &PersistedOutputRuntime) {
    let mut snapshot = (*engine.snapshot()).clone();
    for group in &mut snapshot.groups {
        if let Some(master) = runtime.group_masters.get(&group.id) {
            group.master = *master;
        }
    }
    if let Err(error) = engine.replace_snapshot(snapshot) {
        tracing::warn!(%error, "ignoring persisted group output masters");
    }
}

fn create_speed_groups(configuration: &DeskConfiguration) -> Arc<Mutex<[SpeedGroupController; 5]>> {
    Arc::new(Mutex::new(std::array::from_fn(|index| {
        SpeedGroupController::new(
            configuration.speed_groups_bpm[index],
            configuration.speed_group_sound_to_light[index].clone(),
        )
        .expect("validated Speed Group configuration")
    })))
}
