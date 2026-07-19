async fn post_highlight_action(app: &Router, token: &str, action: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/highlight/action")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(r#"{{"action":"{action}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn verify_bootstrapped_step_highlight(
    app: &Router,
    state: &AppState,
    session: &Session,
    fixture_id: light_core::FixtureId,
) {
    let bootstrap = app
        .clone()
        .oneshot(
            Request::get("/api/v1/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bootstrap.status(), StatusCode::OK);
    let bootstrap = json(bootstrap).await;
    let highlight = bootstrap["highlight_states"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["session_id"] == session.id.0.to_string())
        .unwrap();
    assert_eq!(highlight["state"]["active"], false);
    assert_eq!(highlight["state"]["mode"], "step");
    assert_eq!(highlight["state"]["remembered"].as_array().unwrap().len(), 3);
    assert_eq!(
        highlight["state"]["active_fixture"]["fixture_id"],
        fixture_id.0.to_string()
    );
    let event = state
        .audit_events
        .lock()
        .iter()
        .find(|event| event.kind == "highlight_changed" && event.payload["action"] == "next")
        .cloned()
        .unwrap();
    assert_eq!(event.payload["state"]["active"], false);
    assert_eq!(event.payload["state"]["mode"], "step");
    assert_eq!(
        event.payload["state"]["remembered"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[tokio::test]
async fn rest_prev_next_all_change_the_real_selection_while_high_remains_independent() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
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
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Live step source".into(),
                fixtures: fixture_ids.clone(),
                ..Default::default()
            }],
            ..EngineSnapshot::default()
        })
        .unwrap();
    state.programmers.select(session.id, fixture_ids.clone());

    let before_next = state.application_events.latest_sequence();
    let next = post_highlight_action(&app, &token, "next").await;
    assert_eq!(next["active"], false);
    assert_eq!(next["mode"], "step");
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        fixture_ids[..1]
    );
    assert_programming_selection_event(
        &state,
        &session,
        before_next,
        light_application::ActionSource::Http,
        &fixture_ids[..1],
    );
    verify_bootstrapped_step_highlight(&app, &state, &session, fixture_ids[0]).await;

    let before_all = state.application_events.latest_sequence();
    let all = post_highlight_action(&app, &token, "all").await;
    assert_eq!(all["active"], false);
    assert_eq!(all["mode"], "selection");
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        fixture_ids
    );
    assert_programming_selection_event(
        &state,
        &session,
        before_all,
        light_application::ActionSource::Http,
        &fixture_ids,
    );

    let previous = post_highlight_action(&app, &token, "previous").await;
    assert_eq!(previous["active"], false);
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        fixture_ids[2..]
    );
    let before_high = state.application_events.latest_sequence();
    let high = post_highlight_action(&app, &token, "on").await;
    assert_eq!(high["active"], true);
    assert_eq!(high["mode"], "step");
    assert_eq!(state.engine.highlighted_fixtures(), fixture_ids[2..]);
    assert_eq!(state.application_events.latest_sequence(), before_high);

    // An external selection write resets the step basis without toggling HIGH, including when
    // the new source is live. Editing that Group before ALL is then re-resolved at action time.
    state.programmers.select_expression(
        session.id,
        fixture_ids.clone(),
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "1".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    reconcile_highlight_selection(&state, &session, "test_external_group_selection");
    assert_eq!(
        state
            .engine
            .highlighted_fixtures()
            .into_iter()
            .collect::<HashSet<_>>(),
        fixture_ids.iter().copied().collect::<HashSet<_>>()
    );
    let stepped = post_highlight_action(&app, &token, "next").await;
    assert_eq!(stepped["active"], true);
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        fixture_ids[..1]
    );

    let mut snapshot = (*state.engine.snapshot()).clone();
    snapshot.groups[0].fixtures = vec![fixture_ids[2], fixture_ids[1]];
    state.engine.replace_snapshot(snapshot).unwrap();
    let restored = post_highlight_action(&app, &token, "all").await;
    assert_eq!(restored["active"], true);
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        vec![fixture_ids[2], fixture_ids[1]]
    );
    assert!(matches!(
        state
            .programmers
            .get(session.id)
            .unwrap()
            .selection_expression,
        Some(light_programmer::SelectionExpression::LiveGroup { ref group_id, .. })
            if group_id == "1"
    ));

    // HIGH remains on with an empty actual selection, produces no output, and automatically
    // follows the next external selection without another toggle.
    state.programmers.select(session.id, []);
    reconcile_highlight_selection(&state, &session, "test_clear_selection");
    assert!(state.engine.highlighted_fixtures().is_empty());
    let status = current_highlight_transition(&state, &session).unwrap();
    assert!(status.state.active);
    state.programmers.select(session.id, [fixture_ids[1]]);
    reconcile_highlight_selection(&state, &session, "test_new_selection");
    assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_ids[1]]);

    let before_off = state.application_events.latest_sequence();
    let off = post_highlight_action(&app, &token, "off").await;
    assert_eq!(off["active"], false);
    assert_eq!(state.application_events.latest_sequence(), before_off);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn rest_highlight_status_publishes_only_an_authoritative_selection_repair() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
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
    post_highlight_action(&app, &token, "next").await;

    let mut snapshot = (*state.engine.snapshot()).clone();
    snapshot.fixtures.remove(0);
    state.engine.replace_snapshot(snapshot).unwrap();
    write_desk_lock(
        &state,
        session.desk.id,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();
    let before_status = state.application_events.latest_sequence();
    let status = app
        .clone()
        .oneshot(
            Request::get("/api/v1/highlight")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status = json(status).await;
    assert_eq!(status["active_fixture"]["fixture_id"], fixture_ids[1].0.to_string());
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        fixture_ids[1..2]
    );
    assert_programming_selection_event(
        &state,
        &session,
        before_status,
        light_application::ActionSource::Http,
        &fixture_ids[1..2],
    );

    // GET remains a reconciliation endpoint while the desk is locked; it must not publish when
    // the authoritative projection is already current.
    let before_no_op = state.application_events.latest_sequence();
    let no_op = app
        .oneshot(
            Request::get("/api/v1/highlight")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_op.status(), StatusCode::OK);
    assert_eq!(state.application_events.latest_sequence(), before_no_op);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn same_user_same_desk_highlight_survives_one_session_close_and_clears_with_the_last() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (first_token, first_session_id) = login(&app, "Operator").await;
    let first_session_id = SessionId(Uuid::parse_str(&first_session_id).unwrap());
    let first_session = state.sessions.read()[&first_session_id].clone();
    let second_login = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username":"Operator",
                        "desk_id":first_session.desk.id,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_login.status(), StatusCode::OK);
    let second_login = json(second_login).await;
    let second_token = second_login["token"].as_str().unwrap().to_owned();
    let second_session_id =
        SessionId(Uuid::parse_str(second_login["session_id"].as_str().unwrap()).unwrap());
    let second_session = state.sessions.read()[&second_session_id].clone();
    assert_eq!(second_session.user.id, first_session.user.id);
    assert_eq!(second_session.desk.id, first_session.desk.id);

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
    state
        .programmers
        .select(first_session.id, fixture_ids.clone());
    let activated = app
        .clone()
        .oneshot(
            Request::post("/api/v1/highlight/action")
                .header(header::AUTHORIZATION, format!("Bearer {first_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"action":"on"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(activated.status(), StatusCode::OK);
    assert_eq!(
        state
            .engine
            .highlighted_fixtures()
            .into_iter()
            .collect::<HashSet<_>>(),
        fixture_ids.iter().copied().collect::<HashSet<_>>()
    );

    let shared = app
        .clone()
        .oneshot(
            Request::get("/api/v1/highlight")
                .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(shared.status(), StatusCode::OK);
    let shared = json(shared).await;
    assert_eq!(shared["active"], true);
    assert_eq!(shared["remembered"].as_array().unwrap().len(), 3);

    let first_closed = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v1/sessions/{}", first_session.id.0))
                .header(header::AUTHORIZATION, format!("Bearer {first_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_closed.status(), StatusCode::NO_CONTENT);
    let after_one_close = app
        .clone()
        .oneshot(
            Request::get("/api/v1/highlight")
                .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let after_one_close = json(after_one_close).await;
    assert_eq!(after_one_close["active"], true);
    assert_eq!(
        state
            .engine
            .highlighted_fixtures()
            .into_iter()
            .collect::<HashSet<_>>(),
        fixture_ids.iter().copied().collect::<HashSet<_>>()
    );

    let final_closed = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v1/sessions/{}", second_session.id.0))
                .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(final_closed.status(), StatusCode::NO_CONTENT);
    let summaries = highlight_fixture_summaries(&state.engine.snapshot().fixtures);
    let selection = light_programmer::ProgrammerSelection::default();
    let cleared = state.highlight.status(
        first_session.desk.id,
        first_session.user.id,
        Some(&first_session.user.name),
        &selection,
        &summaries,
        &HashMap::new(),
        false,
    );
    assert!(!cleared.state.active);
    assert!(cleared.state.remembered.is_empty());
    assert!(cleared.output_fixtures.is_empty());
    assert!(state.engine.highlighted_fixtures().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}
