//! The directory used for the media library on the next server start.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{LibrarySettingsView, UpdateLibrarySettings};

pub(super) async fn settings(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(LibrarySettingsView::of(
        &state.configuration.load().library,
        &state.active_configuration.library,
    ))
}

/// Stores a new library directory without moving media or changing the running library.
pub(super) async fn update_settings(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdateLibrarySettings>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };

    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    configuration.library = body
        .applied(&configuration.library)
        .map_err(|error| ApiError::bad_request("library-directory-invalid", error.to_string()))?;
    let view = LibrarySettingsView::of(&configuration.library, &state.active_configuration.library);
    edit::commit(&state, configuration, &body.request_id, &view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn the_library_directory_is_reported_stored_and_restart_explicit() {
        let bench = bench();
        let (status, initial) = send(&bench.router, get("/api/v2/library/settings".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(initial["storedDirectory"], "/tmp/tosklight-media/library");
        assert_eq!(initial["pendingRestart"], false);
        assert_eq!(initial["takesEffectOnRestart"], true);

        let (status, saved) = send(
            &bench.router,
            post(
                "/api/v2/library/settings/update".into(),
                r#"{"requestId":"move","directory":"/srv/show-media"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(saved["storedDirectory"], "/srv/show-media");
        assert_eq!(saved["activeDirectory"], "/tmp/tosklight-media/library");
        assert_eq!(saved["pendingRestart"], true);
        assert_eq!(
            bench.stored.lock().unwrap()[0].library.root,
            std::path::PathBuf::from("/srv/show-media")
        );
    }

    #[tokio::test]
    async fn an_empty_directory_and_an_unstored_change_are_not_applied() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/library/settings/update".into(),
                r#"{"requestId":"empty","directory":"  "}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "library-directory-invalid");
        assert!(bench.stored.lock().unwrap().is_empty());

        bench
            .refuse
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/library/settings/update".into(),
                r#"{"requestId":"refused","directory":"/srv/show-media"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "configuration-not-written");
        assert_eq!(
            bench.configuration.load().library.root,
            std::path::PathBuf::from("/tmp/tosklight-media/library")
        );
    }
}
