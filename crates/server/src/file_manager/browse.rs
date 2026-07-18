use std::{fs, path::Path as FsPath};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Serialize;

use super::super::file_manager_support as support;
use super::super::{ApiError, AppState, authenticate};
use super::paths::{DirectoryQuery, configured_roots, confined, millis, mime_for, relative, root};

#[derive(Serialize)]
pub(super) struct RootInfo {
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
pub(super) struct EntryInfo {
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

#[derive(Serialize)]
pub(super) struct DirectoryListing {
    root_id: String,
    path: String,
    entries: Vec<EntryInfo>,
}

#[derive(Serialize)]
pub(super) struct MetadataInfo {
    root_id: String,
    #[serde(flatten)]
    entry: EntryInfo,
    capabilities: FileSystemCapabilities,
}

pub(super) async fn roots(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RootInfo>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(
        configured_roots(&state)
            .into_iter()
            .map(|(root, removable)| RootInfo {
                writable: fs::metadata(&root.path)
                    .map(|metadata| !metadata.permissions().readonly())
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

pub(super) async fn entries(
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
    let mut result = directory_entries(&canonical_root, &directory, query.hidden)?;
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

fn directory_entries(
    canonical_root: &FsPath,
    directory: &FsPath,
    include_hidden: bool,
) -> Result<Vec<EntryInfo>, ApiError> {
    let mut entries = Vec::new();
    for item in fs::read_dir(directory).map_err(ApiError::io)? {
        let item = match item {
            Ok(item) => item,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(ApiError::io(error)),
        };
        if let Some(entry) = directory_entry_info(canonical_root, item, include_hidden)? {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub(super) fn directory_entry_info(
    canonical_root: &FsPath,
    item: fs::DirEntry,
    include_hidden: bool,
) -> Result<Option<EntryInfo>, ApiError> {
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
    Ok(Some(entry_info(
        canonical_root,
        &path,
        item.file_name().to_string_lossy().into_owned(),
        &metadata,
        hidden,
    )))
}

pub(super) async fn metadata(
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
    let hidden = support::is_hidden(path.file_name().unwrap_or_else(|| path.as_os_str()), &value);
    Ok(Json(MetadataInfo {
        root_id,
        entry: entry_info(&canonical_root, &path, name, &value, hidden),
        capabilities: filesystem_capabilities(&root.path),
    }))
}

fn entry_info(
    canonical_root: &FsPath,
    path: &FsPath,
    name: String,
    metadata: &fs::Metadata,
    hidden: bool,
) -> EntryInfo {
    EntryInfo {
        name,
        path: relative(canonical_root, path),
        kind: if metadata.is_dir() { "folder" } else { "file" },
        size: metadata.len(),
        modified_millis: millis(metadata.modified()),
        created_millis: millis(metadata.created()),
        hidden,
        writable: !metadata.permissions().readonly(),
        mime: mime_for(path),
        note_supported: support::native_notes_supported(path),
        trash_supported: support::trash_supported(),
    }
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
