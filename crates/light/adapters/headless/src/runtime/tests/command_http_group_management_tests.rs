use light_wire::v2::group_management::GroupManagementOutcome;

fn group_management_request(
    request_id: &str,
    group_id: &str,
    operation: serde_json::Value,
    expected_object_revision: u64,
    expected_show_revision: u64,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "group_id": group_id,
        "operation": operation,
        "expected_object_revision": expected_object_revision,
        "expected_show_revision": expected_show_revision,
    })
}

async fn group_authority(
    scenario: &CommandHttpScenario,
    show_id: &str,
    group_id: &str,
) -> (u64, u64) {
    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::get(format!("/api/v2/objects/group/{group_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let snapshot = json(response).await;
    (
        snapshot["show_revision"].as_u64().unwrap(),
        snapshot["object"]["revision"].as_u64().unwrap(),
    )
}

async fn group_settings_snapshot(
    scenario: &CommandHttpScenario,
    show_id: &str,
    group_id: &str,
) -> serde_json::Value {
    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::get(format!("/api/v2/groups/{group_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

fn management_show_revision(outcome: &GroupManagementOutcome) -> u64 {
    match outcome {
        GroupManagementOutcome::Changed { show_revision, .. }
        | GroupManagementOutcome::NoChange { show_revision, .. } => *show_revision,
    }
}

fn rename(name: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "update_properties",
        "properties": {"name": name, "color": "#204060", "icon": "◆"}
    })
}

fn top_grid_mapping() -> serde_json::Value {
    serde_json::json!({
        "projection": {
            "anchor":{"x":0.0,"y":0.0,"z":0.0},
            "view_direction":{"x":0.0,"y":0.0,"z":-1.0},
            "rotation_degrees":0.0,
            "preset":"top"
        },
        "shape":{"type":"grid","angle_degrees":0.0,"direction":"ascending"}
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
    scenario.state.programming.select(scenario.session.id, [fixture]);
    seed_group(&scenario, &show_id, "house").await;
    let baseline = scenario.state.events.latest_sequence();
    let (show_revision, object_revision) = group_authority(&scenario, &show_id, "house").await;
    let request = group_management_request(
        "manage-rename",
        "house",
        rename("Front wash"),
        object_revision,
        show_revision,
    );

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
    let changed_show_revision = management_show_revision(&changed);
    assert_eq!(event_sequence, baseline + 1);
    assert_eq!(
        scenario.state.events.latest_sequence(),
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
        scenario.state.events.latest_sequence(),
        baseline + 1,
        "a replay must not publish another Show event"
    );

    let no_change = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "manage-no-change",
                "house",
                rename("Front wash"),
                2,
                changed_show_revision,
            ),
        )
        .await;
    let no_change = management_outcome(no_change).await;
    assert!(matches!(no_change, GroupManagementOutcome::NoChange { .. }));
    assert_eq!(management_event_sequence(&no_change), None);
    assert_eq!(
        scenario.state.events.latest_sequence(),
        baseline + 1,
        "a semantic no-op must publish no Show event"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn spatial_mapping_set_validate_and_remove_are_atomic_revisioned_object_intents() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group spatial mapping").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programming.select(scenario.session.id, [fixture]);
    seed_group(&scenario, &show_id, "house").await;
    let stored = scenario
        .put_active_object(
            &show_id,
            "group",
            "house",
            1,
            serde_json::json!({
                "id":"house",
                "name":"House",
                "fixtures":[fixture.0],
                "source":{"type":"explicit","fixture_ids":[fixture.0]},
                "programming":{},
                "master":1.0,
                "future_extension":{"retain":true}
            }),
        )
        .await;
    assert_eq!(stored.status(), StatusCode::OK);
    let (show_revision, object_revision) = group_authority(&scenario, &show_id, "house").await;
    let set = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "mapping-set",
                "house",
                serde_json::json!({"type":"set_spatial_mapping","mapping":top_grid_mapping()}),
                object_revision,
                show_revision,
            ),
        )
        .await;
    assert_eq!(set.status(), StatusCode::OK);
    let set = management_outcome(set).await;
    let body = management_group_body(&set);
    assert_eq!(body["mapping"], top_grid_mapping());
    assert_eq!(body["source"]["fixture_ids"], serde_json::json!([fixture.0]));
    assert_eq!(body["fixtures"], serde_json::json!([fixture.0]));
    assert_eq!(body["future_extension"], serde_json::json!({"retain":true}));
    let set_show_revision = management_show_revision(&set);
    let settings = group_settings_snapshot(&scenario, &show_id, "house").await;
    assert_eq!(settings["show_id"], show_id);
    assert_eq!(settings["show_revision"], set_show_revision);
    assert_eq!(settings["group"]["object_revision"], object_revision + 1);
    assert_eq!(
        settings["resolved_spatial"]["mapping_provenance"],
        serde_json::json!({"type":"local","group_id":"house"})
    );
    assert_eq!(
        settings["resolved_spatial"]["source_order"],
        serde_json::json!([fixture.0])
    );
    assert_eq!(settings["resolved_spatial"]["rank_count"], 1);
    assert_eq!(
        settings["resolved_spatial"]["ranks"][0]["fixture_id"],
        serde_json::json!(fixture.0)
    );
    assert_eq!(settings["resolved_spatial"]["ranks"][0]["rank"], 0);
    assert_eq!(
        settings["resolved_spatial"]["projected_positions"][0]["fixture_id"],
        serde_json::json!(fixture.0)
    );
    assert!(settings["resolved_spatial"]["projected_positions"][0].get("u").is_some());
    assert!(settings["resolved_spatial"]["projected_positions"][0].get("v").is_some());
    let baseline = scenario.state.events.latest_sequence();

    let stale_show = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "mapping-stale-show",
                "house",
                serde_json::json!({"type":"remove_spatial_mapping"}),
                object_revision + 1,
                show_revision,
            ),
        )
        .await;
    assert_eq!(stale_show.status(), StatusCode::CONFLICT);
    assert_eq!(json(stale_show).await["current_related_revision"], set_show_revision);
    assert_eq!(scenario.state.events.latest_sequence(), baseline);

    let mut invalid_mapping = top_grid_mapping();
    invalid_mapping["projection"]["view_direction"] =
        serde_json::json!({"x":0.0,"y":0.0,"z":0.0});
    let invalid = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "mapping-invalid",
                "house",
                serde_json::json!({"type":"set_spatial_mapping","mapping":invalid_mapping}),
                object_revision + 1,
                set_show_revision,
            ),
        )
        .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(scenario.state.events.latest_sequence(), baseline);
    let (unchanged_show_revision, unchanged_object_revision) =
        group_authority(&scenario, &show_id, "house").await;
    assert_eq!(unchanged_show_revision, set_show_revision);
    assert_eq!(unchanged_object_revision, object_revision + 1);

    let removed = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "mapping-remove",
                "house",
                serde_json::json!({"type":"remove_spatial_mapping"}),
                unchanged_object_revision,
                unchanged_show_revision,
            ),
        )
        .await;
    assert_eq!(removed.status(), StatusCode::OK);
    let removed = management_outcome(removed).await;
    assert!(management_group_body(&removed).get("mapping").is_none());
    assert_eq!(
        management_group_body(&removed)["future_extension"],
        serde_json::json!({"retain":true})
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn undo_restores_the_previous_body_and_a_stale_revision_conflicts() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group undo").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programming.select(scenario.session.id, [fixture]);
    seed_group(&scenario, &show_id, "house").await;
    let (show_revision, object_revision) = group_authority(&scenario, &show_id, "house").await;
    let renamed = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "manage-rename",
                "house",
                rename("Renamed"),
                object_revision,
                show_revision,
            ),
        )
        .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let renamed = management_outcome(renamed).await;
    let renamed_show_revision = management_show_revision(&renamed);

    let stale = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "manage-stale-undo",
                "house",
                serde_json::json!({"type":"undo"}),
                1,
                renamed_show_revision,
            ),
        )
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(stale.headers()[header::ETAG], "\"2\"");
    assert_eq!(json(stale).await["current_revision"], 2);

    let undo = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "manage-undo",
                "house",
                serde_json::json!({"type":"undo"}),
                2,
                renamed_show_revision,
            ),
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
    scenario.state.programming.select(scenario.session.id, [first]);
    seed_group(&scenario, &show_id, "source").await;
    scenario.state.programming.select(scenario.session.id, [first]);
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
        .programming
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
    let baseline = scenario.state.events.latest_sequence();
    let (show_revision, object_revision) = group_authority(&scenario, &show_id, "frozen").await;

    let refreshed = scenario
        .group_management_action(
            &show_id,
            Some(&scenario.token),
            group_management_request(
                "manage-refresh",
                "frozen",
                serde_json::json!({"type":"refresh_frozen"}),
                object_revision,
                show_revision,
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
            .programming
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
        .programming
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
    let baseline = scenario.state.events.latest_sequence();
    let (show_revision, object_revision) = group_authority(&scenario, &show_id, "derived").await;

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
                object_revision,
                show_revision,
            ),
        )
        .await;
    assert_eq!(mismatched.status(), StatusCode::CONFLICT);
    assert_eq!(
        scenario.state.events.latest_sequence(),
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
                object_revision,
                show_revision,
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
async fn group_management_ignores_forged_scope_but_rejects_missing_auth_and_a_foreign_show() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Group management security").await;
    let fixture = scenario.install_direct_fixture();
    scenario.state.programming.select(scenario.session.id, [fixture]);
    seed_group(&scenario, &show_id, "house").await;
    let (show_revision, object_revision) = group_authority(&scenario, &show_id, "house").await;
    let request = group_management_request(
        "manage-secure",
        "house",
        rename("Secured"),
        object_revision,
        show_revision,
    );

    assert_eq!(
        scenario
            .group_management_action(&show_id, None, request.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    for field in ["desk_id", "user_id", "session_id"] {
        let mut forged = request.clone();
        forged[field] = serde_json::json!("forged");
        assert_eq!(
            scenario
                .group_management_action(&show_id, Some(&scenario.token), forged)
                .await
                .status(),
            StatusCode::OK,
            "{field} must be ignored so scope stays server-authored"
        );
    }
    let mut malformed_revision = request.clone();
    malformed_revision["expected_show_revision"] = serde_json::json!("forged");
    assert_eq!(
        scenario
            .group_management_action(&show_id, Some(&scenario.token), malformed_revision)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        scenario
            .group_management_action(&Uuid::new_v4().to_string(), Some(&scenario.token), request)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
