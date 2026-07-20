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
async fn unauthenticated_bootstrap_keeps_login_discovery_but_omits_programmers() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let discovery = app
        .clone()
        .oneshot(
            Request::get("/api/v1/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(discovery.status(), StatusCode::OK);
    let discovery = json(discovery).await;
    assert!(discovery["users"].as_array().unwrap().iter().any(|user| {
        user["name"] == "Operator" && user["enabled"] == true
    }));
    assert_eq!(discovery["active_programmers"], serde_json::json!([]));

    let (_, session_id) = login(&app, "Operator").await;
    let session_id = SessionId(Uuid::parse_str(&session_id).unwrap());
    state.programmers.set(
        session_id,
        light_core::FixtureId::new(),
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.5),
    );
    let populated = app
        .oneshot(
            Request::get("/api/v1/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(populated.status(), StatusCode::OK);
    assert_eq!(
        json(populated).await["active_programmers"],
        serde_json::json!([])
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn programmer_list_requires_authentication() {
    let (state, data_dir) = test_state();
    let app = router(state);
    for authorization in [None, Some("Bearer invalid-session")] {
        let mut request = Request::get("/api/v1/programmers");
        if let Some(authorization) = authorization {
            request = request.header(header::AUTHORIZATION, authorization);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn programmer_list_returns_only_same_user_session_rows() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (operator_token, first_operator) = login(&app, "Operator").await;
    let (_, second_operator) = login(&app, "Operator").await;
    state
        .desk
        .lock()
        .add_user("Foreign operator")
        .unwrap();
    let (foreign_token, foreign_session) = login(&app, "Foreign operator").await;
    let first_operator = SessionId(Uuid::parse_str(&first_operator).unwrap());
    let second_operator = SessionId(Uuid::parse_str(&second_operator).unwrap());
    let foreign_session = SessionId(Uuid::parse_str(&foreign_session).unwrap());
    let operator_fixture = light_core::FixtureId::new();
    let foreign_fixture = light_core::FixtureId::new();
    state.programmers.set(
        first_operator,
        operator_fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.5),
    );
    state.programmers.set(
        foreign_session,
        foreign_fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.25),
    );

    let operator_rows = authenticated_programmer_rows(&app, &operator_token).await;
    assert_eq!(operator_rows.len(), 2);
    let operator_user = state.sessions.read()[&first_operator].user.id;
    let mut operator_sessions = operator_rows
        .iter()
        .map(|row| row["session_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    operator_sessions.sort_unstable();
    let mut expected_sessions = vec![first_operator.0.to_string(), second_operator.0.to_string()];
    expected_sessions.sort_unstable();
    assert_eq!(operator_sessions, expected_sessions);
    assert!(operator_rows.iter().all(|row| {
        row["user_id"] == operator_user.0.to_string()
            && row["values"].as_array().unwrap().iter().any(|value| {
                value["fixture_id"] == operator_fixture.0.to_string()
            })
            && row["values"].as_array().unwrap().iter().all(|value| {
                value["fixture_id"] != foreign_fixture.0.to_string()
            })
    }));

    let foreign_rows = authenticated_programmer_rows(&app, &foreign_token).await;
    assert_eq!(foreign_rows.len(), 1);
    assert_eq!(
        foreign_rows[0]["session_id"],
        foreign_session.0.to_string()
    );
    assert_eq!(
        foreign_rows[0]["values"][0]["fixture_id"],
        foreign_fixture.0.to_string()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn authenticated_programmer_rows(
    app: &Router,
    token: &str,
) -> Vec<serde_json::Value> {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/programmers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await.as_array().unwrap().clone()
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
