use super::{ApiError, AppState, Session, authenticate, emit, persist_programmer};
use crate::file_manager_support::{self as support, ConflictChoice, TransferOutcome};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use light_core::SessionId;
use parking_lot::Mutex as SyncMutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Component, Path as FsPath, PathBuf},
    sync::{Arc, OnceLock, Weak},
    time::{Duration, Instant, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt},
    sync::{Mutex as AsyncMutex, OwnedMutexGuard},
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;
const FILE_INPUT_CONTEXT_TTL: Duration = Duration::from_secs(120);
const FILE_STREAM_TICKET_TTL: Duration = Duration::from_secs(8 * 60 * 60);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ConfiguredRoot {
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Serialize)]
struct RootInfo {
    id: String,
    label: String,
    icon: String,
    removable: bool,
    writable: bool,
    capabilities: FileSystemCapabilities,
}

#[derive(Clone, Copy, Serialize)]
struct FileSystemCapabilities {
    created_time: bool,
    hidden_attributes: bool,
    native_notes: bool,
    trash: bool,
    range_streaming: bool,
    thumbnails: bool,
}

#[derive(Clone, Serialize)]
struct EntryInfo {
    name: String,
    path: String,
    kind: &'static str,
    size: u64,
    modified_millis: Option<u128>,
    created_millis: Option<u128>,
    hidden: bool,
    writable: bool,
    mime: &'static str,
    note_supported: bool,
    trash_supported: bool,
}

#[derive(Default, Deserialize)]
struct DirectoryQuery {
    #[serde(default)]
    path: String,
    #[serde(default)]
    hidden: bool,
}

#[derive(Default, Deserialize)]
struct ContentQuery {
    #[serde(default)]
    path: String,
    ticket: Option<String>,
}

#[derive(Deserialize)]
struct StreamTicketRequest {
    path: String,
}

#[derive(Serialize)]
struct StreamTicketResponse {
    ticket: String,
    expires_in_millis: u128,
}

#[derive(Deserialize, Serialize)]
struct StreamTicketClaims {
    session_id: SessionId,
    root_id: String,
    path: String,
    expires_at_millis: u128,
    signature: String,
}

#[derive(Serialize)]
struct DirectoryListing {
    root_id: String,
    path: String,
    entries: Vec<EntryInfo>,
}

#[derive(Serialize)]
struct MetadataInfo {
    root_id: String,
    #[serde(flatten)]
    entry: EntryInfo,
    capabilities: FileSystemCapabilities,
}

#[derive(Serialize)]
struct NativeNote {
    root_id: String,
    path: String,
    supported: bool,
    note: Option<String>,
}

#[derive(Deserialize)]
struct SaveNativeNote {
    path: String,
    note: String,
}

#[derive(Default, Deserialize)]
struct ThumbnailQuery {
    path: String,
    #[serde(default = "default_thumbnail_size")]
    max_size: u32,
}

fn default_thumbnail_size() -> u32 {
    256
}

#[derive(Debug, Serialize)]
struct TextDocument {
    root_id: String,
    path: String,
    text: String,
    revision: String,
    read_only: bool,
}

