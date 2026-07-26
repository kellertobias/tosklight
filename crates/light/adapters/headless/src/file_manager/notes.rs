use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use light_wire::v2::files::NativeNoteUpdateRequest;
use serde::Serialize;

use super::super::file_manager_support as support;
use super::super::{
    ApiError, AppState, authenticate, desk_management_v2::ReplayKey,
    show_objects_v2::validate_request_id,
};
use super::paths::{DirectoryQuery, confined, io_api_error, root};
use crate::tolerant_json::TolerantJson;

#[derive(Serialize)]
pub(super) struct NativeNote {
    root_id: String,
    path: String,
    supported: bool,
    note: Option<String>,
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
    TolerantJson(input): TolerantJson<NativeNoteUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&input.request_id)?;
    let key = ReplayKey::new(
        session.id,
        &format!("file-note-update:{root_id}"),
        &input.request_id,
    );
    let fingerprint =
        serde_json::to_value(&input).map_err(|error| ApiError::internal(error.to_string()))?;
    if let Some(value) = state
        .replay
        .lookup_desk_management(&key, &fingerprint)
        .await?
    {
        return Ok(Json(value));
    }
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
    let value = super::intent_value(
        NativeNote {
            root_id,
            path: input.path,
            supported: true,
            note: (!input.note.is_empty()).then_some(input.note),
        },
        &input.request_id,
        false,
    )?;
    state
        .replay
        .insert_desk_management(key, fingerprint, value.clone())
        .await;
    Ok(Json(value))
}
