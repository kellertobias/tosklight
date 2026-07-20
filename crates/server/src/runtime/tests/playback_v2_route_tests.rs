use super::*;

#[tokio::test]
async fn v2_playback_action_is_desk_scoped_typed_and_idempotent() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);

    let denied = post_action(
        &app,
        None,
        desk_id,
        action_request("denied", 1, serde_json::json!({"type":"go","pressed":true})),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json(denied).await["kind"], "unauthorized");

    let wrong_desk = post_action(
        &app,
        Some(&token),
        Uuid::new_v4(),
        action_request(
            "wrong-desk",
            1,
            serde_json::json!({"type":"go","pressed":true}),
        ),
    )
    .await;
    assert_eq!(wrong_desk.status(), StatusCode::FORBIDDEN);

    let request = action_request(
        "go-playback-1",
        1,
        serde_json::json!({"type":"go","pressed":true}),
    );
    let cursor = state.application_events.latest_sequence();
    let first = post_action(&app, Some(&token), desk_id, request.clone()).await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = json(first).await;
    assert_eq!(first["outcome"]["status"], "applied");
    assert_eq!(first["durability"], "durable");
    assert_eq!(first["projection"]["requested"]["playback_number"], 1);
    assert_eq!(first["projection"]["target"], "cue_list");
    assert_eq!(first["projection"]["runtime"]["current"]["number"], 1.0);
    assert_eq!(
        first["projection"]["scope"]["show_id"],
        active_show_id(&state).to_string()
    );
    assert_eq!(first["projection"]["scope"]["show_revision"], 0);
    assert_eq!(first["event_sequence"], cursor + 1);
    assert_eq!(first["replayed"], false);

    let replay = post_action(&app, Some(&token), desk_id, request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["event_sequence"], cursor + 1);
    assert_eq!(replay["replayed"], true);
    assert_eq!(state.application_events.latest_sequence(), cursor + 1);

    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(cursor, &light_application::EventFilter::for_desk(desk_id))
    else {
        panic!("playback event should be retained");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    assert_eq!(
        events[0].correlation_id.unwrap().to_string(),
        first["correlation_id"]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_virtual_activation_returns_and_emits_one_transition_per_changed_playback() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_playback_test_state(&state, desk_id);

    let first = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "activate-zone-peer",
            1,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let cursor = state.application_events.latest_sequence();
    let request = action_request(
        "activate-zone-winner",
        3,
        serde_json::json!({"type":"on","pressed":true}),
    );
    let outcome = json(post_action(&app, Some(&token), desk_id, request.clone()).await).await;

    let primary_sequence = outcome["event_sequence"].as_u64().unwrap();
    assert_eq!(primary_sequence, cursor + 2);
    assert_eq!(outcome["related"].as_array().unwrap().len(), 1);
    assert_eq!(
        outcome["related"][0]["projection"]["requested"],
        serde_json::json!({"kind":"playback","playback_number":1})
    );
    assert_eq!(outcome["related"][0]["event_sequence"], cursor + 1);
    assert_eq!(
        outcome["related"][0]["projection"]["runtime"]["enabled"],
        false
    );
    assert_eq!(playback_event_objects(&state, cursor), vec![1, 3]);
    let after_mutation = state.application_events.latest_sequence();

    let replay = json(post_action(&app, Some(&token), desk_id, request).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["related"], outcome["related"]);
    assert_eq!(state.application_events.latest_sequence(), after_mutation);

    let toggle_off = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            action_request(
                "toggle-zone-winner-off",
                3,
                serde_json::json!({"type":"toggle","pressed":true}),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(toggle_off["outcome"]["status"], "applied");
    assert_eq!(toggle_off["related"], serde_json::json!([]));
    assert!(enabled_playback_numbers(&state).is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_virtual_exclusion_publishes_changed_peer_before_target_and_replays_nothing() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_exclusion_test_state(&state);
    set_pool_enabled(&state, 2, true);
    set_pool_enabled(&state, 4, true);
    put_virtual_exclusion_zone(&app, &token, &[1, 2, 3]).await;
    let cursor = state.application_events.latest_sequence();
    let request = action_request(
        "activate-zoned-playback",
        1,
        serde_json::json!({"type":"on","pressed":true}),
    );

    let response = post_action(&app, Some(&token), desk_id, request.clone()).await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 2);
    assert_eq!(playback_event_state(&events[0]), (2, false));
    assert_eq!(playback_event_state(&events[1]), (1, true));
    let correlation = Uuid::parse_str(response["correlation_id"].as_str().unwrap()).unwrap();
    assert!(
        events
            .iter()
            .all(|event| event.correlation_id == Some(correlation))
    );
    assert_eq!(response["related"].as_array().unwrap().len(), 1);
    assert_eq!(
        response["related"][0]["projection"]["requested"]["playback_number"],
        2
    );
    assert_eq!(
        response["related"][0]["projection"]["runtime"]["enabled"],
        false
    );
    assert_eq!(response["related"][0]["event_sequence"], events[0].sequence);
    assert_eq!(response["event_sequence"], events[1].sequence);
    assert_eq!(
        response["event_sequence"].as_u64(),
        Some(state.application_events.latest_sequence())
    );
    assert!(!pool_is_enabled(&state, 3));
    assert!(pool_is_enabled(&state, 4));

    let after_first = state.application_events.latest_sequence();
    let replay = json(post_action(&app, Some(&token), desk_id, request).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["related"], response["related"]);
    assert_eq!(replay["event_sequence"], response["event_sequence"]);
    assert!(playback_runtime_events(&state, after_first).is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_auto_off_publishes_related_release_before_primary_high_water() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_auto_off_test_state(&state);
    set_pool_enabled(&state, 1, true);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let cursor = state.application_events.latest_sequence();

    let response = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "activate-auto-off-covering-playback",
            2,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 2);
    assert_eq!(playback_event_state(&events[0]), (1, false));
    assert_eq!(playback_event_state(&events[1]), (2, true));
    assert_eq!(response["related"].as_array().unwrap().len(), 1);
    assert_eq!(
        response["related"][0]["projection"]["requested"]["playback_number"],
        1
    );
    assert_eq!(
        response["related"][0]["projection"]["runtime"]["enabled"],
        false
    );
    assert_eq!(response["related"][0]["event_sequence"], events[0].sequence);
    assert_eq!(response["event_sequence"], events[1].sequence);
    assert_eq!(
        response["event_sequence"].as_u64(),
        Some(state.application_events.latest_sequence())
    );
    assert_eq!(enabled_playback_numbers(&state), vec![2]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_crossfade_activation_uses_the_same_atomic_exclusion_transition_set() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_exclusion_test_state(&state);
    set_pool_enabled(&state, 2, true);
    put_virtual_exclusion_zone(&app, &token, &[1, 2]).await;
    let cursor = state.application_events.latest_sequence();

    let response = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "crossfade-zoned-playback",
            1,
            serde_json::json!({"type":"crossfade","enabled":true}),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 2);
    assert_eq!(playback_event_state(&events[0]), (2, false));
    assert_eq!(playback_event_state(&events[1]), (1, true));
    assert_eq!(response["related"][0]["event_sequence"], events[0].sequence);
    assert_eq!(response["event_sequence"], events[1].sequence);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_flash_release_promotion_publishes_its_exclusion_peer() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_exclusion_test_state(&state);
    update_virtual_definition(&state, 2, |definition| {
        definition.flash_release = light_playback::FlashReleaseMode::ReleaseIntensityOnly;
    });
    set_pool_enabled(&state, 1, true);
    put_virtual_exclusion_zone(&app, &token, &[1, 2]).await;

    let press = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            action_request(
                "hold-zoned-flash",
                2,
                serde_json::json!({"type":"flash","pressed":true}),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(press["related"], serde_json::json!([]));
    assert!(pool_is_enabled(&state, 1));
    let cursor = state.application_events.latest_sequence();

    let release = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            action_request(
                "release-zoned-flash",
                2,
                serde_json::json!({"type":"flash","pressed":false}),
            ),
        )
        .await,
    )
    .await;

    assert_eq!(release["related"].as_array().unwrap().len(), 1);
    assert_eq!(playback_event_objects(&state, cursor), vec![1, 2]);
    assert_eq!(enabled_playback_numbers(&state), vec![2]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_zero_manual_xfade_activation_publishes_its_exclusion_peer() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_exclusion_test_state(&state);
    update_virtual_definition(&state, 2, |definition| {
        definition.fader = light_playback::PlaybackFaderMode::XFade;
    });
    set_pool_enabled(&state, 1, true);
    put_virtual_exclusion_zone(&app, &token, &[1, 2]).await;
    let cursor = state.application_events.latest_sequence();

    let response = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            action_request(
                "zero-xfade-zoned-playback",
                2,
                serde_json::json!({"type":"master","value":0.0}),
            ),
        )
        .await,
    )
    .await;

    assert_eq!(response["related"].as_array().unwrap().len(), 1);
    assert_eq!(playback_event_objects(&state, cursor), vec![1, 2]);
    assert_eq!(enabled_playback_numbers(&state), vec![2]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_explicit_non_current_page_does_not_borrow_virtual_exclusions() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_exclusion_test_state(&state);
    add_explicit_virtual_page(&state);
    set_pool_enabled(&state, 2, true);
    put_virtual_exclusion_zone(&app, &token, &[1, 2]).await;
    let cursor = state.application_events.latest_sequence();

    let request = serde_json::json!({
        "request_id":"explicit-non-current-zone-member",
        "address":{"kind":"explicit_page","page":2,"slot":1},
        "action":{"type":"on","pressed":true},
        "surface":"virtual"
    });
    let response = post_action(&app, Some(&token), desk_id, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["related"], serde_json::json!([]));
    assert_eq!(enabled_playback_numbers(&state), vec![1, 2]);
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 1);
    assert_eq!(playback_event_state(&events[0]), (1, true));
    assert_eq!(response["event_sequence"], events[0].sequence);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_virtual_exclusion_configuration_is_desk_scoped() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (first_token, _) = login(&app, "Operator").await;
    let second_desk = state
        .desk
        .lock()
        .add_desk("Playback v2 wing", "playback-v2-wing")
        .unwrap();
    let second_token = login_playback_user_on_desk(&app, "Operator", second_desk.id).await;
    open_playback_test_show(&app, &first_token).await;
    install_virtual_exclusion_test_state(&state);
    set_pool_enabled(&state, 2, true);
    put_virtual_exclusion_zone(&app, &first_token, &[1, 2]).await;
    let cursor = state.application_events.latest_sequence();

    let response = post_action(
        &app,
        Some(&second_token),
        second_desk.id,
        action_request(
            "activate-from-unconfigured-desk",
            1,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["related"], serde_json::json!([]));
    assert!(pool_is_enabled(&state, 2));
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 1);
    assert_eq!(playback_event_state(&events[0]), (1, true));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_group_selection_publishes_one_live_or_static_programming_event() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    let fixture = install_playback_test_state(&state);

    let cursor = state.application_events.latest_sequence();
    let request = action_request(
        "select-live-group",
        2,
        serde_json::json!({"type":"select","pressed":true}),
    );
    let response = post_action(&app, Some(&token), desk_id, request.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    let events = playback_selection_events(&state, desk_id, cursor);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].correlation_id.unwrap().to_string(),
        response["correlation_id"]
    );
    let selection = programming_selection(&events[0]);
    assert_eq!(selection.selected, vec![fixture]);
    assert!(matches!(
        selection.expression.as_ref(),
        Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. })
            if group_id == "front"
    ));

    let after_live = state.application_events.latest_sequence();
    let replay = post_action(&app, Some(&token), desk_id, request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert!(json(replay).await["replayed"].as_bool().unwrap());
    assert!(playback_selection_events(&state, desk_id, after_live).is_empty());

    let cursor = state.application_events.latest_sequence();
    let response = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "select-static-group",
            2,
            serde_json::json!({"type":"select_dereferenced","pressed":true}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let events = playback_selection_events(&state, desk_id, cursor);
    assert_eq!(events.len(), 1);
    let selection = programming_selection(&events[0]);
    assert_eq!(selection.selected, vec![fixture]);
    assert!(matches!(
        selection.expression.as_ref(),
        Some(light_programmer::SelectionExpression::Static)
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn legacy_http_group_playback_selection_uses_the_same_typed_event_boundary() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    let fixture = install_playback_test_state(&state);
    let cursor = state.application_events.latest_sequence();

    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/playback-pool/2/select")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let events = playback_selection_events(&state, desk_id, cursor);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    let selection = programming_selection(&events[0]);
    assert_eq!(selection.selected, vec![fixture]);
    assert!(matches!(
        selection.expression.as_ref(),
        Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. })
            if group_id == "front"
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn osc_group_playback_selection_uses_the_same_typed_event_boundary() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .unwrap();
    let desk_id = session.desk.id;
    open_playback_test_show(&app, &token).await;
    let fixture = install_playback_test_state(&state);
    let source: SocketAddr = "127.0.0.1:9020".parse().unwrap();
    state.osc_subscribers.lock().insert(
        "playback-selection-test".into(),
        OscSubscriber {
            desk_alias: session.desk.osc_alias.clone(),
            target: source,
            command_source: source,
            session_id: session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    let cursor = state.application_events.latest_sequence();

    handle_playback_osc(
        &state,
        "/light/playback/2/select",
        &[OscArgument::Bool(true)],
        Some("127.0.0.1:9020"),
    );

    let events = playback_selection_events(&state, desk_id, cursor);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Osc)
    );
    let selection = programming_selection(&events[0]);
    assert_eq!(selection.selected, vec![fixture]);
    assert!(matches!(
        selection.expression.as_ref(),
        Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. })
            if group_id == "front"
    ));

    let after_press = state.application_events.latest_sequence();
    handle_playback_osc(
        &state,
        "/light/playback/2/select",
        &[OscArgument::Bool(false)],
        Some("127.0.0.1:9020"),
    );
    assert!(playback_selection_events(&state, desk_id, after_press).is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_snapshot_returns_only_requested_runtime_and_a_pre_read_cursor() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    let cursor = state.application_events.latest_sequence();

    let response = app
        .oneshot(
            Request::post(format!("/api/v2/desks/{desk_id}/playback-runtime/snapshot"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "identities": [{"kind":"playback","playback_number":2}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let snapshot = json(response).await;
    assert_eq!(snapshot["cursor"]["sequence"], cursor);
    assert_eq!(snapshot["desk"]["desk_id"], desk_id.to_string());
    assert_eq!(
        snapshot["desk"]["scope"]["show_id"],
        active_show_id(&state).to_string()
    );
    assert_eq!(snapshot["desk"]["scope"]["show_revision"], 0);
    assert_eq!(snapshot["projections"].as_array().unwrap().len(), 1);
    assert_eq!(
        snapshot["projections"][0]["requested"],
        serde_json::json!({"kind":"playback","playback_number":2})
    );
    assert_eq!(snapshot["projections"][0]["target"], "group");
    assert_eq!(
        snapshot["projections"][0]["scope"],
        snapshot["desk"]["scope"]
    );
    assert!(snapshot.to_string().find("playback_number\":1").is_none());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_snapshot_allows_a_desk_only_request() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;

    let response = app
        .oneshot(
            Request::post(format!("/api/v2/desks/{desk_id}/playback-runtime/snapshot"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"identities":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let snapshot = json(response).await;
    assert_eq!(snapshot["desk"]["desk_id"], desk_id.to_string());
    assert_eq!(snapshot["projections"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_playback_rejects_forged_sources_control_ids_and_no_change_emits_nothing() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    let cursor = state.application_events.latest_sequence();

    for invalid in [
        serde_json::json!({
            "request_id":"forged-source",
            "address":{"kind":"playback","playback_number":1},
            "action":{"type":"go","pressed":true},
            "surface":"osc"
        }),
        action_request(
            "line\nbreak",
            1,
            serde_json::json!({"type":"go","pressed":true}),
        ),
    ] {
        let response = post_action(&app, Some(&token), desk_id, invalid).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(json(response).await["kind"], "invalid");
    }

    let no_change = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "released-go",
            1,
            serde_json::json!({"type":"go","pressed":false}),
        ),
    )
    .await;
    assert_eq!(no_change.status(), StatusCode::OK);
    let no_change = json(no_change).await;
    assert_eq!(no_change["outcome"]["status"], "no_change");
    assert!(no_change["event_sequence"].is_null());
    assert_eq!(state.application_events.latest_sequence(), cursor);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn captured_preload_queue_is_replay_safe_snapshot_owned_and_drained_once_by_go() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "enter-queue",
        "preload_entered",
    )
    .await;

    let mut request = action_request(
        "capture-queue-once",
        1,
        serde_json::json!({"type":"go","pressed":true}),
    );
    request["surface"] = "physical".into();
    let first = json(post_action(&app, Some(&token), session.desk.id, request.clone()).await).await;
    assert_eq!(first["outcome"]["status"], "captured");
    let events = preload_queue_events(&state, session.user.id.0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].desk_id, None);
    assert_eq!(
        events[0].delivery,
        light_application::DeliveryPolicy::Replaceable
    );
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    assert_eq!(
        events[0].correlation_id.unwrap().to_string(),
        first["correlation_id"]
    );
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::PreloadPlaybackQueueChanged(change),
    ) = &events[0].payload
    else {
        panic!("captured action should publish the typed queue projection")
    };
    assert_eq!(change.projection.revision, 1);
    let legacy = state
        .audit_events
        .lock()
        .iter()
        .find(|event| {
            event.kind == "programmer_changed"
                && event.payload.get("preload_playback_action").is_some()
        })
        .cloned()
        .unwrap();
    assert_eq!(legacy.payload["user_id"], session.user.id.0.to_string());
    assert_eq!(
        legacy.payload["changes"],
        serde_json::json!(["preload_playback_queue"])
    );
    let replay = json(post_action(&app, Some(&token), session.desk.id, request).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(preload_queue_events(&state, session.user.id.0).len(), 1);

    let snapshot = json(preload_queue_snapshot(&app, Some(&token), session.user.id.0).await).await;
    assert_eq!(snapshot["projection"]["revision"], 1);
    assert_eq!(
        snapshot["projection"]["actions"],
        serde_json::json!([{
            "playback_number": 1,
            "action": "go",
            "surface": "physical",
        }])
    );
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "drain-queue",
        "preload_committed",
    )
    .await;
    assert_eq!(preload_queue_events(&state, session.user.id.0).len(), 2);
    let snapshot = json(preload_queue_snapshot(&app, Some(&token), session.user.id.0).await).await;
    assert_eq!(snapshot["projection"]["revision"], 2);
    assert_eq!(snapshot["projection"]["actions"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn failed_preload_go_rolls_back_queue_generation_and_emits_no_projection() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "enter-failed-queue",
        "preload_entered",
    )
    .await;
    let mut capture = action_request(
        "capture-before-failure",
        1,
        serde_json::json!({"type":"go","pressed":true}),
    );
    capture["surface"] = "physical".into();
    let response = post_action(&app, Some(&token), session.desk.id, capture).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(preload_queue_events(&state, session.user.id.0).len(), 1);
    state
        .engine
        .replace_snapshot(EngineSnapshot::default())
        .unwrap();

    assert_preload_key(&app, &token, session.desk.id, "failed-queue-go", "rejected").await;
    assert_eq!(preload_queue_events(&state, session.user.id.0).len(), 1);
    let snapshot = json(preload_queue_snapshot(&app, Some(&token), session.user.id.0).await).await;
    assert_eq!(snapshot["projection"]["revision"], 1);
    assert_eq!(
        snapshot["projection"]["actions"].as_array().unwrap().len(),
        1
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

fn action_request(
    request_id: &str,
    playback_number: u16,
    action: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "address": {"kind":"playback","playback_number":playback_number},
        "action": action,
        "surface": "virtual"
    })
}

async fn post_action(
    app: &Router,
    token: Option<&str>,
    desk_id: Uuid,
    request: serde_json::Value,
) -> Response {
    let mut builder = Request::post(format!("/api/v2/desks/{desk_id}/playback-actions"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::from(request.to_string())).unwrap())
        .await
        .unwrap()
}

async fn assert_preload_key(
    app: &Router,
    token: &str,
    desk_id: Uuid,
    request_id: &str,
    expected: &str,
) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v2/desks/{desk_id}/command-line/keys"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "key":"PRE",
                        "phase":"press",
                        "request_id":request_id,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    if expected == "rejected" {
        assert_eq!(response["outcome"], "rejected");
    } else {
        assert_eq!(response["action"], expected, "{response}");
    }
}

async fn preload_queue_snapshot(app: &Router, token: Option<&str>, user_id: Uuid) -> Response {
    let mut request = Request::get(format!(
        "/api/v2/users/{user_id}/programmer-preload-playback-queue/snapshot"
    ));
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn preload_queue_events(
    state: &AppState,
    user_id: Uuid,
) -> Vec<std::sync::Arc<light_application::EventEnvelope>> {
    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_preload_playback_queue(user_id));
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &filter)
    else {
        panic!("queue events should remain replayable")
    };
    events
}

fn session_desk_id(state: &AppState, token: &str) -> Uuid {
    state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .unwrap()
        .desk
        .id
}

fn session_for_token(state: &AppState, token: &str) -> Session {
    state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .cloned()
        .unwrap()
}

fn active_show_id(state: &AppState) -> Uuid {
    state.active_show.read().as_ref().unwrap().id.0
}

fn install_virtual_playback_test_state(state: &AppState, desk_id: Uuid) {
    install_playback_test_state(state);
    let cue_list_id = state.engine.snapshot().cue_lists[0].id;
    let mut snapshot = (*state.engine.snapshot()).clone();
    snapshot.playbacks.push(playback_test_definition(
        3,
        light_playback::PlaybackTarget::CueList { cue_list_id },
    ));
    snapshot.playback_pages = vec![light_playback::PlaybackPage {
        number: 1,
        name: "Virtual".into(),
        slots: HashMap::from([(1, 1), (2, 3)]),
    }];
    state.engine.replace_snapshot(snapshot).unwrap();
    let store = VirtualPlaybackExclusionStore::from([(
        desk_id.to_string(),
        VirtualPlaybackExclusionSurfaces::from([(
            "test-surface".into(),
            vec![VirtualPlaybackExclusionZone {
                id: "zone-a".into(),
                name: "Zone A".into(),
                slots: vec![1, 2],
            }],
        )]),
    )]);
    let show_id = state.active_show.read().as_ref().unwrap().id;
    state
        .desk
        .lock()
        .set_setting(
            &virtual_playback_exclusion_setting(show_id),
            &serde_json::to_string(&store).unwrap(),
        )
        .unwrap();
}

fn playback_event_objects(state: &AppState, after: u64) -> Vec<u16> {
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(after, &light_application::EventFilter::default())
    else {
        panic!("Playback events should remain replayable")
    };
    events
        .iter()
        .filter_map(|event| {
            let object = event.object.as_ref()?;
            (object.capability == light_application::EventCapability::Playback)
                .then(|| object.id.strip_prefix("playback:")?.parse().ok())
                .flatten()
        })
        .collect()
}

fn enabled_playback_numbers(state: &AppState) -> Vec<u16> {
    state
        .engine
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter_map(|playback| playback.playback_number)
        .collect()
}

async fn open_playback_test_show(app: &Router, token: &str) {
    let show = create_show(app, token, "Playback v2 show").await;
    let response = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v1/shows/{}/open",
                show["id"].as_str().unwrap()
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"transition":"hold_current"}"#))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

fn install_playback_test_state(state: &AppState) -> light_core::FixtureId {
    let cue_list = playback_test_cue_list();
    let cue_list_id = cue_list.id;
    let fixture = light_core::FixtureId::new();
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list],
            playbacks: vec![
                playback_test_definition(
                    1,
                    light_playback::PlaybackTarget::CueList { cue_list_id },
                ),
                playback_test_definition(
                    2,
                    light_playback::PlaybackTarget::Group {
                        group_id: "front".into(),
                    },
                ),
            ],
            groups: vec![light_programmer::GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                fixtures: vec![fixture],
                master: 0.75,
                ..light_programmer::GroupDefinition::default()
            }],
            ..EngineSnapshot::default()
        })
        .unwrap();
    fixture
}

fn install_virtual_exclusion_test_state(state: &AppState) {
    let cue_list = playback_test_cue_list();
    let cue_list_id = cue_list.id;
    let playbacks = (1..=4)
        .map(|number| {
            let mut definition = playback_test_definition(
                number,
                light_playback::PlaybackTarget::CueList { cue_list_id },
            );
            definition.auto_off = false;
            definition
        })
        .collect();
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list],
            playbacks,
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                slots: std::collections::HashMap::from([(1, 1), (2, 2), (3, 3), (4, 4)]),
            }],
            ..EngineSnapshot::default()
        })
        .unwrap();
}

