//! Importing media into the library.
//!
//! Starting an import is neither live control nor a configuration edit: it is work handed to the
//! process, so it answers with job identities and the jobs report themselves afterwards. No
//! connection is held for the length of a transcode.
//!
//! It still carries a request id, because starting one is not idempotent — a dropped response
//! followed by a retry would transcode a library twice — and the replay window answers the retry
//! with what the first attempt started.

use std::collections::HashSet;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use media_domain::{AssetId, CatalogLocation, MediaAddress};
use serde::Deserialize;
use uuid::Uuid;

use crate::diagnostics::{LibraryEdit, LibraryNoteTarget};
use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{
    CatalogView, DeleteLibraryItem, DeleteLibraryItems, ImportJobView, ImportsView,
    LibraryNoteTargetView, PendingImportView, StartImport, UpdateLibraryFolder, UpdateLibraryItem,
    UpdateLibraryItems, UpdateLibraryNotes, UpdateLibraryThumbnail, UploadAcceptedView,
};

pub(super) const MAX_CUSTOM_THUMBNAIL_BYTES: usize = 16 * 1024 * 1024;

/// What is waiting to be imported, and what every import this run has done.
pub(super) async fn imports(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(view_of(&state))
}

/// Starts importing: everything waiting, or one address.
pub(super) async fn start_import(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<StartImport>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    if !state.diagnostics.imports.available {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "cannot-import",
            "this machine cannot convert media: FFmpeg is not installed or not on PATH",
        ));
    }

    // Naming half an address is not a selection, it is a mistake, and importing a whole folder
    // when one file was meant would be a long and surprising job.
    let address = match (body.folder, body.file) {
        (Some(folder), Some(file)) => Some(MediaAddress::new(folder, file)),
        (None, None) => None,
        _ => {
            return Err(ApiError::bad_request(
                "incomplete-address",
                "name both a folder and a file, or neither to import everything waiting",
            ));
        }
    };

    let started = (state.diagnostics.imports.start)(address);
    if started == 0 {
        return Err(ApiError::not_found(
            "nothing-to-import",
            match address {
                Some(address) => format!("nothing at {address} is waiting to be imported"),
                None => "nothing in the library is waiting to be imported".to_owned(),
            },
        ));
    }

    let view = view_of(&state);
    let serialized = serde_json::to_string(&view).unwrap_or_default();
    state.replays.remember(&body.request_id, serialized.clone());
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}

