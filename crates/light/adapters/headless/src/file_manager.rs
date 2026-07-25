#[path = "file_manager/browse.rs"]
mod browse;
#[path = "file_manager/input_context.rs"]
mod input_context;
#[path = "file_manager/notes.rs"]
mod notes;
#[path = "file_manager/operations/mod.rs"]
mod operations;
#[path = "file_manager/paths.rs"]
mod paths;
#[path = "file_manager/streaming.rs"]
mod streaming;
#[path = "file_manager/text.rs"]
mod text;
#[path = "file_manager/thumbnail.rs"]
mod thumbnail;

use axum::{
    Router,
    routing::{get, post},
};
use tokio::sync::Mutex as AsyncMutex;

use super::AppState;

#[allow(unused_imports)]
pub(crate) use input_context::{
    FileInputAction, FileInputContext, release_session_input, route_osc_input,
    try_claim_input_context,
};
pub(crate) use paths::ConfiguredRoot;

static FILE_MUTATION_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/files/roots", get(browse::roots))
        .route(
            "/api/v2/files/input-context",
            get(input_context::input_context),
        )
        .route(
            "/api/v2/files/input-context/claim",
            post(input_context::claim_input_context),
        )
        .route(
            "/api/v2/files/input-context/release",
            post(input_context::release_input_context),
        )
        .route("/api/v2/files/{root_id}/entries", get(browse::entries))
        .route("/api/v2/files/{root_id}/metadata", get(browse::metadata))
        .route("/api/v2/files/{root_id}/content", get(streaming::content))
        .route(
            "/api/v2/files/{root_id}/stream-ticket",
            post(streaming::stream_ticket),
        )
        .route(
            "/api/v2/files/{root_id}/thumbnail",
            get(thumbnail::thumbnail),
        )
        .route("/api/v2/files/{root_id}/notes", get(notes::read_note))
        .route(
            "/api/v2/files/{root_id}/notes/update",
            post(notes::save_note),
        )
        .route("/api/v2/files/{root_id}/text", get(text::read_text))
        .route("/api/v2/files/{root_id}/text/update", post(text::save_text))
        .route(
            "/api/v2/files/{root_id}/operations",
            post(operations::operate),
        )
}

fn intent_value<T: serde::Serialize>(
    value: T,
    request_id: &str,
    replayed: bool,
) -> Result<serde_json::Value, super::ApiError> {
    let mut value = serde_json::to_value(value)
        .map_err(|error| super::ApiError::internal(error.to_string()))?;
    if let Some(object) = value.as_object_mut() {
        object.insert("request_id".into(), request_id.into());
        object.insert("replayed".into(), replayed.into());
    }
    Ok(value)
}

#[cfg(test)]
#[path = "file_manager/tests.rs"]
mod tests;
