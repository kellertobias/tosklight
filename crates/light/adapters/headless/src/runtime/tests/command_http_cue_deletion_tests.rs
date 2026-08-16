fn cue_deletion_authority(scenario: &CommandHttpScenario, cue_number: &str) -> (u64, u64, Uuid) {
    let show_path = scenario
        .state
        .active_show
        .current()
        .as_ref()
        .unwrap()
        .path
        .clone();
    let store = ShowStore::open(show_path).unwrap();
    let document = store.portable_document().unwrap();
    let object = document.object("cue_list", CUE_LIST_ID).unwrap();
    let list: light_playback::CueList = serde_json::from_value(object.body().clone()).unwrap();
    let cue_id = list
        .cues
        .iter()
        .find(|item| item.number == cue(cue_number))
        .unwrap()
        .id;
    (document.revision().value(), object.revision(), cue_id)
}

fn cue_deletion_request(
    request_id: &str,
    address: serde_json::Value,
    cue_number: &str,
    object_revision: u64,
    cue_id: Uuid,
    playback_number: u16,
) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "address":address,
        "cue_number":cue_number,
        "authority":{
            "playback_number":playback_number,
            "cue_list_id":CUE_LIST_ID,
            "object_id":CUE_LIST_ID,
            "object_revision":object_revision,
            "cue_id":cue_id,
        }
    })
}

