//! Router-level authentication coverage for the v2 event transport.

use super::*;

#[tokio::test]
async fn v2_snapshot_and_socket_protocols_use_the_live_session_authenticator() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut protocols = HeaderMap::new();
    protocols.insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("light.events.v2, light.token.{token}")
            .parse()
            .unwrap(),
    );
    let session = event_transport::authenticate_protocols(&state, &protocols).unwrap();
    assert_eq!(session.token, token);
    assert!(event_transport::authenticate_protocols(&state, &HeaderMap::new()).is_err());

    let denied = app
        .clone()
        .oneshot(
            Request::get("/api/v2/events/playback-snapshot")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let allowed = app
        .oneshot(
            Request::get("/api/v2/events/playback-snapshot")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    let snapshot = json(allowed).await;
    assert_eq!(snapshot["desk_id"], session.desk.id.to_string());
    assert_eq!(snapshot["cursor"]["sequence"], 0);
    assert_eq!(snapshot["playbacks"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(data_dir);
}
