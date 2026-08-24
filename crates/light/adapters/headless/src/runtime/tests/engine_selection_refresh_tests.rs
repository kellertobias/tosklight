use super::*;

#[tokio::test]
async fn active_group_put_and_undo_refresh_the_desk_once_without_deadlocking() {
    let scenario = ActiveGroupScenario::new("Desk Group refresh").await;

    // The desk has one live-group selection, set from whichever surface last touched it.
    scenario.state.programming.select_expression(
        scenario.peer.id,
        vec![scenario.first],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "1".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );

    let before_put = scenario.state.events.latest_sequence();
    let changed = tokio::time::timeout(
        Duration::from_secs(2),
        put_active_object(
            &scenario.state,
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
    let put_events = selection_refresh_events(&scenario.state, before_put);
    assert_eq!(
        changed["event_sequence"],
        put_events.last().unwrap().sequence
    );

    let put_correlation = assert_selection_refresh(
        &scenario.state,
        &scenario.actor,
        before_put,
        &[scenario.first, scenario.second],
        light_application::ActionSource::Http,
        None,
    );
    assert_eq!(put_events.len(), 3);
    assert_selection_events_precede_show_event(&scenario.state, before_put);
    assert_group_membership(&scenario.state, &[scenario.first, scenario.second]);

    let before_undo = scenario.state.events.latest_sequence();
    let undone = tokio::time::timeout(
        Duration::from_secs(2),
        undo_active_object(
            &scenario.state,
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
    let undo_events = selection_refresh_events(&scenario.state, before_undo);
    assert_eq!(
        undone["event_sequence"],
        undo_events.last().unwrap().sequence
    );

    let undo_correlation = assert_selection_refresh(
        &scenario.state,
        &scenario.actor,
        before_undo,
        &[scenario.first],
        light_application::ActionSource::Http,
        None,
    );
    assert_ne!(put_correlation, undo_correlation);
    assert_eq!(undo_events.len(), 3);
    assert_selection_events_precede_show_event(&scenario.state, before_undo);
    assert_group_membership(&scenario.state, &[scenario.first]);

    scenario.cleanup();
}

#[tokio::test]
async fn active_show_install_clears_the_desks_pending_choice_once() {
    let scenario = ActiveGroupScenario::new("Pending choice invalidation").await;
    let command = "COPY CUELIST 1 CUE 1 AT CUELIST 2 CUE 2";
    for session in [&scenario.actor, &scenario.peer] {
        scenario.state.programming.complete_command_execution(
            session.id,
            Some(command),
            Some(light_application::PendingCommandChoice::CueMoveCopy(
                light_application::CueMoveCopyChoice {
                    choice_id: uuid::Uuid::from_u128(1),
                    show_id: uuid::Uuid::from_u128(2),
                    show_revision: 3,
                    operation: light_application::CueTransferOperation::Copy,
                    command: command.into(),
                    options: Vec::new(),
                    cancel_label: "Cancel".into(),
                },
            )),
        );
    }
    let before = scenario.state.events.latest_sequence();

    let response = put_active_object(
        &scenario.state,
        &scenario.actor.token,
        &scenario.show_id,
        "group",
        "1",
        1,
        group_body([scenario.first, scenario.second]),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    // One command line, so every surface sees the choice gone.
    for session in [&scenario.actor, &scenario.peer] {
        assert!(
            scenario
                .state
                .programming
                .command_line_state(session.id)
                .unwrap()
                .pending_choice
                .is_none()
        );
    }
    // ...and one invalidation is published, under the desk the action acted on.
    let desk = scenario.actor.desk.id;
    let filter = light_application::EventFilter::for_desk(desk).with_object(
        light_application::EventObject::programming_command_line(desk),
    );
    let light_application::EventReplay::Events(events) =
        scenario.state.events.replay(before, &filter)
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
    scenario.cleanup();
}

#[tokio::test]
async fn nested_record_group_refreshes_the_desk_once_without_relocking_it() {
    let scenario = ActiveGroupScenario::new("Nested Record Group refresh").await;
    // One ordered selection, whichever surface last touched it.
    scenario.state.programming.select_expression(
        scenario.actor.id,
        vec![scenario.first, scenario.second],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "1".into(),
            rule: light_programmer::SelectionRule::Even,
        },
    );

    let before_record = scenario.state.events.latest_sequence();
    let before_audit = scenario.state.events.audit_events().len();
    let worker_state = scenario.state.clone();
    let worker_actor = scenario.actor.clone();
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::task::spawn_blocking(move || {
            dispatch_live_action(
                &worker_state,
                &worker_actor,
                live_action_frame(
                    &worker_actor,
                    "record-group-1",
                    light_wire::v2::live_action::LiveAction::CommandLineExecute(
                        light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                            value: "RECORD GROUP 1".into(),
                        },
                    ),
                ),
            )
        }),
    )
    .await
    .expect("RECORD GROUP 1 deadlocked across the actor and peer Programming gates")
    .unwrap();
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(response.payload.unwrap()["applied"], 2);

    // Recording the selection into Group 1 changed its membership, so the live-group selection
    // re-resolves and the desk publishes exactly one final selection.
    assert_selection_refresh(
        &scenario.state,
        &scenario.actor,
        before_record,
        &[scenario.second],
        light_application::ActionSource::UserInterface,
        None,
    );
    assert_eq!(
        selection_refresh_events(&scenario.state, before_record).len(),
        3,
        "the mutation must publish one Show event plus one selection and one lifecycle event"
    );
    assert_selection_events_precede_show_event(&scenario.state, before_record);
    assert_eq!(
        scenario
            .state
            .events
            .audit_events()
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
            .events
            .audit_events()
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
                &state,
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
            &state,
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
        let actor_user = state.installation.users().unwrap().remove(0);
        let peer_user = state.installation.add_user("Peer operator").unwrap();
        let actor_desk = state.installation.add_desk("Front").unwrap();
        let peer_desk = state.installation.add_desk("Wing").unwrap();
        (actor_user, peer_user, actor_desk, peer_desk)
    };
    let actor = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        user: actor_user,
        token: "group-refresh-actor".into(),
        connected: true,
        desk: actor_desk,
    };
    let peer = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        user: peer_user,
        token: "group-refresh-peer".into(),
        connected: true,
        desk: peer_desk,
    };
    for session in [&actor, &peer] {
        state.programming.start(session.id, session.user.id);
        attach_session_command_context(state, session);
        state.sessions.insert_session(session.clone());
    }
    (actor, peer)
}