#[derive(Clone, Deserialize)]
struct SaveText {
    path: String,
    text: String,
    revision: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum FileOperationKind {
    CreateFile,
    CreateFolder,
    Rename,
    Copy,
    Move,
    Trash,
    Delete,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RequestedConflict {
    Replace,
    KeepBoth,
    Skip,
}

#[derive(Deserialize)]
struct FileOperation {
    operation: FileOperationKind,
    sources: Vec<String>,
    destination: Option<String>,
    destination_root_id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    replace: bool,
    conflict: Option<RequestedConflict>,
    #[serde(default)]
    apply_to_all: bool,
}

#[derive(Serialize)]
struct FileOperationResult {
    paths: Vec<String>,
    complete: bool,
    items: Vec<FileOperationItem>,
}

#[derive(Serialize)]
struct FileOperationItem {
    source_root_id: String,
    source: String,
    destination_root_id: Option<String>,
    destination: Option<String>,
    status: &'static str,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FileInputAction {
    Rename,
    Copy,
    Move,
    Delete,
}

#[derive(Clone)]
pub(crate) struct FileInputContext {
    pub(crate) instance_id: String,
    pub(crate) action: FileInputAction,
    pub(crate) session_id: SessionId,
    pub(crate) desk_id: Uuid,
    pub(crate) expires_at: Instant,
}

#[derive(Serialize)]
struct FileInputContextResponse {
    instance_id: String,
    action: FileInputAction,
    session_id: SessionId,
    desk_id: Uuid,
    expires_in_millis: u128,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileInputOrigin {
    Pending,
    Toolbar,
}

#[derive(Deserialize)]
struct ClaimFileInput {
    instance_id: String,
    action: FileInputAction,
    origin: FileInputOrigin,
}

#[derive(Default, Deserialize)]
struct FileInputQuery {
    instance_id: Option<String>,
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/files/roots", get(roots))
        .route(
            "/api/v1/files/input-context",
            get(input_context)
                .post(claim_input_context)
                .delete(release_input_context),
        )
        .route("/api/v1/files/{root_id}/entries", get(entries))
        .route("/api/v1/files/{root_id}/metadata", get(metadata))
        .route("/api/v1/files/{root_id}/content", get(content))
        .route("/api/v1/files/{root_id}/stream-ticket", post(stream_ticket))
        .route("/api/v1/files/{root_id}/thumbnail", get(thumbnail))
        .route(
            "/api/v1/files/{root_id}/notes",
            get(read_note).put(save_note),
        )
        .route(
            "/api/v1/files/{root_id}/text",
            get(read_text).put(save_text),
        )
        .route("/api/v1/files/{root_id}/operations", post(operate))
}

fn configured_roots(state: &AppState) -> Vec<(ConfiguredRoot, bool)> {
    configured_roots_from(
        state.configuration.read().file_manager_roots.clone(),
        state.data_dir.join("shows"),
        support::discover_removable_paths(),
    )
}

fn configured_roots_from(
    configured: Vec<ConfiguredRoot>,
    default_shows_path: PathBuf,
    removable_paths: Vec<PathBuf>,
) -> Vec<(ConfiguredRoot, bool)> {
    let mut roots: Vec<_> = if configured.is_empty() {
        vec![(
            ConfiguredRoot {
                id: "shows".into(),
                label: "Shows".into(),
                path: default_shows_path,
                icon: Some("shows".into()),
            },
            false,
        )]
    } else {
        configured.into_iter().map(|root| (root, false)).collect()
    };
    let configured_paths = roots
        .iter()
        .filter_map(|(root, _)| fs::canonicalize(&root.path).ok())
        .collect::<HashSet<_>>();
    let mut ids = roots
        .iter()
        .map(|(root, _)| root.id.clone())
        .collect::<HashSet<_>>();
    roots.extend(
        removable_roots(removable_paths)
            .into_iter()
            .filter(|root| {
                !configured_paths
                    .contains(&fs::canonicalize(&root.path).unwrap_or_else(|_| root.path.clone()))
                    && ids.insert(root.id.clone())
            })
            .map(|root| (root, true)),
    );
    roots
}

fn removable_roots(paths: Vec<PathBuf>) -> Vec<ConfiguredRoot> {
    paths
        .into_iter()
        .map(|path| {
            let label = path
                .file_name()
                .unwrap_or_else(|| path.as_os_str())
                .to_string_lossy()
                .into_owned();
            ConfiguredRoot {
                id: format!(
                    "removable-{}",
                    URL_SAFE_NO_PAD.encode(path.to_string_lossy().as_bytes())
                ),
                label,
                path,
                icon: Some("drive".into()),
            }
        })
        .collect()
}

fn root(state: &AppState, id: &str) -> Result<(ConfiguredRoot, bool), ApiError> {
    configured_roots(state)
        .into_iter()
        .find(|(root, _)| root.id == id)
        .ok_or_else(|| ApiError::not_found("file root"))
}

fn confined(root: &FsPath, relative: &str, allow_missing_leaf: bool) -> Result<PathBuf, ApiError> {
    let relative_path = FsPath::new(relative);
    if relative_path.is_absolute()
        || relative_path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ApiError::bad_request(
            "path must be root-relative and may not traverse parents",
        ));
    }
    let canonical_root =
        fs::canonicalize(root).map_err(|_| ApiError::not_found("file root is unavailable"))?;
    let joined = canonical_root.join(relative_path);
    let checked = if relative_path.as_os_str().is_empty() {
        canonical_root.clone()
    } else if allow_missing_leaf && !joined.exists() {
        let parent = joined
            .parent()
            .ok_or_else(|| ApiError::bad_request("invalid path"))?;
        fs::canonicalize(parent)
            .map_err(|_| ApiError::not_found("parent directory"))?
            .join(
                joined
                    .file_name()
                    .ok_or_else(|| ApiError::bad_request("invalid name"))?,
            )
    } else {
        fs::canonicalize(&joined).map_err(|_| ApiError::not_found("file"))?
    };
    if checked != canonical_root && !checked.starts_with(&canonical_root) {
        return Err(ApiError::bad_request("path escapes the configured root"));
    }
    Ok(checked)
}

fn directory_entry_info(
    canonical_root: &FsPath,
    item: fs::DirEntry,
    include_hidden: bool,
) -> Result<Option<EntryInfo>, ApiError> {
    let name = item.file_name().to_string_lossy().into_owned();
    let path = match fs::canonicalize(item.path()) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ApiError::io(error)),
    };
    if path != canonical_root && !path.starts_with(canonical_root) {
        return Err(ApiError::bad_request("path escapes the configured root"));
    }
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ApiError::io(error)),
    };
    let hidden = support::is_hidden(&item.file_name(), &metadata);
    if hidden && !include_hidden {
        return Ok(None);
    }
    Ok(Some(EntryInfo {
        name,
        path: relative(canonical_root, &path),
        kind: if metadata.is_dir() { "folder" } else { "file" },
        size: metadata.len(),
        modified_millis: millis(metadata.modified()),
        created_millis: millis(metadata.created()),
        hidden,
        writable: !metadata.permissions().readonly(),
        mime: mime_for(&path),
        note_supported: support::native_notes_supported(&path),
        trash_supported: support::trash_supported(),
    }))
}

fn relative(root: &FsPath, path: &FsPath) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn filesystem_capabilities(path: &FsPath) -> FileSystemCapabilities {
    let native = support::capabilities(path);
    FileSystemCapabilities {
        created_time: native.created_time,
        hidden_attributes: native.hidden_attributes,
        native_notes: native.native_notes,
        trash: native.trash,
        range_streaming: true,
        thumbnails: true,
    }
}

fn io_api_error(error: std::io::Error) -> ApiError {
    match error.kind() {
        std::io::ErrorKind::AlreadyExists => ApiError::conflict(error.to_string()),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
            ApiError::bad_request(error.to_string())
        }
        std::io::ErrorKind::NotFound => ApiError::not_found(error.to_string()),
        std::io::ErrorKind::PermissionDenied => ApiError::forbidden(error.to_string()),
        std::io::ErrorKind::Unsupported => ApiError::unavailable(error.to_string()),
        _ => ApiError::io(error),
    }
}

