#[tokio::test]
async fn bootstrap_does_not_relock_the_desk_store() {
    let (state, data_dir) = test_state();
    let response = tokio::time::timeout(
        Duration::from_secs(1),
        router(state).oneshot(
            Request::get("/api/v2/bootstrap")
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
    let recommended = light_core::AttributeConfiguration::recommended();
    let expected_visible_attributes = light_core::ATTRIBUTE_REGISTRY
        .iter()
        .filter(|descriptor| {
            recommended
                .attribute_placement_for(&light_core::AttributeKey(descriptor.id.into()))
                .is_some()
        })
        .count();
    assert_eq!(attributes.len(), expected_visible_attributes);
    let zoom = attributes
        .iter()
        .find(|attribute| attribute["id"] == "zoom")
        .expect("canonical Zoom attribute");
    assert_eq!(zoom["label"], "Zoom");
    assert_eq!(zoom["family"], "focus");
    assert_eq!(zoom["value_type"], "continuous");
    assert_eq!(zoom["default_unit"], "deg");
    assert_eq!(zoom["encoder_group"], "focus");
    assert_eq!(zoom["encoder_page"], 1);
    assert_eq!(zoom["encoder_slot"], 2);
    assert_eq!(zoom["built_in"], true);
    assert_eq!(zoom["retired"], false);
    assert_eq!(zoom["activation_group_id"], "zoom");
    assert!(
        !attributes
            .iter()
            .any(|attribute| attribute["id"] == "beam.zoom")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn unauthenticated_bootstrap_keeps_desk_discovery_but_omits_programmers() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let discovery = app
        .clone()
        .oneshot(
            Request::get("/api/v2/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(discovery.status(), StatusCode::OK);
    let discovery = json(discovery).await;
    assert!(discovery.get("users").is_none());
    assert!(discovery["desk"]["id"].as_str().is_some());
    assert_eq!(discovery["active_programmers"], serde_json::json!([]));

    let (_, session_id) = login(&app, "Operator").await;
    let session_id = SessionId(Uuid::parse_str(&session_id).unwrap());
    state.programming.set(
        session_id,
        light_core::FixtureId::new(),
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.5),
    );
    let populated = app
        .oneshot(
            Request::get("/api/v2/bootstrap")
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
        let mut request = Request::get("/api/v2/programmers");
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
async fn programmer_list_returns_every_session_of_the_one_programmer() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (operator_token, first_operator) = login(&app, "Operator").await;
    let (_, second_operator) = login(&app, "Operator").await;
    let (foreign_token, foreign_session) = login(&app, "Foreign operator").await;
    let first_operator = SessionId(Uuid::parse_str(&first_operator).unwrap());
    let second_operator = SessionId(Uuid::parse_str(&second_operator).unwrap());
    let foreign_session = SessionId(Uuid::parse_str(&foreign_session).unwrap());
    let operator_fixture = light_core::FixtureId::new();
    let foreign_fixture = light_core::FixtureId::new();
    state.programming.set(
        first_operator,
        operator_fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.5),
    );
    state.programming.set(
        foreign_session,
        foreign_fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.25),
    );

    // Three sessions, one Programmer: every session of the desk is listed on its one row.
    let operator_rows = authenticated_programmer_rows(&app, &operator_token).await;
    assert_eq!(operator_rows.len(), 3);
    let mut operator_sessions = operator_rows
        .iter()
        .map(|row| row["session_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    operator_sessions.sort_unstable();
    let mut expected_sessions = vec![
        first_operator.0.to_string(),
        second_operator.0.to_string(),
        foreign_session.0.to_string(),
    ];
    expected_sessions.sort_unstable();
    assert_eq!(operator_sessions, expected_sessions);
    assert!(operator_rows.iter().all(|row| {
        row["values"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value["fixture_id"] == operator_fixture.0.to_string())
            && row["values"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value["fixture_id"] == foreign_fixture.0.to_string())
    }));

    // Another session reads the same Programmer, values and all.
    let legacy_rows = authenticated_programmer_rows(&app, &foreign_token).await;
    assert_eq!(legacy_rows.len(), 3);
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn authenticated_programmer_rows(
    app: &Router,
    token: &str,
) -> Vec<serde_json::Value> {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/programmers")
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
    state.installation.set_desk_token("shared-secret");
    let app = router(state);
    let denied = app
        .clone()
        .oneshot(Request::get("/api/v2/readiness").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let allowed = app
        .clone()
        .oneshot(
            Request::get("/api/v2/readiness")
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
            Request::get("/api/v2/readiness")
                .header(
                    header::SEC_WEBSOCKET_PROTOCOL,
                    "light.events.v2, light.desk.b64.c2hhcmVkLXNlY3JldA",
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
            Request::post("/api/v2/shutdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let (token, _) = login(&app, "Operator").await;
    let response = app
        .oneshot(
            Request::post("/api/v2/shutdown")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(state.lifecycle.is_shutdown_requested());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn emitted_events_have_strictly_sequential_revisions() {
    let (state, data_dir) = test_state();
    let before_application_events = state.events.latest_sequence();
    emit(&state, "first", serde_json::Value::Null);
    emit(&state, "second", serde_json::Value::Null);
    let audit = state.events.audit_events();
    assert_eq!(audit.len(), 2);
    assert_eq!(audit[0].kind, "first");
    assert_eq!(audit[0].revision + 1, audit[1].revision);
    assert_eq!(
        state.events.latest_sequence(),
        before_application_events,
        "audit-only rows must not enter the application event stream"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
