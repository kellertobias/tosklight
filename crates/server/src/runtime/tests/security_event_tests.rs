#[tokio::test]
async fn bootstrap_does_not_relock_the_desk_store() {
    let (state, data_dir) = test_state();
    let response = tokio::time::timeout(
        Duration::from_secs(1),
        router(state).oneshot(
            Request::get("/api/v1/bootstrap")
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .expect("bootstrap must not deadlock")
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    let attributes = body["attribute_registry"].as_array().unwrap();
    assert_eq!(attributes.len(), ATTRIBUTE_REGISTRY.len());
    assert!(attributes.iter().any(|attribute| {
        attribute
            == &serde_json::json!({
                "id": "zoom",
                "label": "Zoom",
                "family": "beam",
                "value_type": "continuous",
                "default_unit": "deg"
            })
    }));
    assert!(
        !attributes
            .iter()
            .any(|attribute| attribute["id"] == "beam.zoom")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn optional_desk_token_guards_the_api_boundary() {
    let (mut state, data_dir) = test_state();
    state.desk_token = Some(Arc::from("shared-secret"));
    let app = router(state);
    let denied = app
        .clone()
        .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let allowed = app
        .clone()
        .oneshot(
            Request::get("/api/v1/health")
                .header("x-light-desk-token", "shared-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    let allowed_ws_boundary = app
        .clone()
        .oneshot(
            Request::get("/api/v1/health")
                .header(
                    header::SEC_WEBSOCKET_PROTOCOL,
                    "light.v1, light.desk.b64.c2hhcmVkLXNlY3JldA",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed_ws_boundary.status(), StatusCode::OK);
    let static_asset = app
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(static_asset.status(), StatusCode::OK);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn authenticated_shutdown_requests_orderly_server_cancellation() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let unauthorized = app
        .clone()
        .oneshot(
            Request::post("/api/v1/shutdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let (token, _) = login(&app, "Operator").await;
    let response = app
        .oneshot(
            Request::post("/api/v1/shutdown")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(state.shutdown.is_cancelled());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn emitted_events_have_strictly_sequential_revisions() {
    let (state, data_dir) = test_state();
    let mut receiver = state.events.subscribe();
    emit(&state, "first", serde_json::Value::Null);
    emit(&state, "second", serde_json::Value::Null);
    let first = receiver.try_recv().unwrap();
    let second = receiver.try_recv().unwrap();
    assert_eq!(first.revision + 1, second.revision);
    let audit = state.audit_events.lock();
    assert_eq!(audit.len(), 2);
    assert_eq!(audit[0].kind, "first");
    assert_eq!(audit[1].revision, second.revision);
    let _ = std::fs::remove_dir_all(data_dir);
}
