use super::*;

#[tokio::test]
async fn active_group_put_and_undo_refresh_each_live_desk_once_without_deadlocking() {
    let scenario = ActiveGroupScenario::new("Multi-desk Group refresh").await;

    scenario.state.programmers.select_expression(
        scenario.actor.id,
        Vec::new(),
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "1".into(),
            rule: light_programmer::SelectionRule::Even,
        },
    );
    scenario.state.programmers.select_expression(
        scenario.peer.id,
        vec![scenario.first],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "1".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );

    let before_put = scenario.state.application_events.latest_sequence();
    let changed = tokio::time::timeout(
        Duration::from_secs(2),
        put_active_object(
            &scenario.app,
            &scenario.actor.token,
            &scenario.show_id,
            "group",
            "1",
            1,
            group_body([scenario.first, scenario.second]),
        ),
    )
    .await
    .expect("the Group PUT deadlocked while refreshing multiple desks");
    assert_eq!(changed.status(), StatusCode::OK);
    let changed = json(changed).await;
    assert_eq!(changed["revision"], 2);
    assert_eq!(changed["event_sequence"], before_put + 5);

    let put_correlation = assert_selection_refresh(
        &scenario.state,
        &scenario.actor,
        before_put,
        &[scenario.second],
        light_application::ActionSource::Http,
        None,
    );
    assert_selection_refresh(
        &scenario.state,
        &scenario.peer,
        before_put,
        &[scenario.first, scenario.second],
        light_application::ActionSource::Http,
        Some(put_correlation),
    );
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        before_put + 5
    );
    assert_selection_events_precede_show_event(&scenario.state, before_put);
    assert_group_membership(&scenario.state, &[scenario.first, scenario.second]);

    let before_undo = scenario.state.application_events.latest_sequence();
    let undone = tokio::time::timeout(
        Duration::from_secs(2),
        undo_active_object(
            &scenario.app,
            &scenario.actor.token,
            &scenario.show_id,
            "group",
            "1",
            2,
        ),
    )
    .await
    .expect("the Group undo deadlocked while refreshing multiple desks");
    assert_eq!(undone.status(), StatusCode::OK);
    let undone = json(undone).await;
    assert_eq!(undone["revision"], 3);
    assert_eq!(undone["event_sequence"], before_undo + 5);

    let undo_correlation = assert_selection_refresh(
        &scenario.state,
        &scenario.actor,
        before_undo,
        &[],
        light_application::ActionSource::Http,
        None,
    );
    assert_selection_refresh(
        &scenario.state,
        &scenario.peer,
        before_undo,
        &[scenario.first],
        light_application::ActionSource::Http,
        Some(undo_correlation),
    );
    assert_ne!(put_correlation, undo_correlation);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        before_undo + 5
    );
    assert_selection_events_precede_show_event(&scenario.state, before_undo);
    assert_group_membership(&scenario.state, &[scenario.first]);

    scenario.cleanup();
}

#[tokio::test]
async fn active_show_install_clears_each_desk_pending_choice_once() {
    let scenario = ActiveGroupScenario::new("Pending choice invalidation").await;
    let command = "COPY SET 1 CUE 1 AT SET 2 CUE 2";
    for session in [&scenario.actor, &scenario.peer] {
        scenario.state.programmers.complete_command_execution(
            session.id,
            Some(command),
            Some(light_application::CueMoveCopyChoice {
                operation: light_application::CueTransferOperation::Copy,
                command: command.into(),
                options: Vec::new(),
                cancel_label: "Cancel".into(),
            }),
        );
    }
    let before = scenario.state.application_events.latest_sequence();

    let response = put_active_object(
        &scenario.app,
        &scenario.actor.token,
        &scenario.show_id,
        "group",
        "1",
        1,
        group_body([scenario.first, scenario.second]),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    for session in [&scenario.actor, &scenario.peer] {
        assert!(
            scenario
                .state
                .programmers
                .command_line_state(session.id)
                .unwrap()
                .pending_choice
                .is_none()
        );
        let filter = light_application::EventFilter::for_desk(session.desk.id).with_object(
            light_application::EventObject::programming_command_line(session.desk.id),
        );
        let light_application::EventReplay::Events(events) =
            scenario.state.application_events.replay(before, &filter)
        else {
            panic!("choice invalidation should remain replayable")
        };
        assert_eq!(events.len(), 1);
        let light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::InteractionChanged(change),
        ) = &events[0].payload
        else {
            panic!("expected a Programming interaction change")
        };
        assert!(change.command_line().unwrap().pending_choice.is_none());
    }
    scenario.cleanup();
}

