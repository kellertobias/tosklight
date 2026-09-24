//! The 3D model library.
//!
//! Reads list the assigned slots. Assigning an imported model is an upload of one self-contained
//! `.glb`; renaming, clearing, and assigning a built-in model are object-intent edits. The runtime's model store validates, imports,
//! and stores the file — this adapter carries bytes and intent, and records the accepted slot in
//! the configuration through the same edit order every other stored change follows.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;
use media_domain::ModelEntry;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{ClearedModelSlotView, ModelSlotView, UpdateModelSlot};

/// The largest `.glb` upload accepted. The model store enforces the same limit.
pub(super) const MAX_MODEL_UPLOAD_BYTES: usize = 256 * 1024 * 1024;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UploadQuery {
    request_id: String,
    /// The slot's name. Defaults to the existing name, then to the uploaded file's name.
    #[serde(default)]
    name: Option<String>,
}

fn validate_slot(slot: u8) -> Result<(), ApiError> {
    if slot == 0 {
        return Err(ApiError::bad_request(
            "reserved-model-slot",
            "model slot 0 is the fixed \"draw flat\" value and cannot hold a model",
        ));
    }
    Ok(())
}

fn failure_for(failures: &[(u8, String)], slot: u8) -> Option<&str> {
    failures
        .iter()
        .find(|(failed, _)| *failed == slot)
        .map(|(_, reason)| reason.as_str())
}

/// A picture of the model in one slot, drawn from the mesh the outputs draw. An empty slot, or one
/// whose model cannot be loaded, has none; the chooser then shows its plain card.
pub(super) async fn preview(
    State(state): State<ApiState>,
    Path(slot): Path<u8>,
) -> Result<Response, ApiError> {
    let draw = state.diagnostics.models.preview.clone();
    let picture = tokio::task::spawn_blocking(move || draw(slot))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| {
            ApiError::not_found(
                "model-preview-not-found",
                "this model slot has no model that can be pictured",
            )
        })?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/png"),
            (axum::http::header::CACHE_CONTROL, "no-store"),
        ],
        picture.as_ref().clone(),
    )
        .into_response())
}

pub(super) async fn models(State(state): State<ApiState>) -> impl IntoResponse {
    let configuration = state.configuration.load();
    let failures = (state.diagnostics.models.failures)();
    axum::Json(
        configuration
            .models
            .entries
            .iter()
            .map(|entry| ModelSlotView::of(entry, failure_for(&failures, entry.slot)))
            .collect::<Vec<_>>(),
    )
}

