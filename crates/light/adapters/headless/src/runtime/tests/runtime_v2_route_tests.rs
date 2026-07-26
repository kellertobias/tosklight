use super::*;

async fn get_json(app: &Router, path: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    (status, json(response).await)
}

#[tokio::test]
async fn runtime_v2_readiness_and_bootstrap_expose_the_current_contract() {
    let (state, data_dir) = test_state();
    let app = router(state);

    let (status, readiness) = get_json(&app, "/api/v2/readiness").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(readiness["status"], "ready");

    let (status, bootstrap) = get_json(&app, "/api/v2/bootstrap").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bootstrap["api_version"], "v2");
    assert!(
        bootstrap["users"]
            .as_array()
            .unwrap()
            .iter()
            .any(|user| { user["name"] == "Operator" && user["enabled"] == true })
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn runtime_v2_session_tolerates_unknown_fields_and_can_close_itself() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "Operator",
                        "future_client_hint": {"ignored": true}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let session = json(response).await;
    assert_eq!(session["user"]["name"], "Operator");
    assert!(session["client_id"].as_str().is_some());
    let session_id = session["session_id"].as_str().unwrap();
    let token = session["token"].as_str().unwrap();

    let close = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v2/sessions/{session_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(close.status(), StatusCode::NO_CONTENT);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn runtime_v2_session_rejects_a_malformed_known_field() {
    let (state, data_dir) = test_state();
    let response = router(state)
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":7}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let message = String::from_utf8_lossy(&body);
    assert!(message.contains("username") || message.contains("string"));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn runtime_v2_preserves_diagnostics_auth_and_recovery_reporting() {
    let (state, data_dir) = test_state();
    state
        .active_show
        .set_error(Some("malformed active show".into()));
    let app = router(state);

    let unauthorized = app
        .clone()
        .oneshot(
            Request::get("/api/v2/diagnostics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let (token, _) = login(&app, "Operator").await;
    let diagnostics = app
        .clone()
        .oneshot(
            Request::get("/api/v2/diagnostics")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(diagnostics.status(), StatusCode::OK);
    assert!(json(diagnostics).await["snapshot_revision"].is_number());

    let (_, readiness) = get_json(&app, "/api/v2/readiness").await;
    assert_eq!(readiness["recovery_mode"], true);
    assert_eq!(readiness["active_show_error"], "malformed active show");
    let (_, bootstrap) = get_json(&app, "/api/v2/bootstrap").await;
    assert_eq!(bootstrap["active_show_error"], "malformed active show");

    let _ = std::fs::remove_dir_all(data_dir);
}
