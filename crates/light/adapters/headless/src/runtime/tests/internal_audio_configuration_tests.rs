use super::*;

#[tokio::test]
async fn media_library_update_is_live_replay_safe_and_desk_persistent() {
    let (state, data_dir) = test_state();
    let library = data_dir.join("audio-library");
    std::fs::create_dir_all(library.join("001")).unwrap();
    std::fs::write(library.join("001/001.wav"), []).unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let request = serde_json::json!({
        "request_id": "audio-library-root-1",
        "patch": {
            "internal_audio_library_roots": {
                "default": library.display().to_string()
            }
        }
    });
    let send = || {
        Request::post("/api/v2/configuration/update")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(request.to_string()))
            .unwrap()
    };

    let first = app.clone().oneshot(send()).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert!(!json(first).await["replayed"].as_bool().unwrap());
    let status = state.internal_audio.status();
    assert_eq!(status.libraries.len(), 1);
    assert_eq!(status.libraries[0].binding, "default");
    assert_eq!(status.libraries[0].entries, 1);

    let replay = app.clone().oneshot(send()).await.unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    assert!(json(replay).await["replayed"].as_bool().unwrap());

    let persisted = state
        .installation
        .setting("server_configuration")
        .unwrap()
        .unwrap();
    let persisted: serde_json::Value = serde_json::from_str(&persisted).unwrap();
    assert_eq!(
        persisted["internal_audio_library_roots"]["default"],
        library.display().to_string()
    );
    assert!(
        persisted.get("active_show_id").is_none(),
        "the absolute machine path must remain outside the portable show"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn media_library_update_rejects_relative_roots_without_mutating_runtime() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let response = app
        .oneshot(
            Request::post("/api/v2/configuration/update")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "audio-library-root-invalid",
                        "patch": {"internal_audio_library_roots": {"default": "Audio"}}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(state.internal_audio.status().libraries.is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}
