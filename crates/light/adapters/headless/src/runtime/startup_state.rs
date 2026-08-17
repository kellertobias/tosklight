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
        let configuration = load_configuration(&desk, osc_bind_override, output_bind_override)?;
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
    let mut addresses = std::collections::HashMap::new();
    for (index, value) in values.iter().enumerate() {
        let Some(body) = value.as_object() else {
            continue;
        };
        let Some(fixture_id) = body.get("fixture_id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(attribute) = body.get("attribute").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let canonical = canonical_migration(attribute).map_or(attribute, |migration| migration.0);
        let key = (fixture_id.to_owned(), canonical.to_owned());
        if let Some(previous) = addresses.insert(key, attribute.to_owned())
            && previous != attribute
        {
            return Err(format!(
                "attribute migration conflict at {path}/{index}: fixture {fixture_id} stores both legacy {attribute} and canonical {canonical} values"
            ));
        }
    }
    for value in values {
        let Some(attribute) = value
            .get("attribute")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let Some((canonical, transform)) = canonical_migration(&attribute) else {
            continue;
        };
        if let Some(stored) = value.get_mut("value") {
            if path.contains("dynamic") {
                migrate_programmer_dynamic_value(stored, transform, path)?;
            } else {
                migrate_programmer_attribute_value(stored, transform, path)?;
            }
        } else if transform == light_core::CanonicalAttributeTransform::InvertNormalized {
            return Err(format!(
                "attribute migration failed at {path}: value is missing"
            ));
        }
        value["attribute"] = serde_json::Value::String(canonical.into());
    }
    Ok(())
}

