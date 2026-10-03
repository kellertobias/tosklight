//! Persistent state loading and engine restoration for process startup.

use super::show_programming_contract::{check_programmer, check_runtime_payload};
use super::{
    ActiveShowRepository, DeskConfiguration, InstallationResource, PersistedOutputRuntime,
    active_playbacks_setting, fixed_test_time, load_active_show_runtime_for_startup,
    output_runtime_setting, sibling_fixture_package_dir, startup_options,
};
use anyhow::Context;
use light_control::speed::SpeedGroupController;
use light_core::{ManualClock, SharedClock, SystemClock};
use light_engine::{Engine, EnginePlaybackCommand};
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

/// TL-552: production supports the semantic programming contract (`PROGRAMMING_CONTRACT_VERSION`).
/// Only the `#[cfg(test)]` startup harness can report another contract; see
/// `e2e_semantic_contract`.
fn supported_programming_contract() -> u16 {
    super::e2e_semantic_contract::startup_programming_contract()
}

#[path = "startup_runtime_recovery.rs"]
mod runtime_recovery;

mod programmer_migration;

use programmer_migration::{migrate_frozen_group_selection, migrate_retired_programmer_attributes};

pub(super) fn rebase_desk_show_paths(
    desk: &DeskStore,
    data_dir: &std::path::Path,
) -> anyhow::Result<()> {
    for entry in desk.library()? {
        let destination = data_dir.join("shows").join(format!("{}.show", entry.name));
        let source = std::path::Path::new(&entry.path);
        if source == destination {
            continue;
        }
        if destination.exists() {
            if super::validate_show_file(&destination).is_ok() {
                desk.relocate_show(entry.id, &destination.display().to_string())?;
            }
        } else if source.exists() {
            ActiveShowRepository::open(source)?.backup_to(&destination)?;
            desk.relocate_show(entry.id, &destination.display().to_string())?;
        }
    }
    for entry in desk.library()? {
        for revision in desk.show_revisions(entry.id)? {
            let Some(file_name) = std::path::Path::new(&revision.path).file_name() else {
                continue;
            };
            let destination = data_dir
                .join("revisions")
                .join(entry.id.0.to_string())
                .join(file_name);
            let source = std::path::Path::new(&revision.path);
            if source == destination {
                continue;
            }
            if destination.exists() {
                if super::validate_show_file(&destination).is_ok() {
                    desk.relocate_show_revision(
                        entry.id,
                        revision.revision,
                        &destination.display().to_string(),
                    )?;
                }
            } else if source.exists() {
                std::fs::create_dir_all(destination.parent().expect("revision directory"))?;
                ActiveShowRepository::open(source)?.backup_to(&destination)?;
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

fn preserve_invalid_default_show(
    data_dir: &std::path::Path,
    path: &std::path::Path,
) -> anyhow::Result<()> {
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

pub(super) fn ensure_default_show_available(
    desk: &DeskStore,
    data_dir: &std::path::Path,
) -> anyhow::Result<ShowEntry> {
    let path = data_dir
        .join("shows")
        .join(format!("{}.show", super::default_show::name()));
    let existing = desk
        .library()?
        .into_iter()
        .find(|entry| entry.name == super::default_show::name());
    if super::validate_show_file(&path).is_err() {
        preserve_invalid_default_show(data_dir, &path)?;
        super::default_show::initialise(&path)?;
    }
    let entry = if let Some(existing) = existing {
        ActiveShowRepository::open(&path)?.set_identity(existing.id, &existing.name, None)?;
        desk.relocate_show(existing.id, &path.display().to_string())?
    } else {
        let entry = desk.upsert_show(
            super::default_show::name(),
            &path.display().to_string(),
            false,
        )?;
        ActiveShowRepository::open(&path)?.set_identity(entry.id, &entry.name, None)?;
        entry
    };
    Ok(entry)
}

pub(super) struct PersistentState {
    pub(super) data_dir: PathBuf,
    pub(super) extensions_dir: PathBuf,
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
            show_file,
            fixture_package_dir,
            extensions_dir,
            bind,
            test_bench,
            osc_bind_override,
            output_bind_override,
        } = options;
        let fixture_package_dir = fixture_package_directory(fixture_package_dir);
        let extensions_dir = extensions_directory(extensions_dir)?;
        tracing::info!(path=%extensions_dir.display(), configuration=%data_dir.join("extensions.json").display(), "resolved native extensions installation paths");
        std::fs::create_dir_all(data_dir.join("shows"))?;
        tracing::info!(path=%data_dir.display(), "opening desk data");
        let desk = DeskStore::open(data_dir.join("desk.sqlite"))?;
        rebase_desk_show_paths(&desk, &data_dir)?;
        let default_show = ensure_default_show_available(&desk, &data_dir)?;
        let fixture_library = InstallationResource::open_fixture_library_for_startup(
            &data_dir,
            fixture_package_dir.as_deref(),
        )?;
        let mut configuration = load_configuration(&desk, osc_bind_override, output_bind_override)?;
        super::internal_audio::ensure_default_library_root(&mut configuration, &data_dir)?;
        let active_show = match &show_file {
            Some(path) => Some(adopt_show_file(&desk, path)?),
            None => load_active_show(&desk, default_show)?,
        };
        tracing::info!(active_show=?active_show.as_ref().map(|show| &show.name), "desk state loaded");
        Ok(Self {
            data_dir,
            extensions_dir,
            bind,
            test_bench,
            desk,
            fixture_library,
            configuration,
            active_show,
        })
    }
}

fn extensions_directory(configured: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(configured) = configured {
        return Ok(configured);
    }
    let executable = env::current_exe()
        .context("cannot resolve the headless executable for the Extensions folder")?;
    Ok(light_extensions_host::effective_extensions_directory(
        light_extensions_host::ExtensionsDirectoryMode::Portable(&executable),
    ))
}

fn fixture_package_directory(configured: Option<PathBuf>) -> Option<PathBuf> {
    configured.or_else(|| {
        env::current_exe()
            .ok()
            .as_deref()
            .and_then(sibling_fixture_package_dir)
    })
}

pub(super) fn load_configuration(
    desk: &DeskStore,
    osc_bind_override: Option<SocketAddr>,
    output_bind_override: Option<IpAddr>,
) -> anyhow::Result<DeskConfiguration> {
    let stored = desk.setting("server_configuration")?;
    let parsed = stored
        .as_deref()
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose();
    let mut recovered_malformed = false;
    let raw = match parsed {
        Ok(raw) => raw,
        Err(error) => {
            recovered_malformed = true;
            if desk
                .setting("server_configuration_recovery_report")?
                .is_none()
            {
                desk.set_setting(
                    "server_configuration_recovery_report",
                    &serde_json::to_string_pretty(&serde_json::json!({
                        "schema_version": 1,
                        "summary": "Malformed server configuration was preserved here and replaced with safe defaults.",
                        "error": error.to_string(),
                        "original": stored,
                    }))?,
                )?;
            }
            None
        }
    };
    let has_legacy = if let Some(raw) = raw.as_ref() {
        let midi_inputs = raw
            .get("midi_inputs")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let rtp_midi_bind = raw
            .get("rtp_midi_bind")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let has_legacy = midi_inputs
            .as_array()
            .is_some_and(|ports| !ports.is_empty())
            || !rtp_midi_bind.is_null();
        if has_legacy && desk.setting("removed_midi_inputs_report")?.is_none() {
            desk.set_setting("removed_midi_inputs_report", &serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "migration": "removed_builtin_midi_and_rtp_midi",
                "summary": "Built-in MIDI and RTP-MIDI inputs were removed. Recreate these endpoints in an approved native extension package.",
                "midi_inputs": midi_inputs,
                "rtp_midi_bind": rtp_midi_bind,
            }))?)?;
        }
        raw.get("midi_inputs").is_some() || raw.get("rtp_midi_bind").is_some()
    } else {
        false
    };
    let mut configuration: DeskConfiguration = raw
        .clone()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    if has_legacy {
        let mut cleaned = raw.expect("legacy configuration must have parsed");
        if let Some(object) = cleaned.as_object_mut() {
            object.remove("midi_inputs");
            object.remove("rtp_midi_bind");
        }
        desk.set_setting("server_configuration", &serde_json::to_string(&cleaned)?)?;
    } else if recovered_malformed {
        desk.set_setting(
            "server_configuration",
            &serde_json::to_string(&configuration)?,
        )?;
    }
    configuration.migrate_speed_group_sources();
    configuration.migrate_highlight_look();
    configuration.migrate_update_settings(desk.desk()?.id);
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

/// Open the show file the operator named on the command line and make it the active show.
///
/// The file is registered in the desk library under its own file name so the rest of the desk
/// treats it exactly like any other show; nothing about the file itself is rewritten.
fn adopt_show_file(desk: &DeskStore, path: &std::path::Path) -> anyhow::Result<ShowEntry> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("show file {} cannot be opened", path.display()))?;
    super::validate_show_file(&path)
        .map_err(|error| anyhow::anyhow!("show file {} is not usable: {error}", path.display()))?;
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("show")
        .to_owned();
    let entry = desk.upsert_show(&name, &path.display().to_string(), true)?;
    ActiveShowRepository::open(&path)?.set_identity(entry.id, &entry.name, None)?;
    desk.set_active_show(Some(entry.id))?;
    tracing::info!(path=%path.display(), show=%entry.name, "opened the show file named at startup");
    Ok(entry)
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
    pub(super) dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    pub(super) active_show_error: Option<String>,
    pub(super) output_runtime: PersistedOutputRuntime,
    pub(super) manual_clock: Option<Arc<ManualClock>>,
    pub(super) speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
}