fn millis(value: std::io::Result<std::time::SystemTime>) -> Option<u128> {
    value
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|v| v.as_millis())
}
fn text_revision(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

type FileLock = AsyncMutex<()>;
type FileLockMap = HashMap<PathBuf, Weak<FileLock>>;
static TEXT_FILE_LOCKS: OnceLock<SyncMutex<FileLockMap>> = OnceLock::new();
static FILE_MUTATION_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

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

fn read_text_document(
    root_id: String,
    root: &FsPath,
    relative_path: String,
) -> Result<TextDocument, ApiError> {
    let path = confined(root, &relative_path, false)?;
    let metadata = fs::metadata(&path).map_err(ApiError::io)?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    if metadata.len() > MAX_TEXT_BYTES {
        return Err(ApiError::bad_request("text file exceeds the 4 MiB limit"));
    }
    let bytes = fs::read(&path).map_err(ApiError::io)?;
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        return Err(ApiError::bad_request("text file exceeds the 4 MiB limit"));
    }
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
    if metadata.len() > MAX_TEXT_BYTES {
        return Err(ApiError::conflict(
            "file changed and now exceeds the 4 MiB limit",
        ));
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ApiError::io(error)),
    };
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        return Err(ApiError::conflict(
            "file changed and now exceeds the 4 MiB limit",
        ));
    }
    Ok(Some((text_revision(&bytes), metadata.permissions())))
}

async fn save_text_document(
    root_id: String,
    root: &FsPath,
    input: SaveText,
) -> Result<TextDocument, ApiError> {
    if input.text.len() as u64 > MAX_TEXT_BYTES {
        return Err(ApiError::bad_request("text file exceeds the 4 MiB limit"));
    }
    let path = confined(root, &input.path, true)?;
    let _mutation_guard = FILE_MUTATION_LOCK.lock().await;
    let _guard = lock_text_file(&path).await;

    let (expected_current, permissions) =
        if let Some((revision, permissions)) = writable_text_state(&path)? {
            if input.revision.as_deref() != Some(revision.as_str()) {
                return Err(ApiError::conflict("file changed since it was opened"));
            }
            (Some(revision), Some(permissions))
        } else {
            if input.revision.is_some() {
                return Err(ApiError::conflict("file was removed since it was opened"));
            }
            (None, None)
        };

    // Prepare the replacement, then compare again immediately before the
    // atomic rename. The per-path lock makes this a real CAS for every desk
    // client, while the second hash also catches an external writer that
    // changed, removed, or created the file during preparation.
    let temporary = temporary_path(&path)?;
    let prepare = (|| -> Result<(), ApiError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(ApiError::io)?;
        if let Some(permissions) = permissions.clone() {
            file.set_permissions(permissions).map_err(ApiError::io)?;
        }
        file.write_all(input.text.as_bytes())
            .map_err(ApiError::io)?;
        file.sync_all().map_err(ApiError::io)?;
        Ok(())
    })();
    if let Err(error) = prepare {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    let current_now = match writable_text_state(&path) {
        Ok(state) => state.map(|(revision, _)| revision),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    };
    if current_now != expected_current {
        let _ = fs::remove_file(&temporary);
        return Err(ApiError::conflict("file changed while it was being saved"));
    }
    if let Err(error) = fs::rename(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(ApiError::io(error));
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

async fn roots(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RootInfo>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(
        configured_roots(&state)
            .into_iter()
            .map(|(root, removable)| RootInfo {
                writable: fs::metadata(&root.path)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false),
                capabilities: filesystem_capabilities(&root.path),
                id: root.id,
                label: root.label,
                icon: root.icon.unwrap_or_else(|| "folder".into()),
                removable,
            })
            .collect(),
    ))
}

async fn entries(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<DirectoryQuery>,
    headers: HeaderMap,
) -> Result<Json<DirectoryListing>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    let canonical_root = fs::canonicalize(&root.path)
        .map_err(|_| ApiError::not_found("file root is unavailable"))?;
    let directory = confined(&root.path, &query.path, false)?;
    if !directory.is_dir() {
        return Err(ApiError::bad_request("path is not a directory"));
    }
    let mut result = Vec::new();
    for item in fs::read_dir(&directory).map_err(ApiError::io)? {
        let item = match item {
            Ok(item) => item,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(ApiError::io(error)),
        };
        if let Some(entry) = directory_entry_info(&canonical_root, item, query.hidden)? {
            result.push(entry);
        }
    }
    result.sort_by(|a, b| {
        (a.kind != "folder", a.name.to_lowercase())
            .cmp(&(b.kind != "folder", b.name.to_lowercase()))
    });
    Ok(Json(DirectoryListing {
        root_id,
        path: relative(&canonical_root, &directory),
        entries: result,
    }))
}

async fn metadata(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<DirectoryQuery>,
    headers: HeaderMap,
) -> Result<Json<MetadataInfo>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    let canonical_root = fs::canonicalize(&root.path)
        .map_err(|_| ApiError::not_found("file root is unavailable"))?;
    let path = confined(&root.path, &query.path, false)?;
    let value = fs::metadata(&path).map_err(ApiError::io)?;
    let name = path
        .file_name()
        .unwrap_or_else(|| path.as_os_str())
        .to_string_lossy()
        .into_owned();
    let entry = EntryInfo {
        name,
        path: relative(&canonical_root, &path),
        kind: if value.is_dir() { "folder" } else { "file" },
        size: value.len(),
        modified_millis: millis(value.modified()),
        created_millis: millis(value.created()),
        hidden: support::is_hidden(path.file_name().unwrap_or_else(|| path.as_os_str()), &value),
        writable: !value.permissions().readonly(),
        mime: mime_for(&path),
        note_supported: support::native_notes_supported(&path),
        trash_supported: support::trash_supported(),
    };
    Ok(Json(MetadataInfo {
        root_id,
        entry,
        capabilities: filesystem_capabilities(&root.path),
    }))
}

