//! Router-level authentication coverage for the v2 event transport.

use super::*;

#[tokio::test]
async fn v2_socket_protocol_uses_live_auth_and_the_broad_snapshot_is_removed() {
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

    let removed = app
        .oneshot(
            Request::get("/api/v2/events/playback-snapshot")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(data_dir);
}
