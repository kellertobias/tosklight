#[tokio::test]
async fn lifecycle_snapshot_is_authenticated_cursor_bound_and_content_safe() {
    let scenario = CommandHttpScenario::new().await;
    assert_eq!(
        scenario.lifecycle_snapshot(None).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let response = scenario.lifecycle_snapshot(Some(&scenario.token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let snapshot = json(response).await;
    assert_eq!(snapshot["cursor"]["sequence"], 1);
    assert_eq!(snapshot["projection"]["revision"], 1);
    let rows = snapshot["projection"]["programmers"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["user_id"], scenario.session.user.id.0.to_string());
    assert_eq!(row["normal_value_count"], 0);
    assert_eq!(row["preload_active"], false);
    assert_eq!(row["sessions"].as_array().unwrap().len(), 1);
    for forbidden in [
        "values",
        "group_values",
        "selected",
        "command_line",
        "preload_pending",
        "blind",
        "priority",
        "highlight",
        "undo",
        "redo",
        "transient_values",
        "preload_group_pending",
        "preload_group_active",
        "preload_playback_pending",
        "preload_capture_programmer",
        "preview",
        "active_context",
        "selection_expression",
    ] {
        assert!(row.get(forbidden).is_none(), "unexpected field {forbidden}");
    }
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn lifecycle_tracks_same_user_desks_foreign_users_disconnect_and_remove_once() {
    let scenario = CommandHttpScenario::new().await;
    let second_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Lifecycle second", "lifecycle-second")
        .unwrap();
    let (second_token, second_user) =
        login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, scenario.session.user.id.0);
    let second_session = scenario
        .state
        .sessions
        .read()
        .values()
        .find(|session| session.token == second_token)
        .unwrap()
        .id;
    let other = scenario
        .state
        .desk
        .lock()
        .add_user("Lifecycle other")
        .unwrap();
    let (other_token, other_user) =
        login_on_desk(&scenario, "Lifecycle other", scenario.session.desk.id).await;
    assert_eq!(other_user, other.id.0);

    let snapshot = json(scenario.lifecycle_snapshot(Some(&other_token)).await).await;
    assert_eq!(snapshot["projection"]["revision"], 3);
    let rows = snapshot["projection"]["programmers"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let own = rows
        .iter()
        .find(|row| row["user_id"] == scenario.session.user.id.0.to_string())
        .unwrap();
    assert_eq!(own["sessions"].as_array().unwrap().len(), 2);

    assert_eq!(
        close_session_request(&scenario, second_session, &second_token)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    let after_peer = json(scenario.lifecycle_snapshot(Some(&other_token)).await).await;
    assert_eq!(after_peer["projection"]["revision"], 4);
    let own = after_peer["projection"]["programmers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["user_id"] == scenario.session.user.id.0.to_string())
        .unwrap();
    assert_eq!(own["sessions"].as_array().unwrap().len(), 1);

    assert_eq!(
        close_session_request(&scenario, scenario.session.id, &scenario.token)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    let removed = json(scenario.lifecycle_snapshot(Some(&other_token)).await).await;
    assert_eq!(removed["projection"]["revision"], 5);
    assert_eq!(
        removed["projection"]["programmers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let lifecycle = replay_lifecycle_events(&scenario.state);
    assert_eq!(lifecycle.len(), 5);
    assert!(matches!(
        lifecycle.last().unwrap().payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::LifecycleChanged(
                light_application::ProgrammingLifecycleChange {
                    delta: light_application::ProgrammingLifecycleDelta::Remove { .. },
                    ..
                }
            )
        )
    ));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

async fn close_session_request(
    scenario: &CommandHttpScenario,
    session_id: SessionId,
    token: &str,
) -> Response {
    scenario
        .app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v1/sessions/{}", session_id.0))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

fn replay_lifecycle_events(state: &AppState) -> Vec<Arc<light_application::EventEnvelope>> {
    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_lifecycle());
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &filter)
    else {
        panic!("lifecycle events should remain replayable")
    };
    events
}
