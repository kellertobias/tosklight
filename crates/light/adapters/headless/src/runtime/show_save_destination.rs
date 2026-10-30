//! Save portable copies and MVR exports to an explicitly selected confined folder.
use super::*;
use light_wire::v2::show_library::MvrExportSummary;
use std::{fs, io::Write, path::Path as FsPath};

pub(super) fn file_name(name: &str, extension: &str) -> Result<String, ApiError> {
    let name = name.trim();
    let stem = name.strip_suffix(&format!(".{extension}")).unwrap_or(name);
    validate_show_name(stem)?;
    if matches!(stem, "." | "..") || stem.chars().any(char::is_control) {
        return Err(ApiError::bad_request("name must be a plain file name"));
    }
    Ok(format!("{stem}.{extension}"))
}

pub(super) fn write_new(path: &FsPath, bytes: &[u8]) -> Result<(), ApiError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                ApiError::conflict("a file with that name already exists")
            } else {
                ApiError::io(error)
            }
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(ApiError::io(error));
    }
    Ok(())
}

pub(super) fn save_copy(
    state: &AppState,
    source_show_id: Option<Uuid>,
    data_base64: Option<String>,
    name: String,
    root_id: String,
    path: String,
    is_base_show: bool,
    overwrite: bool,
) -> Result<ShowEntry, ApiError> {
    let directory = super::file_manager::writable_show_folder(state, &root_id, &path)?;
    let leaf = file_name(&name, "show")?;
    let destination = directory.join(leaf);
    let replaced = if destination.exists() {
        if !overwrite {
            return Err(ApiError::conflict("a file with that name already exists"));
        }
        Some(replaceable_show(state, &destination)?)
    } else {
        None
    };
    let staged = directory.join(format!(".show-save-{}.show", Uuid::new_v4()));
    let prepared = (|| {
        match (source_show_id, data_base64) {
            (Some(id), None) => {
                let source = state
                    .installation
                    .show(light_core::ShowId(id))
                    .map_err(ApiError::store)?
                    .ok_or_else(|| ApiError::not_found("source show"))?;
                ActiveShowRepository::open(&source.path)
                    .map_err(ApiError::store)?
                    .backup_to(&staged)
                    .map_err(ApiError::store)?;
            }
            (None, Some(encoded)) => {
                let bytes = STANDARD.decode(encoded).map_err(|_| {
                    ApiError::bad_request("data_base64 must contain a portable show")
                })?;
                fs::write(&staged, bytes).map_err(ApiError::io)?;
            }
            _ => {
                return Err(ApiError::bad_request(
                    "provide exactly one source_show_id or data_base64",
                ));
            }
        }
        validate_show_file(&staged).map_err(ApiError::store)?;
        // Catalog names are unique even when copies are stored in different folders. A replaced
        // catalogued show keeps its name and identity, so references to it stay valid.
        let catalog_name = match replaced.as_ref().and_then(Option::as_ref) {
            Some(existing) => existing.name.clone(),
            None => available_show_name(
                state,
                name.trim().strip_suffix(".show").unwrap_or(name.trim()),
            )?,
        };
        let store = ActiveShowRepository::open(&staged).map_err(ApiError::store)?;
        let copy_id = light_core::ShowId::new();
        store
            .set_identity(copy_id, &catalog_name, None)
            .map_err(ApiError::store)?;
        store
            .checkpoint_for_replacement()
            .map_err(ApiError::store)?;
        drop(store);
        if let Some(replaced) = &replaced {
            if let Some(existing) = replaced {
                backup_show(state, existing)?;
            }
            // The validated copy lands in one rename, so a failure leaves the old show intact.
            fs::rename(&staged, &destination).map_err(ApiError::io)?;
        } else {
            let bytes = fs::read(&staged).map_err(ApiError::io)?;
            write_new(&destination, &bytes)?;
        }
        let catalogued = replaced.as_ref().is_some_and(Option::is_some);
        let entry = match state.installation.upsert_show(
            &catalog_name,
            &destination.display().to_string(),
            catalogued,
        ) {
            Ok(entry) => entry,
            Err(error) => {
                if replaced.is_none() {
                    let _ = fs::remove_file(&destination);
                }
                return Err(ApiError::store(error));
            }
        };
        let finish = (|| {
            ActiveShowRepository::open(&destination)
                .map_err(ApiError::store)?
                .set_identity(entry.id, &entry.name, None)
                .map_err(ApiError::store)?;
            state
                .installation
                .set_show_base(entry.id, is_base_show)
                .map_err(ApiError::store)
        })();
        match finish {
            Ok(entry) => {
                emit(state, "show_uploaded", serde_json::json!({"show":entry}));
                Ok(entry)
            }
            Err(error) => {
                if replaced.is_none() {
                    let _ = state.installation.remove_show(entry.id);
                    let _ = fs::remove_file(&destination);
                }
                Err(error)
            }
        }
    })();
    let _ = fs::remove_file(&staged);
    prepared
}

/// Checks that an existing destination may be replaced and returns its catalog entry, if any.
///
/// The open show is never replaced: it already saves itself, and its file is in use.
fn replaceable_show(state: &AppState, destination: &FsPath) -> Result<Option<ShowEntry>, ApiError> {
    if !destination.is_file() {
        return Err(ApiError::conflict(
            "a folder with that name already exists; choose another show name",
        ));
    }
    let target = fs::canonicalize(destination).map_err(ApiError::io)?;
    let same_file = |path: &str| fs::canonicalize(path).is_ok_and(|path| path == target);
    if state
        .active_show
        .current()
        .is_some_and(|show| same_file(&show.path))
    {
        return Err(ApiError::conflict(
            "that file is the open show and already saves automatically; choose another name",
        ));
    }
    Ok(state
        .installation
        .show_library()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|show| same_file(&show.path)))
}

pub(super) fn export_mvr_file(
    state: &AppState,
    show_id: Option<Uuid>,
    data_base64: Option<String>,
    name: String,
    root_id: String,
    path: String,
) -> Result<(String, String, Option<MvrExportSummary>), ApiError> {
    let directory = super::file_manager::writable_show_folder(state, &root_id, &path)?;
    let leaf = file_name(&name, "mvr")?;
    let (bytes, summary) = match (show_id, data_base64) {
        (Some(id), None) => {
            let (_, document, summary) = build_mvr_export(state, id)?;
            let bytes = light_mvr::write(&document)
                .map_err(|error| ApiError::internal(error.to_string()))?;
            (bytes, Some(summary))
        }
        (None, Some(encoded)) => {
            let bytes = STANDARD
                .decode(encoded)
                .map_err(|_| ApiError::bad_request("data_base64 must contain an MVR archive"))?;
            if !bytes.starts_with(b"PK") {
                return Err(ApiError::bad_request(
                    "data_base64 must contain an MVR archive",
                ));
            }
            (bytes, None)
        }
        _ => {
            return Err(ApiError::bad_request(
                "provide exactly one show_id or data_base64",
            ));
        }
    };
    write_new(&directory.join(&leaf), &bytes)?;
    let relative = if path.is_empty() {
        leaf
    } else {
        format!("{}/{}", path.trim_end_matches('/'), leaf)
    };
    Ok((root_id, relative, summary))
}
