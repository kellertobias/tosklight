fn group_record_request(
    request_id: &str,
    group_id: &str,
    operation: &str,
    expected_object_revision: u64,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "group_id": group_id,
        "operation": operation,
        "expected_object_revision": expected_object_revision,
    })
}

#[tokio::test]
async fn group_record_route_is_authoritative_replay_safe_and_sparse_on_no_change() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group record route").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programmers.select(scenario.session.id, [fixture]);
    let baseline = scenario.state.application_events.latest_sequence();
    let request = group_record_request("group-route-record", "Front Wash", "overwrite", 0);

    let response = scenario
        .group_recording_action(&show_id, Some(&scenario.token), request.clone())
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"1\"");
    let first: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(first.request_id(), "group-route-record");
    assert!(!first.replayed());
    let light_wire::v2::group_recording::RecordedGroupProjection::Stored {
        id,
        revision,
        body,
    } = first.changed_group().unwrap()
    else {
        panic!("overwrite must return the stored Group")
    };
    assert_eq!(id, "Front Wash");
    assert_eq!(*revision, 1);
    assert_eq!(body["fixtures"], serde_json::json!([fixture.0]));
    let event_sequence = first.event_sequence().unwrap();
    assert_eq!(event_sequence, baseline + 1);
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);
    assert_eq!(
        highlight_reconciliations(&scenario, "show_selection_refresh"),
        1,
        "the standalone typed action must reconcile its owner Highlight once"
    );

    let replay = scenario
        .group_recording_action(&show_id, Some(&scenario.token), request)
        .await;
    let replay: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(replay).await).unwrap();
    assert!(replay.replayed());
    assert_eq!(replay.event_sequence(), Some(event_sequence));
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);

    let no_change = scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            group_record_request("group-route-no-change", "Front Wash", "overwrite", 1),
        )
        .await;
    let no_change: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(no_change).await).unwrap();
    assert!(matches!(
        no_change,
        light_wire::v2::group_recording::GroupRecordOutcome::NoChange { .. }
    ));
    assert_eq!(no_change.event_sequence(), None);
    assert_eq!(no_change.group_revision(), 1);
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);
    assert_eq!(highlight_reconciliations(&scenario, "show_selection_refresh"), 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn group_recording_finishes_gestures_before_show_events_even_without_topology_change() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group gesture ordering").await;
    let fixture = scenario.install_direct_fixture();
    open_fixture_gesture(&scenario, fixture);
    let baseline = scenario.state.application_events.latest_sequence();

    let changed = scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            group_record_request("group-gesture-changed", "1", "overwrite", 0),
        )
        .await;
    assert_eq!(changed.status(), StatusCode::OK);
    let changed: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(changed).await).unwrap();
    assert_event_order(&scenario.state, baseline, changed.event_sequence().unwrap());
    assert!(
        !scenario
            .state
            .programmers
            .selection(scenario.session.id)
            .unwrap()
            .gesture_open
    );

    open_fixture_gesture(&scenario, fixture);
    let before_no_change = scenario.state.application_events.latest_sequence();
    let no_change = scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            group_record_request("group-gesture-no-change", "1", "overwrite", 1),
        )
        .await;
    let no_change: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(no_change).await).unwrap();
    assert!(matches!(
        no_change,
        light_wire::v2::group_recording::GroupRecordOutcome::NoChange { .. }
    ));
    let events = application_events_after(&scenario.state, before_no_change);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::InteractionChanged(_)
        )
    ));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn open_fixture_gesture(scenario: &CommandHttpScenario, fixture: light_core::FixtureId) {
    let groups = scenario
        .state
        .engine
        .snapshot()
        .groups
        .iter()
        .cloned()
        .map(|group| (group.id.clone(), group))
        .collect();
    assert!(scenario.state.programmers.apply_selection_gesture(
        scenario.session.id,
        vec![light_programmer::SelectionReference::Fixture {
            fixture_id: fixture,
        }],
        &groups,
    ));
}

