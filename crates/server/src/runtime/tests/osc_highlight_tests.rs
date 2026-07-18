const HIGHLIGHT_OSC_CLIENT: &str = "authenticated-highlight-hardware";
const HIGHLIGHT_OSC_SOURCE: &str = "127.0.0.1:19031";

fn highlight_subscription(session: &Session) -> ControlEvent {
    ControlEvent::Osc {
        address: "/light/subscribe".into(),
        arguments: vec![
            OscArgument::String(HIGHLIGHT_OSC_CLIENT.into()),
            OscArgument::String(session.desk.osc_alias.clone()),
            OscArgument::Int(19032),
        ],
        source: Some(HIGHLIGHT_OSC_SOURCE.into()),
    }
}

fn send_highlight_osc(state: &AppState, session: &Session, action: &str) {
    handle_control_event(
        state,
        ControlEvent::Osc {
            address: format!("/light/{}/highlight/{action}", session.desk.osc_alias),
            arguments: vec![OscArgument::Bool(true)],
            source: Some(HIGHLIGHT_OSC_SOURCE.into()),
        },
    );
}

fn verify_cross_surface_highlight_dedupe(
    state: &AppState,
    session: &Session,
    fixture_ids: &[light_core::FixtureId],
) {
    send_highlight_osc(state, session, "on");
    assert_eq!(
        state
            .engine
            .highlighted_fixtures()
            .into_iter()
            .collect::<HashSet<_>>(),
        fixture_ids.iter().copied().collect::<HashSet<_>>()
    );
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    let selection = state.programmers.selection(session.id).unwrap();
    let software = state
        .highlight
        .action_guarded(
            session.desk.id,
            session.user.id,
            Some(&session.user.name),
            HighlightAction::Next,
            &selection,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    apply_highlight_selection_write(state, session, software.working_selection.as_ref()).unwrap();
    assert_eq!(software.state.active_index, Some(0));
    send_highlight_osc(state, session, "next");
    let selection = state.programmers.selection(session.id).unwrap();
    let after_echo = state.highlight.status(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        &selection,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(after_echo.state.active_index, Some(0));
}

fn verify_highlight_alias_dedupe(
    state: &AppState,
    session: &Session,
    fixture_ids: &[light_core::FixtureId],
) {
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    for _ in 0..2 {
        let selection = state.programmers.selection(session.id).unwrap();
        let transition = state
            .highlight
            .action(
                session.desk.id,
                session.user.id,
                Some(&session.user.name),
                HighlightAction::Next,
                &selection,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        apply_highlight_selection_write(state, session, transition.working_selection.as_ref())
            .unwrap();
    }
    send_highlight_osc(state, session, "previous");
    send_highlight_osc(state, session, "prev");
    let selection = state.programmers.selection(session.id).unwrap();
    let after_aliases = state.highlight.status(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        &selection,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(after_aliases.state.active_index, Some(1));
    assert_eq!(after_aliases.output_fixtures, vec![fixture_ids[1]]);
    assert!(state.audit_events.lock().iter().any(|event| {
        event.kind == "highlight_changed"
            && event.payload["source"] == "osc"
            && event.payload["action"] == "previous"
    }));
}

fn verify_highlight_osc_feedback(state: &AppState, session: &Session) {
    let feedback = state.osc_feedback_capture.lock();
    let prefix = format!("/light/{}/feedback/highlight", session.desk.osc_alias);
    for (suffix, arguments) in [
        ("active", vec![OscArgument::Bool(true)]),
        ("output", vec![OscArgument::Bool(true)]),
        ("mode", vec![OscArgument::String("step".into())]),
        ("index", vec![OscArgument::Int(2)]),
        ("total", vec![OscArgument::Int(3)]),
        ("can-previous", vec![OscArgument::Bool(true)]),
        ("can-next", vec![OscArgument::Bool(true)]),
    ] {
        assert!(
            feedback.iter().any(|(_, address, actual)| {
                address == &format!("{prefix}/{suffix}") && actual == &arguments
            }),
            "missing Highlight OSC feedback for {suffix}"
        );
    }
}

fn verify_highlight_reconnect(
    state: &AppState,
    session: &Session,
    fixture_ids: &[light_core::FixtureId],
) {
    handle_control_event(
        state,
        ControlEvent::Osc {
            address: "/light/unsubscribe".into(),
            arguments: vec![OscArgument::String(HIGHLIGHT_OSC_CLIENT.into())],
            source: Some(HIGHLIGHT_OSC_SOURCE.into()),
        },
    );
    assert!(!state.osc_subscribers.lock().contains_key(HIGHLIGHT_OSC_CLIENT));
    handle_control_event(state, highlight_subscription(session));
    assert_eq!(
        state.osc_subscribers.lock()[HIGHLIGHT_OSC_CLIENT].session_id,
        session.id
    );
    let snapshot = state.engine.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    let selection = state.programmers.selection(session.id).unwrap();
    let reconnected = state.highlight.status(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        &selection,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(reconnected.state.active_index, Some(1));
    assert_eq!(reconnected.state.remembered.len(), 3);
    assert!(reconnected.state.output_enabled);

    send_highlight_osc(state, session, "capture");
    send_highlight_osc(state, session, "reset");
    let selection = state.programmers.selection(session.id).unwrap();
    let unchanged = state.highlight.status(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        &selection,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(unchanged.state.active_index, Some(1));
    send_highlight_osc(state, session, "all");
    assert_eq!(state.programmers.get(session.id).unwrap().selected, fixture_ids);
    let selection = state.programmers.selection(session.id).unwrap();
    let restored = state.highlight.status(
        session.desk.id,
        session.user.id,
        Some(&session.user.name),
        &selection,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(restored.state.mode, HighlightMode::Selection);
    assert!(restored.state.active);
}

#[tokio::test]
async fn authenticated_osc_highlight_adapter_feedback_dedupe_and_reconnect_are_authoritative() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (_, session_id) = login(&app, "Operator").await;
    let session_id = SessionId(Uuid::parse_str(&session_id).unwrap());
    let session = state.sessions.read()[&session_id].clone();
    let fixtures = highlight_test_fixtures();
    let fixture_ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures,
            ..EngineSnapshot::default()
        })
        .unwrap();
    state.programmers.select(session.id, fixture_ids.clone());
    enable_highlight_test_feedback(&state);
    handle_control_event(&state, highlight_subscription(&session));
    assert_eq!(
        state.osc_subscribers.lock()[HIGHLIGHT_OSC_CLIENT].session_id,
        session.id
    );
    verify_cross_surface_highlight_dedupe(&state, &session, &fixture_ids);
    verify_highlight_alias_dedupe(&state, &session, &fixture_ids);
    verify_highlight_osc_feedback(&state, &session);
    verify_highlight_reconnect(&state, &session, &fixture_ids);
    let _ = std::fs::remove_dir_all(data_dir);
}
