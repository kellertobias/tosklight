#[tokio::test]
async fn rest_session_show_and_revision_flow() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":"Operator"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let session = json(response).await;
    let token = session["token"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/shows")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    r#"{"name":"Tour","data_base64":null,"overwrite":false}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let show = json(response).await;
    let show_id = show["id"].as_str().unwrap();
    let uri = format!("/api/v1/shows/{show_id}/objects/group/front");
    let response = app
        .clone()
        .oneshot(
            Request::put(&uri)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from(r#"{"name":"Front","fixtures":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let conflict = app
        .clone()
        .oneshot(
            Request::put(&uri)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "0")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let objects = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/shows/{show_id}/objects/group"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json(objects).await.as_array().unwrap().len(), 1);
    assert!(
        std::fs::read_dir(data_dir.join("backups"))
            .unwrap()
            .next()
            .is_some()
    );
    let configuration=app.clone().oneshot(Request::put("/api/v1/configuration").header(header::CONTENT_TYPE,"application/json").header(header::AUTHORIZATION,format!("Bearer {token}")).body(Body::from(r#"{"frame_rate_hz":40,"output_bind_ip":"0.0.0.0","osc_bind":null,"art_timecode_bind":null,"backup_retention":5,"speed_groups_bpm":[101,102,103,104],"programmer_fade_millis":1250,"sequence_master_fade_millis":2500}"#)).unwrap()).await.unwrap();
    assert_eq!(configuration.status(), StatusCode::OK);
    assert_eq!(state.output_rate.load(Ordering::Relaxed), 40);
    assert_eq!(
        state.configuration.read().speed_groups_bpm,
        [101.0, 102.0, 103.0, 104.0, 15.0]
    );
    assert_eq!(state.configuration.read().programmer_fade_millis, 1_250);
    assert_eq!(
        state.configuration.read().sequence_master_fade_millis,
        2_500
    );
    let user = app
        .clone()
        .oneshot(
            Request::post("/api/v1/users")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"name":"Video","enabled":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user.status(), StatusCode::CREATED);
    assert!(authenticate_token(&state, "not-a-session-token").is_err());
    let _ = std::fs::remove_dir_all(data_dir);
}
