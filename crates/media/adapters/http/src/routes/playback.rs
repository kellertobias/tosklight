//! Live playback settings.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{PlaybackView, UpdatePlayback};

/// The playback settings in force.
pub(super) async fn playback(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(PlaybackView::of(&state.configuration.load().playback))
}

/// Edits the playback settings.
///
/// The hold is read by every output on every frame, so the next clip switch uses an accepted
/// edit without a restart.
pub(super) async fn update_playback(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdatePlayback>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };

    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    configuration.playback = body
        .applied(&configuration.playback)
        .map_err(|error| ApiError::bad_request("playback-invalid", error.to_string()))?;

    let view = PlaybackView::of(&configuration.playback);
    edit::commit(&state, configuration, &body.request_id, &view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn the_switch_hold_is_reported_edited_and_bounded() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/playback".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["switchHoldMillis"], 500);
        assert_eq!(body["maximumSwitchHoldMillis"], 10_000);

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/playback/update".into(),
                r#"{"requestId":"hold","switchHoldMillis":1200}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["switchHoldMillis"], 1200);
        assert_eq!(
            bench
                .stored
                .lock()
                .unwrap()
                .last()
                .expect("a stored configuration")
                .playback
                .switch_hold_millis,
            1200,
            "an accepted hold is stored"
        );

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/playback/update".into(),
                r#"{"requestId":"overlong","switchHoldMillis":60000}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "playback-invalid");
        assert_eq!(
            bench.stored.lock().unwrap().len(),
            1,
            "a refused edit stores nothing"
        );
    }
}