#[tokio::test]
async fn nested_record_group_refreshes_actor_and_peer_once_without_relocking_the_actor() {
    let scenario = ActiveGroupScenario::new("Nested Record Group refresh").await;
    scenario.state.programmers.select_expression(
        scenario.actor.id,
        vec![scenario.first, scenario.second],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "1".into(),
            rule: light_programmer::SelectionRule::Even,
        },
    );
    scenario.state.programmers.select_expression(
        scenario.peer.id,
        vec![scenario.first],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "1".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );

    let before_record = scenario.state.application_events.latest_sequence();
    let before_audit = scenario.state.audit_events.lock().len();
    let worker_state = scenario.state.clone();
    let worker_actor = scenario.actor.clone();
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::task::spawn_blocking(move || {
            dispatch_ws_command(
                &worker_state,
                &worker_actor,
                WsCommand {
                    protocol_version: 1,
                    request_id: "record-group-1".into(),
                    session_id: worker_actor.id,
                    expected_revision: None,
                    command: "programmer.execute".into(),
                    payload: serde_json::json!({"value":"RECORD GROUP 1"}),
                },
            )
        }),
    )
    .await
    .expect("RECORD GROUP 1 deadlocked across the actor and peer Programming gates")
    .unwrap();
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(response.payload.unwrap()["applied"], 2);

    let correlation = assert_selection_refresh(
        &scenario.state,
        &scenario.actor,
        before_record,
        &[scenario.second],
        light_application::ActionSource::UserInterface,
        None,
    );
    assert_selection_refresh(
        &scenario.state,
        &scenario.peer,
        before_record,
        &[scenario.first, scenario.second],
        light_application::ActionSource::UserInterface,
        Some(correlation),
    );
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        before_record + 5,
        "the mutation must publish one Show event plus one selection and lifecycle event per changed desk"
    );
    assert_selection_events_precede_show_event(&scenario.state, before_record);
    assert_eq!(
        scenario
            .state
            .audit_events
            .lock()
            .iter()
            .skip(before_audit)
            .filter(|event| {
                event.kind == "highlight_changed"
                    && event.payload["desk_id"] == scenario.actor.desk.id.to_string()
                    && event.payload["user_id"] == scenario.actor.user.id.0.to_string()
                    && event.payload["source"] == "programmer_selection"
            })
            .count(),
        1,
        "the nested command must defer owner Highlight reconciliation to the outer interaction"
    );
    assert!(
        scenario
            .state
            .audit_events
            .lock()
            .iter()
            .skip(before_audit)
            .all(|event| {
                event.kind != "highlight_changed"
                    || event.payload["desk_id"] != scenario.actor.desk.id.to_string()
                    || event.payload["source"] != "show_selection_refresh"
            }),
        "the nested install must not also reconcile the owner Highlight"
    );
    assert_group_membership(&scenario.state, &[scenario.first, scenario.second]);

    scenario.cleanup();
}

struct ActiveGroupScenario {
    state: AppState,
    data_dir: PathBuf,
    actor: Session,
    peer: Session,
    app: Router,
    show_id: String,
    first: light_core::FixtureId,
    second: light_core::FixtureId,
}