fn assert_event_order(state: &AppState, baseline: u64, show_event_sequence: u64) {
    let events = application_events_after(state, baseline);
    assert!(matches!(
        events.first().unwrap().payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::InteractionChanged(_)
        )
    ));
    assert!(matches!(
        events.last().unwrap().payload,
        light_application::ApplicationEvent::Show(light_application::ShowEvent::ObjectsChanged(_))
    ));
    assert_eq!(events.last().unwrap().sequence, show_event_sequence);
}

fn application_events_after(
    state: &AppState,
    baseline: u64,
) -> Vec<std::sync::Arc<light_application::EventEnvelope>> {
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(baseline, &light_application::EventFilter::default())
    else {
        panic!("Group recording events must remain replayable")
    };
    events
}

fn highlight_reconciliations(scenario: &CommandHttpScenario, source: &str) -> usize {
    scenario
        .state
        .audit_events
        .lock()
        .iter()
        .filter(|event| {
            event.kind == "highlight_changed"
                && event.payload["desk_id"] == scenario.session.desk.id.to_string()
                && event.payload["user_id"] == scenario.session.user.id.0.to_string()
                && event.payload["source"] == source
        })
        .count()
}

#[tokio::test]
async fn group_record_route_merges_subtracts_deletes_and_checks_revisions() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group record operations").await;
    let first = scenario.install_direct_fixture();
    let second = light_core::FixtureId::new();
    scenario.state.programmers.select(scenario.session.id, [first]);
    let baseline = scenario.state.application_events.latest_sequence();

    let overwrite = scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            group_record_request("group-overwrite", "7", "overwrite", 0),
        )
        .await;
    assert_eq!(overwrite.status(), StatusCode::OK);
    scenario.state.programmers.select(scenario.session.id, [second]);
    let merge = scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            group_record_request("group-merge", "7", "merge", 1),
        )
        .await;
    assert_eq!(merge.status(), StatusCode::OK);
    let merge: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(merge).await).unwrap();
    let light_wire::v2::group_recording::RecordedGroupProjection::Stored { body, .. } =
        merge.changed_group().unwrap()
    else {
        panic!("merge must return the stored Group")
    };
    assert_eq!(body["fixtures"], serde_json::json!([first.0, second.0]));

    let stale = scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            group_record_request("group-stale", "7", "subtract", 1),
        )
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(stale.headers()[header::ETAG], "\"2\"");
    assert_eq!(json(stale).await["current_revision"], 2);

    let subtract = scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            group_record_request("group-subtract", "7", "subtract", 2),
        )
        .await;
    assert_eq!(subtract.status(), StatusCode::OK);
    let subtract: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(subtract).await).unwrap();
    let light_wire::v2::group_recording::RecordedGroupProjection::Stored { body, .. } =
        subtract.changed_group().unwrap()
    else {
        panic!("subtract must return the stored Group")
    };
    assert_eq!(body["fixtures"], serde_json::json!([first.0]));

    open_fixture_gesture(&scenario, second);
    let delete_request = group_record_request("group-delete", "7", "delete", 3);
    let delete = scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            delete_request.clone(),
        )
        .await;
    assert_eq!(delete.status(), StatusCode::OK);
    assert_eq!(delete.headers()[header::ETAG], "\"4\"");
    let delete: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(delete).await).unwrap();
    assert!(matches!(
        delete.changed_group().unwrap(),
        light_wire::v2::group_recording::RecordedGroupProjection::Deleted {
            id,
            revision: 4
        } if id == "7"
    ));
    assert!(
        scenario
            .state
            .programmers
            .selection(scenario.session.id)
            .unwrap()
            .gesture_open,
        "explicit DELETE GROUP must preserve the current selection gesture"
    );
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 4);

    let replay = scenario
        .group_recording_action(&show_id, Some(&scenario.token), delete_request)
        .await;
    let replay: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(replay).await).unwrap();
    assert!(replay.replayed());
    assert!(matches!(
        replay.changed_group().unwrap(),
        light_wire::v2::group_recording::RecordedGroupProjection::Deleted { revision: 4, .. }
    ));
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 4);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn group_record_route_rejects_missing_auth_forged_state_and_wrong_show() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group record security").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programmers.select(scenario.session.id, [fixture]);
    let request = group_record_request("group-secure", "7", "overwrite", 0);

    assert_eq!(
        scenario
            .group_recording_action(&show_id, None, request.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    for field in ["fixtures", "selection", "programmer", "programming"] {
        let mut forged = request.clone();
        forged[field] = serde_json::json!([fixture.0]);
        assert_eq!(
            scenario
                .group_recording_action(&show_id, Some(&scenario.token), forged)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        scenario
            .group_recording_action(
                &Uuid::new_v4().to_string(),
                Some(&scenario.token),
                request,
            )
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn group_recording_captures_each_desk_selection_and_keeps_other_users_isolated() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group recording identity scopes").await;
    let first = scenario.install_direct_fixture();
    let second = light_core::FixtureId::new();
    let third = light_core::FixtureId::new();
    let (same_user, other_user) = group_recording_peer_sessions(&scenario);
    scenario.state.programmers.select(scenario.session.id, [first]);
    scenario.state.programmers.select(same_user.id, [second]);
    scenario.state.programmers.select(other_user.id, [third]);

    let same_user_record = scenario
        .group_recording_action(
            &show_id,
            Some(&same_user.token),
            group_record_request("same-user-record", "shared", "overwrite", 0),
        )
        .await;
    assert_eq!(same_user_record.status(), StatusCode::OK);
    assert_stored_group_fixtures(same_user_record, &[second]).await;

    let other_user_record = scenario
        .group_recording_action(
            &show_id,
            Some(&other_user.token),
            group_record_request("other-user-record", "shared", "overwrite", 1),
        )
        .await;
    assert_eq!(other_user_record.status(), StatusCode::OK);
    assert_stored_group_fixtures(other_user_record, &[third]).await;
    assert_eq!(
        scenario.state.programmers.selection(scenario.session.id).unwrap().selected,
        vec![first]
    );
    assert_eq!(
        scenario.state.programmers.selection(same_user.id).unwrap().selected,
        vec![second]
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn group_recording_port_rejects_forged_user_session_and_desk_contexts() {
    let scenario = CommandHttpScenario::new().await;
    let ports = command_http::ServerProgrammingPorts::new(
        &scenario.state,
        &scenario.session,
        "test_group_record",
        true,
    );
    let valid = light_application::ActionContext::operator(
        scenario.session.desk.id,
        scenario.session.user.id.0,
        scenario.session.id.0,
        light_application::ActionSource::Http,
    );
    for forged in [
        light_application::ActionContext {
            user_id: Some(Uuid::new_v4()),
            ..valid.clone()
        },
        light_application::ActionContext {
            session_id: Some(Uuid::new_v4()),
            ..valid.clone()
        },
        light_application::ActionContext {
            desk_id: Uuid::new_v4(),
            ..valid.clone()
        },
    ] {
        let error = light_application::ProgrammingGroupRecordingPorts::authorize_group_recording(
            &ports, &forged,
        )
        .unwrap_err();
        assert_eq!(error.kind, light_application::ActionErrorKind::Forbidden);
    }
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn group_recording_peer_sessions(
    scenario: &CommandHttpScenario,
) -> (Session, Session) {
    let (same_desk, other_user, other_desk) = {
        let store = scenario.state.desk.lock();
        let same_desk = store.add_desk("Same user wing", "same-user-wing").unwrap();
        let other_user = store.add_user("Other Group operator").unwrap();
        let other_desk = store.add_desk("Other user wing", "other-user-wing").unwrap();
        (same_desk, other_user, other_desk)
    };
    let same_user = Session {
        id: SessionId::new(),
        user: scenario.session.user.clone(),
        token: "same-user-group-record".into(),
        connected: true,
        desk: same_desk,
    };
    let other_user = Session {
        id: SessionId::new(),
        user: other_user,
        token: "other-user-group-record".into(),
        connected: true,
        desk: other_desk,
    };
    for session in [&same_user, &other_user] {
        scenario.state.programmers.start(session.id, session.user.id);
        attach_session_command_context(&scenario.state, session);
        scenario
            .state
            .sessions
            .write()
            .insert(session.id, session.clone());
    }
    (same_user, other_user)
}

async fn assert_stored_group_fixtures(response: Response, expected: &[light_core::FixtureId]) {
    let outcome: light_wire::v2::group_recording::GroupRecordOutcome =
        serde_json::from_value(json(response).await).unwrap();
    let light_wire::v2::group_recording::RecordedGroupProjection::Stored { body, .. } =
        outcome.changed_group().unwrap()
    else {
        panic!("recording must return a stored Group")
    };
    assert_eq!(
        body["fixtures"],
        serde_json::to_value(expected).unwrap()
    );
}

#[tokio::test]
async fn command_keyboard_osc_and_websocket_group_recording_converge_on_typed_capability() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group command convergence").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programmers.select(scenario.session.id, [fixture]);

    let command = scenario.execute("group-command", Some("RECORD GROUP 11")).await;
    assert_eq!(command.status(), StatusCode::OK);
    assert_eq!(json(command).await["outcome"], "accepted");

    for (index, key) in ["REC", "GRP", "1", "2", "ENT"].into_iter().enumerate() {
        let response = scenario
            .press_key(&scenario.token, key, &format!("group-key-{index}"))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let source: SocketAddr = "127.0.0.1:9018".parse().unwrap();
    scenario.state.osc_subscribers.lock().insert(
        "group-record".into(),
        OscSubscriber {
            desk_alias: "main".into(),
            target: source,
            command_source: source,
            session_id: scenario.session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    let pressed = [OscArgument::Bool(true)];
    for action in ["record", "group", "digit-1", "digit-3", "enter"] {
        handle_programmer_osc(
            &scenario.state,
            &format!("/light/main/programmer/{action}"),
            &pressed,
            Some("127.0.0.1:9018"),
        );
    }

    let ws = dispatch_ws_command(
        &scenario.state,
        &scenario.session,
        WsCommand {
            protocol_version: 1,
            request_id: "group-ws".into(),
            session_id: scenario.session.id,
            expected_revision: None,
            command: "programmer.execute".into(),
            payload: serde_json::json!({"value":"RECORD GROUP 14"}),
        },
    );
    assert!(ws.ok, "{:?}", ws.error);

    for id in ["11", "12", "13", "14"] {
        let object = scenario
            .app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{show_id}/objects/group/{id}"))
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", scenario.token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(object.status(), StatusCode::OK, "Group {id} was not stored");
    }
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn websocket_group_request_replay_skips_history_persistence_and_events() {
    let scenario = CommandHttpScenario::new().await;
    let _show_id = scenario.create_and_open_show("Group WebSocket replay").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programmers.select(scenario.session.id, [fixture]);
    let command = || WsCommand {
        protocol_version: 1,
        request_id: "group-ws-replay".into(),
        session_id: scenario.session.id,
        expected_revision: None,
        command: "programmer.execute".into(),
        payload: serde_json::json!({"value":"RECORD GROUP 15"}),
    };

    let first = dispatch_ws_command(&scenario.state, &scenario.session, command());
    assert!(first.ok, "{:?}", first.error);
    let sequence = scenario.state.application_events.latest_sequence();
    let history = scenario.history_len();
    let replay = dispatch_ws_command(&scenario.state, &scenario.session, command());
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(scenario.state.application_events.latest_sequence(), sequence);
    assert_eq!(scenario.history_len(), history);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
