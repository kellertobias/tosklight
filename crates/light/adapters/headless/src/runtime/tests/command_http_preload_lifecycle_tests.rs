async fn install_lifecycle_show(
    scenario: &CommandHttpScenario,
    name: &str,
) -> (Uuid, u64, light_core::FixtureId) {
    let show_id = Uuid::parse_str(&scenario.create_and_open_show(name).await).unwrap();
    let fixture = schema_v2_direct_fixture().0;
    let fixture_id = fixture.fixture_id;
    let mut snapshot = preload_atomicity_test_snapshot();
    snapshot.fixtures = vec![fixture].into();
    std::sync::Arc::make_mut(&mut snapshot.groups)[0].fixtures = vec![fixture_id];
    snapshot.revision = 9_001;
    scenario.state.output.replace_snapshot(snapshot).unwrap();
    let show = scenario.state.active_show.current().clone().unwrap();
    let revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    assert_ne!(revision, scenario.state.output.snapshot().revision);
    (show_id, revision, fixture_id)
}

fn lifecycle_request(
    request_id: &str,
    capture: u64,
    values: u64,
    queue: u64,
    selection: u64,
    action: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "expected_capture_mode_revision":capture,
        "expected_values_revision":values,
        "expected_queue_revision":queue,
        "expected_selection_revision":selection,
        "action":action,
    })
}

fn lifecycle_go(
    request_id: &str,
    capture: u64,
    values: u64,
    queue: u64,
    selection: u64,
    show_id: Uuid,
    show_revision: u64,
    cursor: u64,
) -> serde_json::Value {
    lifecycle_request(
        request_id,
        capture,
        values,
        queue,
        selection,
        serde_json::json!({
            "type":"go",
            "show_id":show_id,
            "expected_show_revision":show_revision,
            "expected_playback_event_sequence":cursor,
        }),
    )
}

fn playback_request(request_id: &str, number: u16, action: &str) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "address":{"kind":"playback","playback_number":number},
        "action":{"type":action,"pressed":true},
        "surface":"physical",
    })
}

fn application_event_count(state: &AppState, object: light_application::EventObject) -> usize {
    let filter = light_application::EventFilter::default().with_object(object);
    let light_application::EventReplay::Events(events) =
        state.events.replay(0, &filter)
    else {
        panic!("focused event history should remain replayable")
    };
    events.len()
}

