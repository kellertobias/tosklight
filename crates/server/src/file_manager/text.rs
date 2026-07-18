use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path as FsPath, PathBuf},
    sync::{Arc, OnceLock, Weak},
};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use parking_lot::Mutex as SyncMutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use uuid::Uuid;

use super::super::{ApiError, AppState, authenticate};
use super::FILE_MUTATION_LOCK;
use super::paths::{DirectoryQuery, confined, root};

const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub(super) struct TextDocument {
    pub(super) root_id: String,
    pub(super) path: String,
    pub(super) text: String,
    pub(super) revision: String,
    pub(super) read_only: bool,
}

#[derive(Clone, Deserialize)]
pub(super) struct SaveText {
    pub(super) path: String,
    pub(super) text: String,
    pub(super) revision: Option<String>,
}

type FileLock = AsyncMutex<()>;
type FileLockMap = HashMap<PathBuf, Weak<FileLock>>;
static TEXT_FILE_LOCKS: OnceLock<SyncMutex<FileLockMap>> = OnceLock::new();

pub(super) fn text_revision(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

async fn lock_text_file(path: &FsPath) -> OwnedMutexGuard<()> {
    let lock = {
        let mut locks = TEXT_FILE_LOCKS
            .get_or_init(|| SyncMutex::new(HashMap::new()))
            .lock();
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(path).and_then(Weak::upgrade) {
            lock
        } else {
            let lock = Arc::new(FileLock::new(()));
            locks.insert(path.to_path_buf(), Arc::downgrade(&lock));
            lock
        }
    };
    lock.lock_owned().await
}

pub(super) fn read_text_document(
    root_id: String,
    root: &FsPath,
    relative_path: String,
) -> Result<TextDocument, ApiError> {
    let path = confined(root, &relative_path, false)?;
    let metadata = fs::metadata(&path).map_err(ApiError::io)?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    reject_oversized_read(metadata.len())?;
    let bytes = fs::read(&path).map_err(ApiError::io)?;
    reject_oversized_read(bytes.len() as u64)?;
    let revision = text_revision(&bytes);
    let text = String::from_utf8(bytes)
        .map_err(|_| ApiError::bad_request("only UTF-8 plain text files are supported"))?;
    Ok(TextDocument {
        root_id,
        path: relative_path,
        text,
        revision,
        read_only: metadata.permissions().readonly(),
    })
}

fn reject_oversized_read(size: u64) -> Result<(), ApiError> {
    if size > MAX_TEXT_BYTES {
        return Err(ApiError::bad_request("text file exceeds the 4 MiB limit"));
    }
    Ok(())
}

fn temporary_path(path: &FsPath) -> Result<PathBuf, ApiError> {
    let parent = path
        .parent()
        .ok_or_else(|| ApiError::bad_request("invalid path"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ApiError::bad_request("invalid file name"))?;
    Ok(parent.join(format!(".{name}.{}.light-tmp", Uuid::new_v4())))
}

fn writable_text_state(path: &FsPath) -> Result<Option<(String, fs::Permissions)>, ApiError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ApiError::io(error)),
    };
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    if metadata.permissions().readonly() {
        return Err(ApiError::forbidden("file is read-only"));
    }
    reject_oversized_change(metadata.len())?;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ApiError::io(error)),
    };
    reject_oversized_change(bytes.len() as u64)?;
    Ok(Some((text_revision(&bytes), metadata.permissions())))
}

fn reject_oversized_change(size: u64) -> Result<(), ApiError> {
    if size > MAX_TEXT_BYTES {
        return Err(ApiError::conflict(
            "file changed and now exceeds the 4 MiB limit",
        ));
    }
    Ok(())
}

fn expected_text_state(
    path: &FsPath,
    requested_revision: Option<&str>,
) -> Result<(Option<String>, Option<fs::Permissions>), ApiError> {
    if let Some((revision, permissions)) = writable_text_state(path)? {
        if requested_revision != Some(revision.as_str()) {
            return Err(ApiError::conflict("file changed since it was opened"));
        }
        return Ok((Some(revision), Some(permissions)));
    }
    if requested_revision.is_some() {
        return Err(ApiError::conflict("file was removed since it was opened"));
    }
    Ok((None, None))
}

fn prepare_replacement(
    temporary: &FsPath,
    text: &str,
    permissions: Option<fs::Permissions>,
) -> Result<(), ApiError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .map_err(ApiError::io)?;
    if let Some(permissions) = permissions {
        file.set_permissions(permissions).map_err(ApiError::io)?;
    }
    file.write_all(text.as_bytes()).map_err(ApiError::io)?;
    file.sync_all().map_err(ApiError::io)
}

fn install_if_unchanged(
    path: &FsPath,
    temporary: &FsPath,
    expected: Option<String>,
) -> Result<(), ApiError> {
    let current = writable_text_state(path)?.map(|(revision, _)| revision);
    if current != expected {
        return Err(ApiError::conflict("file changed while it was being saved"));
    }
    fs::rename(temporary, path).map_err(ApiError::io)
}

pub(super) async fn save_text_document(
    root_id: String,
    root: &FsPath,
    input: SaveText,
) -> Result<TextDocument, ApiError> {
    reject_oversized_read(input.text.len() as u64)?;
    let path = confined(root, &input.path, true)?;
    let _mutation_guard = FILE_MUTATION_LOCK.lock().await;
    let _file_guard = lock_text_file(&path).await;
    let (expected, permissions) = expected_text_state(&path, input.revision.as_deref())?;
    let temporary = temporary_path(&path)?;
    let result = prepare_replacement(&temporary, &input.text, permissions)
        .and_then(|()| install_if_unchanged(&path, &temporary, expected));
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    let metadata = fs::metadata(&path).map_err(ApiError::io)?;
    Ok(TextDocument {
        root_id,
        path: input.path,
        revision: text_revision(input.text.as_bytes()),
        text: input.text,
        read_only: metadata.permissions().readonly(),
    })
}

pub(super) async fn read_text(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<DirectoryQuery>,
    headers: HeaderMap,
) -> Result<Json<TextDocument>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    read_text_document(root_id, &root.path, query.path).map(Json)
}

pub(super) async fn save_text(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<SaveText>,
) -> Result<Json<TextDocument>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    save_text_document(root_id, &root.path, input)
        .await
        .map(Json)
}
