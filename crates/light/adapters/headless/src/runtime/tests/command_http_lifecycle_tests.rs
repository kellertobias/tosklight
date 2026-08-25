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
    assert!(row.get("user_id").is_none(), "the one Programmer does not name a user");
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
async fn lifecycle_tracks_every_surface_of_the_one_programmer_and_removes_it_once() {
    let scenario = CommandHttpScenario::new().await;
    // Legacy desk and user records still exist in older installations. A session logging in on
    // one joins the desk's Programmer rather than opening a second one beside it.
    let second_desk = scenario
        .state
        .installation
        .add_desk("Lifecycle second")
        .unwrap();
    let second_token = login_on_desk(&scenario, second_desk.id).await;
    let second_session = scenario
        .state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == second_token)
        .unwrap()
        .id;
    let other_token = login_on_desk(&scenario, scenario.session.desk.id).await;

    let snapshot = json(scenario.lifecycle_snapshot(Some(&other_token)).await).await;
    assert_eq!(snapshot["projection"]["revision"], 3);
    let rows = snapshot["projection"]["programmers"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "three surfaces are one Programmer");
    assert_eq!(rows[0]["sessions"].as_array().unwrap().len(), 3);

    assert_eq!(
        close_session_request(&scenario, second_session, &second_token)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    let after_peer = json(scenario.lifecycle_snapshot(Some(&other_token)).await).await;
    assert_eq!(after_peer["projection"]["revision"], 4);
    let rows = after_peer["projection"]["programmers"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["sessions"].as_array().unwrap().len(), 2);

    assert_eq!(
        close_session_request(&scenario, scenario.session.id, &scenario.token)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    let after_main = json(scenario.lifecycle_snapshot(Some(&other_token)).await).await;
    assert_eq!(after_main["projection"]["revision"], 5);
    let rows = after_main["projection"]["programmers"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["sessions"].as_array().unwrap().len(), 1);

    let lifecycle = replay_lifecycle_events(&scenario.state);
    assert_eq!(lifecycle.len(), 5);
    // The Programmer is not removed while a surface still operates it.
    assert!(lifecycle.iter().all(|event| !matches!(
        event.payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::LifecycleChanged(
                light_application::ProgrammingLifecycleChange {
                    delta: light_application::ProgrammingLifecycleDelta::Remove { .. },
                    ..
                }
            )
        )
    )));
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
            Request::delete(format!("/api/v2/sessions/{}", session_id.0))
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
        state.events.replay(0, &filter)
    else {
        panic!("lifecycle events should remain replayable")
    };
    events
}