impl StartupState {
    pub(super) fn load(options: startup_options::StartupOptions) -> anyhow::Result<Self> {
        let persistent = PersistentState::open(options)?;
        let (manual_clock, programmers, programmer_recovery) = restore_programmers(&persistent)?;
        let (engine, dynamics, active_show_error) =
            load_engine(&persistent, &programmers, programmer_recovery)?;
        let (output_runtime, output_recovery) =
            load_output_runtime(&persistent, &programmers, active_show_error.as_deref())?;
        let active_show_error = active_show_error.or(output_recovery);
        apply_output_runtime(&engine, &output_runtime);
        let speed_groups = create_speed_groups(&persistent.configuration);
        Ok(Self {
            persistent,
            programmers,
            engine,
            dynamics,
            active_show_error,
            output_runtime,
            manual_clock,
            speed_groups,
        })
    }
}

fn restore_programmers(
    persistent: &PersistentState,
) -> anyhow::Result<(Option<Arc<ManualClock>>, ProgrammerRegistry, Option<String>)> {
    let manual_clock = persistent
        .test_bench
        .then(|| Arc::new(ManualClock::new(fixed_test_time())));
    let programmers = ProgrammerRegistry::with_clock(application_clock(manual_clock.as_ref()));
    // A desk written before the collapse can hold a Programmer per user. Restoring them all would
    // merge divergent work with nothing said about it, so one is chosen and the rest are written
    // out where an operator can get them back.
    let persisted = persistent.desk.persisted_sessions()?;
    let collapse = super::desk_collapse_migration::DeskCollapse::decide(persisted);
    if collapse.superseded_anything() {
        let report = super::desk_collapse_migration::write_collapse_report(
            &persistent.data_dir,
            &collapse,
            &chrono::Utc::now().to_rfc3339(),
        )?;
        tracing::warn!(
            superseded = collapse.superseded.len(),
            report = %report.display(),
            "this desk held more than one Programmer; the most recently touched one was kept and \
             the rest were written out"
        );
    }
    let recovery = collapse
        .canonical
        .map(|session| restore_programmer(&programmers, session, &persistent.data_dir))
        .transpose()?
        .flatten();
    tracing::info!("persisted programmers restored");
    Ok((manual_clock, programmers, recovery))
}

