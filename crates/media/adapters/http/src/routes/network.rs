//! Network settings.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{NetworkView, UpdateNetwork};

/// What this server listens on, and what it reaches out to.
pub(super) async fn network(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(NetworkView::of(&state.configuration.load().network))
}

/// Edits the network settings.
///
/// Every address is validated before anything is written, so a refused edit leaves both the stored
/// configuration and the running listeners exactly as they were. An accepted one is stored and
/// takes effect on the next start — the view says so, because a socket is bound once.
pub(super) async fn update_network(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdateNetwork>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    configuration.network = body
        .applied(&configuration.network)
        .map_err(|error| ApiError::bad_request("network-invalid", error.to_string()))?;

    let view = NetworkView::of(&configuration.network);
    edit::commit(&state, configuration, &body.request_id, &view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn the_settings_report_the_stored_and_the_bound_addresses() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/network".into())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["stored"]["artNetListen"], "0.0.0.0:6454");
        assert_eq!(body["resolved"]["artNetListen"], "0.0.0.0:6454");
        assert_eq!(body["stored"]["httpListen"], "127.0.0.1:8080");
        assert_eq!(body["citpAdvertisedPort"], 4811);
        assert_eq!(body["sameComputerPreset"], false);
        assert_eq!(
            body["takesEffectOnRestart"], true,
            "an operator must not think a rebind already happened"
        );
    }

    #[tokio::test]
    async fn a_listen_address_is_stored_and_the_preset_leaves_it_alone() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/network/update".into(),
                r#"{"requestId":"a","artNetListen":"192.168.1.40:6454","sameComputerPreset":true}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["stored"]["artNetListen"], "192.168.1.40:6454");
        assert_eq!(body["resolved"]["artNetListen"], "127.0.0.1:6454");

        let stored = bench.stored.lock().unwrap();
        assert_eq!(stored.len(), 1, "the edit was written, not just answered");
        assert_eq!(
            stored[0].network.art_net_listen,
            "192.168.1.40:6454".parse().unwrap()
        );
        assert!(stored[0].network.same_computer_preset);
    }

    #[tokio::test]
    async fn a_destination_is_separate_from_every_listen_address() {
        let bench = bench();
        let (_, body) = send(
            &bench.router,
            post(
                "/api/v2/network/update".into(),
                r#"{"requestId":"a","speedGroupEndpoint":"192.168.1.9:9000"}"#,
            ),
        )
        .await;

        assert_eq!(body["stored"]["speedGroupEndpoint"], "192.168.1.9:9000");
        assert_eq!(
            body["stored"]["artNetListen"], "0.0.0.0:6454",
            "setting a destination moved no listener"
        );

        let (_, cleared) = send(
            &bench.router,
            post(
                "/api/v2/network/update".into(),
                r#"{"requestId":"b","speedGroupEndpoint":null}"#,
            ),
        )
        .await;
        assert!(cleared["stored"]["speedGroupEndpoint"].is_null());
    }

    #[tokio::test]
    async fn an_unusable_address_is_refused_and_nothing_is_written() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/network/update".into(),
                r#"{"requestId":"a","citpListen":"the lighting network"}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "network-invalid");
        assert!(body["message"].as_str().unwrap().contains("citpListen"));
        assert!(bench.stored.lock().unwrap().is_empty());
        assert_eq!(
            bench.configuration.load().network.citp_listen.port(),
            4811,
            "the running configuration is untouched"
        );
    }

    #[tokio::test]
    async fn an_edit_the_disk_refused_is_not_applied() {
        let bench = bench();
        bench
            .refuse
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/network/update".into(),
                r#"{"requestId":"a","httpListen":"0.0.0.0:9090"}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "configuration-not-written");
        assert_eq!(
            bench.configuration.load().network.http_listen.port(),
            8080,
            "a change that was not saved must not be live either"
        );
    }

    #[tokio::test]
    async fn a_retried_edit_is_answered_rather_than_repeated() {
        let bench = bench();
        let body = r#"{"requestId":"same","sacnListen":"10.0.0.5:5568"}"#;

        let (_, first) = send(&bench.router, post("/api/v2/network/update".into(), body)).await;
        let (status, second) =
            send(&bench.router, post("/api/v2/network/update".into(), body)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(first, second);
        assert_eq!(bench.stored.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn an_edit_without_a_request_id_is_refused() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/network/update".into(),
                r#"{"requestId":"","httpListen":"0.0.0.0:9090"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "missing-request-id");
    }
}
