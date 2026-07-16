use super::{ApiError, AppState};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path as FsPath, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;

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
}

#[derive(Serialize)]
struct EntryInfo {
    name: String,
    path: String,
    kind: &'static str,
    size: u64,
    modified_millis: Option<u128>,
    created_millis: Option<u128>,
    hidden: bool,
    writable: bool,
}

#[derive(Default, Deserialize)]
struct DirectoryQuery {
    #[serde(default)]
    path: String,
    #[serde(default)]
    hidden: bool,
}

#[derive(Serialize)]
struct DirectoryListing {
    root_id: String,
    path: String,
    entries: Vec<EntryInfo>,
}

#[derive(Serialize)]
struct TextDocument {
    root_id: String,
    path: String,
    text: String,
    revision: String,
    read_only: bool,
}

#[derive(Deserialize)]
struct SaveText {
    path: String,
    text: String,
    revision: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileOperationKind { CreateFile, CreateFolder, Rename, Copy, Move, Delete }

#[derive(Deserialize)]
struct FileOperation {
    operation: FileOperationKind,
    sources: Vec<String>,
    destination: Option<String>,
    name: Option<String>,
    #[serde(default)]
    replace: bool,
}

#[derive(Serialize)]
struct FileOperationResult { paths: Vec<String> }

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/files/roots", get(roots))
        .route("/api/v1/files/{root_id}/entries", get(entries))
        .route("/api/v1/files/{root_id}/content", get(content))
        .route("/api/v1/files/{root_id}/text", get(read_text).put(save_text))
        .route("/api/v1/files/{root_id}/operations", post(operate))
}

fn configured_roots(state: &AppState) -> Vec<(ConfiguredRoot, bool)> {
    let configured = state.configuration.read().file_manager_roots.clone();
    let mut roots: Vec<_> = if configured.is_empty() {
        vec![(ConfiguredRoot { id: "shows".into(), label: "Shows".into(), path: state.data_dir.join("shows"), icon: Some("shows".into()) }, false)]
    } else { configured.into_iter().map(|root| (root, false)).collect() };
    roots.extend(removable_roots().into_iter().map(|root| (root, true)));
    roots
}

fn removable_roots() -> Vec<ConfiguredRoot> {
    #[cfg(target_os = "macos")]
    let parent = FsPath::new("/Volumes");
    #[cfg(target_os = "linux")]
    let parent = FsPath::new("/media");
    #[cfg(target_os = "windows")]
    { return Vec::new(); }
    #[cfg(not(target_os = "windows"))]
    fs::read_dir(parent).into_iter().flatten().flatten().filter_map(|entry| {
        let path = entry.path();
        if !path.is_dir() { return None; }
        let label = entry.file_name().to_string_lossy().into_owned();
        Some(ConfiguredRoot { id: format!("removable-{}", URL_SAFE_NO_PAD.encode(path.to_string_lossy().as_bytes())), label, path, icon: Some("drive".into()) })
    }).collect()
}

fn root(state: &AppState, id: &str) -> Result<(ConfiguredRoot, bool), ApiError> {
    configured_roots(state).into_iter().find(|(root, _)| root.id == id)
        .ok_or_else(|| ApiError::not_found("file root not found"))
}

