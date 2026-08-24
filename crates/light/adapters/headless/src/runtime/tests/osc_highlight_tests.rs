const HIGHLIGHT_OSC_CLIENT: &str = "authenticated-highlight-hardware";
const HIGHLIGHT_OSC_SOURCE: &str = "127.0.0.1:19031";

fn highlight_subscription() -> ControlEvent {
    ControlEvent::Osc {
        address: "/light/subscribe".into(),
        arguments: vec![
            OscArgument::String(HIGHLIGHT_OSC_CLIENT.into()),
            OscArgument::String("desk".to_owned()),
            OscArgument::Int(19032),
        ],
        source: Some(HIGHLIGHT_OSC_SOURCE.into()),
    }
}

fn send_highlight_osc(state: &AppState, action: &str) {
    handle_control_event(
        state,
        ControlEvent::Osc {
            address: format!("/light/desk/highlight/{action}"),
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
    send_highlight_osc(state, "on");
    assert_eq!(
        state
            .output.highlighted_fixtures()
            .into_iter()
            .collect::<HashSet<_>>(),
        fixture_ids.iter().copied().collect::<HashSet<_>>()
    );
    let snapshot = state.output.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    let selection = state.programming.selection(session.id).unwrap();
    let software = state
        .highlight
        .apply_action_guarded(
            session.desk.id,
            session.user.id,
            HighlightAction::Next,
            &selection,
            &fixtures,
            &groups,
            false,
        );
    apply_highlight_selection_write(state, session, software.working_selection.as_ref()).unwrap();
    assert_eq!(software.state.active_index, Some(0));
    send_highlight_osc(state, "next");
    let selection = state.programming.selection(session.id).unwrap();
    let after_echo = state.highlight.transition(
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
    let snapshot = state.output.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    for _ in 0..2 {
        let selection = state.programming.selection(session.id).unwrap();
        let transition = state
            .highlight
            .apply_action(
                                HighlightAction::Next,
                &selection,
                &fixtures,
                &groups,
                false,
            );
        apply_highlight_selection_write(state, session, transition.working_selection.as_ref())
            .unwrap();
    }
    let before_aliases = state.events.latest_sequence();
    send_highlight_osc(state, "previous");
    send_highlight_osc(state, "prev");
    let selection = state.programming.selection(session.id).unwrap();
    let after_aliases = state.highlight.transition(
                &selection,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(after_aliases.state.active_index, Some(1));
    assert_eq!(after_aliases.output_fixtures, vec![fixture_ids[1]]);
    assert_programming_selection_event(
        state,
        session,
        before_aliases,
        light_application::ActionSource::Osc,
        &fixture_ids[1..2],
    );
    assert_eq!(
        state
            .events.audit_events()
            .iter()
            .filter(|event| {
                event.kind == "highlight_changed"
                    && event.payload["source"] == "osc"
                    && event.payload["action"] == "previous"
            })
            .count(),
        1,
        "the previous/prev aliases must share one subscriber-level dedupe key"
    );
}

fn verify_highlight_osc_feedback(state: &AppState) {
    let feedback = state.integrations.captured_osc_feedback();
    let prefix = "/light/desk/feedback/highlight".to_owned();
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
    assert!(state
        .integrations
        .osc_subscriber(HIGHLIGHT_OSC_CLIENT)
        .is_none());
    handle_control_event(state, highlight_subscription());
    assert_eq!(
        state
            .integrations
            .osc_subscriber(HIGHLIGHT_OSC_CLIENT)
            .unwrap()
            .session_id,
        session.id
    );
    let snapshot = state.output.snapshot();
    let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    let selection = state.programming.selection(session.id).unwrap();
    let reconnected = state.highlight.transition(
                &selection,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(reconnected.state.active_index, Some(1));
    assert_eq!(reconnected.state.remembered.len(), 3);
    assert!(reconnected.state.output_enabled);

    send_highlight_osc(state, "capture");
    send_highlight_osc(state, "reset");
    let selection = state.programming.selection(session.id).unwrap();
    let unchanged = state.highlight.transition(
                &selection,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(unchanged.state.active_index, Some(1));
    send_highlight_osc(state, "all");
    assert_eq!(state.programming.get(session.id).unwrap().selected, fixture_ids);
    let selection = state.programming.selection(session.id).unwrap();
    let restored = state.highlight.transition(
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
    let session = state.sessions.session(session_id).unwrap();
    let fixtures = highlight_test_fixtures();
    let fixture_ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    state.programming.select(session.id, fixture_ids.clone());
    enable_highlight_test_feedback(&state);
    handle_control_event(&state, highlight_subscription());
    assert_eq!(
        state
            .integrations
            .osc_subscriber(HIGHLIGHT_OSC_CLIENT)
            .unwrap()
            .session_id,
        session.id
    );
    verify_cross_surface_highlight_dedupe(&state, &session, &fixture_ids);
    verify_highlight_alias_dedupe(&state, &session, &fixture_ids);
    verify_highlight_osc_feedback(&state);
    verify_highlight_reconnect(&state, &session, &fixture_ids);
    let _ = std::fs::remove_dir_all(data_dir);
}
