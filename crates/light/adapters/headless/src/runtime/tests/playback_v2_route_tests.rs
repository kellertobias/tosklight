use super::*;

#[tokio::test]
async fn v2_playback_overview_is_authenticated_and_keeps_desk_scope() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);

    let denied = app
        .clone()
        .oneshot(
            Request::get("/api/v2/playback-overview")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    let overview = app
        .oneshot(
            Request::get("/api/v2/playback-overview")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(overview.status(), StatusCode::OK);
    let overview = json(overview).await;
    assert_eq!(overview["desk"]["id"], desk_id.to_string());
    assert_eq!(overview["active_page"], 1);
    assert_eq!(overview["pool"].as_array().unwrap().len(), 2);
    assert_eq!(overview["cue_lists"].as_array().unwrap().len(), 1);
    assert!(overview["authoritative_controls"].is_object());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn plain_playback_urls_are_fire_and_forget_and_keep_page_addressing_distinct() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    let mut snapshot = (*state.output.snapshot()).clone();
    snapshot.playback_pages = vec![light_playback::PlaybackPage {
        number: 1,
        name: "Main".into(),
        slots: std::collections::HashMap::from([(1, 1)]),
        virtual_playbacks: std::collections::HashMap::new(),
    }]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();
    let show_id = active_show_id(&state);

    let call = |path: &str| {
        Request::get(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header("x-tosk-show", show_id.to_string())
            .body(Body::empty())
            .unwrap()
    };
    let numbered = app
        .clone()
        .oneshot(call("/api/v2/playbacks/1/on"))
        .await
        .unwrap();
    assert_eq!(numbered.status(), StatusCode::OK);
    assert_eq!(
        numbered.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let numbered = json(numbered).await;
    assert!(numbered.get("request_id").is_none());
    assert!(numbered.get("replayed").is_none());

    let current = json(
        app.clone()
            .oneshot(call("/api/v2/playbacks/current/1/off"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(current["requested"]["kind"], "current_page");
    assert_eq!(current["resolved"]["playback_number"], 1);

    let explicit = json(
        app.clone()
            .oneshot(call("/api/v2/playbacks/pages/1/1/on"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(explicit["requested"]["kind"], "explicit_page");
    assert_eq!(explicit["resolved"]["page"], 1);
    assert_eq!(explicit["resolved"]["slot"], 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_context_headers_default_to_the_single_or_main_control_desk() {
    let (single_state, single_data_dir) = test_state();
    let single_app = router(single_state.clone());
    let (single_token, _) = login(&single_app, "Operator").await;
    let single_desk = session_desk_id(&single_state, &single_token);
    open_playback_test_show(&single_app, &single_token).await;
    let single = single_app
        .oneshot(
            Request::post("/api/v2/playback-runtime/snapshot")
                .header(header::AUTHORIZATION, format!("Bearer {single_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"identities":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(single.status(), StatusCode::OK);
    assert_eq!(
        json(single).await["desk"]["desk_id"],
        single_desk.to_string()
    );

    let (main_state, main_data_dir) = test_state();
    let wing = main_state.installation.add_desk("A wing", "wing").unwrap();
    let main = main_state.installation.add_desk("Z main", "main").unwrap();
    let main_app = router(main_state.clone());
    let main_token = login_playback_user_on_desk(&main_app, "Operator", main.id).await;
    open_playback_test_show(&main_app, &main_token).await;
    let defaulted = main_app
        .oneshot(
            Request::post("/api/v2/playback-runtime/snapshot")
                .header(header::AUTHORIZATION, format!("Bearer {main_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"identities":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(defaulted.status(), StatusCode::OK);
    assert_eq!(
        json(defaulted).await["desk"]["desk_id"],
        main.id.to_string()
    );
    assert_ne!(wing.id, main.id);
    let _ = std::fs::remove_dir_all(single_data_dir);
    let _ = std::fs::remove_dir_all(main_data_dir);
}

#[tokio::test]
async fn v2_scoped_playback_action_rejects_stale_show_before_execution() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    let show_id = active_show_id(&state);

    let accepted = post_scoped_action(
        &app,
        Some(&token),
        show_id,
        desk_id,
        action_request(
            "scoped-on",
            1,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let cursor = state.events.latest_sequence();

    let rejected = post_scoped_action(
        &app,
        Some(&token),
        Uuid::new_v4(),
        desk_id,
        action_request(
            "stale-show-off",
            1,
            serde_json::json!({"type":"off","pressed":true}),
        ),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(rejected).await["error"],
        "X-Tosk-Show does not match the active show"
    );
    assert_eq!(state.events.latest_sequence(), cursor);
    let _ = std::fs::remove_dir_all(data_dir);
}

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
    assert_eq!(wrong_desk.status(), StatusCode::NOT_FOUND);

    let request = action_request(
        "go-playback-1",
        1,
        serde_json::json!({"type":"go","pressed":true}),
    );
    let cursor = state.events.latest_sequence();
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
    assert_eq!(state.events.latest_sequence(), cursor + 1);

    let light_application::EventReplay::Events(events) = state
        .events
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
    install_virtual_playback_test_state(&state);

    let first = post_action(
        &app,
        Some(&token),
        desk_id,
        virtual_action_request(
            "activate-zone-peer",
            1,
            1_001,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;
    let first_status = first.status();
    let first_body = json(first).await;
    assert_eq!(
        first_status,
        StatusCode::OK,
        "virtual activation failed: {first_body}"
    );
    let cursor = state.events.latest_sequence();
    let request = virtual_action_request(
        "activate-zone-winner",
        1,
        1_002,
        serde_json::json!({"type":"on","pressed":true}),
    );
    let outcome = json(post_action(&app, Some(&token), desk_id, request.clone()).await).await;

    let primary_sequence = outcome["event_sequence"].as_u64().unwrap();
    assert_eq!(primary_sequence, cursor + 2);
    assert_eq!(outcome["related"].as_array().unwrap().len(), 1);
    assert_eq!(
        outcome["related"][0]["projection"]["requested"],
        serde_json::json!({"kind":"virtual","page":1,"playback_number":1001})
    );
    assert_eq!(outcome["related"][0]["event_sequence"], cursor + 1);
    assert_eq!(
        outcome["related"][0]["projection"]["runtime"]["enabled"],
        false
    );
    assert_eq!(
        playback_event_identities(&state, cursor),
        vec![
            light_application::PlaybackRuntimeIdentity::Virtual(
                light_playback::VirtualPlaybackAddress::new(1, 1_001).unwrap()
            ),
            light_application::PlaybackRuntimeIdentity::Virtual(
                light_playback::VirtualPlaybackAddress::new(1, 1_002).unwrap()
            ),
        ]
    );
    let after_mutation = state.events.latest_sequence();

    let replay = json(post_action(&app, Some(&token), desk_id, request).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["related"], outcome["related"]);
    assert_eq!(state.events.latest_sequence(), after_mutation);

    let toggle_off = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            virtual_action_request(
                "toggle-zone-winner-off",
                1,
                1_002,
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
    set_virtual_enabled(&state, 1, 1_002, true);
    set_virtual_enabled(&state, 1, 1_004, true);
    put_virtual_exclusion_zone(&state, &app, &token, &[1_001, 1_002, 1_003]).await;
    let cursor = state.events.latest_sequence();
    let request = virtual_action_request(
        "activate-zoned-playback",
        1,
        1_001,
        serde_json::json!({"type":"on","pressed":true}),
    );

    let response = post_action(&app, Some(&token), desk_id, request.clone()).await;

    let response_status = response.status();
    let response = json(response).await;
    assert_eq!(
        response_status,
        StatusCode::OK,
        "virtual exclusion activation failed: {response}"
    );
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 2);
    assert_eq!(playback_event_state(&events[0]), (1_002, false));
    assert_eq!(playback_event_state(&events[1]), (1_001, true));
    let correlation = Uuid::parse_str(response["correlation_id"].as_str().unwrap()).unwrap();
    assert!(
        events
            .iter()
            .all(|event| event.correlation_id == Some(correlation))
    );
    assert_eq!(response["related"].as_array().unwrap().len(), 1);
    assert_eq!(
        response["related"][0]["projection"]["requested"]["playback_number"],
        1_002
    );
    assert_eq!(
        response["related"][0]["projection"]["runtime"]["enabled"],
        false
    );
    assert_eq!(response["related"][0]["event_sequence"], events[0].sequence);
    assert_eq!(response["event_sequence"], events[1].sequence);
    assert_eq!(
        response["event_sequence"].as_u64(),
        Some(state.events.latest_sequence())
    );
    assert!(!virtual_is_enabled(&state, 1, 1_003));
    assert!(virtual_is_enabled(&state, 1, 1_004));

    let after_first = state.events.latest_sequence();
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
    let cursor = state.events.latest_sequence();

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
        Some(state.events.latest_sequence())
    );
    assert_eq!(enabled_playback_numbers(&state), vec![2]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_peer_only_auto_off_change_does_not_emit_an_equal_primary_event() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_auto_off_test_state(&state);
    update_pool_definition(&state, 1, |definition| definition.auto_off = false);
    set_pool_enabled(&state, 1, true);
    std::thread::sleep(std::time::Duration::from_millis(2));
    set_pool_enabled(&state, 2, true);
    update_pool_definition(&state, 1, |definition| definition.auto_off = true);
    let cursor = state.events.latest_sequence();
    let request = action_request(
        "repeat-covering-playback",
        2,
        serde_json::json!({"type":"on","pressed":true}),
    );

    let response = json(post_action(&app, Some(&token), desk_id, request.clone()).await).await;

    assert_eq!(response["outcome"]["status"], "applied");
    assert_eq!(response["related"].as_array().unwrap().len(), 1);
    assert_eq!(playback_event_objects(&state, cursor), vec![1]);
    assert_eq!(response["related"][0]["event_sequence"], cursor + 1);
    assert_eq!(response["event_sequence"], cursor + 1);
    assert_eq!(enabled_playback_numbers(&state), vec![2]);
    let replay_cursor = state.events.latest_sequence();
    let replay = json(post_action(&app, Some(&token), desk_id, request).await).await;
    assert_eq!(replay["replayed"], true);
    assert!(playback_runtime_events(&state, replay_cursor).is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_timed_crossfade_retrigger_emits_one_equal_projection_event() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_pool_cuelist_test_state(&state);
    update_pool_definition(&state, 1, |definition| definition.xfade_millis = 1_000);
    let first = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            action_request(
                "start-timed-crossfade",
                1,
                serde_json::json!({"type":"crossfade","enabled":true}),
            ),
        )
        .await,
    )
    .await;
    let cursor = state.events.latest_sequence();
    let request = action_request(
        "retrigger-timed-crossfade",
        1,
        serde_json::json!({"type":"crossfade","enabled":true}),
    );

    let response = json(post_action(&app, Some(&token), desk_id, request.clone()).await).await;

    assert_eq!(response["outcome"]["status"], "applied");
    assert_eq!(response["projection"], first["projection"]);
    assert_eq!(response["related"], serde_json::json!([]));
    assert_eq!(response["event_sequence"], cursor + 1);
    assert_eq!(playback_event_objects(&state, cursor), vec![1]);
    let replay_cursor = state.events.latest_sequence();
    let replay = json(post_action(&app, Some(&token), desk_id, request).await).await;
    assert_eq!(replay["replayed"], true);
    assert!(playback_runtime_events(&state, replay_cursor).is_empty());
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
    set_virtual_enabled(&state, 1, 1_002, true);
    put_virtual_exclusion_zone(&state, &app, &token, &[1_001, 1_002]).await;
    let cursor = state.events.latest_sequence();

    let response = post_action(
        &app,
        Some(&token),
        desk_id,
        virtual_action_request(
            "crossfade-zoned-playback",
            1,
            1_001,
            serde_json::json!({"type":"crossfade","enabled":true}),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 2);
    assert_eq!(playback_event_state(&events[0]), (1_002, false));
    assert_eq!(playback_event_state(&events[1]), (1_001, true));
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
    update_dedicated_virtual_definition(&state, 1, 1_002, |definition| {
        definition.flash_release = light_playback::FlashReleaseMode::ReleaseIntensityOnly;
    });
    set_virtual_enabled(&state, 1, 1_001, true);
    put_virtual_exclusion_zone(&state, &app, &token, &[1_001, 1_002]).await;
    let cursor = state.events.latest_sequence();

    let press = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            virtual_action_request(
                "hold-zoned-flash",
                1,
                1_002,
                serde_json::json!({"type":"flash","pressed":true}),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(press["related"].as_array().unwrap().len(), 1);
    assert!(!virtual_is_enabled(&state, 1, 1_001));

    let release = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            virtual_action_request(
                "release-zoned-flash",
                1,
                1_002,
                serde_json::json!({"type":"flash","pressed":false}),
            ),
        )
        .await,
    )
    .await;

    assert_eq!(release["related"], serde_json::json!([]));
    assert_eq!(
        playback_event_identities(&state, cursor),
        vec![
            light_application::PlaybackRuntimeIdentity::Virtual(
                light_playback::VirtualPlaybackAddress::new(1, 1_001).unwrap()
            ),
            light_application::PlaybackRuntimeIdentity::Virtual(
                light_playback::VirtualPlaybackAddress::new(1, 1_002).unwrap()
            ),
            light_application::PlaybackRuntimeIdentity::Virtual(
                light_playback::VirtualPlaybackAddress::new(1, 1_002).unwrap()
            ),
        ]
    );
    assert_eq!(enabled_playback_numbers(&state), vec![1_002]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_virtual_master_rejects_xfade_without_releasing_its_exclusion_peer() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_exclusion_test_state(&state);
    update_dedicated_virtual_definition(&state, 1, 1_002, |definition| {
        definition.fader = light_playback::PlaybackFaderMode::XFade;
    });
    set_virtual_enabled(&state, 1, 1_001, true);
    put_virtual_exclusion_zone(&state, &app, &token, &[1_001, 1_002]).await;
    let cursor = state.events.latest_sequence();

    let response = post_action(
        &app,
        Some(&token),
        desk_id,
        virtual_action_request(
            "zero-xfade-zoned-playback",
            1,
            1_002,
            serde_json::json!({"type":"master","value":0.0}),
        ),
    )
    .await;
    let response_status = response.status();
    let response = json(response).await;
    assert_eq!(response_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        response["error"],
        "virtual master requires the Master fader mode"
    );
    assert!(playback_event_identities(&state, cursor).is_empty());
    assert_eq!(enabled_playback_numbers(&state), vec![1_001]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_page_two_virtual_does_not_match_page_one_numbered_zone() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_virtual_exclusion_test_state(&state);
    add_explicit_virtual_page(&state);
    set_virtual_enabled(&state, 1, 1_002, true);
    put_virtual_exclusion_zone(&state, &app, &token, &[1_001, 1_002]).await;
    let cursor = state.events.latest_sequence();

    let request = serde_json::json!({
        "request_id":"explicit-non-current-zone-member",
        "address":{"kind":"virtual","page":2,"playback_number":1301},
        "action":{"type":"on","pressed":true},
        "surface":"virtual"
    });
    let response = post_action(&app, Some(&token), desk_id, request).await;

    let response_status = response.status();
    let response = json(response).await;
    assert_eq!(
        response_status,
        StatusCode::OK,
        "page-two virtual activation failed: {response}"
    );
    assert_eq!(response["related"], serde_json::json!([]));
    assert_eq!(enabled_playback_numbers(&state), vec![1_002, 1_301]);
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 1);
    assert_eq!(playback_event_state(&events[0]), (1_301, true));
    assert_eq!(response["event_sequence"], events[0].sequence);
    assert_eq!(
        state
            .output
            .playback_runtime()
            .into_iter()
            .find(|playback| playback.playback_number == Some(1_301))
            .unwrap()
            .activation
            .unwrap()
            .exclusion_scope,
        light_playback::PlaybackExclusionScope::None
    );
    let normalized = normalize_restored_virtual_playback_exclusions(&state).unwrap();
    assert!(normalized.released_playbacks.is_empty());
    assert_eq!(enabled_playback_numbers(&state), vec![1_002, 1_301]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_current_page_and_explicit_page_addresses_resolve_independently() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    let mut snapshot = (*state.output.snapshot()).clone();
    snapshot.playback_pages = vec![
        light_playback::PlaybackPage {
            number: 1,
            name: "Current".into(),
            slots: HashMap::from([(1, 1)]),
            virtual_playbacks: HashMap::new(),
        },
        light_playback::PlaybackPage {
            number: 2,
            name: "Explicit".into(),
            slots: HashMap::from([(1, 2)]),
            virtual_playbacks: HashMap::new(),
        },
    ]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();

    let current = post_action(
        &app,
        Some(&token),
        desk_id,
        serde_json::json!({
            "request_id":"current-page-slot",
            "address":{"kind":"current_page","slot":1},
            "action":{"type":"on","pressed":true},
            "surface":"physical"
        }),
    )
    .await;
    assert_eq!(current.status(), StatusCode::OK);
    let current = json(current).await;
    assert_eq!(current["requested"]["kind"], "current_page");
    assert_eq!(current["resolved"]["playback_number"], 1);

    let explicit = post_action(
        &app,
        Some(&token),
        desk_id,
        serde_json::json!({
            "request_id":"explicit-page-slot",
            "address":{"kind":"explicit_page","page":2,"slot":1},
            "action":{"type":"master","value":0.5},
            "surface":"physical"
        }),
    )
    .await;
    let explicit_status = explicit.status();
    let explicit = json(explicit).await;
    assert_eq!(
        explicit_status,
        StatusCode::OK,
        "explicit-page action failed: {explicit}"
    );
    assert_eq!(explicit["requested"]["kind"], "explicit_page");
    assert_eq!(explicit["resolved"]["playback_number"], 2);
    assert_eq!(explicit["resolved"]["page"], 2);
    assert_eq!(explicit["resolved"]["slot"], 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_shared_virtual_exclusion_configuration_applies_across_desks() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (first_token, _) = login(&app, "Operator").await;
    let second_desk = state
        .installation
        .add_desk("Playback v2 wing", "playback-v2-wing")
        .unwrap();
    let second_token = login_playback_user_on_desk(&app, "Operator", second_desk.id).await;
    open_playback_test_show(&app, &first_token).await;
    install_virtual_exclusion_test_state(&state);
    set_virtual_enabled(&state, 1, 1_002, true);
    put_virtual_exclusion_zone(&state, &app, &first_token, &[1_001, 1_002]).await;
    let cursor = state.events.latest_sequence();

    let response = post_action(
        &app,
        Some(&second_token),
        second_desk.id,
        virtual_action_request(
            "activate-from-second-desk",
            1,
            1_001,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["related"].as_array().unwrap().len(), 1);
    assert!(!virtual_is_enabled(&state, 1, 1_002));
    let events = playback_runtime_events(&state, cursor);
    assert_eq!(events.len(), 2);
    assert_eq!(playback_event_state(&events[0]), (1_002, false));
    assert_eq!(playback_event_state(&events[1]), (1_001, true));
    let activation = state
        .output
        .playback_runtime()
        .into_iter()
        .find(|playback| playback.playback_number == Some(1_001))
        .unwrap()
        .activation
        .unwrap();
    assert_eq!(activation.desk_id, Some(second_desk.id));
    assert_eq!(
        activation.surface,
        light_playback::PlaybackActivationSurface::Virtual
    );
    assert_eq!(
        activation.exclusion_scope,
        light_playback::PlaybackExclusionScope::Show
    );
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

    let cursor = state.events.latest_sequence();
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

    let after_live = state.events.latest_sequence();
    let replay = post_action(&app, Some(&token), desk_id, request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert!(json(replay).await["replayed"].as_bool().unwrap());
    assert!(playback_selection_events(&state, desk_id, after_live).is_empty());

    let cursor = state.events.latest_sequence();
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
async fn v2_http_group_playback_selection_uses_the_same_typed_event_boundary() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    let fixture = install_playback_test_state(&state);
    let cursor = state.events.latest_sequence();

    let response = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "select-live-group-v2",
            2,
            serde_json::json!({"type":"select","pressed":true}),
        ),
    )
    .await;

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
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    let desk_id = session.desk.id;
    open_playback_test_show(&app, &token).await;
    let fixture = install_playback_test_state(&state);
    let source: SocketAddr = "127.0.0.1:9020".parse().unwrap();
    state.integrations.register_osc_subscriber(
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
            selection_grid_all_started: None,
            last_highlight_action: None,
        },
    );
    let cursor = state.events.latest_sequence();

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

    let after_press = state.events.latest_sequence();
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
    let cursor = state.events.latest_sequence();

    let response = app
        .oneshot(
            Request::post("/api/v2/playback-runtime/snapshot")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-desk", desk_id.to_string())
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
async fn v2_group_runtime_actions_use_assigned_runtime_authority() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_group_runtime_test_state(&state);

    let cursor = state.events.latest_sequence();
    let assigned_request = group_action_request(
        "assigned-group-master",
        "front",
        serde_json::json!({"type":"master","value":0.5}),
    );
    let assigned = post_action(&app, Some(&token), desk_id, assigned_request.clone()).await;
    assert_eq!(assigned.status(), StatusCode::OK);
    let assigned = json(assigned).await;
    assert_eq!(
        assigned["requested"],
        serde_json::json!({"kind":"group","group_id":"front"})
    );
    assert_eq!(
        assigned["resolved"],
        serde_json::json!({"kind":"group","group_id":"front","playback_number":2})
    );
    assert_eq!(assigned["projection"]["requested"], assigned["requested"]);
    assert_eq!(assigned["projection"]["playback_number"], 2);
    assert_eq!(assigned["projection"]["target"], "group");
    assert_eq!(assigned["projection"]["master"], 0.5);
    assert_eq!(assigned["outcome"]["status"], "applied");
    let controls = authoritative_playback_controls(&state);
    assert_eq!(controls["groups"].as_array().unwrap().len(), 1);
    assert_eq!(controls["groups"][0]["id"], "front");
    assert_eq!(controls["groups"][0]["master"], 0.5);
    let by_group = playback_events_for_object(
        &state,
        cursor,
        light_application::EventObject::group("front"),
    );
    let by_playback =
        playback_events_for_object(&state, cursor, light_application::EventObject::playback(2));
    assert_eq!(by_group.len(), 1);
    assert_eq!(by_group[0].sequence, by_playback[0].sequence);
    assert_eq!(assigned["event_sequence"], by_group[0].sequence);

    let replay_cursor = state.events.latest_sequence();
    let replay = json(post_action(&app, Some(&token), desk_id, assigned_request).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], assigned["event_sequence"]);
    assert_eq!(state.events.latest_sequence(), replay_cursor);

    let no_change = json(
        post_action(
            &app,
            Some(&token),
            desk_id,
            group_action_request(
                "assigned-group-no-change",
                "front",
                serde_json::json!({"type":"master","value":0.5}),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(no_change["outcome"]["status"], "no_change");
    assert!(no_change["event_sequence"].is_null());
    assert_eq!(state.events.latest_sequence(), replay_cursor);

    assert_group_flash_phase(&app, &state, &token, desk_id, "front", true, 1.0).await;
    assert_group_flash_phase(&app, &state, &token, desk_id, "front", false, 0.0).await;

    let show_id = state.active_show.current().as_ref().unwrap().id;
    let output_key = output_runtime_setting(show_id);
    state
        .installation
        .set_setting(&output_key, "output-sentinel")
        .unwrap();
    let direct = post_action(
        &app,
        Some(&token),
        desk_id,
        group_action_request(
            "direct-unassigned-group-master",
            "side",
            serde_json::json!({"type":"master","value":0.4}),
        ),
    )
    .await;
    assert_eq!(direct.status(), StatusCode::BAD_REQUEST);
    assert_eq!(setting_value(&state, &output_key), "output-sentinel");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_group_snapshot_is_exact_and_rejects_foreign_or_invalid_identity() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_group_runtime_test_state(&state);

    let snapshot = post_playback_snapshot(
        &app,
        Some(&token),
        desk_id,
        serde_json::json!({"identities":[{"kind":"group","group_id":"front"}]}),
    )
    .await;
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot = json(snapshot).await;
    assert_eq!(snapshot["projections"].as_array().unwrap().len(), 1);
    assert_eq!(
        snapshot["projections"][0]["requested"],
        serde_json::json!({"kind":"group","group_id":"front"})
    );
    assert_eq!(snapshot["projections"][0]["playback_number"], 2);
    assert_eq!(snapshot["projections"][0]["master"], 0.75);
    assert_eq!(snapshot["projections"][0]["flash_level"], 0.0);
    assert!(!snapshot.to_string().contains("side"));
    assert!(!snapshot.to_string().contains("cue_list"));

    let denied = post_playback_snapshot(
        &app,
        Some(&token),
        Uuid::new_v4(),
        serde_json::json!({"identities":[{"kind":"group","group_id":"front"}]}),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    for group_id in [String::new(), "front\n".into(), "x".repeat(257)] {
        let rejected = post_playback_snapshot(
            &app,
            Some(&token),
            desk_id,
            serde_json::json!({"identities":[{"kind":"group","group_id":group_id}]}),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
        assert_eq!(json(rejected).await["kind"], "invalid");
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_group_actions_reject_unsupported_missing_and_wrong_assignments() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_group_runtime_test_state(&state);
    let cursor = state.events.latest_sequence();

    for request in [
        group_action_request(
            "unsupported-group-action",
            "front",
            serde_json::json!({"type":"go","pressed":true}),
        ),
        group_action_request(
            "missing-group",
            "missing",
            serde_json::json!({"type":"flash","pressed":true}),
        ),
    ] {
        let rejected = post_action(&app, Some(&token), desk_id, request).await;
        assert!(
            matches!(
                rejected.status(),
                StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
            ),
            "unexpected status {}",
            rejected.status()
        );
    }

    set_group_playback_assignment(&state, "side", Some(9));
    let missing_assignment = post_action(
        &app,
        Some(&token),
        desk_id,
        group_action_request(
            "missing-group-playback",
            "side",
            serde_json::json!({"type":"master","value":0.4}),
        ),
    )
    .await;
    assert_eq!(missing_assignment.status(), StatusCode::CONFLICT);

    set_group_playback_assignment(&state, "side", Some(1));
    let wrong_assignment = post_action(
        &app,
        Some(&token),
        desk_id,
        group_action_request(
            "wrong-group-playback",
            "side",
            serde_json::json!({"type":"flash","pressed":true}),
        ),
    )
    .await;
    assert_eq!(wrong_assignment.status(), StatusCode::CONFLICT);
    assert_eq!(state.events.latest_sequence(), cursor);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn playback_originated_group_change_uses_the_same_group_event_route() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    let cursor = state.events.latest_sequence();

    let response = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "playback-originated-group-master",
            2,
            serde_json::json!({"type":"master","value":0.4}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let by_group = playback_events_for_object(
        &state,
        cursor,
        light_application::EventObject::group("front"),
    );
    let by_playback =
        playback_events_for_object(&state, cursor, light_application::EventObject::playback(2));
    assert_eq!(by_group.len(), 1);
    assert_eq!(by_group[0].sequence, by_playback[0].sequence);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_group_runtime_preserves_peer_desk_scope_and_rejects_stale_show() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (first_token, _) = login(&app, "Operator").await;
    let first_desk = session_desk_id(&state, &first_token);
    let second_desk = state
        .installation
        .add_desk("Group runtime wing", "group-runtime-wing")
        .unwrap();
    let second_token = login_playback_user_on_desk(&app, "Operator", second_desk.id).await;
    open_playback_test_show(&app, &first_token).await;
    install_group_runtime_test_state(&state);
    let cursor = state.events.latest_sequence();

    let foreign_desk = post_action(
        &app,
        Some(&first_token),
        second_desk.id,
        group_action_request(
            "foreign-desk-group-master",
            "side",
            serde_json::json!({"type":"master","value":0.3}),
        ),
    )
    .await;
    assert_eq!(foreign_desk.status(), StatusCode::FORBIDDEN);

    let stale_show = post_scoped_action(
        &app,
        Some(&second_token),
        Uuid::new_v4(),
        second_desk.id,
        group_action_request(
            "stale-show-group-master",
            "side",
            serde_json::json!({"type":"master","value":0.3}),
        ),
    )
    .await;
    assert_eq!(stale_show.status(), StatusCode::CONFLICT);
    assert_eq!(state.events.latest_sequence(), cursor);

    let accepted = post_action(
        &app,
        Some(&second_token),
        second_desk.id,
        group_action_request(
            "peer-desk-group-master",
            "side",
            serde_json::json!({"type":"master","value":0.3}),
        ),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted = json(accepted).await;
    assert_eq!(accepted["projection"]["master"], 0.3);

    let first_filter = light_application::EventFilter::for_desk(first_desk)
        .with_object(light_application::EventObject::group("side"));
    let second_filter = light_application::EventFilter::for_desk(second_desk.id)
        .with_object(light_application::EventObject::group("side"));
    let light_application::EventReplay::Events(first_events) =
        state.events.replay(cursor, &first_filter)
    else {
        panic!("Playback events should remain replayable")
    };
    let light_application::EventReplay::Events(second_events) =
        state.events.replay(cursor, &second_filter)
    else {
        panic!("Playback events should remain replayable")
    };
    assert_eq!(first_events.len(), 1);
    assert_eq!(second_events.len(), 1);
    assert_eq!(first_events[0].sequence, second_events[0].sequence);

    for (token, desk_id) in [(&first_token, first_desk), (&second_token, second_desk.id)] {
        let snapshot = post_playback_snapshot(
            &app,
            Some(token),
            desk_id,
            serde_json::json!({"identities":[{"kind":"group","group_id":"side"}]}),
        )
        .await;
        assert_eq!(snapshot.status(), StatusCode::OK);
        assert_eq!(json(snapshot).await["projections"][0]["master"], 0.3);
    }
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
            Request::post("/api/v2/playback-runtime/snapshot")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-desk", desk_id.to_string())
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
    let cursor = state.events.latest_sequence();

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
    assert_eq!(state.events.latest_sequence(), cursor);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_explicit_playback_states_report_exact_repeat_no_changes() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);

    for (name, action) in [
        ("on", serde_json::json!({"type":"on","pressed":true})),
        ("master", serde_json::json!({"type":"master","value":0.5})),
        ("load", serde_json::json!({"type":"load","cue_number":1.0})),
        (
            "temporary",
            serde_json::json!({"type":"temporary","enabled":true,"pressed":true}),
        ),
        (
            "crossfade",
            serde_json::json!({"type":"crossfade","enabled":true}),
        ),
    ] {
        assert_repeat_is_no_change(&app, &state, &token, desk_id, name, action).await;
    }
    assert_action_is_no_change(
        &app,
        &state,
        &token,
        desk_id,
        "same-group-master",
        2,
        serde_json::json!({"type":"master","value":0.75}),
    )
    .await;
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_playback_actions_persist_only_their_owned_runtime_domain() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let desk_id = session_desk_id(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    let show_id = state.active_show.current().as_ref().unwrap().id;
    let active_key = active_playbacks_setting(show_id);
    let output_key = output_runtime_setting(show_id);

    set_runtime_sentinels(&state, &active_key, &output_key);
    assert_action_is_no_change(
        &app,
        &state,
        &token,
        desk_id,
        "same-group-master-domain",
        2,
        serde_json::json!({"type":"master","value":0.75}),
    )
    .await;
    assert_runtime_sentinels(&state, &active_key, &output_key);

    let flash = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "group-flash-domain",
            2,
            serde_json::json!({"type":"flash","pressed":true}),
        ),
    )
    .await;
    assert_eq!(flash.status(), StatusCode::OK);
    assert_runtime_sentinels(&state, &active_key, &output_key);

    let group_master = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "changed-group-master-domain",
            2,
            serde_json::json!({"type":"master","value":0.5}),
        ),
    )
    .await;
    assert_eq!(group_master.status(), StatusCode::OK);
    assert_eq!(setting_value(&state, &active_key), "active-sentinel");
    assert_ne!(setting_value(&state, &output_key), "output-sentinel");

    state
        .installation
        .set_setting(&output_key, "output-sentinel")
        .unwrap();
    let on = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "changed-cuelist-domain",
            1,
            serde_json::json!({"type":"on","pressed":true}),
        ),
    )
    .await;
    assert_eq!(on.status(), StatusCode::OK);
    assert_ne!(setting_value(&state, &active_key), "active-sentinel");
    assert_eq!(setting_value(&state, &output_key), "output-sentinel");

    set_runtime_sentinels(&state, &active_key, &output_key);
    assert_action_is_no_change(
        &app,
        &state,
        &token,
        desk_id,
        "repeat-on-domain",
        1,
        serde_json::json!({"type":"on","pressed":true}),
    )
    .await;
    assert_runtime_sentinels(&state, &active_key, &output_key);

    let temporary = post_action(
        &app,
        Some(&token),
        desk_id,
        action_request(
            "temporary-hold-domain",
            1,
            serde_json::json!({"type":"temporary","enabled":true,"pressed":true}),
        ),
    )
    .await;
    assert_eq!(temporary.status(), StatusCode::OK);
    assert_runtime_sentinels(&state, &active_key, &output_key);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn set_runtime_sentinels(state: &AppState, active_key: &str, output_key: &str) {
    state
        .installation
        .set_setting(active_key, "active-sentinel")
        .unwrap();
    state
        .installation
        .set_setting(output_key, "output-sentinel")
        .unwrap();
}

fn assert_runtime_sentinels(state: &AppState, active_key: &str, output_key: &str) {
    assert_eq!(setting_value(state, active_key), "active-sentinel");
    assert_eq!(setting_value(state, output_key), "output-sentinel");
}

fn setting_value(state: &AppState, key: &str) -> String {
    state.installation.setting(key).unwrap().unwrap()
}

async fn assert_repeat_is_no_change(
    app: &Router,
    state: &AppState,
    token: &str,
    desk_id: Uuid,
    name: &str,
    action: serde_json::Value,
) {
    let first = post_action(
        app,
        Some(token),
        desk_id,
        action_request(&format!("first-{name}"), 1, action.clone()),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(json(first).await["outcome"]["status"], "applied");
    let cursor = state.events.latest_sequence();

    let repeated = post_action(
        app,
        Some(token),
        desk_id,
        action_request(&format!("repeat-{name}"), 1, action),
    )
    .await;
    assert_eq!(repeated.status(), StatusCode::OK);
    let repeated = json(repeated).await;
    assert_eq!(repeated["outcome"]["status"], "no_change", "{name}");
    assert!(repeated["event_sequence"].is_null(), "{name}");
    assert_eq!(state.events.latest_sequence(), cursor, "{name}");
}

async fn assert_action_is_no_change(
    app: &Router,
    state: &AppState,
    token: &str,
    desk_id: Uuid,
    name: &str,
    playback_number: u16,
    action: serde_json::Value,
) {
    let cursor = state.events.latest_sequence();
    let response = post_action(
        app,
        Some(token),
        desk_id,
        action_request(name, playback_number, action),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["outcome"]["status"], "no_change", "{name}");
    assert!(response["event_sequence"].is_null(), "{name}");
    assert_eq!(state.events.latest_sequence(), cursor, "{name}");
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
        .events
        .audit_events()
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
    let show_id = state.active_show.current().as_ref().unwrap().id;
    let active_key = active_playbacks_setting(show_id);
    let output_key = output_runtime_setting(show_id);
    set_runtime_sentinels(&state, &active_key, &output_key);
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "drain-queue",
        "preload_committed",
    )
    .await;
    assert_ne!(setting_value(&state, &active_key), "active-sentinel");
    assert_eq!(setting_value(&state, &output_key), "output-sentinel");
    assert_eq!(preload_queue_events(&state, session.user.id.0).len(), 2);
    let snapshot = json(preload_queue_snapshot(&app, Some(&token), session.user.id.0).await).await;
    assert_eq!(snapshot["projection"]["revision"], 2);
    assert_eq!(snapshot["projection"]["actions"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn all_no_op_preload_batch_drains_without_runtime_event_or_persistence() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    set_pool_enabled(&state, 1, true);
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "enter-no-op-queue",
        "preload_entered",
    )
    .await;

    let mut request = action_request(
        "capture-no-op-on",
        1,
        serde_json::json!({"type":"on","pressed":true}),
    );
    request["surface"] = "physical".into();
    let captured = post_action(&app, Some(&token), session.desk.id, request).await;
    assert_eq!(captured.status(), StatusCode::OK);
    assert_eq!(json(captured).await["outcome"]["status"], "captured");

    let show_id = state.active_show.current().as_ref().unwrap().id;
    let active_key = active_playbacks_setting(show_id);
    let output_key = output_runtime_setting(show_id);
    set_runtime_sentinels(&state, &active_key, &output_key);
    let cursor = state.events.latest_sequence();
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "commit-no-op-queue",
        "preload_committed",
    )
    .await;

    assert!(playback_event_objects(&state, cursor).is_empty());
    assert_runtime_sentinels(&state, &active_key, &output_key);
    let snapshot = json(preload_queue_snapshot(&app, Some(&token), session.user.id.0).await).await;
    assert_eq!(snapshot["projection"]["actions"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn transient_preload_cancellation_drains_without_runtime_event_or_persistence() {
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
        "enter-cancelled-queue",
        "preload_entered",
    )
    .await;

    for (request_id, enabled) in [("capture-temp-on", true), ("capture-temp-off", false)] {
        let mut request = action_request(
            request_id,
            1,
            serde_json::json!({"type":"temporary","enabled":enabled,"pressed":true}),
        );
        request["surface"] = "physical".into();
        let captured = post_action(&app, Some(&token), session.desk.id, request).await;
        assert_eq!(captured.status(), StatusCode::OK);
        assert_eq!(json(captured).await["outcome"]["status"], "captured");
    }

    let show_id = state.active_show.current().as_ref().unwrap().id;
    let active_key = active_playbacks_setting(show_id);
    let output_key = output_runtime_setting(show_id);
    set_runtime_sentinels(&state, &active_key, &output_key);
    let cursor = state.events.latest_sequence();
    let exclusion_count = state
        .events
        .audit_events()
        .iter()
        .filter(|event| event.kind == "playback_exclusion_applied")
        .count();
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "commit-cancelled-queue",
        "preload_committed",
    )
    .await;

    let committed_actions = state
        .events
        .audit_events()
        .iter()
        .rev()
        .find(|event| event.kind == "preload_committed")
        .unwrap()
        .payload["playback_actions"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(committed_actions, 2);
    assert!(playback_event_objects(&state, cursor).is_empty());
    assert_runtime_sentinels(&state, &active_key, &output_key);
    assert!(state.output.playback_runtime().is_empty());
    assert_eq!(
        state
            .events
            .audit_events()
            .iter()
            .filter(|event| event.kind == "playback_exclusion_applied")
            .count(),
        exclusion_count
    );
    let snapshot = json(preload_queue_snapshot(&app, Some(&token), session.user.id.0).await).await;
    assert_eq!(snapshot["projection"]["actions"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn same_user_preload_commit_keeps_captured_originating_desk_with_show_scope() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (first_token, _) = login(&app, "Operator").await;
    let first_session = session_for_token(&state, &first_token);
    let second_desk = state
        .installation
        .add_desk("Preload origin wing", "preload-origin-wing")
        .unwrap();
    let second_token = login_playback_user_on_desk(&app, "Operator", second_desk.id).await;
    open_playback_test_show(&app, &first_token).await;
    install_virtual_exclusion_test_state(&state);
    set_virtual_enabled(&state, 1, 1_002, true);
    put_virtual_exclusion_zone(&state, &app, &first_token, &[1_001, 1_002]).await;
    state.installation.update_configuration(|configuration| {
        configuration.preload_virtual_playback_actions = true;
    });
    assert_preload_key(
        &app,
        &second_token,
        second_desk.id,
        "enter-origin-queue",
        "preload_entered",
    )
    .await;
    let capture = virtual_action_request(
        "capture-from-second-desk",
        1,
        1_001,
        serde_json::json!({"type":"on","pressed":true}),
    );
    let captured = post_action(&app, Some(&second_token), second_desk.id, capture).await;
    assert_eq!(captured.status(), StatusCode::OK);
    assert_eq!(json(captured).await["outcome"]["status"], "captured");

    assert_preload_key(
        &app,
        &first_token,
        first_session.desk.id,
        "commit-origin-queue",
        "preload_committed",
    )
    .await;

    assert_eq!(enabled_playback_numbers(&state), vec![1_001]);
    let activation = state
        .output
        .playback_runtime()
        .into_iter()
        .find(|playback| playback.playback_number == Some(1_001))
        .unwrap()
        .activation
        .unwrap();
    assert_eq!(activation.desk_id, Some(second_desk.id));
    assert_eq!(
        activation.surface,
        light_playback::PlaybackActivationSurface::Virtual
    );
    assert_eq!(
        activation.exclusion_scope,
        light_playback::PlaybackExclusionScope::Show
    );
    let show_id = state.active_show.current().as_ref().unwrap().id;
    let persisted = state
        .installation
        .setting(&active_playbacks_setting(show_id))
        .unwrap()
        .unwrap();
    assert!(persisted.contains(&second_desk.id.to_string()));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn preload_hidden_addressed_change_emits_one_equal_projection_event() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_playback_test_state(&state);
    update_pool_definition(&state, 1, |definition| definition.xfade_millis = 1_000);
    set_pool_enabled(&state, 1, true);
    let crossfade = post_action(
        &app,
        Some(&token),
        session.desk.id,
        action_request(
            "arm-hidden-crossfade",
            1,
            serde_json::json!({"type":"crossfade","enabled":true}),
        ),
    )
    .await;
    assert_eq!(crossfade.status(), StatusCode::OK);
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "enter-hidden-queue",
        "preload_entered",
    )
    .await;
    let mut request = action_request(
        "capture-hidden-on",
        1,
        serde_json::json!({"type":"on","pressed":true}),
    );
    request["surface"] = "physical".into();
    let captured = post_action(&app, Some(&token), session.desk.id, request).await;
    assert_eq!(captured.status(), StatusCode::OK);
    assert_eq!(json(captured).await["outcome"]["status"], "captured");
    let cursor = state.events.latest_sequence();

    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "commit-hidden-queue",
        "preload_committed",
    )
    .await;

    assert_eq!(playback_event_objects(&state, cursor), vec![1]);
    let snapshot = json(preload_queue_snapshot(&app, Some(&token), session.user.id.0).await).await;
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
        .output
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

fn virtual_action_request(
    request_id: &str,
    page: u16,
    playback_number: u16,
    action: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "address": {
            "kind": "virtual",
            "page": page,
            "playback_number": playback_number
        },
        "action": action,
        "surface": "virtual"
    })
}

fn group_action_request(
    request_id: &str,
    group_id: &str,
    action: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "address": {"kind":"group","group_id":group_id},
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
    let mut builder = Request::post("/api/v2/playback-actions")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-tosk-desk", desk_id.to_string());
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::from(request.to_string())).unwrap())
        .await
        .unwrap()
}

async fn post_scoped_action(
    app: &Router,
    token: Option<&str>,
    show_id: Uuid,
    desk_id: Uuid,
    request: serde_json::Value,
) -> Response {
    let mut builder = Request::post("/api/v2/playback-actions")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-tosk-show", show_id.to_string())
        .header("x-tosk-desk", desk_id.to_string());
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::from(request.to_string())).unwrap())
        .await
        .unwrap()
}

async fn post_playback_snapshot(
    app: &Router,
    token: Option<&str>,
    desk_id: Uuid,
    request: serde_json::Value,
) -> Response {
    let mut builder = Request::post("/api/v2/playback-runtime/snapshot")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-tosk-desk", desk_id.to_string());
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
) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/command-line/keys")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-desk", desk_id.to_string())
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
    response
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
    let light_application::EventReplay::Events(events) = state.events.replay(0, &filter) else {
        panic!("queue events should remain replayable")
    };
    events
}

fn session_desk_id(state: &AppState, token: &str) -> Uuid {
    state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap()
        .desk
        .id
}

fn session_for_token(state: &AppState, token: &str) -> Session {
    state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap()
}

fn active_show_id(state: &AppState) -> Uuid {
    state.active_show.current().as_ref().unwrap().id.0
}

fn install_virtual_playback_test_state(state: &AppState) {
    install_playback_test_state(state);
    let mut snapshot = (*state.output.snapshot()).clone();
    let first = playback_test_cue_list();
    let first_id = first.id;
    let second = playback_test_cue_list();
    let second_id = second.id;
    std::sync::Arc::make_mut(&mut snapshot.cue_lists).extend([first, second]);
    snapshot.playback_pages = vec![light_playback::PlaybackPage {
        number: 1,
        name: "Virtual".into(),
        slots: HashMap::new(),
        virtual_playbacks: HashMap::from([
            (
                1_001,
                playback_test_definition(
                    1_001,
                    light_playback::PlaybackTarget::CueList {
                        cue_list_id: first_id,
                    },
                ),
            ),
            (
                1_002,
                playback_test_definition(
                    1_002,
                    light_playback::PlaybackTarget::CueList {
                        cue_list_id: second_id,
                    },
                ),
            ),
        ]),
    }]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();
    let store = test_virtual_playback_exclusion_store(vec![VirtualPlaybackExclusionZone {
        id: "zone-a".into(),
        name: "Zone A".into(),
        playback_numbers: vec![1_001, 1_002],
    }]);
    let show_id = state.active_show.current().as_ref().unwrap().id;
    persist_test_virtual_playback_exclusions(state, show_id, &store);
}

fn playback_event_identities(
    state: &AppState,
    after: u64,
) -> Vec<light_application::PlaybackRuntimeIdentity> {
    playback_runtime_events(state, after)
        .into_iter()
        .filter_map(|event| {
            let light_application::ApplicationEvent::Playback(
                light_application::PlaybackEvent::RuntimeChanged(change),
            ) = &event.payload
            else {
                return None;
            };
            Some(change.projection.requested.clone())
        })
        .collect()
}

fn playback_event_objects(state: &AppState, after: u64) -> Vec<u16> {
    let light_application::EventReplay::Events(events) = state
        .events
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
        .output
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
        .oneshot(open_show_request(token, show["id"].as_str().unwrap()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

fn install_playback_test_state(state: &AppState) -> light_core::FixtureId {
    let cue_list = playback_test_cue_list();
    let cue_list_id = cue_list.id;
    let fixture = light_core::FixtureId::new();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list].into(),
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
            ]
            .into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                fixtures: vec![fixture],
                master: 0.75,
                ..light_programmer::GroupDefinition::default()
            }]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    fixture
}

fn install_group_runtime_test_state(state: &AppState) {
    install_playback_test_state(state);
    let mut snapshot = (*state.output.snapshot()).clone();
    let groups = std::sync::Arc::make_mut(&mut snapshot.groups);
    groups
        .iter_mut()
        .find(|group| group.id == "front")
        .unwrap()
        .playback_fader = Some(2);
    groups.push(light_programmer::GroupDefinition {
        id: "side".into(),
        name: "Side".into(),
        master: 0.6,
        ..light_programmer::GroupDefinition::default()
    });
    state.output.replace_snapshot(snapshot).unwrap();
}

fn set_group_playback_assignment(state: &AppState, group_id: &str, playback: Option<u8>) {
    let mut snapshot = (*state.output.snapshot()).clone();
    std::sync::Arc::make_mut(&mut snapshot.groups)
        .iter_mut()
        .find(|group| group.id == group_id)
        .unwrap()
        .playback_fader = playback;
    state.output.replace_snapshot(snapshot).unwrap();
}

fn install_pool_cuelist_test_state(state: &AppState) {
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
        .collect::<Vec<_>>();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list].into(),
            playbacks: playbacks.into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                slots: std::collections::HashMap::from([(1, 1), (2, 2), (3, 3), (4, 4)]),
                virtual_playbacks: std::collections::HashMap::new(),
            }]
            .into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
}

fn install_virtual_exclusion_test_state(state: &AppState) {
    let cue_lists = (1_001..=1_004)
        .map(|number| (number, playback_test_cue_list()))
        .collect::<Vec<_>>();
    let virtual_playbacks = cue_lists
        .iter()
        .map(|(number, cue_list)| {
            let mut definition = playback_test_definition(
                *number,
                light_playback::PlaybackTarget::CueList {
                    cue_list_id: cue_list.id,
                },
            );
            definition.auto_off = false;
            (*number, definition)
        })
        .collect::<HashMap<_, _>>();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: cue_lists
                .into_iter()
                .map(|(_, cue_list)| cue_list)
                .collect::<Vec<_>>()
                .into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                slots: HashMap::new(),
                virtual_playbacks,
            }]
            .into(),
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
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![first, second].into(),
            playbacks: vec![first_definition, second_definition].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
}

fn update_pool_definition(
    state: &AppState,
    number: u16,
    update: impl FnOnce(&mut light_playback::PlaybackDefinition),
) {
    let mut snapshot = (*state.output.snapshot()).clone();
    let definition = std::sync::Arc::make_mut(&mut snapshot.playbacks)
        .iter_mut()
        .find(|definition| definition.number == number)
        .unwrap();
    update(definition);
    state.output.replace_snapshot(snapshot).unwrap();
}

fn update_dedicated_virtual_definition(
    state: &AppState,
    page: u8,
    number: u16,
    update: impl FnOnce(&mut light_playback::PlaybackDefinition),
) {
    let mut snapshot = (*state.output.snapshot()).clone();
    let definition = std::sync::Arc::make_mut(&mut snapshot.playback_pages)
        .iter_mut()
        .find(|candidate| candidate.number == page)
        .and_then(|candidate| candidate.virtual_playbacks.get_mut(&number))
        .unwrap();
    update(definition);
    state.output.replace_snapshot(snapshot).unwrap();
}

fn add_explicit_virtual_page(state: &AppState) {
    let mut snapshot = (*state.output.snapshot()).clone();
    let cue_list = playback_test_cue_list();
    let cue_list_id = cue_list.id;
    std::sync::Arc::make_mut(&mut snapshot.cue_lists).push(cue_list);
    std::sync::Arc::make_mut(&mut snapshot.playback_pages).push(light_playback::PlaybackPage {
        number: 2,
        name: "Explicit".into(),
        slots: HashMap::new(),
        virtual_playbacks: HashMap::from([(
            1_301,
            playback_test_definition(
                1_301,
                light_playback::PlaybackTarget::CueList { cue_list_id },
            ),
        )]),
    });
    state.output.replace_snapshot(snapshot).unwrap();
}

fn set_pool_enabled(state: &AppState, number: u16, enabled: bool) {
    state
        .output
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

fn set_virtual_enabled(state: &AppState, page: u8, number: u16, enabled: bool) {
    state
        .output
        .execute_playback(light_engine::EnginePlaybackCommand::Virtual {
            address: light_playback::VirtualPlaybackAddress::new(page, number).unwrap(),
            action: if enabled {
                light_engine::VirtualPlaybackAction::On
            } else {
                light_engine::VirtualPlaybackAction::Off
            },
            exclusion_zones: Vec::new(),
            activation_origin: None,
        })
        .unwrap();
}

fn virtual_is_enabled(state: &AppState, page: u8, number: u16) -> bool {
    state.output.playback_runtime().iter().any(|playback| {
        playback.enabled
            && playback.playback_identity
                == Some(light_playback::PlaybackIdentity::Virtual(
                    light_playback::VirtualPlaybackAddress::new(page, number).unwrap(),
                ))
    })
}

async fn put_virtual_exclusion_zone(
    state: &AppState,
    app: &Router,
    token: &str,
    playback_numbers: &[u16],
) {
    let show_id = state.active_show.current().as_ref().unwrap().id.0;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/virtual-playback-exclusion-zones/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": Uuid::new_v4().to_string(),
                        "expected_revision": 0,
                        "zones":[{
                            "id":"v2-route-zone",
                            "name":"v2 route zone",
                            "playback_numbers":playback_numbers
                        }]
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
            Request::post("/api/v2/sessions")
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
    let light_application::EventReplay::Events(events) = state.events.replay(after, &filter) else {
        panic!("Playback events should remain replayable")
    };
    events
}

fn playback_events_for_object(
    state: &AppState,
    after: u64,
    object: light_application::EventObject,
) -> Vec<Arc<light_application::EventEnvelope>> {
    let filter = light_application::EventFilter::default().with_object(object);
    let light_application::EventReplay::Events(events) = state.events.replay(after, &filter) else {
        panic!("Playback events should remain replayable")
    };
    events
}

async fn assert_group_flash_phase(
    app: &Router,
    state: &AppState,
    token: &str,
    desk_id: Uuid,
    group_id: &str,
    pressed: bool,
    expected_level: f64,
) {
    let cursor = state.events.latest_sequence();
    let response = post_action(
        app,
        Some(token),
        desk_id,
        group_action_request(
            &format!("{group_id}-flash-{pressed}"),
            group_id,
            serde_json::json!({"type":"flash","pressed":pressed}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["outcome"]["status"], "applied");
    assert_eq!(response["projection"]["flash_level"], expected_level);
    let events = playback_events_for_object(
        state,
        cursor,
        light_application::EventObject::group(group_id),
    );
    assert_eq!(events.len(), 1);
    assert_eq!(response["event_sequence"], events[0].sequence);
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
    let light_application::EventReplay::Events(events) = state.events.replay(after, &filter) else {
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