fn confined(root: &FsPath, relative: &str, allow_missing_leaf: bool) -> Result<PathBuf, ApiError> {
    let relative_path = FsPath::new(relative);
    if relative_path.is_absolute() || relative_path.components().any(|part| matches!(part, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
        return Err(ApiError::bad_request("path must be root-relative and may not traverse parents"));
    }
    let canonical_root = fs::canonicalize(root).map_err(|_| ApiError::not_found("file root is unavailable"))?;
    let joined = canonical_root.join(relative_path);
    let checked = if allow_missing_leaf && !joined.exists() {
        let parent = joined.parent().ok_or_else(|| ApiError::bad_request("invalid path"))?;
        fs::canonicalize(parent).map_err(|_| ApiError::not_found("parent directory not found"))?.join(joined.file_name().ok_or_else(|| ApiError::bad_request("invalid name"))?)
    } else {
        fs::canonicalize(&joined).map_err(|_| ApiError::not_found("file not found"))?
    };
    if checked != canonical_root && !checked.starts_with(&canonical_root) {
        return Err(ApiError::bad_request("path escapes the configured root"));
    }
    Ok(checked)
}

fn relative(root: &FsPath, path: &FsPath) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn millis(value: std::io::Result<std::time::SystemTime>) -> Option<u128> { value.ok()?.duration_since(UNIX_EPOCH).ok().map(|v| v.as_millis()) }
fn revision(metadata: &fs::Metadata) -> String { format!("{}-{}", metadata.len(), millis(metadata.modified()).unwrap_or(0)) }

async fn roots(State(state): State<AppState>) -> Json<Vec<RootInfo>> {
    Json(configured_roots(&state).into_iter().map(|(root, removable)| RootInfo {
        writable: fs::metadata(&root.path).map(|m| !m.permissions().readonly()).unwrap_or(false),
        id: root.id, label: root.label, icon: root.icon.unwrap_or_else(|| "folder".into()), removable,
    }).collect())
}

async fn entries(State(state): State<AppState>, Path(root_id): Path<String>, Query(query): Query<DirectoryQuery>) -> Result<Json<DirectoryListing>, ApiError> {
    let (root, _) = root(&state, &root_id)?;
    let canonical_root = fs::canonicalize(&root.path).map_err(|_| ApiError::not_found("file root is unavailable"))?;
    let directory = confined(&root.path, &query.path, false)?;
    if !directory.is_dir() { return Err(ApiError::bad_request("path is not a directory")); }
    let mut result = Vec::new();
    for item in fs::read_dir(&directory).map_err(ApiError::io)? {
        let item = item.map_err(ApiError::io)?;
        let name = item.file_name().to_string_lossy().into_owned();
        let hidden = name.starts_with('.');
        if hidden && !query.hidden { continue; }
        let path = confined(&root.path, &relative(&canonical_root, &item.path()), false)?;
        let metadata = fs::metadata(&path).map_err(ApiError::io)?;
        result.push(EntryInfo { name, path: relative(&canonical_root, &path), kind: if metadata.is_dir() { "folder" } else { "file" }, size: metadata.len(), modified_millis: millis(metadata.modified()), created_millis: millis(metadata.created()), hidden, writable: !metadata.permissions().readonly() });
    }
    result.sort_by(|a, b| (a.kind != "folder", a.name.to_lowercase()).cmp(&(b.kind != "folder", b.name.to_lowercase())));
    Ok(Json(DirectoryListing { root_id, path: relative(&canonical_root, &directory), entries: result }))
}

async fn read_text(State(state): State<AppState>, Path(root_id): Path<String>, Query(query): Query<DirectoryQuery>) -> Result<Json<TextDocument>, ApiError> {
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &query.path, false)?;
    let metadata = fs::metadata(&path).map_err(ApiError::io)?;
    if !metadata.is_file() { return Err(ApiError::bad_request("path is not a file")); }
    if metadata.len() > MAX_TEXT_BYTES { return Err(ApiError::bad_request("text file exceeds the 4 MiB limit")); }
    let bytes = fs::read(&path).map_err(ApiError::io)?;
    let text = String::from_utf8(bytes).map_err(|_| ApiError::bad_request("only UTF-8 plain text files are supported"))?;
    Ok(Json(TextDocument { root_id, path: query.path, text, revision: revision(&metadata), read_only: metadata.permissions().readonly() }))
}

