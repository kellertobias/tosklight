//! Save portable copies and MVR exports to an explicitly selected confined folder.
use super::*;
use std::{fs, io::Write, path::Path as FsPath};

fn file_name(name: &str, extension: &str) -> Result<String, ApiError> {
    let name = name.trim();
    let stem = name.strip_suffix(&format!(".{extension}")).unwrap_or(name);
    validate_show_name(stem)?;
    if matches!(stem, "." | "..") || stem.chars().any(char::is_control) {
        return Err(ApiError::bad_request("name must be a plain file name"));
    }
    Ok(format!("{stem}.{extension}"))
}

fn write_new(path: &FsPath, bytes: &[u8]) -> Result<(), ApiError> {
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
) -> Result<ShowEntry, ApiError> {
    let directory = super::file_manager::writable_show_folder(state, &root_id, &path)?;
    let leaf = file_name(&name, "show")?;
    let destination = directory.join(leaf);
    if destination.exists() {
        return Err(ApiError::conflict("a file with that name already exists"));
    }
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
        // Catalog names are unique even when copies are stored in different folders.
        let catalog_name = available_show_name(
            state,
            name.trim().strip_suffix(".show").unwrap_or(name.trim()),
        )?;
        let store = ActiveShowRepository::open(&staged).map_err(ApiError::store)?;
        let copy_id = light_core::ShowId::new();
        store
            .set_identity(copy_id, &catalog_name, None)
            .map_err(ApiError::store)?;
        store
            .checkpoint_for_replacement()
            .map_err(ApiError::store)?;
        drop(store);
        let bytes = fs::read(&staged).map_err(ApiError::io)?;
        write_new(&destination, &bytes)?;
        let entry = match state.installation.upsert_show(
            &catalog_name,
            &destination.display().to_string(),
            false,
        ) {
            Ok(entry) => entry,
            Err(error) => {
                let _ = fs::remove_file(&destination);
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
                let _ = state.installation.remove_show(entry.id);
                let _ = fs::remove_file(&destination);
                Err(error)
            }
        }
    })();
    let _ = fs::remove_file(&staged);
    prepared
}

pub(super) fn export_mvr_file(
    state: &AppState,
    show_id: Option<Uuid>,
    data_base64: Option<String>,
    name: String,
    root_id: String,
    path: String,
) -> Result<(String, String), ApiError> {
    let directory = super::file_manager::writable_show_folder(state, &root_id, &path)?;
    let leaf = file_name(&name, "mvr")?;
    let bytes = match (show_id, data_base64) {
        (Some(id), None) => {
            let (_, document, _) = build_mvr_export(state, id)?;
            light_mvr::write(&document).map_err(|error| ApiError::internal(error.to_string()))?
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
            bytes
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
    Ok((root_id, relative))
}
