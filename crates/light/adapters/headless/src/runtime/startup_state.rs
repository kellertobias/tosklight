//! Persistent state loading and engine restoration for process startup.

use super::{
    ActiveShowRepository, DeskConfiguration, InstallationResource, PersistedOutputRuntime,
    active_playbacks_setting, compile_active_show_for_startup, fixed_test_time,
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
        let fixture_library = InstallationResource::open_fixture_library_for_startup(
            &data_dir,
            fixture_package_dir.as_deref(),
        )?;
        let configuration = load_configuration(&desk, osc_bind_override, output_bind_override)?;
        let active_show = match &show_file {
            Some(path) => Some(adopt_show_file(&desk, path)?),
            None => load_active_show(&desk, default_show)?,
        };
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
    configuration.migrate_speed_group_sources();
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
    pub(super) active_show_error: Option<String>,
    pub(super) output_runtime: PersistedOutputRuntime,
    pub(super) manual_clock: Option<Arc<ManualClock>>,
    pub(super) speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
}

impl StartupState {
    pub(super) fn load(options: startup_options::StartupOptions) -> anyhow::Result<Self> {
        let persistent = PersistentState::open(options)?;
        let (manual_clock, programmers) = restore_programmers(&persistent)?;
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
    let parsed = (|| -> anyhow::Result<light_programmer::ProgrammerState> {
        let mut value = serde_json::from_str::<serde_json::Value>(&session.programmer_json)?;
        migrate_frozen_group_selection(&mut value);
        migrate_retired_programmer_attributes(&mut value).map_err(anyhow::Error::msg)?;
        Ok(serde_json::from_value(value)?)
    })();
    match parsed {
        Ok(mut programmer) => {
            programmer.connected = false;
            programmers.restore(programmer);
        }
        Err(error) => {
            tracing::warn!(session_id=%session.id.0, %error, "ignoring invalid persisted programmer")
        }
    }
}

/// Normalizes retired canonical identities in durable Programmer and Preload state. This stays a
/// scoped JSON migration instead of changing `AttributeKey` deserialization globally: fixture
/// profiles deliberately retain their fixture-facing source identity.
fn migrate_retired_programmer_attributes(value: &mut serde_json::Value) -> Result<(), String> {
    let mut migrated = value.clone();
    migrate_retired_programmer_attributes_in_place(&mut migrated)?;
    *value = migrated;
    Ok(())
}

fn migrate_retired_programmer_attributes_in_place(
    value: &mut serde_json::Value,
) -> Result<(), String> {
    let Some(programmer) = value.as_object_mut() else {
        return Ok(());
    };
    for field in [
        "values",
        "dynamic_values",
        "preload_pending",
        "preload_active",
        "preload_dynamic_pending",
        "preload_dynamic_active",
    ] {
        if let Some(values) = programmer
            .get_mut(field)
            .and_then(serde_json::Value::as_array_mut)
        {
            migrate_programmer_value_records(values, field)?;
        }
    }
    for field in [
        "group_values",
        "preload_group_pending",
        "preload_group_active",
    ] {
        if let Some(groups) = programmer
            .get_mut(field)
            .and_then(serde_json::Value::as_object_mut)
        {
            for (group_id, attributes) in groups {
                let Some(attributes) = attributes.as_object_mut() else {
                    continue;
                };
                migrate_programmer_attribute_map(attributes, &format!("{field}/{group_id}"))?;
            }
        }
    }
    for history in ["undo", "redo"] {
        if let Some(snapshots) = programmer
            .get_mut(history)
            .and_then(serde_json::Value::as_array_mut)
        {
            for snapshot in snapshots {
                migrate_retired_programmer_attributes_in_place(snapshot)?;
            }
        }
    }
    migrate_embedded_programmer_attributes(value, "programmer")?;
    Ok(())
}

fn migrate_programmer_value_records(
    values: &mut [serde_json::Value],
    path: &str,
) -> Result<(), String> {
    let mut legacy = std::collections::HashSet::new();
    let mut canonical = std::collections::HashSet::new();
    for (index, value) in values.iter().enumerate() {
        let Some(body) = value.as_object() else {
            continue;
        };
        let Some(fixture_id) = body.get("fixture_id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        match body.get("attribute").and_then(serde_json::Value::as_str) {
            Some(attribute) if is_legacy_strobe_attribute(attribute) => {
                legacy.insert(fixture_id.to_owned());
            }
            Some("shutter") => {
                canonical.insert(fixture_id.to_owned());
            }
            _ => continue,
        }
        if legacy.contains(fixture_id) && canonical.contains(fixture_id) {
            return Err(format!(
                "attribute migration conflict at {path}/{index}: fixture {fixture_id} stores both legacy Strobe and canonical Shutter values"
            ));
        }
    }
    for value in values {
        migrate_programmer_attribute_field(value);
    }
    Ok(())
}

fn migrate_programmer_attribute_map(
    attributes: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<(), String> {
    let legacy = attributes
        .keys()
        .find(|attribute| is_legacy_strobe_attribute(attribute))
        .cloned();
    if legacy.is_some() && attributes.contains_key("shutter") {
        return Err(format!(
            "attribute migration conflict at {path}: stored Group values contain both legacy Strobe and canonical Shutter"
        ));
    }
    if let Some(value) = legacy.and_then(|attribute| attributes.remove(&attribute)) {
        attributes.insert("shutter".into(), value);
    }
    Ok(())
}

fn migrate_embedded_programmer_attributes(
    value: &mut serde_json::Value,
    path: &str,
) -> Result<(), String> {
    if let Some(body) = value.as_object_mut() {
        if body.get("target_binding").is_some()
            && let Some(lanes) = body
                .get_mut("lanes")
                .and_then(serde_json::Value::as_array_mut)
        {
            let has_legacy = lanes.iter().any(|lane| {
                lane.get("attribute")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(is_legacy_strobe_attribute)
            });
            let has_canonical = lanes.iter().any(|lane| lane["attribute"] == "shutter");
            if has_legacy && has_canonical {
                return Err(format!(
                    "attribute migration conflict at {path}/lanes: Dynamic stores both legacy Strobe and canonical Shutter lanes"
                ));
            }
        }
        if body
            .get("attribute")
            .and_then(serde_json::Value::as_str)
            .is_some_and(is_legacy_strobe_attribute)
        {
            body.insert(
                "attribute".into(),
                serde_json::Value::String("shutter".into()),
            );
        }
        for (field, value) in body {
            migrate_embedded_programmer_attributes(value, &format!("{path}/{field}"))?;
        }
    } else if let Some(values) = value.as_array_mut() {
        for (index, value) in values.iter_mut().enumerate() {
            migrate_embedded_programmer_attributes(value, &format!("{path}/{index}"))?;
        }
    }
    Ok(())
}

fn migrate_programmer_attribute_field(value: &mut serde_json::Value) {
    if let Some(body) = value.as_object_mut()
        && body
            .get("attribute")
            .and_then(serde_json::Value::as_str)
            .is_some_and(is_legacy_strobe_attribute)
    {
        body.insert(
            "attribute".into(),
            serde_json::Value::String("shutter".into()),
        );
    }
}

fn is_legacy_strobe_attribute(attribute: &str) -> bool {
    matches!(
        light_core::canonical_attribute_migration_id(attribute),
        Some(("shutter", light_core::CanonicalAttributeTransform::Identity))
    )
}

/// Programmers persisted before the DEGRP rework may carry the removed `frozen_group` selection
/// expression (including inside undo/redo snapshots). Dereference it to the concrete fixtures the
/// selection already resolved to, matching current DEGRP semantics.
fn migrate_frozen_group_selection(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let frozen = object
        .get("selection_expression")
        .and_then(|expression| expression.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("frozen_group");
    if frozen {
        let items = object
            .get("selected")
            .and_then(serde_json::Value::as_array)
            .map(|selected| {
                selected
                    .iter()
                    .map(|fixture_id| {
                        serde_json::json!({"type": "fixture", "fixture_id": fixture_id})
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        object.insert(
            "selection_expression".into(),
            serde_json::json!({"type": "sources", "items": items}),
        );
    }
    for history in ["undo", "redo"] {
        if let Some(snapshots) = object
            .get_mut(history)
            .and_then(serde_json::Value::as_array_mut)
        {
            for snapshot in snapshots {
                migrate_frozen_group_selection(snapshot);
            }
        }
    }
}

fn load_engine(
    persistent: &PersistentState,
    programmers: &ProgrammerRegistry,
) -> anyhow::Result<(Arc<Engine>, Option<String>)> {
    let engine = Arc::new(Engine::new(programmers.clone()));
    let active_show_error = compile_active_show(&engine, persistent);
    tracing::info!("engine snapshot ready");
    configure_engine(&engine, &persistent.configuration)?;
    restore_active_playbacks(persistent, &engine, active_show_error.as_deref())?;
    Ok((engine, active_show_error))
}

fn compile_active_show(engine: &Engine, persistent: &PersistentState) -> Option<String> {
    let active = persistent.active_show.as_ref()?;
    tracing::info!(show=%active.name, "compiling active show");
    let message = compile_active_show_for_startup(
        engine,
        active,
        &persistent.data_dir,
        persistent.configuration.backup_retention,
    )?;
    tracing::error!(show=%active.name, error=%message, "starting in show recovery mode");
    Some(message)
}

fn configure_engine(engine: &Engine, configuration: &DeskConfiguration) -> anyhow::Result<()> {
    engine.set_control_timing(
        configuration.speed_groups_bpm,
        configuration.programmer_fade_millis,
        configuration.sequence_master_fade_millis,
    );
    engine
        .set_highlight_look(configuration.highlight_look.clone())
        .map_err(|error| anyhow::anyhow!(error.to_string()))
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
        Ok(playbacks) => {
            engine
                .execute_playback(EnginePlaybackCommand::RestoreActive(playbacks))
                .expect("restoring validated Playback state is infallible");
        }
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
mod tests {
    use super::{migrate_frozen_group_selection, migrate_retired_programmer_attributes};

    /// Restart recovery of a durable programmer persisted before the DEGRP rework: the removed
    /// `frozen_group` selection expression (top level and inside undo/redo snapshots) must map to
    /// the dereferenced `sources` form instead of dropping the whole programmer.
    #[test]
    fn legacy_frozen_group_programmer_snapshots_restore_as_dereferenced_sources() {
        let fixture_a = uuid::Uuid::new_v4();
        let fixture_b = uuid::Uuid::new_v4();
        let mut value = serde_json::json!({
            "id": uuid::Uuid::new_v4(),
            "session_id": uuid::Uuid::new_v4(),
            "user_id": uuid::Uuid::new_v4(),
            "priority": 0,
            "selected": [fixture_a, fixture_b],
            "selection_expression": {"type": "frozen_group", "group_id": "7", "source_revision": 4},
            "values": [],
            "connected": true,
            "last_activity": "2026-07-20T09:30:00Z",
            "undo": [{
                "selected": [fixture_a],
                "selection_expression": {"type": "frozen_group", "group_id": "7", "source_revision": 3},
            }],
        });
        migrate_frozen_group_selection(&mut value);
        let programmer: light_programmer::ProgrammerState =
            serde_json::from_value(value).expect("legacy frozen_group programmer deserializes");
        let expected = |fixtures: &[uuid::Uuid]| light_programmer::SelectionExpression::Sources {
            items: fixtures
                .iter()
                .map(|id| light_programmer::SelectionReference::Fixture {
                    fixture_id: light_core::FixtureId(*id),
                })
                .collect(),
        };
        assert_eq!(
            programmer.selection_expression,
            Some(expected(&[fixture_a, fixture_b]))
        );
        assert_eq!(
            programmer.undo[0].selection_expression,
            Some(expected(&[fixture_a]))
        );
        assert_eq!(
            programmer.selected,
            vec![
                light_core::FixtureId(fixture_a),
                light_core::FixtureId(fixture_b)
            ]
        );
    }

    #[test]
    fn retired_strobe_programmer_values_migrate_across_normal_preload_dynamic_and_history_state() {
        let fixture = uuid::Uuid::new_v4();
        let mut value = serde_json::json!({
            "values": [{"fixture_id": fixture, "attribute": "strobe", "value": 0.4}],
            "dynamic_values": [{
                "fixture_id": fixture,
                "attribute": "strobe",
                "value": {"type": "dynamic_on", "dynamic": {
                    "embedded_fallback": {"definition": {
                        "target_binding": {"type": "targetless"},
                        "lanes": [{
                            "id": uuid::Uuid::new_v4(),
                            "attribute": "strobe",
                            "keyframes": {"points": [{"source": {
                                "type": "preset", "attribute": "strobe"
                            }}]}
                        }],
                        "phase": {},
                        "speed": {}
                    }}
                }}
            }],
            "preload_pending": [{"fixture_id": fixture, "attribute": "strobe"}],
            "group_values": {"front": {"strobe": {"value": 0.5}}},
            "preload_group_active": {"front": {"strobe": {"value": 0.6}}},
            "undo": [{
                "values": [{"fixture_id": fixture, "attribute": "strobe"}],
                "group_values": {"front": {"strobe": {"value": 0.3}}}
            }],
            "future_programmer": {"kept": true}
        });

        migrate_retired_programmer_attributes(&mut value).unwrap();

        assert_eq!(value["values"][0]["attribute"], "shutter");
        assert_eq!(value["dynamic_values"][0]["attribute"], "shutter");
        assert_eq!(
            value["dynamic_values"][0]["value"]["dynamic"]["embedded_fallback"]["definition"]["lanes"]
                [0]["attribute"],
            "shutter"
        );
        assert_eq!(
            value["dynamic_values"][0]["value"]["dynamic"]["embedded_fallback"]["definition"]["lanes"]
                [0]["keyframes"]["points"][0]["source"]["attribute"],
            "shutter"
        );
        assert_eq!(value["preload_pending"][0]["attribute"], "shutter");
        assert_eq!(value["group_values"]["front"]["shutter"]["value"], 0.5);
        assert_eq!(
            value["preload_group_active"]["front"]["shutter"]["value"],
            0.6
        );
        assert_eq!(value["undo"][0]["values"][0]["attribute"], "shutter");
        assert_eq!(
            value["undo"][0]["group_values"]["front"]["shutter"]["value"],
            0.3
        );
        assert_eq!(
            value["future_programmer"],
            serde_json::json!({"kept": true})
        );

        let once = value.clone();
        migrate_retired_programmer_attributes(&mut value).unwrap();
        assert_eq!(value, once, "Programmer migration must be idempotent");
    }

    #[test]
    fn retired_strobe_programmer_conflict_preserves_the_original_json() {
        let fixture = uuid::Uuid::new_v4();
        let original = serde_json::json!({
            "values": [
                {"fixture_id": fixture, "attribute": "strobe"},
                {"fixture_id": fixture, "attribute": "shutter"}
            ]
        });
        let mut value = original.clone();

        let error = migrate_retired_programmer_attributes(&mut value).unwrap_err();

        assert!(error.contains("attribute migration conflict at values/1"));
        assert_eq!(value, original);
    }
}