#[tokio::test]
async fn preload_lifecycle_http_is_sparse_replay_safe_and_shared_across_surfaces() {
    let scenario = CommandHttpScenario::new().await;
    scenario.state.installation.update_configuration(|configuration| {
        configuration.preload_physical_playback_actions = true;
    });
    let (show_id, show_revision, fixture) =
        install_lifecycle_show(&scenario, "Typed Preload lifecycle").await;
    let enter = lifecycle_request(
        "preload-enter-http",
        0,
        0,
        0,
        0,
        serde_json::json!({"type":"enter"}),
    );
    let entered = json(scenario.preload_lifecycle_action(enter.clone()).await).await;
    assert_eq!(entered["status"], "changed");
    assert_eq!(entered["active"], false);
    assert_eq!(entered["capture_mode"]["blind"], true);
    assert_eq!(entered["capture_mode"]["revision"], 1);
    assert!(entered.get("values_projection").is_none());
    assert!(entered.get("queue_projection").is_none());
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::programming_capture_mode(),
        ),
        1
    );

    // Replay is resolved before the now-stale selection precondition.
    scenario
        .state
        .programming
        .select(scenario.session.id, [fixture]);
    let cursor = scenario.state.events.latest_sequence();
    let replay = json(scenario.preload_lifecycle_action(enter).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["selection_revision"], 0);
    assert_eq!(scenario.state.events.latest_sequence(), cursor);

    let second_desk = scenario
        .state
        .installation.desk()
        .unwrap();
    let second_token = login_on_desk(&scenario, second_desk.id).await;
    // The peer is a surface of the same desk, so it must satisfy the desk's current selection
    // revision rather than one of its own.
    let peer_enter = lifecycle_request(
        "preload-enter-peer",
        1,
        0,
        0,
        1,
        serde_json::json!({"type":"enter"}),
    );
    let peer = json(
        scenario
            .preload_lifecycle_action_for( &second_token, peer_enter)
            .await,
    )
    .await;
    assert_eq!(peer["status"], "no_change");
    assert_eq!(peer["capture_mode"]["blind"], true);

    // An identity from before the collapse acts on the desk rather than being turned away.
    let legacy = scenario
        .preload_lifecycle_action_for(            &scenario.token,
            lifecycle_request(
                "preload-legacy-path",
                1,
                0,
                0,
                1,
                serde_json::json!({"type":"enter"}),
            ))
        .await;
    assert_eq!(legacy.status(), StatusCode::OK);
    assert_eq!(json(legacy).await["status"], "no_change");

    let pending = scenario
        .preload_values_action(preload_fixture_request(
            "preload-lifecycle-value",
            0,
            1,
            fixture.0,
            0.6,
        ))
        .await;
    assert_eq!(pending.status(), StatusCode::OK);
    let captured = scenario
        .playback_action_for(
            &scenario.token,
            scenario.session.desk.id,
            playback_request("preload-lifecycle-capture", 1, "go"),
        )
        .await;
    assert_eq!(captured.status(), StatusCode::OK);
    assert_eq!(json(captured).await["outcome"]["status"], "captured");
    let expected_cursor = scenario.state.events.latest_sequence();

    // A later unrelated Programmer event must not invalidate the queued Playback target.
    assert_eq!(
        scenario
            .priority_action_for(                &scenario.token,
                serde_json::json!({
                    "request_id":"preload-unrelated-priority",
                    "expected_revision":0,
                    "priority":70,
                }))
            .await
            .status(),
        StatusCode::OK
    );
    let go = lifecycle_go(
        "preload-go-http",
        1,
        1,
        1,
        1,
        show_id,
        show_revision,
        expected_cursor,
    );
    let go_response = scenario.preload_lifecycle_action(go.clone()).await;
    assert_eq!(go_response.status(), StatusCode::OK);
    let committed = json(go_response).await;
    assert_eq!(committed["status"], "changed");
    assert_eq!(committed["active"], true);
    assert_eq!(committed["capture_mode"]["blind"], false);
    assert_eq!(committed["commit"]["show_revision"], show_revision);
    assert_eq!(committed["commit"]["executed_playback_actions"], 1);
    assert_eq!(committed["commit"]["executed"][0]["playback_number"], 1);
    assert_eq!(
        committed["commit"]["runtime_changes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        committed["commit"]["playback_event_sequence_after"],
        committed["commit"]["runtime_changes"][0]["event_sequence"]
    );
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::programming_capture_mode(),
        ),
        2
    );
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::programming_preload_values(),
        ),
        2
    );
    assert_eq!(
        application_event_count(
            &scenario.state,
            light_application::EventObject::programming_preload_playback_queue(),
        ),
        2
    );
    assert_eq!(
        application_event_count(&scenario.state, light_application::EventObject::playback(1),),
        1
    );
    let event_cursor = scenario.state.events.latest_sequence();
    let replay = json(scenario.preload_lifecycle_action(go).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(
        scenario.state.events.latest_sequence(),
        event_cursor
    );

    let reenter = lifecycle_request(
        "preload-reenter-peer",
        2,
        2,
        2,
        1,
        serde_json::json!({"type":"enter"}),
    );
    assert_eq!(
        scenario
            .preload_lifecycle_action_for( &second_token, reenter)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .preload_values_action_for(                &second_token,
                preload_fixture_request("preload-peer-pending", 2, 3, fixture.0, 0.8,))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .playback_action_for(
                &second_token,
                second_desk.id,
                playback_request("preload-peer-capture", 1, "go"),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let clear = lifecycle_request(
        "preload-clear-http",
        3,
        3,
        3,
        1,
        serde_json::json!({"type":"clear_pending"}),
    );
    let cleared = json(
        scenario
            .preload_lifecycle_action_for( &second_token, clear)
            .await,
    )
    .await;
    assert_eq!(cleared["status"], "changed");
    assert_eq!(cleared["active"], true);
    assert!(
        cleared["values_projection"]["fixture_values"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        cleared["queue_projection"]["actions"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let clear_no_change = json(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "preload-clear-empty",
                3,
                4,
                4,
                1,
                serde_json::json!({"type":"clear_pending"}),
            ))
            .await,
    )
    .await;
    assert_eq!(clear_no_change["status"], "no_change");
    assert!(clear_no_change.get("values_projection").is_none());
    assert!(clear_no_change.get("queue_projection").is_none());

    let released = json(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "preload-release-http",
                3,
                4,
                4,
                1,
                serde_json::json!({"type":"release"}),
            ))
            .await,
    )
    .await;
    assert_eq!(released["status"], "changed");
    assert_eq!(released["active"], false);
    assert!(released.get("values_projection").is_none());
    let release_no_change = json(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "preload-release-empty",
                4,
                4,
                4,
                1,
                serde_json::json!({"type":"release"}),
            ))
            .await,
    )
    .await;
    assert_eq!(release_no_change["status"], "no_change");
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preload_go_rejects_show_target_and_gap_conflicts_with_explicit_authorities() {
    let scenario = CommandHttpScenario::new().await;
    scenario.state.installation.update_configuration(|configuration| {
        configuration.preload_physical_playback_actions = true;
    });
    let (show_id, show_revision, _) =
        install_lifecycle_show(&scenario, "Preload cursor authority").await;
    assert_eq!(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "cursor-enter",
                0,
                0,
                0,
                0,
                serde_json::json!({"type":"enter"}),
            ))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .playback_action_for(
                &scenario.token,
                scenario.session.desk.id,
                playback_request("cursor-capture", 1, "go"),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let cursor = scenario.state.events.latest_sequence();

    let stale_show = scenario
        .preload_lifecycle_action(lifecycle_go(
            "cursor-show-conflict",
            1,
            0,
            1,
            0,
            show_id,
            show_revision + 1,
            cursor,
        ))
        .await;
    assert_eq!(stale_show.status(), StatusCode::CONFLICT);
    let stale_show = json(stale_show).await;
    assert_eq!(stale_show["current_revision"], show_revision);
    assert!(stale_show.get("current_related_revision").is_none());

    let other_desk = scenario
        .state
        .installation.desk()
        .unwrap();
    let other_token = login_on_desk(&scenario, other_desk.id).await;
    assert_eq!(
        scenario
            .playback_action_for(
                &other_token,
                other_desk.id,
                playback_request("cursor-target-change", 1, "on"),
            )
            .await
            .status(),
        StatusCode::OK
    );
    let current = scenario.state.events.latest_sequence();
    let target = scenario
        .preload_lifecycle_action(lifecycle_go(
            "cursor-target-conflict",
            1,
            0,
            1,
            0,
            show_id,
            show_revision,
            cursor,
        ))
        .await;
    // Every surface shares the desk's Preload, so a second surface pressing a playback key
    // queues into the same pending queue rather than moving playback under this Go. The Go is
    // still refused, by the queue revision that surface advanced.
    assert_eq!(target.status(), StatusCode::CONFLICT);
    let target = json(target).await;
    assert_eq!(
        target["current_revision"], 2,
        "the peer surface's queued action advanced the desk's Preload queue"
    );
    assert!(current >= cursor);
    assert!(
        scenario
            .state
            .programming
            .capture_mode(scenario.session.id)
            .unwrap()
            .blind
    );

    for revision in 0..2_049 {
        let context = light_application::ActionContext::system(
            Uuid::nil(),
            light_application::ActionSource::System,
        );
        let change = light_application::ProgrammingPriorityChange::Upsert {
            projection: light_application::ProgrammingPriorityProjection {
                revision,
                priority: 100,
                changed_at: chrono::Utc::now(),
            },
        };
        scenario.state.events.publish(
            light_application::EventDraft::programming_priority_changed(&context, change),
        );
    }
    let latest = scenario.state.events.latest_sequence();
    let gap = scenario
        .preload_lifecycle_action(lifecycle_go(
            "cursor-gap-conflict",
            1,
            0,
            // Carrying the desk's current Preload queue revision, so the retained-history gap is
            // the precondition under test rather than the queue.
            2,
            0,
            show_id,
            show_revision,
            cursor,
        ))
        .await;
    assert_eq!(gap.status(), StatusCode::CONFLICT);
    let gap = json(gap).await;
    assert_eq!(gap["current_revision"], latest);
    assert_eq!(gap["current_related_revision"], show_revision);
    assert!(gap["error"].as_str().unwrap().contains("retained history"));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn failed_typed_preload_go_rolls_back_programmer_queue_runtime_and_events() {
    let scenario = CommandHttpScenario::new().await;
    let (show_id, show_revision, _) =
        install_lifecycle_show(&scenario, "Typed Preload rollback").await;
    assert_eq!(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "rollback-enter",
                0,
                0,
                0,
                0,
                serde_json::json!({"type":"enter"}),
            ))
            .await
            .status(),
        StatusCode::OK
    );
    scenario.state.programming.queue_preload_playback_action(
        scenario.session.id,
        1,
        None,
        light_programmer::PreloadPlaybackQueueAction::Go,
        light_programmer::PreloadPlaybackQueueSurface::Physical,
    );
    scenario.state.programming.queue_preload_playback_action(
        scenario.session.id,
        3,
        None,
        light_programmer::PreloadPlaybackQueueAction::On,
        light_programmer::PreloadPlaybackQueueSurface::Virtual,
    );
    let before = scenario.state.programming.get(scenario.session.id).unwrap();
    let cursor = scenario.state.events.latest_sequence();
    let response = scenario
        .preload_lifecycle_action(lifecycle_go(
            "rollback-go",
            1,
            0,
            0,
            0,
            show_id,
            show_revision,
            cursor,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        json(response).await["error"]
            .as_str()
            .unwrap()
            .contains("group playback")
    );
    let after = scenario.state.programming.get(scenario.session.id).unwrap();
    assert_eq!(
        after.preload_playback_pending,
        before.preload_playback_pending
    );
    assert_eq!(after.blind, before.blind);
    assert!(scenario.state.output.playback_runtime().is_empty());
    assert_eq!(scenario.state.events.latest_sequence(), cursor);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn preload_inspection_removes_one_ordered_action_and_rejects_stale_rows() {
    let scenario = CommandHttpScenario::new().await;
    assert_eq!(scenario.preload_lifecycle_action(lifecycle_request(
        "inspect-enter", 0, 0, 0, 0, serde_json::json!({"type":"enter"}),
    )).await.status(), StatusCode::OK);
    for number in [4, 4, 7] {
        assert!(scenario.state.programming.queue_preload_playback_action(
            scenario.session.id, number, Some(2),
            light_programmer::PreloadPlaybackQueueAction::Go,
            light_programmer::PreloadPlaybackQueueSurface::Physical,
        ));
    }
    let before = scenario.state.programming.get(scenario.session.id).unwrap();
    let output_before = scenario.state.output.snapshot();
    let queue_revision = json(scenario.preload_playback_queue_snapshot().await).await["projection"]["revision"].as_u64().unwrap();
    let remove = lifecycle_request("inspect-remove", 1, 0, queue_revision, 0,
        serde_json::json!({"type":"remove_pending_playback","index":1}));
    let response = scenario.preload_lifecycle_action(remove.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["queue_revision"], queue_revision + 1);
    assert!(body.get("commit").is_none());
    assert_eq!(body["queue_projection"]["actions"].as_array().unwrap().len(), 2);
    let after = scenario.state.programming.get(scenario.session.id).unwrap();
    assert_eq!(after.preload_playback_pending, vec![before.preload_playback_pending[0].clone(), before.preload_playback_pending[2].clone()]);
    assert_eq!(scenario.state.output.snapshot().revision, output_before.revision);
    assert_eq!(json(scenario.preload_lifecycle_action(remove).await).await["replayed"], true);
    let stale = scenario.preload_lifecycle_action(lifecycle_request(
        "inspect-stale", 1, 0, queue_revision, 0, serde_json::json!({"type":"remove_pending_playback","index":1}),
    )).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let invalid = scenario.preload_lifecycle_action(lifecycle_request(
        "inspect-invalid", 1, 0, queue_revision + 1, 0, serde_json::json!({"type":"remove_pending_playback","index":99}),
    )).await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(scenario.state.programming.get(scenario.session.id).unwrap().preload_playback_pending, after.preload_playback_pending);
}

#[tokio::test]
async fn preload_inspection_dynamic_and_group_release_removal_is_revision_bound() {
 let scenario=CommandHttpScenario::new().await;
 assert_eq!(scenario.preload_lifecycle_action(lifecycle_request("release-inspect-enter",0,0,0,0,serde_json::json!({"type":"enter"}))).await.status(),StatusCode::OK);
 let fixture=light_core::FixtureId::new();let attribute=light_core::AttributeKey::intensity();
 let mut state=scenario.state.programming.get(scenario.session.id).unwrap();
 state.preload_dynamic_pending=Arc::new([9,2,7].map(|programmer_order| light_dynamics::DynamicAddressValue {fixture_id:fixture,attribute:attribute.clone(),programmer_order,changed_at_millis:0,value:light_dynamics::DynamicSemanticValue::Release}).into());
 state.preload_group_release_pending=[8,1,6].map(|programmer_order| light_programmer::GroupReleaseProgrammerValue {group_id:"3".into(),attribute:attribute.clone(),programmer_order,changed_at_millis:0}).into();
 let original_live=state.values.clone(); let original_dynamic=state.dynamic_values.clone();let original_groups=state.group_release_values.clone();
 scenario.state.programming.restore(state);
 let before=json(scenario.preload_values_snapshot().await).await;
 assert_eq!(before["projection"]["dynamic_values"].as_array().unwrap().len(),3);
 assert_eq!(before["projection"]["group_release_values"].as_array().unwrap().len(),3);
 let output_before=scenario.state.output.snapshot().revision;
 for (action,prefix) in [("remove_pending_dynamic","dynamic"),("remove_pending_group_release","group")] {
  let revision=json(scenario.preload_values_snapshot().await).await["projection"]["revision"].as_u64().unwrap();
  let queue=json(scenario.preload_playback_queue_snapshot().await).await["projection"]["revision"].as_u64().unwrap();
  let request=lifecycle_request(&format!("{prefix}-remove"),1,revision,queue,0,serde_json::json!({"type":action,"index":1}));
  let response=scenario.preload_lifecycle_action(request.clone()).await;assert_eq!(response.status(),StatusCode::OK);
  let body=json(response).await;assert!(body.get("commit").is_none());assert_eq!(body["queue_revision"],queue);
  assert_eq!(json(scenario.preload_lifecycle_action(request).await).await["replayed"],true);
  assert_eq!(scenario.preload_lifecycle_action(lifecycle_request(&format!("{prefix}-stale"),1,revision,queue,0,serde_json::json!({"type":action,"index":1}))).await.status(),StatusCode::CONFLICT);
  let current=json(scenario.preload_values_snapshot().await).await["projection"]["revision"].as_u64().unwrap();
  assert_eq!(scenario.preload_lifecycle_action(lifecycle_request(&format!("{prefix}-invalid"),1,current,queue,0,serde_json::json!({"type":action,"index":99}))).await.status(),StatusCode::BAD_REQUEST);
 }
 let after=scenario.state.programming.get(scenario.session.id).unwrap();
 assert_eq!(after.preload_dynamic_pending.iter().map(|v|v.programmer_order).collect::<Vec<_>>(),[9,7]);
 assert_eq!(after.preload_group_release_pending.iter().map(|v|v.programmer_order).collect::<Vec<_>>(),[8,6]);
 assert_eq!(after.values,original_live);assert_eq!(after.dynamic_values,original_dynamic);assert_eq!(after.group_release_values,original_groups);assert_eq!(scenario.state.output.snapshot().revision,output_before);
}

#[tokio::test]
async fn preload_inspection_static_removal_rejects_newer_value_at_same_address() {
 let scenario=CommandHttpScenario::new().await;
 let fixture=scenario.install_direct_fixture();
 scenario.state.programming.set(scenario.session.id,fixture,light_core::AttributeKey::intensity(),light_core::AttributeValue::Normalized(0.6));
 assert_eq!(scenario.preload_lifecycle_action(lifecycle_request("static-enter",0,0,0,0,serde_json::json!({"type":"enter"}))).await.status(),StatusCode::OK);
 let seed=scenario.preload_values_action(serde_json::json!({"request_id":"static-seed","expected_revision":0,"expected_capture_mode_revision":1,"action":{"type":"batch","mutations":[{"type":"set_fixture","fixture_id":fixture.0,"attribute":"intensity","value":{"kind":"normalized","value":0.5}},{"type":"set_group","group_id":"1","attribute":"focus","value":{"kind":"normalized","value":0.2}}]}})).await;
 assert_eq!(seed.status(),StatusCode::OK);
 let displayed=json(scenario.preload_values_snapshot().await).await["projection"]["revision"].as_u64().unwrap();
 let edit=scenario.preload_values_action(serde_json::json!({"request_id":"static-newer","expected_revision":displayed,"expected_capture_mode_revision":1,"action":{"type":"set_fixture","fixture_id":fixture.0,"attribute":"intensity","value":{"kind":"normalized","value":0.8}}})).await;
 assert_eq!(edit.status(),StatusCode::OK);
 let mut restored=scenario.state.programming.get(scenario.session.id).unwrap();
 restored.preload_dynamic_pending=Arc::new(vec![light_dynamics::DynamicAddressValue {fixture_id:fixture,attribute:light_core::AttributeKey::intensity(),programmer_order:90,changed_at_millis:0,value:light_dynamics::DynamicSemanticValue::Release}]);
 restored.preload_group_release_pending.push(light_programmer::GroupReleaseProgrammerValue {group_id:"1".into(),attribute:light_core::AttributeKey("focus".into()),programmer_order:91,changed_at_millis:0});
 scenario.state.programming.restore(restored);
 let before=scenario.state.programming.get(scenario.session.id).unwrap();assert!(!before.values.is_empty());let output=scenario.state.output.snapshot().revision;
 let remove=serde_json::json!({"type":"remove_pending_fixture_value","fixture_id":fixture.0,"attribute":"intensity"});
 assert_eq!(scenario.preload_lifecycle_action(lifecycle_request("static-stale",1,displayed,0,0,remove.clone())).await.status(),StatusCode::CONFLICT);
 assert_eq!(scenario.state.programming.get(scenario.session.id).unwrap().preload_pending,before.preload_pending);
 let current=json(scenario.preload_values_snapshot().await).await["projection"]["revision"].as_u64().unwrap();
 let response=scenario.preload_lifecycle_action(lifecycle_request("static-remove",1,current,0,0,remove.clone())).await;
 assert_eq!(response.status(),StatusCode::OK);assert!(json(response).await.get("commit").is_none());
 let current=json(scenario.preload_values_snapshot().await).await["projection"]["revision"].as_u64().unwrap();
 assert_eq!(scenario.preload_lifecycle_action(lifecycle_request("static-absent",1,current,0,0,remove)).await.status(),StatusCode::BAD_REQUEST);
 let group=serde_json::json!({"type":"remove_pending_group_value","group_id":"1","attribute":"focus"});
 assert_eq!(scenario.preload_lifecycle_action(lifecycle_request("static-group-remove",1,current,0,0,group)).await.status(),StatusCode::OK);
 let after=scenario.state.programming.get(scenario.session.id).unwrap();assert!(after.preload_pending.is_empty());assert!(after.preload_group_pending.is_empty());assert_eq!(after.values,before.values);assert_eq!(after.preload_dynamic_pending,before.preload_dynamic_pending);assert_eq!(after.preload_group_release_pending,before.preload_group_release_pending);assert_eq!(scenario.state.output.snapshot().revision,output);
}