pub(super) async fn upload_model(
    State(state): State<ApiState>,
    Path(slot): Path<u8>,
    Query(query): Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    validate_slot(slot)?;
    let _upload = match edit::begin_upload(&state, &query.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let mut upload: Option<(Option<String>, Vec<u8>)> = None;
    while let Some(mut field) = multipart.next_field().await.map_err(|_| {
        ApiError::bad_request(
            "invalid-model-upload",
            "the multipart upload could not be read",
        )
    })? {
        if field.name() != Some("file") {
            continue;
        }
        if upload.is_some() {
            return Err(ApiError::bad_request(
                "multiple-models",
                "upload exactly one .glb file per model slot",
            ));
        }
        let filename = field.file_name().map(str::to_owned);
        let mut bytes = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(|_| {
            ApiError::bad_request(
                "invalid-model-upload",
                "the uploaded model could not be read; try the upload again",
            )
        })? {
            if bytes.len().saturating_add(chunk.len()) > MAX_MODEL_UPLOAD_BYTES {
                return Err(ApiError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "model-too-large",
                    "3D models may be at most 256 MiB; reduce the mesh or texture size and export again",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        upload = Some((filename, bytes));
    }
    let (filename, bytes) = upload.ok_or_else(|| {
        ApiError::bad_request("missing-model", "the multipart body has no file field")
    })?;
    if bytes.is_empty() {
        return Err(ApiError::bad_request(
            "empty-model",
            "the uploaded model is empty",
        ));
    }

    // Parsing a large mesh is real work; it must not stall the other routes.
    let import = state.diagnostics.models.import.clone();
    let imported = tokio::task::spawn_blocking(move || import(slot, &bytes))
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model-import-failed",
                "the model import stopped unexpectedly; try the upload again",
            )
        })?
        .map_err(|rejection| {
            let status = if rejection.code == "model-not-stored" {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            ApiError::new(status, rejection.code, rejection.message)
        })?;

    // The file is stored; record the slot inside the configuration transaction so a concurrent
    // edit cannot overwrite it with an older document.
    let _transaction = state.replays.transaction().await;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let replaced = configuration.models.resolve(slot).cloned();
    let name = query
        .name
        .filter(|name| !name.trim().is_empty())
        // A built-in model's name describes that shape, not the uploaded one.
        .or_else(|| {
            replaced
                .as_ref()
                .filter(|entry| entry.builtin.is_none())
                .map(|entry| entry.name.clone())
        })
        .or_else(|| {
            filename
                .as_deref()
                .map(display_name)
                .filter(|name| !name.is_empty())
        })
        .unwrap_or_else(|| format!("Model {slot}"));
    configuration
        .models
        .assign(ModelEntry {
            slot,
            name,
            file: imported.file,
            builtin: None,
            vertices: imported.vertices,
            triangles: imported.triangles,
        })
        .map_err(|error| ApiError::bad_request("model-not-assigned", error.to_string()))?;
    let view = ModelSlotView::of(
        configuration
            .models
            .resolve(slot)
            .expect("the assigned model is immediately addressable"),
        None,
    );
    edit::commit(&state, configuration, &query.request_id, &view)
}

/// `Stage Cube.glb` becomes `Stage Cube`.
fn display_name(filename: &str) -> String {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let stem = match base.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => base,
    };
    stem.trim().chars().take(80).collect()
}

pub(super) async fn update_model(
    State(state): State<ApiState>,
    Path(slot): Path<u8>,
    TolerantJson(body): TolerantJson<UpdateModelSlot>,
) -> Result<Response, ApiError> {
    validate_slot(slot)?;
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());

    if body.clear.unwrap_or(false) {
        let removed = configuration.models.remove(slot);
        let response = edit::commit(
            &state,
            configuration,
            &body.request_id,
            &ClearedModelSlotView {
                slot,
                assigned: false,
            },
        )?;
        // The file goes only once the slot no longer points at it.
        if let Some(entry) = removed.filter(|entry| entry.builtin.is_none())
            && let Err(detail) = (state.diagnostics.models.remove)(&entry.file)
        {
            tracing::warn!(slot, %detail, "a cleared model's file could not be removed");
        }
        return Ok(response);
    }

    if let Some(builtin) = body.builtin {
        return assign_builtin(&state, configuration, slot, builtin.into(), body);
    }

    let Some(name) = body.name else {
        return Err(ApiError::bad_request(
            "nothing-to-update",
            "send a name to rename the slot, builtin to put a built-in model in it, or clear to \
             empty it; upload a .glb to assign an imported model",
        ));
    };
    let renamed = configuration.models.rename(slot, &name).map_err(|_| {
        ApiError::bad_request(
            "empty-name",
            "a model needs a name an operator can find it by",
        )
    })?;
    if !renamed {
        return Err(ApiError::not_found(
            "model-slot-empty",
            format!("model slot {slot} is empty; upload a .glb to assign it"),
        ));
    }
    let failures = (state.diagnostics.models.failures)();
    let view = ModelSlotView::of(
        configuration
            .models
            .resolve(slot)
            .expect("a renamed slot is assigned"),
        failure_for(&failures, slot),
    );
    edit::commit(&state, configuration, &body.request_id, &view)
}

/// Puts a built-in model in a slot. An imported file the slot held is deleted once the stored
/// configuration no longer points at it.
fn assign_builtin(
    state: &ApiState,
    mut configuration: MediaConfiguration,
    slot: u8,
    builtin: media_domain::BuiltinModel,
    body: UpdateModelSlot,
) -> Result<Response, ApiError> {
    let mut entry = ModelEntry::builtin(slot, builtin);
    if let Some(name) = body.name.filter(|name| !name.trim().is_empty()) {
        entry.name = name;
    }
    let replaced = configuration.models.resolve(slot).cloned();
    configuration
        .models
        .assign(entry)
        .map_err(|error| ApiError::bad_request("model-not-assigned", error.to_string()))?;
    let view = ModelSlotView::of(
        configuration
            .models
            .resolve(slot)
            .expect("the assigned model is immediately addressable"),
        None,
    );
    let response = edit::commit(state, configuration, &body.request_id, &view)?;
    if let Some(replaced) = replaced.filter(|entry| entry.builtin.is_none())
        && let Err(detail) = (state.diagnostics.models.remove)(&replaced.file)
    {
        tracing::warn!(slot, %detail, "a replaced model's file could not be removed");
    }
    Ok(response)
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