async fn save_text(State(state): State<AppState>, Path(root_id): Path<String>, Json(input): Json<SaveText>) -> Result<Json<TextDocument>, ApiError> {
    if input.text.len() as u64 > MAX_TEXT_BYTES { return Err(ApiError::bad_request("text file exceeds the 4 MiB limit")); }
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &input.path, true)?;
    if path.exists() {
        let metadata = fs::metadata(&path).map_err(ApiError::io)?;
        if metadata.permissions().readonly() { return Err(ApiError::forbidden("file is read-only")); }
        if input.revision.as_deref() != Some(&revision(&metadata)) { return Err(ApiError::conflict("file changed since it was opened")); }
    } else if input.revision.is_some() { return Err(ApiError::conflict("file was removed since it was opened")); }
    let temporary = path.with_extension(format!("{}.light-tmp", path.extension().and_then(|v| v.to_str()).unwrap_or("txt")));
    fs::write(&temporary, input.text.as_bytes()).map_err(ApiError::io)?;
    fs::rename(&temporary, &path).map_err(ApiError::io)?;
    let metadata = fs::metadata(&path).map_err(ApiError::io)?;
    Ok(Json(TextDocument { root_id, path: input.path, text: input.text, revision: revision(&metadata), read_only: false }))
}

async fn content(State(state): State<AppState>, Path(root_id): Path<String>, Query(query): Query<DirectoryQuery>, headers: HeaderMap) -> Result<Response, ApiError> {
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &query.path, false)?;
    let bytes = fs::read(&path).map_err(ApiError::io)?;
    let total = bytes.len();
    let (start, end, status) = parse_range(headers.get(header::RANGE).and_then(|v| v.to_str().ok()), total)?;
    let mime = mime_for(&path);
    Response::builder().status(status).header(header::CONTENT_TYPE, mime).header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, end.saturating_sub(start).to_string())
        .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end.saturating_sub(1), total))
        .body(Body::from(bytes[start..end].to_vec())).map_err(|_| ApiError::bad_request("could not stream file"))
}

fn parse_range(range: Option<&str>, total: usize) -> Result<(usize, usize, StatusCode), ApiError> {
    let Some(value) = range else { return Ok((0, total, StatusCode::OK)); };
    let value = value.strip_prefix("bytes=").ok_or_else(|| ApiError::bad_request("invalid range"))?;
    let (start, end) = value.split_once('-').ok_or_else(|| ApiError::bad_request("invalid range"))?;
    let start: usize = start.parse().map_err(|_| ApiError::bad_request("invalid range"))?;
    let end = if end.is_empty() { total } else { end.parse::<usize>().map_err(|_| ApiError::bad_request("invalid range"))?.saturating_add(1).min(total) };
    if start >= end || start >= total { return Err(ApiError::bad_request("range is outside the file")); }
    Ok((start, end, StatusCode::PARTIAL_CONTENT))
}

fn mime_for(path: &FsPath) -> &'static str { match path.extension().and_then(|v| v.to_str()).unwrap_or("").to_ascii_lowercase().as_str() { "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "webp" => "image/webp", "mp3" => "audio/mpeg", "wav" => "audio/wav", "txt" | "md" | "csv" | "log" => "text/plain; charset=utf-8", _ => "application/octet-stream" } }

