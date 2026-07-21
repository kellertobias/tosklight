async fn install_lifecycle_show(
    scenario: &CommandHttpScenario,
    name: &str,
) -> (Uuid, u64, light_core::FixtureId) {
    let show_id = Uuid::parse_str(&scenario.create_and_open_show(name).await).unwrap();
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    let mut snapshot = preload_atomicity_test_snapshot();
    snapshot.fixtures = vec![fixture];
    snapshot.groups[0].fixtures = vec![fixture_id];
    snapshot.revision = 9_001;
    scenario.state.engine.replace_snapshot(snapshot).unwrap();
    let show = scenario.state.active_show.read().clone().unwrap();
    let revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    assert_ne!(revision, scenario.state.engine.snapshot().revision);
    (show_id, revision, fixture_id)
}

fn lifecycle_request(
    request_id: &str,
    capture: u64,
    values: u64,
    queue: u64,
    selection: u64,
    action: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "expected_capture_mode_revision":capture,
        "expected_values_revision":values,
        "expected_queue_revision":queue,
        "expected_selection_revision":selection,
        "action":action,
    })
}

fn lifecycle_go(
    request_id: &str,
    capture: u64,
    values: u64,
    queue: u64,
    selection: u64,
    show_id: Uuid,
    show_revision: u64,
    cursor: u64,
) -> serde_json::Value {
    lifecycle_request(
        request_id,
        capture,
        values,
        queue,
        selection,
        serde_json::json!({
            "type":"go",
            "show_id":show_id,
            "expected_show_revision":show_revision,
            "expected_playback_event_sequence":cursor,
        }),
    )
}

fn playback_request(request_id: &str, number: u16, action: &str) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "address":{"kind":"playback","playback_number":number},
        "action":{"type":action,"pressed":true},
        "surface":"physical",
    })
}

fn application_event_count(state: &AppState, object: light_application::EventObject) -> usize {
    let filter = light_application::EventFilter::default().with_object(object);
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &filter)
    else {
        panic!("focused event history should remain replayable")
    };
    events.len()
}

fn audit_count(state: &AppState, kind: &str) -> usize {
    state
        .audit_events
        .lock()
        .iter()
        .filter(|event| event.kind == kind)
        .count()
}