fn migrate_programmer_attribute_map(
    attributes: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<(), String> {
    let migrations = attributes
        .keys()
        .filter_map(|source| {
            canonical_migration(source)
                .map(|(target, transform)| (source.clone(), target, transform))
        })
        .collect::<Vec<_>>();
    for (source, target, _) in &migrations {
        if source != target && attributes.contains_key(*target) {
            return Err(format!(
                "attribute migration conflict at {path}: stored Group values contain both legacy {source} and canonical {target}"
            ));
        }
    }
    for (source, target, transform) in migrations {
        let mut stored = attributes
            .remove(&source)
            .expect("collected Programmer migration source remains present");
        if stored.get("kind").is_some() {
            migrate_programmer_attribute_value(&mut stored, transform, path)?;
        } else if let Some(value) = stored.get_mut("value") {
            migrate_programmer_attribute_value(value, transform, path)?;
        } else if transform == light_core::CanonicalAttributeTransform::InvertNormalized {
            return Err(format!(
                "attribute migration failed at {path}/{source}: Group value payload is missing"
            ));
        }
        attributes.insert(target.into(), stored);
    }
    Ok(())
}

fn migrate_programmer_dynamic_value(
    value: &mut serde_json::Value,
    transform: light_core::CanonicalAttributeTransform,
    path: &str,
) -> Result<(), String> {
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("static") => {
            let stored = value.get_mut("value").ok_or_else(|| {
                format!("attribute migration failed at {path}: static value is missing")
            })?;
            migrate_programmer_attribute_value(stored, transform, path)
        }
        Some("fix_at")
            if transform == light_core::CanonicalAttributeTransform::InvertNormalized =>
        {
            let stored = value
                .get("value")
                .and_then(serde_json::Value::as_f64)
                .ok_or_else(|| {
                    format!("attribute migration failed at {path}: Fix At value must be a number")
                })?;
            value["value"] = serde_json::to_value(light_core::transform_canonical_normalized(
                stored as f32,
                transform,
            ))
            .map_err(|error| error.to_string())?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn migrate_programmer_attribute_value(
    value: &mut serde_json::Value,
    transform: light_core::CanonicalAttributeTransform,
    path: &str,
) -> Result<(), String> {
    if transform == light_core::CanonicalAttributeTransform::Identity {
        return Ok(());
    }
    let stored = value.clone();
    let mut typed = serde_json::from_value::<light_core::AttributeValue>(value.clone())
        .map_err(|error| format!("attribute migration failed at {path}: {error}"))?;
    let before = serde_json::to_value(&typed).map_err(|error| error.to_string())?;
    light_core::transform_canonical_value(&mut typed, transform)
        .map_err(|error| format!("attribute migration failed at {path}: {error}"))?;
    let after = serde_json::to_value(typed).map_err(|error| error.to_string())?;
    *value = stored;
    light_application::lossless_json::apply_delta(value, &before, &after);
    Ok(())
}

fn migrate_embedded_programmer_attributes(
    value: &mut serde_json::Value,
    path: &str,
) -> Result<(), String> {
    let looks_like_dynamic = value.get("target_binding").is_some() && value.get("lanes").is_some();
    if looks_like_dynamic
        && let Ok(mut definition) =
            serde_json::from_value::<light_dynamics::DynamicDefinition>(value.clone())
        && light_dynamics::validate_definition(&definition).is_ok()
    {
        let before = serde_json::to_value(&definition).map_err(|error| error.to_string())?;
        light_dynamics::migrate_canonical_attributes(&mut definition)?;
        let after = serde_json::to_value(definition).map_err(|error| error.to_string())?;
        light_application::lossless_json::apply_delta(value, &before, &after);
    }
    if let Some(body) = value.as_object_mut() {
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

fn is_legacy_strobe_attribute(attribute: &str) -> bool {
    matches!(
        light_core::canonical_attribute_migration_id(attribute),
        Some(("shutter", light_core::CanonicalAttributeTransform::Identity))
    )
}

fn canonical_migration(
    attribute: &str,
) -> Option<(&'static str, light_core::CanonicalAttributeTransform)> {
    light_core::canonical_attribute_migration_id(attribute)
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

    #[test]
    fn legacy_cmy_programmer_values_migrate_inverse_across_normal_preload_and_dynamic_state() {
        let fixture = uuid::Uuid::new_v4();
        let mut value = serde_json::json!({
            "values": [{
                "fixture_id": fixture,
                "attribute": "color.cyan",
                "value": {"kind":"normalized","value":0.2}
            }, {
                "fixture_id": fixture,
                "attribute": "color.cold_white",
                "value": {"kind":"normalized","value":0.35,"future_value":"kept"}
            }, {
                "fixture_id": fixture,
                "attribute": "frost.1",
                "value": {"kind":"normalized","value":0.55}
            }],
            "dynamic_values": [{
                "fixture_id": fixture,
                "attribute": "color.magenta",
                "value": {"type":"fix_at","value":0.3,"timing":{}}
            }],
            "preload_pending": [{
                "fixture_id": fixture,
                "attribute": "color.yellow",
                "value": {"kind":"spread","value":[0.0,0.25,1.0]}
            }, {
                "fixture_id": fixture,
                "attribute": "color.warm_white",
                "value": {"kind":"normalized","value":0.65}
            }],
            "group_values": {"front": {"color.cyan": {
                "value":{"kind":"normalized","value":0.4},
                "changed_at":"2026-08-04T00:00:00Z"
            }}},
            "preload_group_active": {"front": {"color.magenta": {
                "kind":"normalized","value":0.1
            }}},
            "future_programmer": {"kept":true}
        });

        migrate_retired_programmer_attributes(&mut value).unwrap();

        assert_eq!(value["values"][0]["attribute"], "color.red");
        assert_migrated_number(&value["values"][0]["value"]["value"], 0.8);
        assert_eq!(value["values"][1]["attribute"], "color.white");
        assert_migrated_number(&value["values"][1]["value"]["value"], 0.35);
        assert_eq!(value["values"][1]["value"]["future_value"], "kept");
        assert_eq!(value["values"][2]["attribute"], "softness");
        assert_migrated_number(&value["values"][2]["value"]["value"], 0.55);
        assert_eq!(value["dynamic_values"][0]["attribute"], "color.green");
        assert_migrated_number(&value["dynamic_values"][0]["value"]["value"], 0.7);
        assert_eq!(value["preload_pending"][0]["attribute"], "color.blue");
        let spread = value["preload_pending"][0]["value"]["value"]
            .as_array()
            .unwrap();
        for (actual, expected) in spread.iter().zip([1.0, 0.75, 0.0]) {
            assert_migrated_number(actual, expected);
        }
        assert_eq!(value["preload_pending"][1]["attribute"], "color.amber");
        assert_migrated_number(&value["preload_pending"][1]["value"]["value"], 0.65);
        assert_migrated_number(
            &value["group_values"]["front"]["color.red"]["value"]["value"],
            0.6,
        );
        assert_migrated_number(
            &value["preload_group_active"]["front"]["color.green"]["value"],
            0.9,
        );
        assert_eq!(value["future_programmer"], serde_json::json!({"kept":true}));
    }

    #[test]
    fn legacy_position_movement_programmer_values_migrate_and_conflicts_stay_atomic() {
        let fixture = uuid::Uuid::new_v4();
        let mut value = serde_json::json!({
            "values": [{
                "fixture_id": fixture,
                "attribute": "fixture.mspeed",
                "value": {"kind":"normalized","value":0.25}
            }],
            "preload_pending": [{
                "fixture_id": fixture,
                "attribute": "fixture.pan_tilt_speed_time",
                "value": {"kind":"normalized","value":0.75}
            }]
        });
        migrate_retired_programmer_attributes(&mut value).unwrap();
        assert_eq!(value["values"][0]["attribute"], "position.movement");
        assert_eq!(
            value["preload_pending"][0]["attribute"],
            "position.movement"
        );

        let original = serde_json::json!({
            "values": [
                {"fixture_id": fixture, "attribute": "pan.time"},
                {"fixture_id": fixture, "attribute": "tilt.time"}
            ]
        });
        let mut conflict = original.clone();
        let error = migrate_retired_programmer_attributes(&mut conflict).unwrap_err();
        assert!(error.contains("attribute migration conflict"));
        assert!(error.contains("position.movement"));
        assert_eq!(conflict, original);
    }

    #[test]
    fn legacy_media_programmer_values_migrate_and_conflicts_stay_atomic() {
        let fixture = uuid::Uuid::new_v4();
        let mut value = serde_json::json!({
            "values": [{
                "fixture_id": fixture,
                "attribute": "media.opacity",
                "value": {"kind":"normalized","value":0.25}
            }],
            "preload_pending": [{
                "fixture_id": fixture,
                "attribute": "media.rotation",
                "value": {"kind":"normalized","value":0.75}
            }, {
                "fixture_id": fixture,
                "attribute": "media.tint",
                "value": {"kind":"color_xyz","value":{"x":0.2,"y":0.3,"z":0.4}}
            }]
        });
        migrate_retired_programmer_attributes(&mut value).unwrap();
        assert_eq!(value["values"][0]["attribute"], "intensity");
        assert_eq!(
            value["preload_pending"][0]["attribute"],
            "position.rotation"
        );
        assert_eq!(value["preload_pending"][1]["attribute"], "color");

        for (original, target) in [
            (
                serde_json::json!({
                    "values": [
                        {"fixture_id": fixture, "attribute": "media.opacity"},
                        {"fixture_id": fixture, "attribute": "intensity"}
                    ]
                }),
                "intensity",
            ),
            (
                serde_json::json!({
                    "values": [
                        {"fixture_id": fixture, "attribute": "media.tint"},
                        {"fixture_id": fixture, "attribute": "color"}
                    ]
                }),
                "color",
            ),
        ] {
            let mut conflict = original.clone();
            let error = migrate_retired_programmer_attributes(&mut conflict).unwrap_err();
            assert!(error.contains("attribute migration conflict"));
            assert!(error.contains(target));
            assert_eq!(conflict, original);
        }
    }

    #[test]
    fn legacy_endless_axis_programmer_values_migrate_and_conflicts_stay_atomic() {
        let fixture = uuid::Uuid::new_v4();
        let mut value = serde_json::json!({
            "values": [
                {"fixture_id": fixture, "attribute": "pan.continuous"},
                {"fixture_id": fixture, "attribute": "tilt.continuous"}
            ]
        });
        migrate_retired_programmer_attributes(&mut value).unwrap();
        assert_eq!(value["values"][0]["attribute"], "pan");
        assert_eq!(value["values"][1]["attribute"], "tilt");

        for (source, target) in [("pan.continuous", "pan"), ("tilt.continuous", "tilt")] {
            let original = serde_json::json!({
                "values": [
                    {"fixture_id": fixture, "attribute": source},
                    {"fixture_id": fixture, "attribute": target}
                ]
            });
            let mut conflict = original.clone();
            let error = migrate_retired_programmer_attributes(&mut conflict).unwrap_err();
            assert!(error.contains("attribute migration conflict"));
            assert!(error.contains(target));
            assert_eq!(conflict, original);
        }
    }

    fn assert_migrated_number(value: &serde_json::Value, expected: f64) {
        let actual = value.as_f64().expect("expected JSON number");
        assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
    }
}
