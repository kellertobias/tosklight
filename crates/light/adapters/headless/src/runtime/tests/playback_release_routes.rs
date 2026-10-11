use super::*;

#[tokio::test]
async fn running_panel_release_stops_exact_pool_owner_without_changing_peer_or_programmer() {
    assert_running_panel_release(false).await;
}

#[tokio::test]
async fn running_panel_release_remains_immediate_while_preload_is_armed() {
    assert_running_panel_release(true).await;
}

async fn assert_running_panel_release(preload: bool) {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_pool_cuelist_test_state(&state);
    let mut snapshot = (*state.output.snapshot()).clone();
    let assignments =
        std::mem::take(&mut Arc::make_mut(&mut snapshot.playback_pages)[0].assignments);
    Arc::make_mut(&mut snapshot.playback_pages)[0].assignments = assignments
        .into_iter()
        .map(|(position, definition)| (position + 8, definition))
        .collect();
    let mut peer = snapshot.cue_lists[0].clone();
    peer.id = light_core::CueListId::new();
    Arc::make_mut(&mut snapshot.playback_pages)[0]
        .assignments
        .get_mut(&10)
        .unwrap()
        .target = light_playback::PlaybackTarget::CueList {
        cue_list_id: peer.id,
    };
    Arc::make_mut(&mut snapshot.cue_lists).push(peer);
    state.output.replace_snapshot(snapshot).unwrap();

    for number in [9, 10] {
        let response = post_action(
            &app,
            Some(&token),
            session.desk.id,
            action_request(
                &format!("start-{number}"),
                number,
                serde_json::json!({"type":"on","pressed":true}),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    if preload {
        assert_preload_key(
            &app,
            &token,
            session.desk.id,
            "arm-preload-before-release",
            "preload_entered",
        )
        .await;
    }
    let before = serde_json::to_value(state.programming.get(session.id)).unwrap();
    assert!(
        state
            .output
            .playback_runtime_status_at(light_playback::PlaybackAddress::new(1, 10).unwrap())
            .is_some_and(|runtime| runtime.playback.enabled)
    );
    let response = post_action(
        &app,
        Some(&token),
        session.desk.id,
        action_request(
            "running-panel-release",
            9,
            serde_json::json!({"type":"release"}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["projection"]["identity"]["position"], 9);
    assert_eq!(response["outcome"]["status"], "applied");
    assert!(
        !state
            .output
            .playback_runtime_status_at(light_playback::PlaybackAddress::new(1, 9).unwrap())
            .is_some_and(|runtime| runtime.playback.enabled)
    );
    assert!(
        state
            .output
            .playback_runtime_status_at(light_playback::PlaybackAddress::new(1, 10).unwrap())
            .is_some_and(|runtime| runtime.playback.enabled)
    );
    assert_eq!(
        serde_json::to_value(state.programming.get(session.id)).unwrap(),
        before
    );
    assert_action_is_no_change(
        &app,
        &state,
        &token,
        session.desk.id,
        "repeat-running-panel-release",
        9,
        serde_json::json!({"type":"release"}),
    )
    .await;
    let _ = std::fs::remove_dir_all(data_dir);
}
