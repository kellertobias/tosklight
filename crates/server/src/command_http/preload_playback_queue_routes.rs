use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};
use uuid::Uuid;

use super::super::{ApiError, AppState, authenticate};
use super::{
    preload_playback_queue_wire, programming_ports::ServerProgrammingPorts, routes::http_context,
};

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/users/{user_id}/programmer-preload-playback-queue/snapshot",
        get(get_snapshot),
    )
}

async fn get_snapshot(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    let requested =
        Uuid::parse_str(&user_id).map_err(|_| ApiError::bad_request("user_id must be a UUID"))?;
    if session.user.id.0 != requested {
        return Err(ApiError::forbidden(
            "session is not authorized for this Programmer user",
        ));
    }
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
