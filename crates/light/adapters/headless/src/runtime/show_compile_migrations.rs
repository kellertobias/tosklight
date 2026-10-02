use super::*;

struct PreparedStartupRuntime {
    engine: PreparedEngineSnapshot,
    dynamics: light_dynamics::DynamicRuntime,
}

fn prepare_startup_runtime(
    engine: &Engine,
    snapshot: EngineSnapshot,
) -> Result<PreparedStartupRuntime, EngineError> {
    let native_models = Arc::clone(&snapshot.native_color_sources);
    let prepared = engine.prepare_snapshot(snapshot)?;
    let mut dynamics = light_dynamics::DynamicRuntime::with_native_color_models(
        engine.supported_programming_contract(),
        native_models,
    );
    dynamics
        .install_definitions(prepared.snapshot().dynamics.iter().cloned())
        .map_err(|error| EngineError::Invalid(error.to_string()))?;
    Ok(PreparedStartupRuntime {
        engine: prepared,
        dynamics,
    })
}

/// Prepare both registries before committing portable migrations. The returned runtime owns the
/// exact source catalogue used to validate this snapshot; bootstrap must retain this instance.
pub(super) fn load_active_show_runtime_for_startup(
    engine: &Engine,
    entry: &ShowEntry,
    data_dir: &FsPath,
    backup_retention: usize,
) -> Result<light_dynamics::DynamicRuntime, String> {
    let backup = ShowMutationBackupPlan::migration(data_dir, entry, backup_retention);
    // TL-560: read-only programming-contract gate before anything opens the file for writing.
    if let Err(message) = super::show_programming_contract::check_show_file(
        FsPath::new(&entry.path),
        engine.supported_programming_contract(),
    ) {
        return Err(format!(
            "The active show '{}' could not be loaded: {message}",
            entry.name
        ));
    }
    let result = prepare_show_load(entry, None)
        .and_then(|prepared| {
            prepared.prepare_runtime(|snapshot| prepare_startup_runtime(engine, snapshot))
        })
        .and_then(|prepared| prepared.commit_migration(&backup));
    match result {
        Ok(prepared) => {
            engine.install_prepared_snapshot(prepared.engine);
            Ok(prepared.dynamics)
        }
        Err(error) => Err(format!(
            "The active show '{}' could not be loaded and might be corrupted or incompatible: {error}",
            entry.name
        )),
    }
}

#[cfg(test)]
pub(super) fn compile_active_show_for_startup(
    engine: &Engine,
    entry: &ShowEntry,
    data_dir: &FsPath,
    backup_retention: usize,
) -> Option<String> {
    load_active_show_runtime_for_startup(engine, entry, data_dir, backup_retention).err()
}

#[cfg(test)]
mod tests;