fn application_clock(manual_clock: Option<&Arc<ManualClock>>) -> SharedClock {
    manual_clock
        .map(|clock| Arc::clone(clock) as SharedClock)
        .unwrap_or_else(|| Arc::new(SystemClock))
}

fn restore_programmer(
    programmers: &ProgrammerRegistry,
    session: light_show::PersistedSession,
    data_dir: &std::path::Path,
) -> anyhow::Result<Option<String>> {
    let parsed = (|| -> anyhow::Result<light_programmer::ProgrammerState> {
        let mut value = serde_json::from_str::<serde_json::Value>(&session.programmer_json)?;
        migrate_frozen_group_selection(&mut value);
        migrate_retired_programmer_attributes(&mut value).map_err(anyhow::Error::msg)?;
        check_programmer(&value, supported_programming_contract())?;
        let programmer: light_programmer::ProgrammerState = serde_json::from_value(value)?;
        anyhow::ensure!(
            programmer.required_programming_contract() <= supported_programming_contract(),
            "stored Programmer requires programming contract {}; this runtime supports {}",
            programmer.required_programming_contract(),
            supported_programming_contract()
        );
        programmer
            .validate_programming()
            .map_err(anyhow::Error::msg)?;
        Ok(programmer)
    })();
    match parsed {
        Ok(mut programmer) => {
            programmer.connected = false;
            programmers.restore(programmer);
            Ok(None)
        }
        Err(error) => runtime_recovery::preserve(
            data_dir,
            "Programmer",
            session.id.0,
            &session.programmer_json,
            &error,
        )
        .map(Some),
    }
}

