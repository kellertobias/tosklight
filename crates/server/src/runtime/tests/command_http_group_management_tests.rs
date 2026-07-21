use light_wire::v2::group_management::GroupManagementOutcome;

fn group_management_request(
    request_id: &str,
    group_id: &str,
    operation: serde_json::Value,
    expected_object_revision: u64,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "group_id": group_id,
        "operation": operation,
        "expected_object_revision": expected_object_revision,
    })
}

fn rename(name: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "update_properties",
        "properties": {"name": name, "color": "#204060", "icon": "◆"}
    })
}

async fn management_outcome(response: Response) -> GroupManagementOutcome {
    serde_json::from_value(json(response).await).unwrap()
}

fn management_group_body(outcome: &GroupManagementOutcome) -> &serde_json::Value {
    match outcome {
        GroupManagementOutcome::Changed { group, .. }
        | GroupManagementOutcome::NoChange { group, .. } => &group.body,
    }
}

fn management_event_sequence(outcome: &GroupManagementOutcome) -> Option<u64> {
    match outcome {
        GroupManagementOutcome::Changed {
            show_event_sequence, ..
        } => Some(*show_event_sequence),
        GroupManagementOutcome::NoChange { .. } => None,
    }
}

fn management_replayed(outcome: &GroupManagementOutcome) -> bool {
    match outcome {
        GroupManagementOutcome::Changed { replayed, .. }
        | GroupManagementOutcome::NoChange { replayed, .. } => *replayed,
    }
}