async fn read_note(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<DirectoryQuery>,
    headers: HeaderMap,
) -> Result<Json<NativeNote>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &query.path, false)?;
    let supported = support::native_notes_supported(&path);
    let note = if supported {
        support::read_native_note(&path).map_err(io_api_error)?
    } else {
        None
    };
    Ok(Json(NativeNote {
        root_id,
        path: query.path,
        supported,
        note,
    }))
}

async fn save_note(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<SaveNativeNote>,
) -> Result<Json<NativeNote>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    if input.note.len() > 64 * 1024 {
        return Err(ApiError::bad_request("native notes are limited to 64 KiB"));
    }
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &input.path, false)?;
    if !support::native_notes_supported(&path) {
        return Err(ApiError::unavailable(
            "native notes are unavailable on this filesystem",
        ));
    }
    support::write_native_note(&path, &input.note).map_err(io_api_error)?;
    Ok(Json(NativeNote {
        root_id,
        path: input.path,
        supported: true,
        note: (!input.note.is_empty()).then_some(input.note),
    }))
}

async fn read_text(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<DirectoryQuery>,
    headers: HeaderMap,
) -> Result<Json<TextDocument>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    read_text_document(root_id, &root.path, query.path).map(Json)
}

async fn save_text(
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

fn stream_ticket_signature(
    token: &str,
    session_id: SessionId,
    root_id: &str,
    path: &str,
    expires_at_millis: u128,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"tosklight-file-stream-v1\0");
    digest.update(session_id.0.as_bytes());
    digest.update(root_id.as_bytes());
    digest.update([0]);
    digest.update(path.as_bytes());
    digest.update(expires_at_millis.to_le_bytes());
    // Keeping the secret at the end avoids the length-extension weakness of a
    // naive secret-prefix SHA-256 construction. The ticket is also bound to an
    // active server session and a short expiry.
    digest.update(token.as_bytes());
    URL_SAFE_NO_PAD.encode(digest.finalize())
}

fn now_millis() -> u128 {
    UNIX_EPOCH.elapsed().unwrap_or_default().as_millis()
}

fn validate_stream_ticket(
    state: &AppState,
    encoded: &str,
    root_id: &str,
    path: &str,
) -> Result<Session, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| ApiError::unauthorized("invalid file stream ticket"))?;
    let claims: StreamTicketClaims = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::unauthorized("invalid file stream ticket"))?;
    if claims.root_id != root_id || claims.path != path || claims.expires_at_millis < now_millis() {
        return Err(ApiError::unauthorized(
            "expired or mismatched file stream ticket",
        ));
    }
    let session = state
        .sessions
        .read()
        .get(&claims.session_id)
        .filter(|session| session.connected)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("file stream session is no longer active"))?;
    let expected = stream_ticket_signature(
        &session.token,
        session.id,
        root_id,
        path,
        claims.expires_at_millis,
    );
    if claims.signature != expected {
        return Err(ApiError::unauthorized("invalid file stream ticket"));
    }
    Ok(session)
}

async fn stream_ticket(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<StreamTicketRequest>,
) -> Result<Json<StreamTicketResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &input.path, false)?;
    if !path.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let expires_at_millis = now_millis() + FILE_STREAM_TICKET_TTL.as_millis();
    let signature = stream_ticket_signature(
        &session.token,
        session.id,
        &root_id,
        &input.path,
        expires_at_millis,
    );
    let claims = StreamTicketClaims {
        session_id: session.id,
        root_id: root_id.clone(),
        path: input.path,
        expires_at_millis,
        signature,
    };
    let ticket = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&claims).map_err(|error| ApiError::internal(error.to_string()))?,
    );
    Ok(Json(StreamTicketResponse {
        ticket,
        expires_in_millis: FILE_STREAM_TICKET_TTL.as_millis(),
    }))
}

async fn content(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<ContentQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = if let Some(ticket) = query.ticket.as_deref() {
        validate_stream_ticket(&state, ticket, &root_id, &query.path)?
    } else {
        authenticate(&state, &headers)?
    };
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &query.path, false)?;
    let mut file = tokio::fs::File::open(&path).await.map_err(ApiError::io)?;
    let metadata = file.metadata().await.map_err(ApiError::io)?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let total = metadata.len();
    let (start, end, status) = parse_range(
        headers.get(header::RANGE).and_then(|v| v.to_str().ok()),
        total,
    )?;
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(ApiError::io)?;
    let length = end.saturating_sub(start);
    let mime = mime_for(&path);
    let stream = ReaderStream::new(file.take(length));
    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length.to_string());
    if status == StatusCode::PARTIAL_CONTENT {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{}/{total}", end.saturating_sub(1)),
        );
    }
    response
        .body(Body::from_stream(stream))
        .map_err(|_| ApiError::internal("could not stream file"))
}

fn parse_range(range: Option<&str>, total: u64) -> Result<(u64, u64, StatusCode), ApiError> {
    let Some(value) = range else {
        return Ok((0, total, StatusCode::OK));
    };
    let value = value
        .strip_prefix("bytes=")
        .ok_or_else(|| ApiError::bad_request("invalid range"))?;
    if value.contains(',') {
        return Err(ApiError::bad_request(
            "multiple byte ranges are not supported",
        ));
    }
    let (start, end) = value
        .split_once('-')
        .ok_or_else(|| ApiError::bad_request("invalid range"))?;
    if total == 0 {
        return Err(range_error("range is outside the empty file"));
    }
    let (start, end) = if start.is_empty() {
        let suffix = end
            .parse::<u64>()
            .map_err(|_| ApiError::bad_request("invalid suffix range"))?;
        if suffix == 0 {
            return Err(range_error("suffix range must be greater than zero"));
        }
        (total.saturating_sub(suffix.min(total)), total)
    } else {
        let start = start
            .parse::<u64>()
            .map_err(|_| ApiError::bad_request("invalid range start"))?;
        let end = if end.is_empty() {
            total
        } else {
            end.parse::<u64>()
                .map_err(|_| ApiError::bad_request("invalid range end"))?
                .saturating_add(1)
                .min(total)
        };
        (start, end)
    };
    if start >= end || start >= total {
        return Err(range_error("range is outside the file"));
    }
    Ok((start, end, StatusCode::PARTIAL_CONTENT))
}