fn load_engine(
    persistent: &PersistentState,
    programmers: &ProgrammerRegistry,
    programmer_recovery: Option<String>,
) -> anyhow::Result<(
    Arc<Engine>,
    Arc<Mutex<light_dynamics::DynamicRuntime>>,
    Option<String>,
)> {
    let engine = Arc::new(Engine::with_programming_contract_support(
        programmers.clone(),
        supported_programming_contract(),
    ));
    let (dynamics, active_show_error) = match programmer_recovery {
        Some(error) => (None, Some(error)),
        None => match compile_active_show(&engine, persistent) {
            Ok(dynamics) => (dynamics, None),
            Err(error) => (None, Some(error)),
        },
    };
    // Failed or absent shows retain the empty Engine and its empty source catalogue. Never carry
    // a rejected candidate's models into Programmer reconstruction or checkpoint recovery.
    let dynamics = Arc::new(Mutex::new(dynamics.unwrap_or_else(|| {
        light_dynamics::DynamicRuntime::with_native_color_models(
            engine.supported_programming_contract(),
            engine.snapshot().native_color_sources.clone(),
        )
    })));
    tracing::info!("engine snapshot ready");
    configure_engine(&engine, &persistent.configuration)?;
    let playback_recovery =
        restore_active_playbacks(persistent, &engine, active_show_error.as_deref())?;
    let active_show_error = active_show_error.or(playback_recovery);
    Ok((engine, dynamics, active_show_error))
}

fn compile_active_show(
    engine: &Engine,
    persistent: &PersistentState,
) -> Result<Option<light_dynamics::DynamicRuntime>, String> {
    let Some(active) = persistent.active_show.as_ref() else {
        return Ok(None);
    };
    tracing::info!(show=%active.name, "compiling active show");
    load_active_show_runtime_for_startup(
        engine,
        active,
        &persistent.data_dir,
        persistent.configuration.backup_retention,
    )
    .map(Some)
    .map_err(|message| {
        tracing::error!(show=%active.name, error=%message, "starting in show recovery mode");
        message
    })
}

