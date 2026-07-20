#[tokio::test]
async fn preload_playback_queue_snapshot_is_exact_user_authenticated_and_narrow() {
    let scenario = CommandHttpScenario::new().await;

    let denied = scenario
        .preload_playback_queue_snapshot_for(scenario.session.user.id.0, None)
        .await;
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    let foreign = scenario
        .preload_playback_queue_snapshot_for(Uuid::new_v4(), Some(&scenario.token))
        .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);

    scenario.state.programmers.queue_preload_playback_action(
        scenario.session.id,
        7,
        light_programmer::PreloadPlaybackQueueAction::TemporaryOn,
        light_programmer::PreloadPlaybackQueueSurface::Osc,
    );
    let response = scenario.preload_playback_queue_snapshot().await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"0\"");
    let snapshot = json(response).await;
    assert_eq!(snapshot["projection"]["user_id"], scenario.session.user.id.0.to_string());
    assert_eq!(snapshot["projection"]["revision"], 0);
    assert_eq!(snapshot["projection"]["actions"], serde_json::json!([{
        "playback_number": 7,
        "action": "temporary_on",
        "surface": "osc",
    }]));
    assert!(snapshot.get("programmer").is_none());
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}
