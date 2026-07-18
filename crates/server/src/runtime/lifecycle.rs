use super::*;

pub(super) fn open_fixture_library_for_startup(
    data_dir: &FsPath,
    fixture_package_dir: Option<&FsPath>,
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

pub(super) fn sibling_fixture_package_dir(executable: &FsPath) -> Option<PathBuf> {
    let directory = executable.parent()?.join("fixture-library");
    directory.is_dir().then_some(directory)
}

pub(super) fn rebase_desk_show_paths(desk: &DeskStore, data_dir: &FsPath) -> anyhow::Result<()> {
    for entry in desk.library()? {
        let destination = data_dir.join("shows").join(format!("{}.show", entry.name));
        let source = FsPath::new(&entry.path);
        if source == destination {
            continue;
        }
        if destination.exists() {
            if validate_show_file(&destination).is_ok() {
                desk.relocate_show(entry.id, &destination.display().to_string())?;
            }
        } else if source.exists() {
            ShowStore::open(source)?.backup_to(&destination)?;
            desk.relocate_show(entry.id, &destination.display().to_string())?;
        }
    }
    for entry in desk.library()? {
        for revision in desk.show_revisions(entry.id)? {
            let Some(file_name) = FsPath::new(&revision.path).file_name() else {
                continue;
            };
            let destination = data_dir
                .join("revisions")
                .join(entry.id.0.to_string())
                .join(file_name);
            let source = FsPath::new(&revision.path);
            if source == destination {
                continue;
            }
            if destination.exists() {
                if validate_show_file(&destination).is_ok() {
                    desk.relocate_show_revision(
                        entry.id,
                        revision.revision,
                        &destination.display().to_string(),
                    )?;
                }
            } else if source.exists() {
                std::fs::create_dir_all(destination.parent().expect("revision directory"))?;
                ShowStore::open(source)?.backup_to(&destination)?;
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

pub(super) fn preserve_invalid_default_show(
    data_dir: &FsPath,
    path: &FsPath,
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
    data_dir: &FsPath,
) -> anyhow::Result<ShowEntry> {
    let path = data_dir
        .join("shows")
        .join(format!("{}.show", default_show::name()));
    let existing = desk
        .library()?
        .into_iter()
        .find(|entry| entry.name == default_show::name());
    if validate_show_file(&path).is_err() {
        preserve_invalid_default_show(data_dir, &path)?;
        default_show::initialise(&path)?;
    }
    let entry = if let Some(existing) = existing {
        ShowStore::open(&path)?.set_identity(existing.id, &existing.name, None)?;
        desk.relocate_show(existing.id, &path.display().to_string())?
    } else {
        let entry = desk.upsert_show(default_show::name(), &path.display().to_string(), false)?;
        ShowStore::open(&path)?.set_identity(entry.id, &entry.name, None)?;
        entry
    };
    Ok(entry)
}

pub(super) async fn run_server() -> anyhow::Result<()> {
    bootstrap::run().await
}

pub(super) fn router(state: AppState) -> Router {
    http_router::build(state)
}
