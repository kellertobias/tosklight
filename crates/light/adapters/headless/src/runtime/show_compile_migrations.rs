use super::*;

pub(super) fn compile_active_show_for_startup(
    engine: &Engine,
    entry: &ShowEntry,
    data_dir: &FsPath,
    backup_retention: usize,
) -> Option<String> {
    let backup = ShowMutationBackupPlan::migration(data_dir, entry, backup_retention);
    let result = prepare_show_load(entry, None)
        .and_then(|prepared| prepared.prepare_runtime(engine))
        .and_then(|prepared| prepared.commit_migration(&backup));
    match result {
        Ok(prepared) => {
            engine.install_prepared_snapshot(prepared);
            None
        }
        Err(error) => Some(format!(
            "The active show '{}' could not be loaded and might be corrupted or incompatible: {error}",
            entry.name
        )),
    }
}
