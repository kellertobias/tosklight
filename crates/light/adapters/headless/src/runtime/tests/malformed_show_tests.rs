#[tokio::test]
async fn malformed_show_upload_is_rejected_before_library_insert() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let login = app
        .clone()
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":"Operator"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let token = json(login).await["token"].as_str().unwrap().to_owned();
    let encoded=STANDARD.encode(b"not sqlite but made long enough to pass an old superficial size check; this payload is deliberately invalid and should never enter the library........................................");
    let response = app
        .clone()
        .oneshot(show_action_request(
            &token,
            serde_json::json!({
                "type": "create",
                "name": "Bad",
                "data_base64": encoded,
                "overwrite": false
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let shows = app
        .oneshot(show_snapshot_request(&token))
        .await
        .unwrap();
    assert!(json(shows).await["shows"].as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}