#[tokio::test]
async fn preload_lifecycle_http_is_sparse_replay_safe_and_shared_across_user_desks() {
    let scenario = CommandHttpScenario::new().await;
    let (show_id, show_revision, fixture) =
        install_lifecycle_show(&scenario, "Typed Preload lifecycle").await;
    let user_id = scenario.session.user.id.0;
    let enter = lifecycle_request(
        "preload-enter-http",
        0,
        0,
        0,
        0,
        serde_json::json!({"type":"enter"}),
    );
    let entered = json(scenario.preload_lifecycle_action(enter.clone()).await).await;
    assert_eq!(entered["status"], "changed");
    assert_eq!(entered["active"], false);
    assert_eq!(entered["capture_mode"]["blind"], true);
    assert_eq!(entered["capture_mode"]["revision"], 1);
    assert!(entered.get("values_projection").is_none());
    assert!(entered.get("queue_projection").is_none());
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::programming_capture_mode(user_id),
        ),
        1
    );

    // Replay is resolved before the now-stale selection precondition.
    scenario.state.programmers.select(scenario.session.id, [fixture]);
    let cursor = scenario.state.application_events.latest_sequence();
    let replay = json(scenario.preload_lifecycle_action(enter).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["selection_revision"], 0);
    assert_eq!(scenario.state.application_events.latest_sequence(), cursor);

    let second_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Preload lifecycle peer", "preload-lifecycle-peer")
        .unwrap();
    let (second_token, second_user) =
        login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, user_id);
    let peer_enter = lifecycle_request(
        "preload-enter-peer",
        1,
        0,
        0,
        0,
        serde_json::json!({"type":"enter"}),
    );
    let peer = json(
        scenario
            .preload_lifecycle_action_for(user_id, &second_token, peer_enter)
            .await,
    )
    .await;
    assert_eq!(peer["status"], "no_change");
    assert_eq!(peer["capture_mode"]["blind"], true);

    let foreign = scenario
        .preload_lifecycle_action_for(
            Uuid::new_v4(),
            &scenario.token,
            lifecycle_request(
                "preload-foreign-path",
                1,
                0,
                0,
                1,
                serde_json::json!({"type":"enter"}),
            ),
        )
        .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    assert_eq!(json(foreign).await["kind"], "forbidden");

    let pending = scenario
        .preload_values_action(preload_fixture_request(
            "preload-lifecycle-value",
            0,
            1,
            fixture.0,
            0.6,
        ))
        .await;
    assert_eq!(pending.status(), StatusCode::OK);
    let captured = scenario
        .playback_action_for(
            &scenario.token,
            scenario.session.desk.id,
            playback_request("preload-lifecycle-capture", 1, "go"),
        )
        .await;
    assert_eq!(captured.status(), StatusCode::OK);
    assert_eq!(json(captured).await["outcome"]["status"], "captured");
    let expected_cursor = scenario.state.application_events.latest_sequence();

    // A later unrelated Programmer event must not invalidate the queued Playback target.
    assert_eq!(
        scenario
            .priority_action_for(
                user_id,
                &scenario.token,
                serde_json::json!({
                    "request_id":"preload-unrelated-priority",
                    "expected_revision":0,
                    "priority":70,
                }),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let go = lifecycle_go(
        "preload-go-http",
        1,
        1,
        1,
        1,
        show_id,
        show_revision,
        expected_cursor,
    );
    let go_response = scenario.preload_lifecycle_action(go.clone()).await;
    assert_eq!(go_response.status(), StatusCode::OK);
    let committed = json(go_response).await;
    assert_eq!(committed["status"], "changed");
    assert_eq!(committed["active"], true);
    assert_eq!(committed["capture_mode"]["blind"], false);
    assert_eq!(committed["commit"]["show_revision"], show_revision);
    assert_eq!(committed["commit"]["executed_playback_actions"], 1);
    assert_eq!(committed["commit"]["executed"][0]["playback_number"], 1);
    assert_eq!(committed["commit"]["runtime_changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        committed["commit"]["playback_event_sequence_after"],
        committed["commit"]["runtime_changes"][0]["event_sequence"]
    );
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::programming_capture_mode(user_id),
        ),
        2
    );
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::programming_preload_values(user_id),
        ),
        2
    );
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::programming_preload_playback_queue(user_id),
        ),
        2
    );
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::playback(1),
        ),
        1
    );
    let event_cursor = scenario.state.application_events.latest_sequence();
    let replay = json(scenario.preload_lifecycle_action(go).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(scenario.state.application_events.latest_sequence(), event_cursor);

    let reenter = lifecycle_request(
        "preload-reenter-peer",
        2,
        2,
        2,
        0,
        serde_json::json!({"type":"enter"}),
    );
    assert_eq!(
        scenario
            .preload_lifecycle_action_for(user_id, &second_token, reenter)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .preload_values_action_for(
                user_id,
                &second_token,
                preload_fixture_request(
                    "preload-peer-pending",
                    2,
                    3,
                    fixture.0,
                    0.8,
                ),
            )
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .playback_action_for(
                &second_token,
                second_desk.id,
                playback_request("preload-peer-capture", 1, "go"),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let clear = lifecycle_request(
        "preload-clear-http",
        3,
        3,
        3,
        1,
        serde_json::json!({"type":"clear_pending"}),
    );
    let cleared = json(
        scenario
            .preload_lifecycle_action_for(user_id, &second_token, clear)
            .await,
    )
    .await;
    assert_eq!(cleared["status"], "changed");
    assert_eq!(cleared["active"], true);
    assert!(cleared["values_projection"]["fixture_values"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(cleared["queue_projection"]["actions"]
        .as_array()
        .unwrap()
        .is_empty());

    let clear_no_change = json(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "preload-clear-empty",
                3,
                4,
                4,
                1,
                serde_json::json!({"type":"clear_pending"}),
            ))
            .await,
    )
    .await;
    assert_eq!(clear_no_change["status"], "no_change");
    assert!(clear_no_change.get("values_projection").is_none());
    assert!(clear_no_change.get("queue_projection").is_none());

    let released = json(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "preload-release-http",
                3,
                4,
                4,
                1,
                serde_json::json!({"type":"release"}),
            ))
            .await,
    )
    .await;
    assert_eq!(released["status"], "changed");
    assert_eq!(released["active"], false);
    assert!(released.get("values_projection").is_none());
    let release_no_change = json(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "preload-release-empty",
                4,
                4,
                4,
                1,
                serde_json::json!({"type":"release"}),
            ))
            .await,
    )
    .await;
    assert_eq!(release_no_change["status"], "no_change");
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preload_go_rejects_show_target_and_gap_conflicts_with_explicit_authorities() {
    let scenario = CommandHttpScenario::new().await;
    let (show_id, show_revision, _) =
        install_lifecycle_show(&scenario, "Preload cursor authority").await;
    assert_eq!(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "cursor-enter",
                0,
                0,
                0,
                0,
                serde_json::json!({"type":"enter"}),
            ))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .playback_action_for(
                &scenario.token,
                scenario.session.desk.id,
                playback_request("cursor-capture", 1, "go"),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let cursor = scenario.state.application_events.latest_sequence();

    let stale_show = scenario
        .preload_lifecycle_action(lifecycle_go(
            "cursor-show-conflict",
            1,
            0,
            1,
            0,
            show_id,
            show_revision + 1,
            cursor,
        ))
        .await;
    assert_eq!(stale_show.status(), StatusCode::CONFLICT);
    let stale_show = json(stale_show).await;
    assert_eq!(stale_show["current_revision"], show_revision);
    assert!(stale_show.get("current_related_revision").is_none());

    let other_user = scenario.state.desk.lock().add_user("Preload cursor other").unwrap();
    let other_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Preload cursor other", "preload-cursor-other")
        .unwrap();
    let (other_token, logged_user) =
        login_on_desk(&scenario, "Preload cursor other", other_desk.id).await;
    assert_eq!(logged_user, other_user.id.0);
    assert_eq!(
        scenario
            .playback_action_for(
                &other_token,
                other_desk.id,
                playback_request("cursor-target-change", 1, "on"),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let current = scenario.state.application_events.latest_sequence();
    let target = scenario
        .preload_lifecycle_action(lifecycle_go(
            "cursor-target-conflict",
            1,
            0,
            1,
            0,
            show_id,
            show_revision,
            cursor,
        ))
        .await;
    assert_eq!(target.status(), StatusCode::CONFLICT);
    let target = json(target).await;
    assert_eq!(target["current_revision"], current);
    assert_eq!(target["current_related_revision"], show_revision);
    assert!(scenario
        .state
        .programmers
        .capture_mode(scenario.session.id)
        .unwrap()
        .blind);

    for revision in 0..2_049 {
        let context = light_application::ActionContext::system(
            Uuid::nil(),
            light_application::ActionSource::System,
        );
        let change = light_application::ProgrammingPriorityChange::Upsert {
            projection: light_application::ProgrammingPriorityProjection {
                user_id: light_core::UserId::new(),
                revision,
                priority: 100,
                changed_at: chrono::Utc::now(),
            },
        };
        scenario.state.application_events.publish(
            light_application::EventDraft::programming_priority_changed(&context, change),
        );
    }
    let latest = scenario.state.application_events.latest_sequence();
    let gap = scenario
        .preload_lifecycle_action(lifecycle_go(
            "cursor-gap-conflict",
            1,
            0,
            1,
            0,
            show_id,
            show_revision,
            cursor,
        ))
        .await;
    assert_eq!(gap.status(), StatusCode::CONFLICT);
    let gap = json(gap).await;
    assert_eq!(gap["current_revision"], latest);
    assert_eq!(gap["current_related_revision"], show_revision);
    assert!(gap["error"].as_str().unwrap().contains("retained history"));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn failed_typed_preload_go_rolls_back_programmer_queue_runtime_and_events() {
    let scenario = CommandHttpScenario::new().await;
    let (show_id, show_revision, _) =
        install_lifecycle_show(&scenario, "Typed Preload rollback").await;
    assert_eq!(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "rollback-enter",
                0,
                0,
                0,
                0,
                serde_json::json!({"type":"enter"}),
            ))
            .await
            .status(),
        StatusCode::OK
    );
    scenario.state.programmers.queue_preload_playback_action(
        scenario.session.id,
        1,
        None,
        light_programmer::PreloadPlaybackQueueAction::Go,
        light_programmer::PreloadPlaybackQueueSurface::Physical,
    );
    scenario.state.programmers.queue_preload_playback_action(
        scenario.session.id,
        3,
        None,
        light_programmer::PreloadPlaybackQueueAction::On,
        light_programmer::PreloadPlaybackQueueSurface::Virtual,
    );
    let before = scenario.state.programmers.get(scenario.session.id).unwrap();
    let cursor = scenario.state.application_events.latest_sequence();
    let response = scenario
        .preload_lifecycle_action(lifecycle_go(
            "rollback-go",
            1,
            0,
            0,
            0,
            show_id,
            show_revision,
            cursor,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(json(response).await["error"]
        .as_str()
        .unwrap()
        .contains("group playback"));
    let after = scenario.state.programmers.get(scenario.session.id).unwrap();
    assert_eq!(after.preload_playback_pending, before.preload_playback_pending);
    assert_eq!(after.blind, before.blind);
    assert!(scenario.state.engine.playback_runtime().is_empty());
    assert_eq!(scenario.state.application_events.latest_sequence(), cursor);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preload_v1_compatibility_reuses_typed_actions_and_replays_without_duplicate_events() {
    let scenario = CommandHttpScenario::new().await;
    install_lifecycle_show(&scenario, "Preload v1 compatibility").await;
    let command = |request_id: &str, name: &str| WsCommand {
        protocol_version: 1,
        request_id: request_id.into(),
        session_id: scenario.session.id,
        expected_revision: None,
        command: name.into(),
        payload: serde_json::json!({}),
    };
    let applied_before = audit_count(&scenario.state, "command_applied");
    let changed_before = audit_count(&scenario.state, "programmer_changed");
    let first = dispatch_ws_command(
        &scenario.state,
        &scenario.session,
        command("preload-v1-enter", "preload.enter"),
    );
    assert!(first.ok, "{:?}", first.error);
    assert_eq!(first.payload.unwrap(), serde_json::json!({"blind":true}));
    assert_eq!(audit_count(&scenario.state, "command_applied"), applied_before + 1);
    assert_eq!(audit_count(&scenario.state, "programmer_changed"), changed_before);
    assert!(
        dispatch_ws_command(
            &scenario.state,
            &scenario.session,
            command("preload-v1-enter", "preload.enter"),
        )
        .ok
    );
    assert_eq!(audit_count(&scenario.state, "command_applied"), applied_before + 1);

    scenario.state.programmers.queue_preload_playback_action(
        scenario.session.id,
        1,
        None,
        light_programmer::PreloadPlaybackQueueAction::Go,
        light_programmer::PreloadPlaybackQueueSurface::Physical,
    );
    let committed_before = audit_count(&scenario.state, "preload_committed");
    let playback_before = audit_count(&scenario.state, "playback_changed");
    let first = dispatch_ws_command(
        &scenario.state,
        &scenario.session,
        command("preload-v1-go", "preload.go"),
    );
    assert!(first.ok, "{:?}", first.error);
    let payload = first.payload.unwrap();
    assert_eq!(payload["active"], true);
    assert_eq!(payload["playback_actions"][0]["action"], "go");
    assert_eq!(payload["playback_actions"][0]["surface"], "physical");
    assert_eq!(audit_count(&scenario.state, "preload_committed"), committed_before + 1);
    assert_eq!(audit_count(&scenario.state, "playback_changed"), playback_before + 1);
    let applied = audit_count(&scenario.state, "command_applied");
    let changed = audit_count(&scenario.state, "programmer_changed");
    assert!(
        dispatch_ws_command(
            &scenario.state,
            &scenario.session,
            command("preload-v1-go", "preload.go"),
        )
        .ok
    );
    assert_eq!(audit_count(&scenario.state, "preload_committed"), committed_before + 1);
    assert_eq!(audit_count(&scenario.state, "playback_changed"), playback_before + 1);
    assert_eq!(audit_count(&scenario.state, "command_applied"), applied);
    assert_eq!(audit_count(&scenario.state, "programmer_changed"), changed);

    let reenter = command("preload-v1-reenter", "preload.enter");
    assert!(dispatch_ws_command(&scenario.state, &scenario.session, reenter).ok);
    scenario.state.programmers.queue_preload_playback_action(
        scenario.session.id,
        1,
        None,
        light_programmer::PreloadPlaybackQueueAction::Go,
        light_programmer::PreloadPlaybackQueueSurface::Virtual,
    );
    let cleared = dispatch_ws_command(
        &scenario.state,
        &scenario.session,
        command("preload-v1-clear", "preload.clear"),
    );
    assert!(cleared.ok);
    assert_eq!(
        cleared.payload.unwrap(),
        serde_json::json!({"pending_cleared":true,"active_unchanged":true})
    );
    let applied = audit_count(&scenario.state, "command_applied");
    let changed = audit_count(&scenario.state, "programmer_changed");
    assert!(
        dispatch_ws_command(
            &scenario.state,
            &scenario.session,
            command("preload-v1-clear", "preload.clear"),
        )
        .ok
    );
    assert_eq!(audit_count(&scenario.state, "command_applied"), applied);
    assert_eq!(audit_count(&scenario.state, "programmer_changed"), changed);

    let released = dispatch_ws_command(
        &scenario.state,
        &scenario.session,
        command("preload-v1-release", "preload.release"),
    );
    assert!(released.ok);
    assert_eq!(released.payload.unwrap(), serde_json::json!({"released":true}));
    let applied = audit_count(&scenario.state, "command_applied");
    assert!(
        dispatch_ws_command(
            &scenario.state,
            &scenario.session,
            command("preload-v1-release", "preload.release"),
        )
        .ok
    );
    assert_eq!(audit_count(&scenario.state, "command_applied"), applied);
    let no_change = dispatch_ws_command(
        &scenario.state,
        &scenario.session,
        command("preload-v1-release-empty", "preload.release"),
    );
    assert!(no_change.ok);
    assert_eq!(no_change.payload.unwrap(), serde_json::json!({"released":false}));
    assert_eq!(audit_count(&scenario.state, "command_applied"), applied);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
