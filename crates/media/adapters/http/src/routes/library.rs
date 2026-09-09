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
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    use crate::diagnostics::{
        Diagnostics, ImportJob, ImportOutcome, Imports, LibraryAccess, LibraryEdit,
        LibraryNoteTarget, PendingImport, UploadStream,
    };
    use crate::routes::bench::{bench, bench_with, get, post, send};

    /// A process with two files waiting and a record of what it was asked to start.
    fn ready() -> (
        Diagnostics,
        Arc<Mutex<Vec<Option<media_domain::MediaAddress>>>>,
    ) {
        let asked: Arc<Mutex<Vec<Option<media_domain::MediaAddress>>>> =
            Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&asked);
        let diagnostics = Diagnostics {
            imports: Imports {
                state: Arc::new(|| {
                    (
                        vec![
                            PendingImport {
                                destination: media_domain::MediaAddress::new(1, 1),
                                name: "Bars".to_owned(),
                                filename: "001-Bars.mp4".to_owned(),
                            },
                            PendingImport {
                                destination: media_domain::MediaAddress::new(1, 4),
                                name: "LoopTest".to_owned(),
                                filename: "004-LoopTest.mp4".to_owned(),
                            },
                        ],
                        vec![ImportJob {
                            id: "job-1".to_owned(),
                            batch_id: None,
                            destination: media_domain::MediaAddress::new(2, 1),
                            filename: "001.png".to_owned(),
                            outcome: ImportOutcome::Running,
                            attempts: 1,
                            fraction: Some(0.5),
                            frames_done: Some(50),
                            frames_total: Some(100),
                        }],
                    )
                }),
                start: Arc::new(move |address| {
                    recorder.lock().unwrap().push(address);
                    2
                }),
                cancel: Arc::new(|job| job == "job-1"),
                available: true,
            },
            ..Default::default()
        };
        (diagnostics, asked)
    }

    #[tokio::test]
    async fn the_library_reports_what_is_waiting_and_what_is_running() {
        let (diagnostics, _) = ready();
        let bench = bench_with(diagnostics);
        let (status, body) = send(&bench.router, get("/api/v2/library/imports".into())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["canImport"], true);
        assert_eq!(body["pending"][0]["address"]["folder"], 1);
        assert_eq!(body["pending"][0]["filename"], "001-Bars.mp4");
        assert_eq!(body["pending"][1]["name"], "LoopTest");
        assert_eq!(body["jobs"][0]["state"], "running");
        assert_eq!(body["jobs"][0]["fraction"], 0.5);
    }

    #[tokio::test]
    async fn importing_everything_waiting_names_no_address() {
        let (diagnostics, asked) = ready();
        let bench = bench_with(diagnostics);
        let (status, body) = send(
            &bench.router,
            post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(asked.lock().unwrap()[0], None, "everything waiting");
        assert!(body["jobs"].as_array().is_some());
    }

    #[tokio::test]
    async fn one_address_imports_only_that_one() {
        let (diagnostics, asked) = ready();
        let bench = bench_with(diagnostics);
        send(
            &bench.router,
            post(
                "/api/v2/library/import".into(),
                r#"{"requestId":"a","folder":1,"file":4}"#,
            ),
        )
        .await;

        assert_eq!(
            asked.lock().unwrap()[0],
            Some(media_domain::MediaAddress::new(1, 4))
        );
    }

    #[tokio::test]
    async fn half_an_address_is_refused_rather_than_guessed_at() {
        let (diagnostics, asked) = ready();
        let bench = bench_with(diagnostics);
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/library/import".into(),
                r#"{"requestId":"a","folder":1}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "incomplete-address");
        assert!(
            asked.lock().unwrap().is_empty(),
            "a folder's worth of transcoding did not start by accident"
        );
    }

    #[tokio::test]
    async fn a_retried_start_does_not_transcode_the_library_twice() {
        let (diagnostics, asked) = ready();
        let bench = bench_with(diagnostics);
        let body = r#"{"requestId":"same"}"#;

        let (_, first) = send(&bench.router, post("/api/v2/library/import".into(), body)).await;
        let (status, second) =
            send(&bench.router, post("/api/v2/library/import".into(), body)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(first, second);
        assert_eq!(asked.lock().unwrap().len(), 1, "it was started once");
    }

    #[tokio::test]
    async fn a_machine_that_cannot_transcode_says_so_before_anything_is_queued() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "cannot-import");
        assert!(body["message"].as_str().unwrap().contains("FFmpeg"));
    }

    #[tokio::test]
    async fn importing_when_nothing_is_waiting_says_so() {
        let diagnostics = Diagnostics {
            imports: Imports {
                available: true,
                start: Arc::new(|_| 0),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let (status, body) = send(
            &bench.router,
            post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "nothing-to-import");
    }

    #[tokio::test]
    async fn a_running_import_can_be_stopped_and_stopping_it_twice_says_so() {
        let (diagnostics, _) = ready();
        let bench = bench_with(diagnostics);

        use tower::ServiceExt as _;
        let response = bench
            .router
            .clone()
            .oneshot(get("/api/v2/library/imports/job-1/cancel".into()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()[axum::http::header::CACHE_CONTROL],
            "no-store"
        );

        let (status, body) = send(
            &bench.router,
            get("/api/v2/library/imports/job-9/cancel".into()),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-import");
    }

    #[tokio::test]
    async fn item_and_folder_edits_carry_stable_intent_to_the_library() {
        let edits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&edits);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                edit: Arc::new(move |edit| {
                    recorded.lock().unwrap().push(edit);
                    Ok(())
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let id = uuid::Uuid::new_v4();
        let (status, _) = send(
            &bench.router,
            post(
                format!("/api/v2/library/items/{id}/update"),
                r#"{"requestId":"rename","name":"Opening"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                "/api/v2/library/folders/7/update".into(),
                r#"{"requestId":"compact-folder","compact":true}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                "/api/v2/library/folders/7/update".into(),
                r#"{"requestId":"park-folder","swapWith":900}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                format!("/api/v2/library/items/{id}/update"),
                r#"{"requestId":"park-item","folder":900,"file":1}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                format!("/api/v2/library/items/{id}/update"),
                r#"{"requestId":"bpm","intrinsicBpm":128.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                "/api/v2/library/folders/7/update".into(),
                r#"{"requestId":"folder","name":"Looks"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                "/api/v2/library/folders/7/update".into(),
                r#"{"requestId":"folder-icon","icon":"▶"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                "/api/v2/library/notes/update".into(),
                &format!(
                    r#"{{"requestId":"notes","targets":[{{"kind":"item","id":"{id}"}},{{"kind":"folder","folder":7}}],"note":"Licence: CC BY 4.0"}}"#
                ),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                format!("/api/v2/library/items/{id}/update"),
                r#"{"requestId":"disable","enabled":false}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                format!("/api/v2/library/items/{id}/update"),
                r#"{"requestId":"move","folder":2,"file":8,"swap":true}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &bench.router,
            post(
                format!("/api/v2/library/items/{id}/delete"),
                r#"{"requestId":"delete"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        assert_eq!(
            edits.lock().unwrap().as_slice(),
            [
                LibraryEdit::RenameItem {
                    id: media_domain::AssetId::from_uuid(id),
                    name: "Opening".to_owned(),
                },
                LibraryEdit::CompactFolder { folder: 7 },
                LibraryEdit::SwapFolders {
                    first: 7,
                    second: 900,
                },
                LibraryEdit::MoveItem {
                    id: media_domain::AssetId::from_uuid(id),
                    destination: media_domain::CatalogLocation::new(900, 1),
                    swap: false,
                },
                LibraryEdit::SetItemBpm {
                    id: media_domain::AssetId::from_uuid(id),
                    bpm: Some(128.5),
                },
                LibraryEdit::RenameFolder {
                    folder: 7,
                    name: Some("Looks".to_owned()),
                },
                LibraryEdit::SetFolderIcon {
                    folder: 7,
                    icon: Some("▶".to_owned()),
                },
                LibraryEdit::SetNotes {
                    targets: vec![
                        LibraryNoteTarget::Item(media_domain::AssetId::from_uuid(id)),
                        LibraryNoteTarget::Folder(7),
                    ],
                    note: Some("Licence: CC BY 4.0".to_owned()),
                },
                LibraryEdit::SetItemEnabled {
                    id: media_domain::AssetId::from_uuid(id),
                    enabled: false
                },
                LibraryEdit::MoveItem {
                    id: media_domain::AssetId::from_uuid(id),
                    destination: media_domain::MediaAddress::new(2, 8).into(),
                    swap: true,
                },
                LibraryEdit::DeleteItem {
                    id: media_domain::AssetId::from_uuid(id)
                },
            ]
        );
    }

    #[tokio::test]
    async fn invalid_or_duplicate_note_targets_are_refused_before_the_library_changes() {
        let edits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&edits);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                edit: Arc::new(move |edit| {
                    recorded.lock().unwrap().push(edit);
                    Ok(())
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/library/notes/update".into(),
                r#"{"requestId":"empty","targets":[],"note":"Licence"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "invalid-note-targets");

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/library/notes/update".into(),
                r#"{"requestId":"duplicate","targets":[{"kind":"folder","folder":7},{"kind":"folder","folder":7}],"note":"Licence"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "duplicate-note-target");
        assert!(edits.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_a_library_item_is_replay_safe() {
        let edits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&edits);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                edit: Arc::new(move |edit| {
                    recorded.lock().unwrap().push(edit);
                    Ok(())
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let id = uuid::Uuid::new_v4();
        let request = post(
            format!("/api/v2/library/items/{id}/delete"),
            r#"{"requestId":"same-delete"}"#,
        );
        let (first, _) = send(&bench.router, request).await;
        let (second, _) = send(
            &bench.router,
            post(
                format!("/api/v2/library/items/{id}/delete"),
                r#"{"requestId":"same-delete"}"#,
            ),
        )
        .await;

        assert_eq!(first, StatusCode::OK);
        assert_eq!(second, StatusCode::OK);
        assert_eq!(
            edits.lock().unwrap().as_slice(),
            [LibraryEdit::DeleteItem {
                id: media_domain::AssetId::from_uuid(id),
            }]
        );
    }

    #[tokio::test]
    async fn bulk_item_edits_use_stable_ids_and_are_replay_safe() {
        let edits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&edits);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                edit: Arc::new(move |edit| {
                    recorded.lock().unwrap().push(edit);
                    Ok(())
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let first = uuid::Uuid::new_v4();
        let second = uuid::Uuid::new_v4();
        let enable_body = format!(
            r#"{{"requestId":"bulk-enable","ids":["{first}","{second}"],"enabled":false}}"#
        );
        let delete_body = format!(r#"{{"requestId":"bulk-delete","ids":["{first}","{second}"]}}"#);

        for _ in 0..2 {
            let (status, _) = send(
                &bench.router,
                post("/api/v2/library/items/update".into(), &enable_body),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        for _ in 0..2 {
            let (status, _) = send(
                &bench.router,
                post("/api/v2/library/items/delete".into(), &delete_body),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }

        let ids = vec![
            media_domain::AssetId::from_uuid(first),
            media_domain::AssetId::from_uuid(second),
        ];
        assert_eq!(
            edits.lock().unwrap().as_slice(),
            [
                LibraryEdit::SetItemsEnabled {
                    ids: ids.clone(),
                    enabled: false,
                },
                LibraryEdit::DeleteItems { ids },
            ]
        );
    }

    #[tokio::test]
    async fn bulk_item_edits_refuse_empty_and_duplicate_selections() {
        let edits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&edits);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                edit: Arc::new(move |edit| {
                    recorded.lock().unwrap().push(edit);
                    Ok(())
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let id = uuid::Uuid::new_v4();
        let (empty, _) = send(
            &bench.router,
            post(
                "/api/v2/library/items/update".into(),
                r#"{"requestId":"empty-bulk","ids":[],"enabled":false}"#,
            ),
        )
        .await;
        let (duplicate, body) = send(
            &bench.router,
            post(
                "/api/v2/library/items/delete".into(),
                &format!(r#"{{"requestId":"duplicate-bulk","ids":["{id}","{id}"]}}"#),
            ),
        )
        .await;

        assert_eq!(empty, StatusCode::BAD_REQUEST);
        assert_eq!(duplicate, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "duplicate-item-selection");
        assert!(edits.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn compacting_a_library_folder_is_replay_safe() {
        let edits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&edits);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                edit: Arc::new(move |edit| {
                    recorded.lock().unwrap().push(edit);
                    Ok(())
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let request = post(
            "/api/v2/library/folders/7/update".into(),
            r#"{"requestId":"same-compact","compact":true}"#,
        );
        let (first, _) = send(&bench.router, request).await;
        let (second, _) = send(
            &bench.router,
            post(
                "/api/v2/library/folders/7/update".into(),
                r#"{"requestId":"same-compact","compact":true}"#,
            ),
        )
        .await;

        assert_eq!(first, StatusCode::OK);
        assert_eq!(second, StatusCode::OK);
        assert_eq!(
            edits.lock().unwrap().as_slice(),
            [LibraryEdit::CompactFolder { folder: 7 }]
        );
    }

    #[tokio::test]
    async fn a_thumbnail_is_a_jpeg_and_a_missing_one_says_so() {
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                thumbnail: Arc::new(|address| {
                    if address == media_domain::MediaAddress::new(3, 7).into() {
                        Ok(vec![0xff, 0xd8, 0xff, 0xd9])
                    } else {
                        Err("missing".to_owned())
                    }
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);

        let response = bench
            .router
            .clone()
            .oneshot(get("/api/v2/library/3/7/thumbnail".into()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "image/jpeg"
        );
        assert_eq!(
            response.headers()[axum::http::header::CACHE_CONTROL],
            "no-store"
        );
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            &[0xff, 0xd8, 0xff, 0xd9][..]
        );

        let (status, body) = send(&bench.router, get("/api/v2/library/3/8/thumbnail".into())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "thumbnail-not-found");
    }

    #[tokio::test]
    async fn retrying_a_thumbnail_uses_one_stable_id_and_replays_safely() {
        let regenerated = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&regenerated);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                regenerate_thumbnail: Arc::new(move |id| {
                    recorded.lock().unwrap().push(id);
                    Ok(())
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let id = uuid::Uuid::new_v4();
        for _ in 0..2 {
            let (status, _) = send(
                &bench.router,
                post(
                    format!("/api/v2/library/items/{id}/thumbnail/retry"),
                    r#"{"requestId":"same-thumbnail-retry"}"#,
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        assert_eq!(
            regenerated.lock().unwrap().as_slice(),
            &[media_domain::AssetId::from_uuid(id)]
        );
    }

    #[tokio::test]
    async fn custom_thumbnail_upload_is_bounded_typed_and_replay_safe() {
        let stored = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&stored);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                set_custom_thumbnail: Arc::new(move |id, bytes| {
                    recorded.lock().unwrap().push((id, bytes.to_vec()));
                    Ok(())
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let id = uuid::Uuid::new_v4();
        let boundary = "thumbnail-boundary";
        let payload = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"thumbnail.png\"\r\nContent-Type: image/png\r\n\r\nPNG-BYTES\r\n--{boundary}--\r\n"
        );
        for _ in 0..2 {
            let request = Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v2/library/items/{id}/thumbnail/upload?requestId=same-custom-thumbnail"
                ))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(payload.clone()))
                .unwrap();
            let response = bench.router.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        assert_eq!(stored.lock().unwrap().len(), 1);
        assert_eq!(stored.lock().unwrap()[0].1, b"PNG-BYTES");

        let boundary = "bad-thumbnail-boundary";
        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "/api/v2/library/items/{id}/thumbnail/upload?requestId=bad-thumbnail"
            ))
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"bad.txt\"\r\nContent-Type: text/plain\r\n\r\nNO\r\n--{boundary}--\r\n"
            )))
            .unwrap();
        let response = bench.router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    struct RecordedUpload {
        bytes: Arc<Mutex<Vec<u8>>>,
        finished: Arc<AtomicBool>,
    }

    impl UploadStream for RecordedUpload {
        fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.bytes.lock().unwrap().extend_from_slice(bytes);
            Ok(())
        }

        fn finish(self: Box<Self>) -> Result<String, String> {
            self.finished.store(true, Ordering::SeqCst);
            Ok("job-upload".to_owned())
        }
    }

    #[tokio::test]
    async fn upload_streams_one_file_and_returns_the_import_job() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let finished = Arc::new(AtomicBool::new(false));
        let received = Arc::new(Mutex::new(None));
        let diagnostics = Diagnostics {
            imports: Imports {
                available: true,
                ..Default::default()
            },
            library: LibraryAccess {
                begin_upload: {
                    let bytes = Arc::clone(&bytes);
                    let finished = Arc::clone(&finished);
                    let received = Arc::clone(&received);
                    Arc::new(move |address, name, filename, replace| {
                        *received.lock().unwrap() =
                            Some((address, name.to_owned(), filename.to_owned(), replace));
                        Ok(Box::new(RecordedUpload {
                            bytes: Arc::clone(&bytes),
                            finished: Arc::clone(&finished),
                        }))
                    })
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let boundary = "media-boundary";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"opening.mov\"\r\nContent-Type: video/quicktime\r\n\r\npixels\r\n--{boundary}--\r\n"
        );
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/v2/library/3/7/upload?requestId=upload&name=Opening&replace=true")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(axum::body::Body::from(body))
            .unwrap();
        let (status, answer) = send(&bench.router, request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(answer["jobId"], "job-upload");
        assert_eq!(&*bytes.lock().unwrap(), b"pixels");
        assert!(finished.load(Ordering::SeqCst));
        assert_eq!(
            *received.lock().unwrap(),
            Some((
                media_domain::MediaAddress::new(3, 7),
                "Opening".to_owned(),
                "opening.mov".to_owned(),
                true,
            ))
        );
    }
}
