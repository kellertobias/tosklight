use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};

use super::super::{ApiError, AppState, authenticate};
use super::{
    lifecycle_wire::lifecycle_snapshot, programming_ports::ServerProgrammingPorts,
    routes::http_context,
};

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/programmer-lifecycle/snapshot",
        get(get_lifecycle_snapshot),
    )
}

async fn get_lifecycle_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    let context = http_context(&session, None);
    let ports = ServerProgrammingPorts::new(&state, &session, "http_lifecycle", false);
    let snapshot = state
        .programming
        .lifecycle_snapshot(&context, &ports)
        .map_err(super::super::programming_interaction::programming_action_error)?;
    let response = lifecycle_snapshot(snapshot);
    Ok(json_with_etag(response.projection.revision, response))
}

fn json_with_etag<T: serde::Serialize>(revision: u64, body: T) -> Response {
    let mut response = Json(body).into_response();
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{revision}\""))
            .expect("a lifecycle revision always forms a valid ETag"),
    );
    response
}