async fn delete_cue_action(
    scenario: &CommandHttpScenario,
    desk_id: Uuid,
    show_id: &str,
    expected_revision: u64,
    request: serde_json::Value,
) -> Response {
    scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v2/cues/delete")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header("x-tosk-desk", desk_id.to_string())
                .header("x-tosk-show", show_id)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, expected_revision.to_string())
                .body(Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

fn cue_deletion_show_events(scenario: &CommandHttpScenario, baseline: u64) -> usize {
    let light_application::EventReplay::Events(events) = scenario
        .state
        .events
        .replay(baseline, &light_application::EventFilter::default())
    else {
        return usize::MAX;
    };
    events
        .iter()
        .filter(|event| {
            matches!(
                event.payload,
                light_application::ApplicationEvent::Show(
                    light_application::ShowEvent::ObjectsChanged(_)
                )
            )
        })
        .count()
}

fn cue_delete_compatibility_events(scenario: &CommandHttpScenario) -> Vec<Event> {
    scenario
        .state
        .events.audit_events()
        .iter()
        .filter(|event| event.kind == "show_object_changed")
        .cloned()
        .collect()
}

#[tokio::test]
async fn atomic_command_deletes_once_preserves_runtime_hold_and_replays_without_v1_event() {
    let (scenario, _) = cue_navigation_scenario().await;
    let active = scenario
        .execute("activate-before-delete", Some("GO TO PBK 1 CUE 2"))
        .await;
    assert_eq!(active.status(), StatusCode::OK);
    assert_eq!(current_cue(&scenario, 1), Some("2".into()));
    assert_eq!(current_cue(&scenario, 2), Some("2".into()));
    let baseline = scenario.state.events.latest_sequence();
    let compatibility = cue_delete_compatibility_events(&scenario).len();
    let history = history_len(&scenario);

    let response = scenario
        .execute("typed-cue-delete", Some("DELETE SET 1 CUE 2"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["outcome"], "accepted");
    assert_eq!(response["command_line"]["text"], "FIXTURE");
    assert_eq!(cue_deletion_show_events(&scenario, baseline), 1);
    assert_eq!(
        cue_delete_compatibility_events(&scenario).len(),
        compatibility
    );
    let runtime = active_playback(&scenario, 1).unwrap();
    assert_eq!(runtime.current_cue_number, Some(cue("2")));
    assert_eq!(runtime.deleted_cue_hold.unwrap().deleted_number, cue("2"));
    assert_eq!(
        active_playback(&scenario, 2)
            .unwrap()
            .deleted_cue_hold
            .unwrap()
            .deleted_number,
        cue("2")
    );

    let replay = scenario
        .execute("typed-cue-delete", Some("DELETE SET 1 CUE 2"))
        .await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["outcome"], "accepted");
    assert_eq!(cue_deletion_show_events(&scenario, baseline), 1);
    assert_eq!(history_len(&scenario), history + 1);

    let missing = scenario
        .execute("missing-after-delete", Some("DELETE SET 1 CUE 2"))
        .await;
    assert_eq!(missing.status(), StatusCode::OK);
    let missing = json(missing).await;
    assert_eq!(missing["outcome"], "rejected");
    assert!(
        missing["error"]
            .as_str()
            .unwrap()
            .contains("does not exist")
    );
    assert_eq!(cue_deletion_show_events(&scenario, baseline), 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn direct_current_page_action_returns_authority_and_replays_before_page_and_lock_checks() {
    let (scenario, show_id) = cue_navigation_scenario().await;
    let (show_revision, object_revision, cue_id) = cue_deletion_authority(&scenario, "2.5");
    let request = cue_deletion_request(
        "direct-cue-delete",
        serde_json::json!({"type":"current_page","expected_page":1,"slot":2}),
        "2.5",
        object_revision,
        cue_id,
        2,
    );
    let baseline = scenario.state.events.latest_sequence();
    let compatibility = cue_delete_compatibility_events(&scenario).len();
    let response = delete_cue_action(
        &scenario,
        scenario.session.desk.id,
        &show_id,
        show_revision,
        request.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::ETAG],
        format!("\"{}\"", show_revision + 1)
    );
    let body = json(response).await;
    assert_eq!(body["status"], "changed");
    assert_eq!(body["request_id"], "direct-cue-delete");
    assert_eq!(body["replayed"], false);
    assert_eq!(body["deleted_cue"]["id"], cue_id.to_string());
    assert_eq!(body["cue_list"]["object_revision"], object_revision + 1);
    assert_eq!(cue_deletion_show_events(&scenario, baseline), 1);
    assert_eq!(
        cue_delete_compatibility_events(&scenario).len(),
        compatibility
    );

    scenario
        .state
        .installation.set_desk_page(
            scenario.session.desk.id,
            light_core::ShowId(Uuid::parse_str(&show_id).unwrap()),
            2,
        )
        .unwrap();
    write_desk_lock(
        &scenario.state,
        scenario.session.desk.id,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();
    let replay = delete_cue_action(
        &scenario,
        scenario.session.desk.id,
        &show_id,
        show_revision,
        request.clone(),
    )
    .await;
    let replay_status = replay.status();
    let replay = json(replay).await;
    assert_eq!(replay_status, StatusCode::OK, "{replay}");
    assert_eq!(replay["replayed"], true);
    assert_eq!(cue_deletion_show_events(&scenario, baseline), 1);

    let changed_if_match = delete_cue_action(
        &scenario,
        scenario.session.desk.id,
        &show_id,
        show_revision + 1,
        request,
    )
    .await;
    assert_eq!(changed_if_match.status(), StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn direct_action_rejects_stale_authority_and_foreign_desk_atomically() {
    let (scenario, show_id) = cue_navigation_scenario().await;
    let (show_revision, object_revision, cue_id) = cue_deletion_authority(&scenario, "2");
    let request = cue_deletion_request(
        "secure-delete",
        serde_json::json!({"type":"pool","playback_number":1}),
        "2",
        object_revision - 1,
        cue_id,
        1,
    );
    let baseline = scenario.state.events.latest_sequence();
    let stale = delete_cue_action(
        &scenario,
        scenario.session.desk.id,
        &show_id,
        show_revision,
        request,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(stale).await["current_related_revision"],
        object_revision
    );
    assert_eq!(cue_deletion_show_events(&scenario, baseline), 0);

    let (_, current_object_revision, cue_id) = cue_deletion_authority(&scenario, "2");
    let request = cue_deletion_request(
        "foreign-desk-delete",
        serde_json::json!({"type":"pool","playback_number":1}),
        "2",
        current_object_revision,
        cue_id,
        1,
    );
    let foreign =
        delete_cue_action(&scenario, Uuid::new_v4(), &show_id, show_revision, request).await;
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
    assert_eq!(cue_deletion_show_events(&scenario, baseline), 0);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn multiplexed_websocket_uses_typed_delete_and_emits_one_exact_facade_notification() {
    let (scenario, _) = cue_navigation_scenario().await;
    let baseline = scenario.state.events.latest_sequence();
    let compatibility = cue_delete_compatibility_events(&scenario).len();
    let (_, object_revision, _) = cue_deletion_authority(&scenario, "2");
    let response = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            "compatibility-cue-delete",
            light_wire::v2::live_action::LiveAction::CommandLineExecute(
                light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                    value: "DELETE SET 1 . 2 CUE 2".into(),
                },
            ),
        ),
    );
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(cue_deletion_show_events(&scenario, baseline), 1);
    let events = cue_delete_compatibility_events(&scenario);
    assert_eq!(events.len(), compatibility + 1);
    assert_eq!(
        events[compatibility].payload,
        serde_json::json!({
            "show_id":scenario.state.active_show.current().as_ref().unwrap().id,
            "kind":"cue_list",
            "id":CUE_LIST_ID,
            "revision":object_revision + 1,
        })
    );
    assert!(events[compatibility].payload.get("deleted").is_none());
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