fn configure_engine(engine: &Engine, configuration: &DeskConfiguration) -> anyhow::Result<()> {
    engine.set_control_timing(
        configuration.speed_groups_bpm,
        configuration.programmer_fade_millis,
        configuration.sequence_master_fade_millis,
        configuration.release_fade_millis,
    );
    engine
        .set_highlight_look(configuration.highlight_look.clone())
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

fn restore_active_playbacks(
    persistent: &PersistentState,
    engine: &Engine,
    recovery_error: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let Some(show) = available_show(persistent, recovery_error) else {
        return Ok(None);
    };
    let Some(serialized) = persistent
        .desk
        .setting(&active_playbacks_setting(show.id))?
    else {
        return Ok(None);
    };
    let parsed = (|| -> anyhow::Result<Vec<light_playback::ActivePlayback>> {
        check_runtime_payload("Playback", &serialized, supported_programming_contract())?;
        let playbacks = serde_json::from_str::<Vec<light_playback::ActivePlayback>>(&serialized)?;
        let required = playbacks
            .iter()
            .map(light_playback::ActivePlayback::required_programming_contract)
            .max()
            .unwrap_or(0);
        anyhow::ensure!(
            required <= engine.supported_programming_contract(),
            "stored Playback runtime requires programming contract {required}; this runtime supports {}",
            engine.supported_programming_contract()
        );
        Ok(playbacks)
    })();
    match parsed {
        Ok(playbacks) => {
            engine
                .execute_playback(EnginePlaybackCommand::RestoreActive(playbacks))
                .expect("restoring validated Playback state is infallible");
            Ok(None)
        }
        Err(error) => runtime_recovery::preserve(
            &persistent.data_dir,
            "Playback",
            show.id.0,
            &serialized,
            &error,
        )
        .map(Some),
    }
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
    programmers: &ProgrammerRegistry,
    recovery_error: Option<&str>,
) -> anyhow::Result<(PersistedOutputRuntime, Option<String>)> {
    let Some(show) = available_show(persistent, recovery_error) else {
        return Ok((PersistedOutputRuntime::default(), None));
    };
    let Some(serialized) = persistent.desk.setting(&output_runtime_setting(show.id))? else {
        return Ok((PersistedOutputRuntime::default(), None));
    };
    let candidate = parse_output_runtime(&serialized).and_then(|mut runtime| {
        if let Some(snapshot) = &mut runtime.dynamic_runtime {
            super::normalize_programmer_dynamic_checkpoint(programmers, snapshot)?;
        }
        // Legacy controller-key normalization must not leave a previously valid source
        // catalogue pointing at different retained expression/controller identities.
        runtime.validate_for_support(supported_programming_contract())?;
        Ok(runtime)
    });
    match candidate {
        Ok(runtime) => Ok((runtime, None)),
        Err(error) => {
            let message = runtime_recovery::preserve(
                &persistent.data_dir,
                "Output",
                show.id.0,
                &serialized,
                &error,
            )?;
            Ok((PersistedOutputRuntime::default(), Some(message)))
        }
    }
}

fn parse_output_runtime(serialized: &str) -> anyhow::Result<PersistedOutputRuntime> {
    check_runtime_payload("Output", serialized, supported_programming_contract())?;
    // Embedded/deleted Dynamic definitions are independent of the active show's gate.
    PersistedOutputRuntime::decode_for_support(serialized, supported_programming_contract())
}

fn apply_output_runtime(engine: &Engine, runtime: &PersistedOutputRuntime) {
    if !runtime.group_masters.is_empty() {
        apply_group_masters(engine, runtime);
    }
    engine
        .execute_playback(EnginePlaybackCommand::RestoreDynamicsPausedSince(
            runtime.dynamics_paused_at,
        ))
        .expect("restoring dynamics pause state is infallible");
    engine
        .execute_playback(EnginePlaybackCommand::RestoreActiveDynamics(
            runtime.dynamic_playbacks.clone(),
        ))
        .expect("restoring validated Dynamic Playback state is infallible");
}

fn apply_group_masters(engine: &Engine, runtime: &PersistedOutputRuntime) {
    for (group_id, master) in &runtime.group_masters {
        if let Err(error) = engine.set_group_master(group_id, *master) {
            tracing::warn!(%group_id, %error, "ignoring unassigned persisted Group Master");
        }
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

#[cfg(test)]
mod tests;

/// Source-only originals, captured before normalization or any success-path persistence.
/// Session authentication data is deliberately excluded from this type.
#[derive(serde::Serialize)]
pub(super) struct OriginalStartupOwners {
    show_id: Option<light_core::ShowId>,
    programmers: Vec<OriginalStartupProgrammer>,
    playback: Option<String>,
    output: Option<String>,
}

#[derive(serde::Serialize)]
struct OriginalStartupProgrammer {
    session_id: light_core::SessionId,
    programmer_json: String,
}

impl OriginalStartupOwners {
    pub(super) fn capture(persistent: &PersistentState) -> anyhow::Result<Self> {
        let show_id = persistent.active_show.as_ref().map(|show| show.id);
        let programmers = persistent
            .desk
            .persisted_sessions()?
            .into_iter()
            .map(|session| OriginalStartupProgrammer {
                session_id: session.id,
                programmer_json: session.programmer_json,
            })
            .collect();
        let (playback, output) = match show_id {
            Some(show) => (
                persistent.desk.setting(&active_playbacks_setting(show))?,
                persistent.desk.setting(&output_runtime_setting(show))?,
            ),
            None => (None, None),
        };
        Ok(Self {
            show_id,
            programmers,
            playback,
            output,
        })
    }
}

/// Process startup only, before rendering/control inputs/server tasks begin.
/// Live show activation retains its separate destination preflight boundary.
pub(super) fn finalize_restored_owners_for_startup(
    state: &super::AppState,
    originals: OriginalStartupOwners,
) -> anyhow::Result<()> {
    if state.active_show.error().is_some() {
        return Ok(());
    }
    let normalized = super::playback_exclusion_normalization::
        normalize_restored_virtual_playback_exclusions_deferred(state)
        .map_err(|error| anyhow::anyhow!(error.message))?;
    match state
        .output
        .finalize_restored_owners(&state.playback.render_capability())
    {
        Ok(()) => {
            if normalized.persistence_pending {
                if let Err(error) = super::persist_active_playbacks(state) {
                    tracing::warn!(error=%error.message,
                        "validated restored Playback normalization persistence is pending");
                }
            }
            Ok(())
        }
        Err(error) => {
            // Prepare the complete empty pair before touching Programmer or published state.
            let prepared = state.output.prepare_snapshot(Default::default())?;
            let checkpoint = super::dynamic_source_origins::DynamicRuntimeSourceCheckpoint {
                runtime: Default::default(),
                origins: Some(
                    super::dynamic_source_origins::DynamicSourceOrigins::default().snapshot(),
                ),
            };
            let prepared = state
                .output
                .prepare_snapshot_restore(prepared, checkpoint)?;
            let serialized = serde_json::to_string(&originals)?;
            let message = runtime_recovery::preserve(
                state.installation.data_dir(),
                "Dynamic owners",
                originals
                    .show_id
                    .map(|id| id.0)
                    .unwrap_or_else(uuid::Uuid::nil),
                &serialized,
                &anyhow::anyhow!(error.to_string()),
            )?;
            // Reporting must succeed before the irreversible in-memory recovery commit.
            // This guard prevents controls/shutdown from overwriting either saved checkpoint.
            state.active_show.set_error(Some(message));
            state.programming.reset_all();
            state
                .output
                .install_prepared_snapshot_releasing_playback(prepared);
            super::restore_prevalidated_output_controls(state, &PersistedOutputRuntime::default());
            Ok(())
        }
    }
}
