fn cue_record_request(
    request_id: &str,
    target: serde_json::Value,
    cue_number: Option<f64>,
    capture_policy: &str,
    activation_policy: &str,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "target": target,
        "operation": "overwrite",
        "cue_number": cue_number,
        "timing": {},
        "cue_only": false,
        "name": null,
        "capture_policy": capture_policy,
        "activation_policy": activation_policy,
    })
}

fn set_cue_record_value(scenario: &CommandHttpScenario) -> light_core::FixtureId {
    let fixture = scenario.install_direct_fixture();
    scenario.state.programmers.set(
        scenario.session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.5),
    );
    fixture
}

fn active_show_revision(scenario: &CommandHttpScenario) -> u64 {
    let entry = scenario.state.active_show.read().clone().unwrap();
    ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap()
        .revision()
        .value()
}

#[tokio::test]
async fn cue_record_route_is_atomic_replay_safe_sparse_and_revisioned() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Cue record route").await;
    let fixture = set_cue_record_value(&scenario);
    let initial_revision = active_show_revision(&scenario);
    let baseline = scenario.state.application_events.latest_sequence();
    let compatibility_baseline = scenario.cue_list_compatibility_payloads().len();
    let request = cue_record_request(
        "cue-route-record",
        serde_json::json!({"kind":"pool","playback_number":27}),
        Some(2.5),
        "current_capture",
        "hold",
    );

    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(initial_revision),
            request.clone(),
        )
        .await;
    if response.status() != StatusCode::OK {
        panic!("Cue record failed: {}", json(response).await);
    }
    assert_eq!(
        response.headers()[header::ETAG],
        format!("\"{}\"", initial_revision + 1)
    );
    let first: light_wire::v2::cue_recording::CueRecordOutcome =
        serde_json::from_value(json(response).await).unwrap();
    let light_wire::v2::cue_recording::CueRecordOutcome::Changed {
        projections,
        recorded_cue,
        show_event_sequence,
        runtime,
        ..
    } = &first
    else {
        panic!("first Cue capture must change the show")
    };
    assert_eq!(recorded_cue.number, 2.5);
    assert_eq!(
        projections.cue_list.body["cues"][0]["changes"][0]["fixture_id"],
        serde_json::json!(fixture.0)
    );
    assert_eq!(projections.playback.as_ref().unwrap().id, "27");
    assert!(projections.page.is_none());
    assert!(runtime.is_none(), "pool recording with Hold must stay inactive");
    assert_eq!(*show_event_sequence, baseline + 1);
    assert_one_show_batch(&scenario.state, baseline, 2);
    let compatibility = scenario.cue_list_compatibility_payloads();
    assert_eq!(compatibility.len(), compatibility_baseline + 1);
    assert_eq!(
        compatibility.last().unwrap(),
        &serde_json::json!({
            "show_id": show_id,
            "kind": "cue_list",
            "id": projections.cue_list.id,
            "revision": projections.cue_list.revision,
            "application_event_sequence": show_event_sequence,
        })
    );

    let replay = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(initial_revision),
            request.clone(),
        )
        .await;
    let replay: light_wire::v2::cue_recording::CueRecordOutcome =
        serde_json::from_value(json(replay).await).unwrap();
    assert!(matches!(
        replay,
        light_wire::v2::cue_recording::CueRecordOutcome::Changed { replayed: true, .. }
    ));
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);
    assert_eq!(
        scenario.cue_list_compatibility_payloads().len(),
        compatibility_baseline + 1
    );

    let no_change = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(initial_revision + 1),
            cue_record_request(
                "cue-route-no-change",
                serde_json::json!({"kind":"pool","playback_number":27}),
                Some(2.5),
                "current_capture",
                "hold",
            ),
        )
        .await;
    let no_change: light_wire::v2::cue_recording::CueRecordOutcome =
        serde_json::from_value(json(no_change).await).unwrap();
    assert!(matches!(
        no_change,
        light_wire::v2::cue_recording::CueRecordOutcome::NoChange { .. }
    ));
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);
    assert_eq!(
        scenario.cue_list_compatibility_payloads().len(),
        compatibility_baseline + 1
    );

    let stale = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(initial_revision),
            cue_record_request(
                "cue-route-stale",
                serde_json::json!({"kind":"pool","playback_number":27}),
                Some(3.0),
                "current_capture",
                "hold",
            ),
        )
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        stale.headers()[header::ETAG],
        format!("\"{}\"", initial_revision + 1)
    );
    assert_eq!(
        json(stale).await["current_revision"],
        initial_revision + 1
    );

    let mut changed_replay = request;
    changed_replay["cue_number"] = 4.0.into();
    assert_eq!(
        scenario
            .cue_recording_action(
                &show_id,
                Some(&scenario.token),
                Some(initial_revision),
                changed_replay,
            )
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn cue_record_route_rejects_missing_authority_and_forged_context() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Cue record security").await;
    set_cue_record_value(&scenario);
    let request = cue_record_request(
        "cue-route-secure",
        serde_json::json!({"kind":"pool","playback_number":9}),
        Some(1.0),
        "current_capture",
        "hold",
    );
    assert_eq!(
        scenario
            .cue_recording_action(&show_id, None, Some(0), request.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        scenario
            .cue_recording_action(&show_id, Some(&scenario.token), None, request.clone())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        scenario
            .cue_recording_action("not-a-uuid", Some(&scenario.token), Some(0), request.clone())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    for field in ["values", "programmer", "selection", "user_id", "session_id", "desk_id"] {
        let mut forged = request.clone();
        forged[field] = serde_json::json!({"forged":true});
        assert_eq!(
            scenario
                .cue_recording_action(&show_id, Some(&scenario.token), Some(0), forged)
                .await
                .status(),
            StatusCode::BAD_REQUEST,
            "forged {field} was accepted"
        );
    }
    let mut unsafe_timing = request.clone();
    unsafe_timing["timing"]["fade_millis"] = serde_json::json!(9_007_199_254_740_992_u64);
    assert_eq!(
        scenario
            .cue_recording_action(
                &show_id,
                Some(&scenario.token),
                Some(0),
                unsafe_timing,
            )
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        scenario
            .cue_recording_action(
                &Uuid::new_v4().to_string(),
                Some(&scenario.token),
                Some(0),
                request,
            )
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    let ports = command_http::ServerProgrammingPorts::new(
        &scenario.state,
        &scenario.session,
        "cue_auth_test",
        true,
    );
    let valid = light_application::ActionContext::operator(
        scenario.session.desk.id,
        scenario.session.user.id.0,
        scenario.session.id.0,
        light_application::ActionSource::Http,
    );
    for forged in [
        light_application::ActionContext { user_id: Some(Uuid::new_v4()), ..valid.clone() },
        light_application::ActionContext { session_id: Some(Uuid::new_v4()), ..valid.clone() },
        light_application::ActionContext { desk_id: Uuid::new_v4(), ..valid.clone() },
    ] {
        let error = light_application::ProgrammingCueRecordingPorts::authorize_cue_recording(
            &ports, &forged,
        )
        .unwrap_err();
        assert_eq!(error.kind, light_application::ActionErrorKind::Forbidden);
    }
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn page_slot_recording_is_exact_activates_normal_and_holds_preload() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Cue page recording").await;
    set_cue_record_value(&scenario);
    let initial_revision = active_show_revision(&scenario);
    let baseline = scenario.state.application_events.latest_sequence();
    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(initial_revision),
            cue_record_request(
                "cue-page-normal",
                serde_json::json!({"kind":"page_slot","page":4,"slot":7}),
                None,
                "current_capture",
                "go_to_if_normal",
            ),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome: light_wire::v2::cue_recording::CueRecordOutcome =
        serde_json::from_value(json(response).await).unwrap();
    let light_wire::v2::cue_recording::CueRecordOutcome::Changed {
        captured_source,
        projections,
        show_event_sequence,
        runtime: Some(runtime),
        ..
    } = outcome
    else {
        panic!("normal page recording must return its activation")
    };
    assert_eq!(captured_source, light_wire::v2::cue_recording::CueRecordCapturedSource::Normal);
    assert_eq!(projections.page.as_ref().unwrap().id, "4");
    assert!(projections.page.as_ref().unwrap().body["slots"].get("7").is_some());
    assert_eq!(show_event_sequence, baseline + 1);
    assert_eq!(runtime.event_sequence, baseline + 2);
    assert_one_show_batch(&scenario.state, baseline, 3);

    assert!(scenario.state.programmers.set_preload_group(
        scenario.session.id,
        "front".into(),
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.7),
    ));
    let before_preload = scenario.state.application_events.latest_sequence();
    let preload = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(initial_revision + 1),
            cue_record_request(
                "cue-page-preload",
                serde_json::json!({"kind":"page_slot","page":4,"slot":8}),
                None,
                "pending_or_active_preload",
                "go_to_if_normal",
            ),
        )
        .await;
    let preload: light_wire::v2::cue_recording::CueRecordOutcome =
        serde_json::from_value(json(preload).await).unwrap();
    assert!(matches!(
        preload,
        light_wire::v2::cue_recording::CueRecordOutcome::Changed {
            captured_source: light_wire::v2::cue_recording::CueRecordCapturedSource::PendingPreload,
            runtime: None,
            ..
        }
    ));
    assert_eq!(scenario.state.application_events.latest_sequence(), before_preload + 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn already_current_cue_activation_can_complete_without_a_second_runtime_event() {
    let clock = Arc::new(ManualClock::new(chrono::Utc::now()));
    let scenario = CommandHttpScenario::with_clock(clock).await;
    let show_id = scenario.create_and_open_show("Already current Cue").await;
    let fixture = set_cue_record_value(&scenario);
    let first = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(active_show_revision(&scenario)),
            cue_record_request(
                "current-cue-first",
                serde_json::json!({"kind":"pool","playback_number":63}),
                Some(1.0),
                "current_capture",
                "go_to_if_normal",
            ),
        )
        .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = json(first).await;
    assert!(!first["runtime"].is_null());

    scenario.state.programmers.set(
        scenario.session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.6),
    );
    let revision = active_show_revision(&scenario);
    let baseline = scenario.state.application_events.latest_sequence();
    let request = cue_record_request(
        "current-cue-overwrite",
        serde_json::json!({"kind":"pool","playback_number":63}),
        Some(1.0),
        "current_capture",
        "go_to_if_normal",
    );
    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision),
            request.clone(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "changed");
    assert!(outcome["runtime"].is_null());
    assert_eq!(outcome["show_event_sequence"], baseline + 1);
    assert_one_show_batch(&scenario.state, baseline, 1);

    let after = scenario.state.application_events.latest_sequence();
    let replay = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision),
            request,
        )
        .await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert!(replay["runtime"].is_null());
    assert_eq!(scenario.state.application_events.latest_sequence(), after);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn cue_recording_shares_one_users_values_across_desks_and_isolates_another_user() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Cue record ownership").await;
    let fixture = set_cue_record_value(&scenario);
    let second_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Cue peer", "cue-peer")
        .unwrap();
    let (second_token, second_user) = login_on_desk(&scenario, "Operator", second_desk.id).await;
    assert_eq!(second_user, scenario.session.user.id.0);
    let initial_revision = active_show_revision(&scenario);

    let shared = scenario
        .cue_recording_action(
            &show_id,
            Some(&second_token),
            Some(initial_revision),
            cue_record_request(
                "cue-shared-user",
                serde_json::json!({"kind":"pool","playback_number":40}),
                Some(1.0),
                "current_capture",
                "hold",
            ),
        )
        .await;
    assert_eq!(shared.status(), StatusCode::OK);
    assert_eq!(recorded_fixture_value(&scenario, 40, fixture), 0.5);

    let other_user = scenario.state.desk.lock().add_user("Other Cue user").unwrap();
    let other_desk = scenario
        .state
        .desk
        .lock()
        .add_desk("Other Cue desk", "other-cue")
        .unwrap();
    let (other_token, logged_in_user) =
        login_on_desk(&scenario, "Other Cue user", other_desk.id).await;
    assert_eq!(logged_in_user, other_user.id.0);
    let other_session = scenario
        .state
        .sessions
        .read()
        .values()
        .find(|session| session.token == other_token)
        .cloned()
        .unwrap();
    scenario.state.programmers.set(
        other_session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.8),
    );

    let isolated = scenario
        .cue_recording_action(
            &show_id,
            Some(&other_token),
            Some(initial_revision + 1),
            cue_record_request(
                "cue-other-user",
                serde_json::json!({"kind":"pool","playback_number":41}),
                Some(1.0),
                "current_capture",
                "hold",
            ),
        )
        .await;
    assert_eq!(isolated.status(), StatusCode::OK);
    assert_eq!(recorded_fixture_value(&scenario, 40, fixture), 0.5);
    assert_eq!(recorded_fixture_value(&scenario, 41, fixture), 0.8);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn selected_playback_and_exact_cue_list_targets_do_not_readdress_the_recording() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Exact Cue targets").await;
    set_cue_record_value(&scenario);
    let revision = active_show_revision(&scenario);
    let pool = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision),
            cue_record_request(
                "cue-target-pool",
                serde_json::json!({"kind":"pool","playback_number":50}),
                Some(1.0),
                "current_capture",
                "hold",
            ),
        )
        .await;
    assert_eq!(pool.status(), StatusCode::OK);
    scenario
        .state
        .desk
        .lock()
        .set_selected_playback(
            scenario.session.desk.id,
            light_core::ShowId(Uuid::parse_str(&show_id).unwrap()),
            Some(50),
        )
        .unwrap();

    let selected = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision + 1),
            cue_record_request(
                "cue-target-selected",
                serde_json::json!({"kind":"selected_playback"}),
                Some(2.0),
                "current_capture",
                "hold",
            ),
        )
        .await;
    assert_eq!(selected.status(), StatusCode::OK);
    let cue_list_id = stored_cue_list(&scenario, 50).2.id;
    assert_eq!(stored_cue_list(&scenario, 50).2.cues.len(), 2);

    let exact = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision + 2),
            cue_record_request(
                "cue-target-exact",
                serde_json::json!({"kind":"cue_list","cue_list_id":cue_list_id.0}),
                Some(3.0),
                "current_capture",
                "hold",
            ),
        )
        .await;
    assert_eq!(exact.status(), StatusCode::OK);
    let exact: light_wire::v2::cue_recording::CueRecordOutcome =
        serde_json::from_value(json(exact).await).unwrap();
    let light_wire::v2::cue_recording::CueRecordOutcome::Changed { projections, .. } = exact else {
        panic!("the exact Cuelist recording must append its Cue")
    };
    assert!(projections.playback.is_none());
    assert!(projections.page.is_none());
    assert_eq!(stored_cue_list(&scenario, 50).2.cues.len(), 3);
    assert_eq!(scenario.state.engine.snapshot().playbacks.len(), 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn explicit_cue_address_ignores_divergent_active_cues_but_implicit_merge_rejects_them() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Divergent active Cues").await;
    let fixture = set_cue_record_value(&scenario);
    for (request_id, cue_number) in [("divergent-cue-1", 1.0), ("divergent-cue-2", 2.0)] {
        let response = scenario
            .cue_recording_action(
                &show_id,
                Some(&scenario.token),
                Some(active_show_revision(&scenario)),
                cue_record_request(
                    request_id,
                    serde_json::json!({"kind":"pool","playback_number":50}),
                    Some(cue_number),
                    "current_capture",
                    "hold",
                ),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let (mut second, _, cue_list) = stored_cue_list(&scenario, 50);
    second.number = 51;
    second.name = "Second active instance".into();
    let response = scenario
        .put_active_object(
            &show_id,
            "playback",
            "51",
            0,
            serde_json::to_value(second).unwrap(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    for (request_id, playback, cue_number) in
        [("divergent-go-50", 50, 1.0), ("divergent-go-51", 51, 2.0)]
    {
        let response = scenario
            .playback_go_to(request_id, playback, cue_number)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json(response).await["projection"]["runtime"]["current"]["number"],
            cue_number
        );
    }

    scenario.state.programmers.set(
        scenario.session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.6),
    );
    let baseline = scenario.state.application_events.latest_sequence();
    let revision = active_show_revision(&scenario);
    let mut explicit = cue_record_request(
        "divergent-explicit-merge",
        serde_json::json!({"kind":"cue_list","cue_list_id":cue_list.id.0}),
        Some(1.0),
        "current_capture",
        "hold",
    );
    explicit["operation"] = "merge".into();
    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision),
            explicit,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "changed");
    assert_eq!(outcome["show_revision"], revision + 1);
    assert_eq!(outcome["show_event_sequence"], baseline + 1);
    assert!(outcome["projections"]["playback"].is_null());
    assert!(outcome["projections"]["page"].is_null());
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);
    assert_one_show_batch(&scenario.state, baseline, 1);

    let mut implicit = cue_record_request(
        "divergent-implicit-merge",
        serde_json::json!({"kind":"cue_list","cue_list_id":cue_list.id.0}),
        None,
        "current_capture",
        "hold",
    );
    implicit["operation"] = "merge".into();
    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision + 1),
            implicit,
        )
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = json(response).await;
    assert_eq!(body["kind"], "conflict");
    assert_eq!(
        body["error"],
        "the Cuelist is active on multiple different Cues; supply an explicit Cue number"
    );
    assert_eq!(active_show_revision(&scenario), revision + 1);
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn active_preload_capture_persists_its_release_on_the_direct_v2_route() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Active Preload persistence").await;
    scenario.install_direct_fixture();
    assert!(scenario.state.programmers.set_preload_group(
        scenario.session.id,
        "1".into(),
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.7),
    ));
    assert!(scenario.state.programmers.activate_preload(scenario.session.id));
    persist_programmer(&scenario.state, &scenario.session).unwrap();
    assert!(!persisted_programmer_state(&scenario).preload_group_active.is_empty());

    let stale = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(active_show_revision(&scenario) + 1),
            cue_record_request(
                "active-preload-stale",
                serde_json::json!({"kind":"pool","playback_number":60}),
                Some(1.0),
                "pending_or_active_preload",
                "hold",
            ),
        )
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert!(!persisted_programmer_state(&scenario).preload_group_active.is_empty());

    let revision = active_show_revision(&scenario);
    let baseline = scenario.state.application_events.latest_sequence();
    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision),
            cue_record_request(
                "active-preload-record",
                serde_json::json!({"kind":"pool","playback_number":60}),
                Some(1.0),
                "pending_or_active_preload",
                "hold",
            ),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "changed");
    assert_eq!(outcome["captured_source"], "active_preload");
    assert_eq!(outcome["show_revision"], revision + 1);
    assert_eq!(outcome["show_event_sequence"], baseline + 1);
    assert_one_show_batch(&scenario.state, baseline, 2);
    assert_released_preload(&scenario.state.programmers.get(scenario.session.id).unwrap());
    assert_released_preload(&persisted_programmer_state(&scenario));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn v2_merge_no_change_and_empty_subtract_use_one_authoritative_action_each() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Cue operation mapping").await;
    let fixture = set_cue_record_value(&scenario);
    for (request_id, cue_number, value) in [
        ("operation-cue-1", 1.0, 0.5),
        ("operation-cue-2", 2.0, 0.7),
    ] {
        scenario.state.programmers.set(
            scenario.session.id,
            fixture,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(value),
        );
        let response = scenario
            .cue_recording_action(
                &show_id,
                Some(&scenario.token),
                Some(active_show_revision(&scenario)),
                cue_record_request(
                    request_id,
                    serde_json::json!({"kind":"pool","playback_number":61}),
                    Some(cue_number),
                    "current_capture",
                    "hold",
                ),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    scenario.state.programmers.set(
        scenario.session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.8),
    );
    let revision = active_show_revision(&scenario);
    let baseline = scenario.state.application_events.latest_sequence();
    let mut merge = cue_record_request(
        "operation-merge",
        serde_json::json!({"kind":"pool","playback_number":61}),
        Some(1.0),
        "current_capture",
        "hold",
    );
    merge["operation"] = "merge".into();
    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision),
            merge,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "changed");
    assert_eq!(outcome["show_revision"], revision + 1);
    assert_eq!(outcome["show_event_sequence"], baseline + 1);
    assert_one_show_batch(&scenario.state, baseline, 1);

    let after_merge = scenario.state.application_events.latest_sequence();
    let mut no_change = cue_record_request(
        "operation-merge-no-change",
        serde_json::json!({"kind":"pool","playback_number":61}),
        Some(1.0),
        "current_capture",
        "hold",
    );
    no_change["operation"] = "merge".into();
    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision + 1),
            no_change,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "no_change");
    assert_eq!(outcome["replayed"], false);
    assert_eq!(outcome["show_revision"], revision + 1);
    assert!(outcome.get("show_event_sequence").is_none());
    assert_eq!(scenario.state.application_events.latest_sequence(), after_merge);

    assert!(scenario.state.programmers.clear_normal_values(scenario.session.id));
    let baseline = scenario.state.application_events.latest_sequence();
    let mut subtract = cue_record_request(
        "operation-empty-subtract",
        serde_json::json!({"kind":"pool","playback_number":61}),
        Some(2.0),
        "current_capture",
        "go_to_if_normal",
    );
    subtract["operation"] = "subtract".into();
    let response = scenario
        .cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(revision + 1),
            subtract,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "changed");
    assert_eq!(outcome["recorded_cue"]["number"], 2.0);
    assert_eq!(outcome["recorded_cue"]["deleted"], true);
    assert_eq!(outcome["show_revision"], revision + 2);
    assert_eq!(outcome["show_event_sequence"], baseline + 1);
    assert!(outcome["runtime"].is_null());
    assert_eq!(outcome["projections"]["cue_list"]["body"]["cues"].as_array().unwrap().len(), 1);
    assert_one_show_batch(&scenario.state, baseline, 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn persisted_programmer_state(
    scenario: &CommandHttpScenario,
) -> light_programmer::ProgrammerState {
    let row = scenario
        .state
        .desk
        .lock()
        .persisted_sessions()
        .unwrap()
        .into_iter()
        .find(|row| row.id == scenario.session.id)
        .unwrap();
    serde_json::from_str(&row.programmer_json).unwrap()
}

fn assert_released_preload(programmer: &light_programmer::ProgrammerState) {
    assert!(programmer.preload_pending.is_empty());
    assert!(programmer.preload_active.is_empty());
    assert!(programmer.preload_group_pending.is_empty());
    assert!(programmer.preload_group_active.is_empty());
    assert!(programmer.preload_playback_pending.is_empty());
    assert!(!programmer.blind);
}

fn recorded_fixture_value(
    scenario: &CommandHttpScenario,
    playback: u16,
    fixture: light_core::FixtureId,
) -> f32 {
    let cue_list = stored_cue_list(scenario, playback).2;
    let change = cue_list.cues[0]
        .changes
        .iter()
        .find(|change| change.fixture_id == fixture)
        .unwrap();
    let Some(light_core::AttributeValue::Normalized(value)) = change.value else {
        panic!("recorded Cue must contain the fixture's normalized value")
    };
    value
}

fn assert_one_show_batch(state: &AppState, baseline: u64, expected_changes: usize) {
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(baseline, &light_application::EventFilter::default())
    else {
        panic!("Cue recording event must remain replayable")
    };
    let show = events
        .iter()
        .filter_map(|event| match &event.payload {
            light_application::ApplicationEvent::Show(
                light_application::ShowEvent::ObjectsChanged(change),
            ) => Some(change),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(show.len(), 1);
    assert_eq!(show[0].changes.len(), expected_changes);
}