impl ActiveGroupScenario {
    async fn new(name: &str) -> Self {
        let (state, data_dir) = test_state();
        let (actor, peer) = two_desk_sessions(&state);
        let app = router(state.clone());
        let show = create_show(&app, &actor.token, name).await;
        let show_id = show["id"].as_str().unwrap().to_owned();
        open_show(&app, &actor.token, &show_id).await;

        let first = schema_v2_direct_fixture().0;
        let mut second = schema_v2_direct_fixture().0;
        second.fixture_number = Some(2);
        second.address = Some(3);
        for fixture in [&first, &second] {
            let response = put_active_object(
                &app,
                &actor.token,
                &show_id,
                "patched_fixture",
                &fixture.fixture_id.0.to_string(),
                0,
                serde_json::to_value(fixture).unwrap(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        let seed = put_active_object(
            &app,
            &actor.token,
            &show_id,
            "group",
            "1",
            0,
            group_body([first.fixture_id]),
        )
        .await;
        assert_eq!(seed.status(), StatusCode::OK);
        assert_eq!(json(seed).await["revision"], 1);

        Self {
            state,
            data_dir,
            actor,
            peer,
            app,
            show_id,
            first: first.fixture_id,
            second: second.fixture_id,
        }
    }

    fn cleanup(self) {
        let _ = std::fs::remove_dir_all(self.data_dir);
    }
}

fn two_desk_sessions(state: &AppState) -> (Session, Session) {
    let (actor_user, peer_user, actor_desk, peer_desk) = {
        let store = state.desk.lock();
        let actor_user = store.users().unwrap().remove(0);
        let peer_user = store.add_user("Peer operator").unwrap();
        let actor_desk = store.add_desk("Front", "front").unwrap();
        let peer_desk = store.add_desk("Wing", "wing").unwrap();
        (actor_user, peer_user, actor_desk, peer_desk)
    };
    let actor = Session {
        id: SessionId::new(),
        user: actor_user,
        token: "group-refresh-actor".into(),
        connected: true,
        desk: actor_desk,
    };
    let peer = Session {
        id: SessionId::new(),
        user: peer_user,
        token: "group-refresh-peer".into(),
        connected: true,
        desk: peer_desk,
    };
    for session in [&actor, &peer] {
        state.programmers.start(session.id, session.user.id);
        attach_session_command_context(state, session);
        state.sessions.write().insert(session.id, session.clone());
    }
    (actor, peer)
}

async fn open_show(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/open"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn put_active_object(
    app: &Router,
    token: &str,
    show_id: &str,
    kind: &str,
    object_id: &str,
    expected_revision: u64,
    body: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::put(format!(
                "/api/v1/shows/{show_id}/objects/{kind}/{object_id}"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::IF_MATCH, expected_revision.to_string())
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap()
}

async fn undo_active_object(
    app: &Router,
    token: &str,
    show_id: &str,
    kind: &str,
    object_id: &str,
    expected_revision: u64,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!(
                "/api/v1/shows/{show_id}/objects/{kind}/{object_id}/undo"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_MATCH, expected_revision.to_string())
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap()
}

fn group_body(fixtures: impl IntoIterator<Item = light_core::FixtureId>) -> serde_json::Value {
    serde_json::json!({
        "id": "1",
        "name": "Group 1",
        "fixtures": fixtures.into_iter().collect::<Vec<_>>(),
    })
}

fn assert_group_membership(state: &AppState, expected: &[light_core::FixtureId]) {
    assert_eq!(
        state
            .engine
            .snapshot()
            .groups
            .iter()
            .find(|group| group.id == "1")
            .unwrap()
            .fixtures,
        expected
    );
}

fn assert_selection_events_precede_show_event(state: &AppState, after_sequence: u64) {
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(after_sequence, &light_application::EventFilter::default())
    else {
        panic!("the mutation events should remain replayable")
    };
    assert_eq!(events.len(), 5);
    assert!(events[..2].iter().all(|event| matches!(
        &event.payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::InteractionChanged(_)
        )
    )));
    assert!(events[2..4].iter().all(|event| matches!(
        &event.payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::LifecycleChanged(_)
        )
    )));
    assert!(matches!(
        &events[4].payload,
        light_application::ApplicationEvent::Show(light_application::ShowEvent::ObjectsChanged(_))
    ));
}

fn assert_selection_refresh(
    state: &AppState,
    session: &Session,
    after_sequence: u64,
    expected: &[light_core::FixtureId],
    expected_source: light_application::ActionSource,
    expected_correlation: Option<Uuid>,
) -> Uuid {
    let filter = light_application::EventFilter::for_desk(session.desk.id).with_object(
        light_application::EventObject::programming_selection(session.desk.id),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(after_sequence, &filter)
    else {
        panic!("the selection event should remain replayable")
    };
    assert_eq!(
        events.len(),
        1,
        "each changed desk must receive exactly one final selection event"
    );
    let event = &events[0];
    assert_eq!(event.desk_id, Some(session.desk.id));
    assert_eq!(
        event.source,
        light_application::EventSource::Action(expected_source)
    );
    let correlation = event.correlation_id.expect("mutation correlation id");
    if let Some(expected_correlation) = expected_correlation {
        assert_eq!(correlation, expected_correlation);
    }
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::InteractionChanged(change),
    ) = &event.payload
    else {
        panic!("expected a typed Programming interaction change")
    };
    assert_eq!(change.desk_id(), session.desk.id);
    assert!(change.command_line().is_none());
    assert_eq!(change.selection().unwrap().selected, expected);
    assert_eq!(
        state.programmers.selection(session.id).unwrap().selected,
        expected
    );
    correlation
}