fn install_auto_off_test_state(state: &AppState) {
    let fixture = light_core::FixtureId::new();
    let mut first = playback_test_cue_list();
    first.name = "Auto-off source".into();
    first.cues[0].changes.push(light_playback::CueChange::set(
        fixture,
        light_core::AttributeKey("pan".into()),
        light_core::AttributeValue::Normalized(0.2),
    ));
    let mut second = playback_test_cue_list();
    second.name = "Covering playback".into();
    second.cues[0].changes.push(light_playback::CueChange::set(
        fixture,
        light_core::AttributeKey("pan".into()),
        light_core::AttributeValue::Normalized(0.8),
    ));
    let first_id = first.id;
    let second_id = second.id;
    let first_definition = playback_test_definition(
        1,
        light_playback::PlaybackTarget::CueList {
            cue_list_id: first_id,
        },
    );
    let mut second_definition = playback_test_definition(
        2,
        light_playback::PlaybackTarget::CueList {
            cue_list_id: second_id,
        },
    );
    second_definition.auto_off = false;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![first, second],
            playbacks: vec![first_definition, second_definition],
            ..EngineSnapshot::default()
        })
        .unwrap();
}

fn update_virtual_definition(
    state: &AppState,
    number: u16,
    update: impl FnOnce(&mut light_playback::PlaybackDefinition),
) {
    let mut snapshot = (*state.engine.snapshot()).clone();
    let definition = snapshot
        .playbacks
        .iter_mut()
        .find(|definition| definition.number == number)
        .unwrap();
    update(definition);
    state.engine.replace_snapshot(snapshot).unwrap();
}