/// Records `group_id` from the current desk selection so management has an authoritative target.
async fn seed_group(scenario: &CommandHttpScenario, show_id: &str, group_id: &str) {
    let response = scenario
        .group_recording_action(
            show_id,
            Some(&scenario.token),
            serde_json::json!({
                "request_id": format!("seed-{group_id}"),
                "group_id": group_id,
                "operation": "overwrite",
                "expected_object_revision": 0,
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn property_update_is_authoritative_replay_safe_and_sparse_on_no_change() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group property update").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programmers.select(scenario.session.id, [fixture]);
    seed_group(&scenario, &show_id, "house").await;
    let baseline = scenario.state.application_events.latest_sequence();
    let request = group_management_request("manage-rename", "house", rename("Front wash"), 1);

    let response = scenario
        .group_management_action(&show_id, Some(&scenario.token), request.clone())
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"2\"");
    let changed = management_outcome(response).await;
    assert!(!management_replayed(&changed));
    let body = management_group_body(&changed);
    assert_eq!(body["name"], "Front wash");
    assert_eq!(body["color"], "#204060");
    assert_eq!(body["icon"], "◆");
    assert_eq!(
        body["fixtures"],
        serde_json::json!([fixture.0]),
        "a property update must not disturb ordered membership"
    );
    let event_sequence = management_event_sequence(&changed).unwrap();
    assert_eq!(event_sequence, baseline + 1);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        baseline + 1,
        "one semantic mutation must publish exactly one Show event"
    );

    let replay = scenario
        .group_management_action(&show_id, Some(&scenario.token), request)
        .await;
    let replay = management_outcome(replay).await;
    assert!(management_replayed(&replay));
    assert_eq!(management_event_sequence(&replay), Some(event_sequence));
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        baseline + 1,
        "a replay must not publish another Show event"
    );

    let no_change = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request("manage-no-change", "house", rename("Front wash"), 2),
        )
        .await;
    let no_change = management_outcome(no_change).await;
    assert!(matches!(no_change, GroupManagementOutcome::NoChange { .. }));
    assert_eq!(management_event_sequence(&no_change), None);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        baseline + 1,
        "a semantic no-op must publish no Show event"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn undo_restores_the_previous_body_and_a_stale_revision_conflicts() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group undo").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programmers.select(scenario.session.id, [fixture]);
    seed_group(&scenario, &show_id, "house").await;
    let renamed = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request("manage-rename", "house", rename("Renamed"), 1),
        )
        .await;
    assert_eq!(renamed.status(), StatusCode::OK);

    let stale = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request("manage-stale-undo", "house", serde_json::json!({"type":"undo"}), 1),
        )
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(stale.headers()[header::ETAG], "\"2\"");
    assert_eq!(json(stale).await["current_revision"], 2);

    let undo = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request("manage-undo", "house", serde_json::json!({"type":"undo"}), 2),
        )
        .await;
    assert_eq!(undo.status(), StatusCode::OK);
    let undo = management_outcome(undo).await;
    assert_eq!(
        management_group_body(&undo)["name"],
        "Group house",
        "undo must restore the exact previous stored body"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn frozen_refresh_publishes_its_selection_before_the_owning_show_event() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Frozen refresh ordering").await;
    let first = scenario.install_direct_fixture();
    let second = light_core::FixtureId::new();
    scenario.state.programmers.select(scenario.session.id, [first]);
    seed_group(&scenario, &show_id, "source").await;
    scenario.state.programmers.select(scenario.session.id, [first]);
    seed_group(&scenario, &show_id, "frozen").await;
    // Make "frozen" a frozen snapshot of "source", then widen the source membership.
    let stored = scenario
        .put_active_object(
            &show_id,
            "group",
            "frozen",
            1,
            serde_json::json!({
                "id": "frozen",
                "name": "Frozen",
                "fixtures": [first.0],
                "frozen_from": {
                    "source_group_id": "source",
                    "source_revision": 1,
                    "captured_at": "2020-01-01T00:00:00Z"
                },
                "programming": {},
                "master": 1.0
            }),
        )
        .await;
    assert_eq!(stored.status(), StatusCode::OK);
    scenario
        .state
        .programmers
        .select(scenario.session.id, [first, second]);
    scenario
        .group_recording_action(
            &show_id,
            Some(&scenario.token),
            serde_json::json!({
                "request_id": "widen-source",
                "group_id": "source",
                "operation": "overwrite",
                "expected_object_revision": 1,
            }),
        )
        .await;
    let baseline = scenario.state.application_events.latest_sequence();

    let refreshed = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "manage-refresh",
                "frozen",
                serde_json::json!({"type":"refresh_frozen"}),
                2,
            ),
        )
        .await;
    assert_eq!(refreshed.status(), StatusCode::OK);
    let refreshed = management_outcome(refreshed).await;
    let body = management_group_body(&refreshed);
    assert_eq!(
        body["fixtures"],
        serde_json::json!([first.0, second.0]),
        "a frozen refresh recaptures the ordered source membership"
    );
    assert_eq!(body["frozen_from"]["source_group_id"], "source");
    assert_ne!(body["frozen_from"]["captured_at"], "2020-01-01T00:00:00Z");

    let events = application_events_after(&scenario.state, baseline);
    assert!(
        matches!(
            events.first().unwrap().payload,
            light_application::ApplicationEvent::Programming(
                light_application::ProgrammingEvent::InteractionChanged(_)
            )
        ),
        "the resulting desk selection must be published before the owning Show event"
    );
    assert!(matches!(
        events.last().unwrap().payload,
        light_application::ApplicationEvent::Show(light_application::ShowEvent::ObjectsChanged(_))
    ));
    assert_eq!(
        events.last().unwrap().sequence,
        management_event_sequence(&refreshed).unwrap()
    );
    assert_eq!(
        scenario
            .state
            .programmers
            .selection(scenario.session.id)
            .unwrap()
            .selected,
        vec![first, second],
        "the originating desk selection is left on the frozen source"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn detach_derived_freezes_membership_and_an_invalid_source_mutates_nothing() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Detach derived").await;
    let first = scenario.install_direct_fixture();
    let second = light_core::FixtureId::new();
    scenario
        .state
        .programmers
        .select(scenario.session.id, [first, second]);
    seed_group(&scenario, &show_id, "source").await;
    seed_group(&scenario, &show_id, "derived").await;
    let stored = scenario
        .put_active_object(
            &show_id,
            "group",
            "derived",
            1,
            serde_json::json!({
                "id": "derived",
                "name": "Derived",
                "fixtures": [],
                "derived_from": {"source_group_id": "source", "rule": {"type": "all"}},
                "programming": {},
                "master": 1.0
            }),
        )
        .await;
    assert_eq!(stored.status(), StatusCode::OK);
    let baseline = scenario.state.application_events.latest_sequence();

    let mismatched = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "manage-bad-source",
                "derived",
                serde_json::json!({
                    "type":"detach_derived",
                    "expected_source":{"source_group_id":"other"}
                }),
                2,
            ),
        )
        .await;
    assert_eq!(mismatched.status(), StatusCode::CONFLICT);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        baseline,
        "an invalid source must mutate nothing and publish nothing"
    );

    let detached = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "manage-detach",
                "derived",
                serde_json::json!({"type":"detach_derived"}),
                2,
            ),
        )
        .await;
    assert_eq!(detached.status(), StatusCode::OK);
    let detached = management_outcome(detached).await;
    let body = management_group_body(&detached);
    assert!(body["derived_from"].is_null());
    assert_eq!(body["fixtures"], serde_json::json!([first.0, second.0]));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn group_management_rejects_missing_auth_forged_scope_and_a_foreign_show() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group management security").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programmers.select(scenario.session.id, [fixture]);
    seed_group(&scenario, &show_id, "house").await;
    let request = group_management_request("manage-secure", "house", rename("Secured"), 1);

    assert_eq!(
        scenario
            .group_management_action(&show_id, None, request.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    for field in ["desk_id", "user_id", "session_id", "expected_show_revision"] {
        let mut forged = request.clone();
        forged[field] = serde_json::json!("forged");
        assert_eq!(
            scenario
                .group_management_action(&show_id, Some(&scenario.token), forged)
                .await
                .status(),
            StatusCode::BAD_REQUEST,
            "{field} must stay server-authored"
        );
    }
    assert_eq!(
        scenario
            .group_management_action(&Uuid::new_v4().to_string(), Some(&scenario.token), request)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