async fn operate(State(state): State<AppState>, Path(root_id): Path<String>, Json(input): Json<FileOperation>) -> Result<Json<FileOperationResult>, ApiError> {
    let (root, _) = root(&state, &root_id)?;
    let canonical_root = fs::canonicalize(&root.path).map_err(|_| ApiError::not_found("file root is unavailable"))?;
    let destination = input.destination.as_deref().map(|value| confined(&root.path, value, false)).transpose()?;
    let mut paths = Vec::new();
    match input.operation {
        FileOperationKind::CreateFile | FileOperationKind::CreateFolder => {
            let directory = destination.unwrap_or_else(|| canonical_root.clone());
            let name = safe_name(input.name.as_deref())?;
            let path = confined(&root.path, &relative(&canonical_root, &directory.join(name)), true)?;
            if path.exists() { return Err(ApiError::conflict("an item with that name already exists")); }
            if matches!(input.operation, FileOperationKind::CreateFolder) { fs::create_dir(&path).map_err(ApiError::io)?; } else { fs::write(&path, []).map_err(ApiError::io)?; }
            paths.push(relative(&canonical_root, &path));
        }
        FileOperationKind::Rename => {
            let source = one_source(&root.path, &input.sources)?;
            let name = safe_name(input.name.as_deref())?;
            let target = confined(&root.path, &relative(&canonical_root, &source.parent().unwrap_or(&canonical_root).join(name)), true)?;
            if target.exists() && !input.replace { return Err(ApiError::conflict("an item with that name already exists")); }
            if target.exists() { remove(&target)?; }
            fs::rename(source, &target).map_err(ApiError::io)?;
            paths.push(relative(&canonical_root, &target));
        }
        FileOperationKind::Delete => for source in &input.sources { let source = confined(&root.path, source, false)?; remove(&source)?; },
        FileOperationKind::Copy | FileOperationKind::Move => {
            let destination = destination.ok_or_else(|| ApiError::bad_request("destination is required"))?;
            if !destination.is_dir() { return Err(ApiError::bad_request("destination is not a folder")); }
            for source in &input.sources {
                let source = confined(&root.path, source, false)?;
                let target = confined(&root.path, &relative(&canonical_root, &destination.join(source.file_name().ok_or_else(|| ApiError::bad_request("invalid source"))?)), true)?;
                if target.exists() && !input.replace { return Err(ApiError::conflict("an item with that name already exists")); }
                if target.exists() { remove(&target)?; }
                if matches!(input.operation, FileOperationKind::Move) { fs::rename(&source, &target).map_err(ApiError::io)?; } else { copy_recursive(&source, &target)?; }
                paths.push(relative(&canonical_root, &target));
            }
        }
    }
    Ok(Json(FileOperationResult { paths }))
}

fn safe_name(value: Option<&str>) -> Result<&str, ApiError> { let value = value.map(str::trim).filter(|v| !v.is_empty()).ok_or_else(|| ApiError::bad_request("name is required"))?; if value == "." || value == ".." || value.contains('/') || value.contains('\\') { return Err(ApiError::bad_request("name may not contain path separators")); } Ok(value) }
fn one_source(root: &FsPath, sources: &[String]) -> Result<PathBuf, ApiError> { if sources.len() != 1 { return Err(ApiError::bad_request("exactly one source is required")); } confined(root, &sources[0], false) }
fn remove(path: &FsPath) -> Result<(), ApiError> { if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }.map_err(ApiError::io) }
fn copy_recursive(source: &FsPath, target: &FsPath) -> Result<(), ApiError> { if source.is_dir() { fs::create_dir(target).map_err(ApiError::io)?; for entry in fs::read_dir(source).map_err(ApiError::io)? { let entry = entry.map_err(ApiError::io)?; copy_recursive(&entry.path(), &target.join(entry.file_name()))?; } Ok(()) } else { fs::copy(source, target).map(|_| ()).map_err(ApiError::io) } }

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temporary_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("light-file-manager-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("folder")).unwrap();
        fs::write(root.join("folder/note.txt"), b"hello").unwrap();
        root
    }

    #[test]
    fn confinement_rejects_parent_and_symlink_escapes() {
        let root = temporary_root();
        assert!(confined(&root, "folder/note.txt", false).is_ok());
        assert!(confined(&root, "../outside", false).is_err());
        #[cfg(unix)] {
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("escape")).unwrap();
            assert!(confined(&root, "escape", false).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ranges_are_inclusive_at_the_http_boundary() {
        assert_eq!(parse_range(None, 10).unwrap(), (0, 10, StatusCode::OK));
        assert_eq!(parse_range(Some("bytes=2-4"), 10).unwrap(), (2, 5, StatusCode::PARTIAL_CONTENT));
        assert!(parse_range(Some("bytes=10-12"), 10).is_err());
    }
}