fn add_explicit_virtual_page(state: &AppState) {
    let mut snapshot = (*state.engine.snapshot()).clone();
    snapshot.playback_pages.push(light_playback::PlaybackPage {
        number: 2,
        name: "Explicit".into(),
        slots: std::collections::HashMap::from([(1, 1)]),
    });
    state.engine.replace_snapshot(snapshot).unwrap();
}

fn set_pool_enabled(state: &AppState, number: u16, enabled: bool) {
    state
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::Pool {
            number,
            action: if enabled {
                light_engine::PoolPlaybackAction::On
            } else {
                light_engine::PoolPlaybackAction::Off
            },
        })
        .unwrap();
}

fn pool_is_enabled(state: &AppState, number: u16) -> bool {
    state
        .engine
        .playback_runtime()
        .iter()
        .any(|playback| playback.playback_number == Some(number) && playback.enabled)
}

async fn put_virtual_exclusion_zone(app: &Router, token: &str, slots: &[u8]) {
    let response = app
        .clone()
        .oneshot(
            Request::put("/api/v1/virtual-playback-exclusion-zones/v2-route-test")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "zones":[{"id":"v2-route-zone","name":"v2 route zone","slots":slots}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn login_playback_user_on_desk(app: &Router, username: &str, desk_id: Uuid) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"username":username,"desk_id":desk_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await["token"].as_str().unwrap().to_owned()
}

fn playback_runtime_events(
    state: &AppState,
    after: u64,
) -> Vec<Arc<light_application::EventEnvelope>> {
    let filter = light_application::EventFilter::default()
        .with_capability(light_application::EventCapability::Playback);
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(after, &filter)
    else {
        panic!("Playback events should remain replayable")
    };
    events
}

fn playback_event_state(event: &light_application::EventEnvelope) -> (u16, bool) {
    let light_application::ApplicationEvent::Playback(
        light_application::PlaybackEvent::RuntimeChanged(change),
    ) = &event.payload
    else {
        panic!("expected a Playback runtime event")
    };
    (
        change.projection.playback_number.unwrap(),
        change.projection.cue_list_runtime().unwrap().enabled,
    )
}

fn playback_selection_events(
    state: &AppState,
    desk_id: Uuid,
    after: u64,
) -> Vec<Arc<light_application::EventEnvelope>> {
    let filter = light_application::EventFilter::for_desk(desk_id).with_object(
        light_application::EventObject::programming_selection(desk_id),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(after, &filter)
    else {
        panic!("selection events should remain replayable")
    };
    events
}

fn programming_selection(
    event: &light_application::EventEnvelope,
) -> &light_programmer::ProgrammerSelection {
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::InteractionChanged(change),
    ) = &event.payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    change.selection().unwrap()
}

fn playback_test_cue_list() -> light_playback::CueList {
    light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Main".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![light_playback::Cue::new(1.0), light_playback::Cue::new(2.0)],
    }
}

fn playback_test_definition(
    number: u16,
    target: light_playback::PlaybackTarget,
) -> light_playback::PlaybackDefinition {
    light_playback::PlaybackDefinition {
        number,
        name: format!("Playback {number}"),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: light_playback::PlaybackDefinition::default_fader(&target),
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target,
    }
}