fn range_error(message: impl Into<String>) -> ApiError {
    ApiError {
        status: StatusCode::RANGE_NOT_SATISFIABLE,
        message: message.into(),
    }
}

async fn thumbnail(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<ThumbnailQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &query.path, false)?;
    let metadata = fs::metadata(&path).map_err(ApiError::io)?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let size = query.max_size.clamp(32, 1_024);
    let key = thumbnail_cache_key(&root_id, &query.path, &metadata, size);
    let cache = state.data_dir.join("cache/file-thumbnails");
    fs::create_dir_all(&cache).map_err(ApiError::io)?;
    let cached = cache.join(format!("{key}.png"));
    let bytes = if cached.is_file() {
        fs::read(&cached).map_err(ApiError::io)?
    } else {
        let source = path.clone();
        let bytes = tokio::task::spawn_blocking(move || support::thumbnail_png(&source, size))
            .await
            .map_err(|error| ApiError::internal(error.to_string()))?
            .map_err(io_api_error)?;
        let temporary = cache.join(format!(".{key}.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, &bytes).map_err(ApiError::io)?;
        match fs::rename(&temporary, &cached) {
            Ok(()) => {}
            Err(_) if cached.exists() => {
                let _ = fs::remove_file(&temporary);
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(ApiError::io(error));
            }
        }
        bytes
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header(header::ETAG, format!("\"{key}\""))
        .header(header::CONTENT_LENGTH, bytes.len().to_string())
        .body(Body::from(bytes))
        .map_err(|_| ApiError::internal("could not return thumbnail"))
}

fn thumbnail_cache_key(root_id: &str, path: &str, metadata: &fs::Metadata, size: u32) -> String {
    let mut digest = Sha256::new();
    digest.update(root_id.as_bytes());
    digest.update([0]);
    digest.update(path.as_bytes());
    digest.update(metadata.len().to_le_bytes());
    digest.update(
        millis(metadata.modified())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    digest.update(size.to_le_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn mime_for(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "txt" | "md" | "csv" | "log" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn operate(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<FileOperation>,
) -> Result<Json<FileOperationResult>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let _mutation_guard = FILE_MUTATION_LOCK.lock().await;
    if input.sources.len() > 1_000 {
        return Err(ApiError::bad_request(
            "a file operation is limited to 1000 sources",
        ));
    }
    if input.sources.iter().collect::<HashSet<_>>().len() != input.sources.len() {
        return Err(ApiError::bad_request(
            "a file operation may not contain duplicate sources",
        ));
    }
    let _apply_to_all = input.apply_to_all;
    let (source_root, _) = root(&state, &root_id)?;
    let canonical_source_root = fs::canonicalize(&source_root.path)
        .map_err(|_| ApiError::not_found("file root is unavailable"))?;
    let destination_root_id = input
        .destination_root_id
        .clone()
        .unwrap_or_else(|| root_id.clone());
    let (destination_root, _) = root(&state, &destination_root_id)?;
    let canonical_destination_root = fs::canonicalize(&destination_root.path)
        .map_err(|_| ApiError::not_found("destination file root is unavailable"))?;
    let conflict = input
        .conflict
        .map(|value| match value {
            RequestedConflict::Replace => ConflictChoice::Replace,
            RequestedConflict::KeepBoth => ConflictChoice::KeepBoth,
            RequestedConflict::Skip => ConflictChoice::Skip,
        })
        .unwrap_or(if input.replace {
            ConflictChoice::Replace
        } else {
            ConflictChoice::Error
        });
    let mut paths = Vec::new();
    let mut items = Vec::new();
    match input.operation {
        FileOperationKind::CreateFile | FileOperationKind::CreateFolder => {
            if destination_root_id != root_id {
                return Err(ApiError::bad_request(
                    "create operations use the root in the request path",
                ));
            }
            let directory = input
                .destination
                .as_deref()
                .map(|value| confined(&source_root.path, value, false))
                .transpose()?
                .unwrap_or_else(|| canonical_source_root.clone());
            if !directory.is_dir() {
                return Err(ApiError::bad_request("destination is not a folder"));
            }
            let name = safe_name(input.name.as_deref())?;
            let path = confined(
                &source_root.path,
                &relative(&canonical_source_root, &directory.join(name)),
                true,
            )?;
            if path.exists() {
                return Err(ApiError::conflict("an item with that name already exists"));
            }
            if matches!(input.operation, FileOperationKind::CreateFolder) {
                fs::create_dir(&path).map_err(ApiError::io)?;
            } else {
                fs::write(&path, []).map_err(ApiError::io)?;
            }
            let created = relative(&canonical_source_root, &path);
            paths.push(created.clone());
            items.push(operation_item(
                &root_id,
                "",
                Some(&root_id),
                Some(created),
                "completed",
                None,
            ));
        }
        FileOperationKind::Rename => {
            if destination_root_id != root_id {
                return Err(ApiError::bad_request(
                    "rename cannot change file roots; use move",
                ));
            }
            let source = one_source(&source_root.path, &input.sources)?;
            reject_root_source(&canonical_source_root, &source)?;
            let name = safe_name(input.name.as_deref())?;
            let target = confined(
                &source_root.path,
                &relative(
                    &canonical_source_root,
                    &source.parent().unwrap_or(&canonical_source_root).join(name),
                ),
                true,
            )?;
            let outcome = support::copy_or_move(&source, &target, true, false, conflict)
                .map_err(io_api_error)?;
            let (target, status) = match outcome {
                TransferOutcome::Completed(path) => (path, "completed"),
                TransferOutcome::Skipped(path) => (path, "skipped"),
            };
            let target = relative(&canonical_source_root, &target);
            if status == "completed" {
                paths.push(target.clone());
            }
            items.push(operation_item(
                &root_id,
                &input.sources[0],
                Some(&root_id),
                Some(target),
                status,
                None,
            ));
        }
        FileOperationKind::Delete | FileOperationKind::Trash => {
            if destination_root_id != root_id {
                return Err(ApiError::bad_request(
                    "delete operations use the root in the request path",
                ));
            }
            if input.sources.is_empty() {
                return Err(ApiError::bad_request("at least one source is required"));
            }
            let mut resolved = Vec::new();
            for source in &input.sources {
                let path = confined(&source_root.path, source, false)?;
                reject_root_source(&canonical_source_root, &path)?;
                resolved.push((source.clone(), path));
            }
            for (source, path) in resolved {
                let result = if matches!(input.operation, FileOperationKind::Trash) {
                    support::trash_path(&path)
                } else {
                    support::remove_permanent(&path)
                };
                match result {
                    Ok(()) => items.push(operation_item(
                        &root_id,
                        &source,
                        None,
                        None,
                        "completed",
                        None,
                    )),
                    Err(error) => items.push(operation_item(
                        &root_id,
                        &source,
                        None,
                        None,
                        "failed",
                        Some(error.to_string()),
                    )),
                }
            }
        }
        FileOperationKind::Copy | FileOperationKind::Move => {
            if input.sources.is_empty() {
                return Err(ApiError::bad_request("at least one source is required"));
            }
            let destination = input
                .destination
                .as_deref()
                .map(|value| confined(&destination_root.path, value, false))
                .transpose()?
                .unwrap_or_else(|| canonical_destination_root.clone());
            if !destination.is_dir() {
                return Err(ApiError::bad_request("destination is not a folder"));
            }
            let mut resolved = Vec::new();
            for source_relative in &input.sources {
                let source = confined(&source_root.path, source_relative, false)?;
                reject_root_source(&canonical_source_root, &source)?;
                let name = source
                    .file_name()
                    .ok_or_else(|| ApiError::bad_request("invalid source"))?;
                let requested_target = confined(
                    &destination_root.path,
                    &relative(&canonical_destination_root, &destination.join(name)),
                    true,
                )?;
                resolved.push((source_relative.clone(), source, requested_target));
            }
            // With no decision, report every conflict before mutating anything.
            // This makes a subsequent Apply-to-All retry deterministic.
            if conflict == ConflictChoice::Error
                && resolved.iter().any(|(_, _, target)| target.exists())
            {
                return Err(ApiError::conflict(
                    "one or more destination names already exist",
                ));
            }
            let move_source = matches!(input.operation, FileOperationKind::Move);
            let cross_root = root_id != destination_root_id;
            for (source_relative, source, requested_target) in resolved {
                match support::copy_or_move(
                    &source,
                    &requested_target,
                    move_source,
                    cross_root,
                    conflict,
                ) {
                    Ok(TransferOutcome::Completed(target)) => {
                        let target = relative(&canonical_destination_root, &target);
                        paths.push(target.clone());
                        items.push(operation_item(
                            &root_id,
                            &source_relative,
                            Some(&destination_root_id),
                            Some(target),
                            "completed",
                            None,
                        ));
                    }
                    Ok(TransferOutcome::Skipped(target)) => {
                        let target = relative(&canonical_destination_root, &target);
                        items.push(operation_item(
                            &root_id,
                            &source_relative,
                            Some(&destination_root_id),
                            Some(target),
                            "skipped",
                            None,
                        ));
                    }
                    Err(error) => items.push(operation_item(
                        &root_id,
                        &source_relative,
                        Some(&destination_root_id),
                        Some(relative(&canonical_destination_root, &requested_target)),
                        "failed",
                        Some(error.to_string()),
                    )),
                }
            }
        }
    }
    let complete = items.iter().all(|item| item.status != "failed");
    emit(
        &state,
        "file_operation_completed",
        serde_json::json!({
            "operation": input.operation,
            "items": &items,
        }),
    );
    Ok(Json(FileOperationResult {
        paths,
        complete,
        items,
    }))
}

fn operation_item(
    source_root_id: &str,
    source: &str,
    destination_root_id: Option<&str>,
    destination: Option<String>,
    status: &'static str,
    error: Option<String>,
) -> FileOperationItem {
    FileOperationItem {
        source_root_id: source_root_id.into(),
        source: source.into(),
        destination_root_id: destination_root_id.map(str::to_owned),
        destination,
        status,
        error,
    }
}

fn safe_name(value: Option<&str>) -> Result<&str, ApiError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("name is required"))?;
    let upper = value
        .trim_end_matches(['.', ' '])
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    let reserved = matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if value == "."
        || value == ".."
        || value.contains(['/', '\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "name may not be a dot path or contain path separators or control characters",
        ));
    }
    if value.len() > 255 || value.ends_with(['.', ' ']) || reserved {
        return Err(ApiError::bad_request(
            "name is not portable across supported filesystems",
        ));
    }
    Ok(value)
}

fn one_source(root: &FsPath, sources: &[String]) -> Result<PathBuf, ApiError> {
    if sources.len() != 1 {
        return Err(ApiError::bad_request("exactly one source is required"));
    }
    confined(root, &sources[0], false)
}

fn reject_root_source(root: &FsPath, source: &FsPath) -> Result<(), ApiError> {
    if root == source {
        Err(ApiError::forbidden(
            "the configured root itself cannot be changed",
        ))
    } else {
        Ok(())
    }
}

fn context_response(context: &FileInputContext) -> FileInputContextResponse {
    FileInputContextResponse {
        instance_id: context.instance_id.clone(),
        action: context.action,
        session_id: context.session_id,
        desk_id: context.desk_id,
        expires_in_millis: context
            .expires_at
            .saturating_duration_since(Instant::now())
            .as_millis(),
    }
}

fn prune_input_contexts(state: &AppState) {
    let now = Instant::now();
    state
        .file_input_contexts
        .lock()
        .retain(|_, context| context.expires_at > now);
}

pub(crate) fn try_claim_input_context(
    state: &AppState,
    context: FileInputContext,
    prepare: impl FnOnce() -> Result<(), ApiError>,
) -> Result<(), ApiError> {
    let mut contexts = state.file_input_contexts.lock();
    contexts.retain(|_, current| current.expires_at > Instant::now());
    if let Some(existing) = contexts.get(&context.desk_id)
        && existing.instance_id != context.instance_id
    {
        return Err(ApiError::conflict(
            "another File Manager instance owns this session's file input context",
        ));
    }
    // `prepare` is synchronous and may persist the pending command-line
    // transition. Keeping the Desk context lock through that transition means
    // a losing concurrent pane can never consume the pending command before
    // discovering that another pane won the claim.
    prepare()?;
    contexts.insert(context.desk_id, context);
    Ok(())
}

async fn input_context(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<FileInputContextResponse>>, ApiError> {
    let session = authenticate(&state, &headers)?;
    prune_input_contexts(&state);
    Ok(Json(
        state
            .file_input_contexts
            .lock()
            .get(&session.desk.id)
            .map(context_response),
    ))
}

async fn claim_input_context(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ClaimFileInput>,
) -> Result<Json<FileInputContextResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let instance_id = input.instance_id.trim();
    if instance_id.is_empty()
        || instance_id.len() > 128
        || !instance_id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | ':' | '.'))
    {
        return Err(ApiError::bad_request("File Manager instance_id is invalid"));
    }
    let pending_origin = matches!(input.origin, FileInputOrigin::Pending);
    let context = FileInputContext {
        instance_id: instance_id.to_owned(),
        action: input.action,
        session_id: session.id,
        desk_id: session.desk.id,
        expires_at: Instant::now() + FILE_INPUT_CONTEXT_TTL,
    };
    try_claim_input_context(&state, context.clone(), || {
        if !pending_origin {
            return Ok(());
        }
        let command_line = state
            .programmers
            .get(session.id)
            .map(|programmer| programmer.command_line)
            .ok_or_else(|| ApiError::not_found("programmer"))?;
        if pending_file_action(&command_line) != Some(input.action) {
            return Err(ApiError::conflict(
                "the desk does not have the matching unowned file action",
            ));
        }
        state
            .programmers
            .set_command_line(session.id, String::new());
        if let Err(error) = persist_programmer(&state, &session) {
            state.programmers.set_command_line(session.id, command_line);
            let _ = persist_programmer(&state, &session);
            return Err(error);
        }
        Ok(())
    })?;
    if pending_origin {
        emit(
            &state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id}),
        );
    }
    emit(
        &state,
        "file_input_context_changed",
        serde_json::json!({"session_id":session.id,"desk_id":session.desk.id,"instance_id":context.instance_id,"action":context.action,"claimed":true}),
    );
    Ok(Json(context_response(&context)))
}

