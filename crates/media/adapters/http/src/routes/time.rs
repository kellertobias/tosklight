//! The server's UTC offset.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{TimeView, UpdateTime};

/// What this server's wall clock reads.
pub(super) async fn time(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(TimeView::of(&state.configuration.load().time))
}

/// Edits the server's UTC offset.
///
/// The offset is stored configuration and takes effect on the next rendered frame, so a clock on
/// screen follows an accepted edit without a restart.
pub(super) async fn update_time(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdateTime>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    configuration.time = body
        .applied(&configuration.time)
        .map_err(|error| ApiError::bad_request("time-invalid", error.to_string()))?;

    let view = TimeView::of(&configuration.time);
    edit::commit(&state, configuration, &body.request_id, &view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn the_offset_is_reported_edited_and_bounded() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/time".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["utcOffsetMinutes"], 0);
        assert_eq!(body["maximumUtcOffsetMinutes"], 840);

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/time/update".into(),
                r#"{"requestId":"offset","utcOffsetMinutes":345}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["utcOffsetMinutes"], 345);
        assert_eq!(
            bench
                .stored
                .lock()
                .unwrap()
                .last()
                .expect("a stored configuration")
                .time
                .utc_offset_minutes,
            345,
            "an accepted offset is stored"
        );

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/time/update".into(),
                r#"{"requestId":"impossible","utcOffsetMinutes":1000}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "time-invalid");
        assert_eq!(
            bench.stored.lock().unwrap().len(),
            1,
            "a refused edit stores nothing"
        );
    }
}
