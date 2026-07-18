use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};

use super::super::file_manager_support as support;
use super::super::{ApiError, AppState, authenticate};
use super::paths::{DirectoryQuery, confined, io_api_error, root};

#[derive(Serialize)]
pub(super) struct NativeNote {
    root_id: String,
    path: String,
    supported: bool,
    note: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SaveNativeNote {
    path: String,
    note: String,
}

pub(super) async fn read_note(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<DirectoryQuery>,
    headers: HeaderMap,
) -> Result<Json<NativeNote>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &query.path, false)?;
    let supported = support::native_notes_supported(&path);
    let note = supported
        .then(|| support::read_native_note(&path).map_err(io_api_error))
        .transpose()?
        .flatten();
    Ok(Json(NativeNote {
        root_id,
        path: query.path,
        supported,
        note,
    }))
}

pub(super) async fn save_note(
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