async fn release_input_context(
    State(state): State<AppState>,
    Query(query): Query<FileInputQuery>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let session = authenticate(&state, &headers)?;
    let released = {
        let mut contexts = state.file_input_contexts.lock();
        let matches = contexts.get(&session.desk.id).is_some_and(|context| {
            query
                .instance_id
                .as_deref()
                .is_none_or(|instance| instance == context.instance_id)
        });
        matches.then(|| contexts.remove(&session.desk.id)).flatten()
    };
    if let Some(context) = released {
        emit(
            &state,
            "file_input_context_changed",
            serde_json::json!({"session_id":session.id,"desk_id":session.desk.id,"instance_id":context.instance_id,"action":context.action,"claimed":false}),
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

fn pending_file_action(command_line: &str) -> Option<FileInputAction> {
    match command_line.trim().to_ascii_uppercase().as_str() {
        "SET" => Some(FileInputAction::Rename),
        "CPY" | "COPY" => Some(FileInputAction::Copy),
        "MOV" | "MOVE" => Some(FileInputAction::Move),
        "DEL" | "DELETE" => Some(FileInputAction::Delete),
        _ => None,
    }
}

pub(crate) fn route_osc_input(state: &AppState, session: &Session, action: &str) -> bool {
    prune_input_contexts(state);
    let context = {
        let mut contexts = state.file_input_contexts.lock();
        let Some(context) = contexts.get_mut(&session.desk.id) else {
            return false;
        };
        if context.desk_id != session.desk.id {
            return false;
        }
        context.expires_at = Instant::now() + FILE_INPUT_CONTEXT_TTL;
        if !matches!(action, "enter" | "escape" | "esc") {
            return true;
        }
        let context = context.clone();
        if matches!(action, "escape" | "esc") {
            contexts.remove(&session.desk.id);
        }
        context
    };
    emit(
        state,
        "file_input_action",
        serde_json::json!({
            "session_id":context.session_id,
            "source_session_id":session.id,
            "desk_id":session.desk.id,
            "instance_id":context.instance_id,
            "operation":context.action,
            "action":if action == "enter" { "enter" } else { "escape" },
            "source":"osc",
        }),
    );
    true
}

pub(crate) fn release_session_input(state: &AppState, session: &Session, reason: &str) {
    let released = {
        let mut contexts = state.file_input_contexts.lock();
        let owned = contexts
            .get(&session.desk.id)
            .is_some_and(|context| context.session_id == session.id);
        owned.then(|| contexts.remove(&session.desk.id)).flatten()
    };
    if let Some(context) = released {
        emit(
            state,
            "file_input_context_changed",
            serde_json::json!({"session_id":context.session_id,"desk_id":context.desk_id,"instance_id":context.instance_id,"action":context.action,"claimed":false,"reason":reason}),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("light-file-manager-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("folder")).unwrap();
        fs::write(root.join("folder/note.txt"), b"hello").unwrap();
        root
    }

    #[test]
    fn confinement_rejects_parent_and_symlink_escapes() {
        let root = temporary_root();
        assert_eq!(
            confined(&root, "", false).unwrap(),
            fs::canonicalize(&root).unwrap()
        );
        assert!(confined(&root, "folder/note.txt", false).is_ok());
        assert!(confined(&root, "../outside", false).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("escape")).unwrap();
            assert!(confined(&root, "escape", false).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn directory_listing_skips_an_entry_that_disappears_after_enumeration() {
        let root = temporary_root();
        let transient = root.join("temporary.show-wal");
        fs::write(&transient, b"transient").unwrap();
        let item = fs::read_dir(&root)
            .unwrap()
            .map(Result::unwrap)
            .find(|item| item.file_name() == "temporary.show-wal")
            .unwrap();
        fs::remove_file(&transient).unwrap();

        assert!(
            directory_entry_info(&fs::canonicalize(&root).unwrap(), item, true)
                .unwrap()
                .is_none()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ranges_are_inclusive_at_the_http_boundary() {
        assert_eq!(parse_range(None, 10).unwrap(), (0, 10, StatusCode::OK));
        assert_eq!(
            parse_range(Some("bytes=2-4"), 10).unwrap(),
            (2, 5, StatusCode::PARTIAL_CONTENT)
        );
        assert_eq!(
            parse_range(Some("bytes=7-"), 10).unwrap(),
            (7, 10, StatusCode::PARTIAL_CONTENT)
        );
        assert_eq!(
            parse_range(Some("bytes=-3"), 10).unwrap(),
            (7, 10, StatusCode::PARTIAL_CONTENT)
        );
        assert!(parse_range(Some("bytes=10-12"), 10).is_err());
        assert_eq!(
            parse_range(Some("bytes=10-12"), 10).unwrap_err().status,
            StatusCode::RANGE_NOT_SATISFIABLE
        );
        assert!(parse_range(Some("bytes=0-1,4-5"), 10).is_err());
    }

    #[test]
    fn portable_names_and_pending_file_keys_are_strict() {
        assert_eq!(safe_name(Some("Cue Notes.txt")).unwrap(), "Cue Notes.txt");
        for name in ["", ".", "..", "../escape", "CON", "name.", "bad\0name"] {
            assert!(
                safe_name(Some(name)).is_err(),
                "{name:?} should be rejected"
            );
        }
        assert_eq!(pending_file_action(" COPY "), Some(FileInputAction::Copy));
        assert_eq!(pending_file_action("MOVE"), Some(FileInputAction::Move));
        assert_eq!(pending_file_action("DELETE 2"), None);
    }

    #[test]
    fn removable_roots_are_runtime_only_and_disappear_from_the_next_discovery_snapshot() {
        let default = PathBuf::from("/desk/shows");
        let removable = PathBuf::from("/media/operator/TOUR_USB");
        let attached = configured_roots_from(Vec::new(), default.clone(), vec![removable.clone()]);
        assert!(
            attached
                .iter()
                .any(|(root, runtime)| *runtime && root.path == removable)
        );
        assert!(
            attached
                .iter()
                .any(|(root, runtime)| !runtime && root.id == "shows")
        );

        let detached = configured_roots_from(Vec::new(), default, Vec::new());
        assert_eq!(detached.len(), 1);
        assert_eq!(detached[0].0.id, "shows");
        assert!(!detached[0].1);
    }

    #[test]
    fn text_revisions_identify_content_even_when_size_and_timestamp_could_match() {
        let first = text_revision(b"ABCD");
        let second = text_revision(b"WXYZ");
        assert_ne!(first, second);
        assert_eq!(first, text_revision(b"ABCD"));
        assert!(first.starts_with("sha256:"));
    }

    #[tokio::test]
    async fn concurrent_saves_with_one_revision_have_exactly_one_winner() {
        let root = temporary_root();
        let original = read_text_document("test".into(), &root, "folder/note.txt".into()).unwrap();
        let first = save_text_document(
            "test".into(),
            &root,
            SaveText {
                path: "folder/note.txt".into(),
                text: "first writer".into(),
                revision: Some(original.revision.clone()),
            },
        );
        let second = save_text_document(
            "test".into(),
            &root,
            SaveText {
                path: "folder/note.txt".into(),
                text: "other writer".into(),
                revision: Some(original.revision),
            },
        );

        let (first, second) = tokio::join!(first, second);
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        let conflict = first.err().or_else(|| second.err()).unwrap();
        assert_eq!(conflict.status, StatusCode::CONFLICT);
        let stored = fs::read_to_string(root.join("folder/note.txt")).unwrap();
        assert!(stored == "first writer" || stored == "other writer");
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn save_as_is_atomic_and_refuses_to_replace_an_existing_file() {
        let root = temporary_root();
        let created = save_text_document(
            "test".into(),
            &root,
            SaveText {
                path: "folder/copy.txt".into(),
                text: "copy".into(),
                revision: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(created.revision, text_revision(b"copy"));
        let conflict = save_text_document(
            "test".into(),
            &root,
            SaveText {
                path: "folder/copy.txt".into(),
                text: "replace".into(),
                revision: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(conflict.status, StatusCode::CONFLICT);
        assert_eq!(
            fs::read_to_string(root.join("folder/copy.txt")).unwrap(),
            "copy"
        );
        assert!(fs::read_dir(root.join("folder")).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".light-tmp")
        }));
        fs::remove_dir_all(root).unwrap();
    }
}