async fn open_show(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn put_active_object(
    state: &AppState,
    token: &str,
    show_id: &str,
    kind: &str,
    object_id: &str,
    expected_revision: u64,
    body: serde_json::Value,
) -> Response {
    seed_show_object(
        state,
        token,
        show_id,
        kind,
        object_id,
        expected_revision,
        body,
    )
    .await
}

async fn undo_active_object(
    state: &AppState,
    token: &str,
    show_id: &str,
    kind: &str,
    object_id: &str,
    expected_revision: u64,
) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    let session = authenticate(state, &headers).unwrap();
    let action = undo_active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http),
        light_core::ShowId(Uuid::parse_str(show_id).unwrap()),
        light_application::ActiveShowObjectKind::from_storage_kind(kind).unwrap(),
        object_id,
        expected_revision,
    );
    let activation = state.active_show.acquire().await;
    match run_active_show_object_undo_async(state, activation, action).await {
        Ok((result, _activation)) => Json(serde_json::json!({
            "revision": result.change.object_revision,
            "event_sequence": result.event_sequence
        }))
        .into_response(),
        Err(error) => error.into_response(),
    }
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
            .output
            .snapshot()
            .groups
            .iter()
            .find(|group| group.id == "1")
            .unwrap()
            .fixtures,
        expected
    );
}

/// The desk's final selection and lifecycle must reach a client before the Show change that
/// caused them, so a client never renders a Show generation against a stale selection.
fn assert_selection_events_precede_show_event(state: &AppState, after_sequence: u64) {
    let events = selection_refresh_events(state, after_sequence);
    assert_eq!(events.len(), 3);
    assert!(matches!(
        &events[0].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::InteractionChanged(_)
        )
    ));
    assert!(matches!(
        &events[1].payload,
        light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::LifecycleChanged(_)
        )
    ));
    assert!(matches!(
        &events[2].payload,
        light_application::ApplicationEvent::Show(light_application::ShowEvent::ObjectsChanged(_))
    ));
}

fn selection_refresh_events(
    state: &AppState,
    after_sequence: u64,
) -> Vec<std::sync::Arc<light_application::EventEnvelope>> {
    let filter = light_application::EventFilter::default()
        .with_capability(light_application::EventCapability::Desk)
        .with_capability(light_application::EventCapability::Programmer)
        .with_capability(light_application::EventCapability::Show);
    let light_application::EventReplay::Events(events) =
        state.events.replay(after_sequence, &filter)
    else {
        panic!("the scoped selection refresh events should remain replayable")
    };
    events
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
        state.events.replay(after_sequence, &filter)
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
        state.programming.selection(session.id).unwrap().selected,
        expected
    );
    correlation
}