/// Stops one import. A payload-free action, so it is a `GET` an integrator can trigger.
pub(super) async fn cancel_import(
    State(state): State<ApiState>,
    Path(job): Path<String>,
) -> Result<Response, ApiError> {
    if !(state.diagnostics.imports.cancel)(&job) {
        return Err(ApiError::not_found(
            "unknown-import",
            "no import with that identity is still running",
        ));
    }
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

/// Renames or moves one stable item. A destination collision is refused unless the operator
/// explicitly asks to exchange the two addresses.
pub(super) async fn update_item(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    TolerantJson(body): TolerantJson<UpdateLibraryItem>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let id = Uuid::parse_str(&id)
        .map(AssetId::from_uuid)
        .map_err(|_| ApiError::bad_request("invalid-asset-id", "the catalog item id is invalid"))?;
    let operation = match (
        body.name,
        body.folder,
        body.file,
        body.intrinsic_bpm,
        body.enabled,
    ) {
        (Some(name), None, None, None, None) => {
            let name = name.trim().to_owned();
            if name.is_empty() {
                return Err(ApiError::bad_request(
                    "empty-item-name",
                    "a media item name cannot be empty",
                ));
            }
            LibraryEdit::RenameItem { id, name }
        }
        (None, Some(folder), Some(file), None, None) => LibraryEdit::MoveItem {
            id,
            destination: CatalogLocation::new(folder, file),
            swap: body.swap,
        },
        (None, None, None, Some(bpm), None) => LibraryEdit::SetItemBpm { id, bpm },
        (None, None, None, None, Some(enabled)) => LibraryEdit::SetItemEnabled { id, enabled },
        _ => {
            return Err(ApiError::bad_request(
                "ambiguous-library-edit",
                "rename, move, set intrinsic BPM, or set enabled with exactly one edit",
            ));
        }
    };
    (state.diagnostics.library.edit)(operation)
        .map_err(|detail| library_edit_error("library-item-not-updated", detail))?;
    remember_catalog(&state, &body.request_id)
}

/// Permanently removes one media item after the UI has obtained explicit operator confirmation.
pub(super) async fn delete_item(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    TolerantJson(body): TolerantJson<DeleteLibraryItem>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let id = Uuid::parse_str(&id)
        .map(AssetId::from_uuid)
        .map_err(|_| ApiError::bad_request("invalid-asset-id", "the catalog item id is invalid"))?;
    (state.diagnostics.library.edit)(LibraryEdit::DeleteItem { id })
        .map_err(|detail| library_edit_error("library-item-not-deleted", detail))?;
    remember_catalog(&state, &body.request_id)
}

/// Enables or disables all selected stable items as one retry-safe library edit.
pub(super) async fn update_items(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdateLibraryItems>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let ids = selected_item_ids(body.ids)?;
    (state.diagnostics.library.edit)(LibraryEdit::SetItemsEnabled {
        ids,
        enabled: body.enabled,
    })
    .map_err(|detail| library_edit_error("library-items-not-updated", detail))?;
    remember_catalog(&state, &body.request_id)
}

/// Permanently removes all selected stable items after one operator confirmation in the UI.
pub(super) async fn delete_items(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<DeleteLibraryItems>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let ids = selected_item_ids(body.ids)?;
    (state.diagnostics.library.edit)(LibraryEdit::DeleteItems { ids })
        .map_err(|detail| library_edit_error("library-items-not-deleted", detail))?;
    remember_catalog(&state, &body.request_id)
}

fn selected_item_ids(ids: Vec<String>) -> Result<Vec<AssetId>, ApiError> {
    if ids.is_empty() || ids.len() > 1_000 {
        return Err(ApiError::bad_request(
            "invalid-item-selection",
            "select between 1 and 1000 media items",
        ));
    }
    let mut selected = Vec::with_capacity(ids.len());
    let mut unique = HashSet::with_capacity(ids.len());
    for id in ids {
        let id = Uuid::parse_str(&id).map(AssetId::from_uuid).map_err(|_| {
            ApiError::bad_request("invalid-asset-id", "a selected media item id is invalid")
        })?;
        if !unique.insert(id) {
            return Err(ApiError::bad_request(
                "duplicate-item-selection",
                "each selected media item may appear only once",
            ));
        }
        selected.push(id);
    }
    Ok(selected)
}

/// Reruns bounded best-frame selection for one stable media item.
pub(super) async fn retry_thumbnail(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    TolerantJson(body): TolerantJson<UpdateLibraryThumbnail>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let id = parse_item_id(&id)?;
    (state.diagnostics.library.regenerate_thumbnail)(id)
        .map_err(|detail| library_edit_error("library-thumbnail-not-generated", detail))?;
    remember_catalog(&state, &body.request_id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ThumbnailUploadQuery {
    request_id: String,
}

/// Validates and normalizes one uploaded image as the stable item's custom thumbnail.
pub(super) async fn upload_thumbnail(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(query): Query<ThumbnailUploadQuery>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin_upload(&state, &query.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let id = parse_item_id(&id)?;
    let mut upload = None;
    while let Some(mut field) = multipart.next_field().await.map_err(|_| {
        ApiError::bad_request(
            "invalid-custom-thumbnail",
            "the custom thumbnail upload could not be read",
        )
    })? {
        if field.name() != Some("file") {
            continue;
        }
        if upload.is_some() {
            return Err(ApiError::bad_request(
                "multiple-custom-thumbnails",
                "upload exactly one custom thumbnail",
            ));
        }
        let content_type = field.content_type().unwrap_or_default();
        if !matches!(
            content_type,
            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
        ) {
            return Err(ApiError::bad_request(
                "invalid-custom-thumbnail-type",
                "custom thumbnails must be PNG, JPEG, GIF, or WebP images",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(|_| {
            ApiError::bad_request(
                "invalid-custom-thumbnail",
                "the custom thumbnail upload could not be read",
            )
        })? {
            if bytes.len().saturating_add(chunk.len()) > MAX_CUSTOM_THUMBNAIL_BYTES {
                return Err(ApiError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "custom-thumbnail-too-large",
                    "custom thumbnails may be at most 16 MiB",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err(ApiError::bad_request(
                "empty-custom-thumbnail",
                "the uploaded custom thumbnail is empty",
            ));
        }
        upload = Some(bytes);
    }
    let bytes = upload.ok_or_else(|| {
        ApiError::bad_request(
            "missing-custom-thumbnail",
            "the multipart body has no file field",
        )
    })?;
    (state.diagnostics.library.set_custom_thumbnail)(id, &bytes)
        .map_err(|detail| library_edit_error("custom-thumbnail-not-stored", detail))?;
    remember_catalog(&state, &query.request_id)
}

fn parse_item_id(id: &str) -> Result<AssetId, ApiError> {
    Uuid::parse_str(id)
        .map(AssetId::from_uuid)
        .map_err(|_| ApiError::bad_request("invalid-asset-id", "the catalog item id is invalid"))
}

/// Changes a folder's optional visible label. Clearing the field removes `.info` deliberately.
pub(super) async fn update_folder(
    State(state): State<ApiState>,
    Path(folder): Path<u16>,
    TolerantJson(body): TolerantJson<UpdateLibraryFolder>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let operation = match (body.name, body.icon, body.swap_with, body.compact) {
        (Some(name), None, None, None) => {
            let name = name.trim();
            LibraryEdit::RenameFolder {
                folder,
                name: (!name.is_empty()).then(|| name.to_owned()),
            }
        }
        (None, Some(icon), None, None) => {
            let icon = icon.trim();
            LibraryEdit::SetFolderIcon {
                folder,
                icon: (!icon.is_empty()).then(|| icon.to_owned()),
            }
        }
        (None, None, Some(second), None) => LibraryEdit::SwapFolders {
            first: folder,
            second,
        },
        (None, None, None, Some(true)) => LibraryEdit::CompactFolder { folder },
        _ => {
            return Err(ApiError::bad_request(
                "ambiguous-library-folder-edit",
                "set name, set icon, reorder with swapWith, or compact with compact: true",
            ));
        }
    };
    (state.diagnostics.library.edit)(operation)
        .map_err(|detail| library_edit_error("library-folder-not-updated", detail))?;
    remember_catalog(&state, &body.request_id)
}

/// Applies one exact note to all selected media items or folders.
pub(super) async fn update_notes(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdateLibraryNotes>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    if body.targets.is_empty() || body.targets.len() > 1_000 {
        return Err(ApiError::bad_request(
            "invalid-note-targets",
            "select between 1 and 1000 media items or folders",
        ));
    }
    if body.note.len() > 64 * 1024 {
        return Err(ApiError::bad_request(
            "note-too-large",
            "a media note cannot exceed 64 KiB",
        ));
    }

    let mut targets = Vec::with_capacity(body.targets.len());
    let mut unique = HashSet::with_capacity(body.targets.len());
    for target in body.targets {
        let target = match target {
            LibraryNoteTargetView::Item { id } => Uuid::parse_str(&id)
                .map(AssetId::from_uuid)
                .map(LibraryNoteTarget::Item)
                .map_err(|_| {
                    ApiError::bad_request("invalid-asset-id", "a selected media item id is invalid")
                })?,
            LibraryNoteTargetView::Folder { folder } => LibraryNoteTarget::Folder(folder),
        };
        if !unique.insert(target) {
            return Err(ApiError::bad_request(
                "duplicate-note-target",
                "each media item or folder may be selected only once",
            ));
        }
        targets.push(target);
    }

    (state.diagnostics.library.edit)(LibraryEdit::SetNotes {
        targets,
        note: (!body.note.is_empty()).then_some(body.note),
    })
    .map_err(|detail| library_edit_error("library-notes-not-updated", detail))?;
    remember_catalog(&state, &body.request_id)
}

/// Serves only the generated thumbnail belonging to one address; no caller-provided path reaches
/// the filesystem.
pub(super) async fn thumbnail(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u16, u8)>,
) -> Result<Response, ApiError> {
    let bytes = (state.diagnostics.library.thumbnail)(CatalogLocation::new(folder, file)).map_err(
        |_| {
            ApiError::not_found(
                "thumbnail-not-found",
                "this media item has no thumbnail yet",
            )
        },
    )?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
pub(super) struct PreviewQuery {
    #[serde(default)]
    frame: usize,
}

/// Serves one decoded native-clip frame for the browser inspector. It is intentionally a sequence
/// of bounded JPEGs rather than the source file: `.toskclip`/HAP is not a browser media format.
pub(super) async fn preview(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u16, u8)>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, ApiError> {
    let location = CatalogLocation::new(folder, file);
    let catalog = state.catalog.load();
    let item = catalog
        .folder(folder)
        .and_then(|entry| entry.item(file))
        .ok_or_else(|| {
            ApiError::not_found(
                "media-not-found",
                "this media item is no longer in the library",
            )
        })?;
    let bytes = (state.diagnostics.library.preview_frame)(location, &item.name, query.frame)
        .map_err(|_| {
            ApiError::not_found(
                "media-preview-not-found",
                "this media item cannot be previewed",
            )
        })?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UploadQuery {
    request_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    replace: bool,
}

/// Streams one browser-selected source into a hidden staging file and immediately queues its HAP
/// import. The source is never held in memory and never overwrites an occupied address.
pub(super) async fn upload(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u8, u8)>,
    Query(query): Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin_upload(&state, &query.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    if !state.diagnostics.imports.available {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "cannot-import",
            "this machine cannot convert media: FFmpeg is not installed or not on PATH",
        ));
    }
    let address = MediaAddress::new(folder, file);
    let mut upload = None;
    while let Some(mut field) = multipart.next_field().await.map_err(|_| {
        ApiError::bad_request("invalid-upload", "the multipart upload could not be read")
    })? {
        if field.name() != Some("file") {
            continue;
        }
        if upload.is_some() {
            return Err(ApiError::bad_request(
                "multiple-upload-files",
                "upload exactly one media file at a time",
            ));
        }
        let filename = field.file_name().unwrap_or("upload").to_owned();
        let mut stream = (state.diagnostics.library.begin_upload)(
            address,
            query.name.trim(),
            &filename,
            query.replace,
        )
        .map_err(|detail| library_edit_error("upload-not-started", detail))?;
        while let Some(chunk) = field.chunk().await.map_err(|_| {
            ApiError::bad_request("invalid-upload", "the uploaded media could not be read")
        })? {
            stream
                .write(&chunk)
                .map_err(|detail| library_edit_error("upload-not-written", detail))?;
        }
        upload = Some(stream);
    }
    let stream = upload.ok_or_else(|| {
        ApiError::bad_request(
            "missing-upload-file",
            "the multipart body has no file field",
        )
    })?;
    let job_id = stream
        .finish()
        .map_err(|detail| library_edit_error("upload-not-queued", detail))?;
    let view = UploadAcceptedView {
        job_id,
        address: crate::wire::AddressView::of(address),
    };
    let serialized = serde_json::to_string(&view).unwrap_or_default();
    state
        .replays
        .remember(&query.request_id, serialized.clone());
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}

fn remember_catalog(state: &ApiState, request_id: &str) -> Result<Response, ApiError> {
    let catalog = state.catalog.load();
    let serialized = serde_json::to_string(&CatalogView::of(&catalog)).unwrap_or_default();
    state.replays.remember(request_id, serialized.clone());
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}

fn library_edit_error(code: &'static str, detail: String) -> ApiError {
    let lower = detail.to_ascii_lowercase();
    let status = if lower.contains("not found") || lower.contains("no item") {
        StatusCode::NOT_FOUND
    } else if lower.contains("occupied") || lower.contains("already has") {
        StatusCode::CONFLICT
    } else if lower.contains("outside") || lower.contains("sentinel") || lower.contains("empty") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    ApiError::new(status, code, detail)
}

fn view_of(state: &ApiState) -> ImportsView {
    let (pending, jobs) = (state.diagnostics.imports.state)();
    ImportsView {
        pending: pending.iter().map(PendingImportView::of).collect(),
        jobs: jobs.iter().map(ImportJobView::of).collect(),
        can_import: state.diagnostics.imports.available,
    }
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
