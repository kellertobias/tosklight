use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};

use super::super::{ApiError, AppState, authenticate};
use super::{
    preload_playback_queue_wire, programming_ports::ServerProgrammingPorts, routes::http_context,
};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        // The desk's Programmer. There is one, so its routes do not name whose it is.
        .route(
            "/api/v2/programmer/preload-playback-queue/snapshot",
            get(get_snapshot),
        )
}

async fn get_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    let context = http_context(&session, None);
    let ports = ServerProgrammingPorts::new(&state, &session, "http_preload_playback_queue", false);
    let snapshot = state
        .programming
        .preload_playback_queue_snapshot(&context, &ports)
        .map_err(super::super::programming_interaction::programming_action_error)?;
    let response = preload_playback_queue_wire::snapshot(snapshot);
    Ok(json_with_etag(response.projection.revision, response))
}

fn json_with_etag<T: serde::Serialize>(revision: u64, body: T) -> Response {
    let mut response = Json(body).into_response();
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{revision}\""))
            .expect("a queue revision always forms a valid ETag"),
    );
    response
}
