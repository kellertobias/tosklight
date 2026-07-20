use super::{playback_topology_route_support::*, *};

#[tokio::test]
async fn configure_no_change_replay_conflict_and_clear_are_one_event_actions() {
    let scenario = TopologyScenario::new("Playback topology lifecycle").await;
    let initial_revision = scenario.show_revision();
    let cursor = scenario.state.application_events.latest_sequence();
    let mut compatibility = scenario.state.events.subscribe();
    let configure = configure_request("configure-house", 0, 0);

    let response = scenario.action(initial_revision, configure.clone()).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_etag(&response, initial_revision + 1);
    let configured = json(response).await;
    assert_eq!(configured["request_id"], "configure-house");
    assert_eq!(configured["status"], "changed");
    assert_eq!(configured["show_revision"], initial_revision + 1);
    assert_eq!(configured["resolution"]["kind"], "page_slot");
    assert_eq!(configured["resolution"]["playback_number"], 1);
    assert_eq!(configured["event_sequence"], cursor + 1);
    assert_eq!(configured["replayed"], false);
    let playback_revision = projection_revision(&configured, "playback");
    let page_revision = projection_revision(&configured, "playback_page");
    assert_eq!(show_events(&scenario.state, cursor).len(), 1);
    assert_one_topology_event(&scenario.state, cursor, 2);
    let compatibility_events =
        std::iter::from_fn(|| compatibility.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(compatibility_events.len(), 2);
    assert!(
        compatibility_events
            .iter()
            .all(|event| event.kind == "show_object_changed")
    );

    let replay = scenario.action(initial_revision, configure).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_etag(&replay, initial_revision + 1);
    let replay = json(replay).await;
    assert_eq!(replay["status"], "changed");
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], cursor + 1);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );

    let no_change = scenario
        .action(
            initial_revision + 1,
            configure_request("configure-identical", page_revision, playback_revision),
        )
        .await;
    assert_eq!(no_change.status(), StatusCode::OK);
    assert_etag(&no_change, initial_revision + 1);
    let no_change = json(no_change).await;
    assert_eq!(no_change["status"], "no_change");
    assert!(no_change.get("event_sequence").is_none());
    assert_eq!(no_change["objects"].as_array().unwrap().len(), 2);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );

    let stale_show = scenario
        .action(
            initial_revision,
            configure_request("stale-show", page_revision, playback_revision),
        )
        .await;
    assert_eq!(stale_show.status(), StatusCode::CONFLICT);
    assert_etag(&stale_show, initial_revision + 1);
    let stale_show = json(stale_show).await;
    assert_eq!(stale_show["kind"], "conflict");
    assert_eq!(stale_show["current_revision"], initial_revision + 1);
    assert!(stale_show.get("current_related_revision").is_none());

    let stale_page = scenario
        .action(
            initial_revision + 1,
            configure_request("stale-page", 0, playback_revision),
        )
        .await;
    assert_eq!(stale_page.status(), StatusCode::CONFLICT);
    assert_etag(&stale_page, initial_revision + 1);
    let stale_page = json(stale_page).await;
    assert_eq!(stale_page["current_revision"], initial_revision + 1);
    assert_eq!(stale_page["current_related_revision"], page_revision);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );

    let clear = scenario
        .action(
            initial_revision + 1,
            clear_request("clear-house", page_revision, playback_revision),
        )
        .await;
    assert_eq!(clear.status(), StatusCode::OK);
    assert_etag(&clear, initial_revision + 2);
    let clear = json(clear).await;
    assert_eq!(clear["status"], "changed");
    assert_eq!(clear["event_sequence"], cursor + 2);
    assert!(clear["objects"].as_array().unwrap().iter().any(|object| {
        object["kind"] == "playback" && object["object_id"] == "1" && object["state"] == "deleted"
    }));
    assert_one_topology_event(&scenario.state, cursor + 1, 2);
    assert!(scenario.document().object("playback", "1").is_none());
    assert_eq!(scenario.show_revision(), initial_revision + 2);
    scenario.cleanup();
}

#[tokio::test]
async fn save_cue_list_preserves_extensions_and_returns_the_committed_projection() {
    let scenario = TopologyScenario::new("Playback topology Cuelist").await;
    let revision = scenario.show_revision();
    let cursor = scenario.state.application_events.latest_sequence();
    let request = save_request("save-lossless", 0);
    let cue_list_id = request["action"]["cue_list_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let response = scenario.action(revision, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_etag(&response, revision + 1);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "changed");
    assert_eq!(outcome["resolution"]["kind"], "cue_list");
    assert_eq!(outcome["resolution"]["cue_list_id"], cue_list_id);
    assert_eq!(outcome["objects"].as_array().unwrap().len(), 1);
    assert_eq!(
        outcome["objects"][0]["body"]["future_topology"]["retained"],
        true
    );
    assert_one_topology_event(&scenario.state, cursor, 1);
    let document = scenario.document();
    let stored = document.object("cue_list", &cue_list_id).unwrap();
    assert_eq!(stored.body()["future_topology"]["retained"], true);
    scenario.cleanup();
}

#[tokio::test]
async fn topology_route_rejects_missing_authority_and_forged_scope() {
    let scenario = TopologyScenario::new("Playback topology authorization").await;
    let revision = scenario.show_revision();
    let request = configure_request("authorization", 0, 0);
    let cursor = scenario.state.application_events.latest_sequence();

    let unauthorized = post_topology(
        &scenario.app,
        None,
        &scenario.show_id,
        Some(revision),
        request.clone(),
        None,
    )
    .await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json(unauthorized).await["kind"], "unauthorized");

    let missing_revision = post_topology(
        &scenario.app,
        Some(&scenario.token),
        &scenario.show_id,
        None,
        request.clone(),
        None,
    )
    .await;
    assert_eq!(missing_revision.status(), StatusCode::BAD_REQUEST);
    let missing_revision = json(missing_revision).await;
    assert_eq!(missing_revision["kind"], "invalid");
    assert!(
        missing_revision["error"]
            .as_str()
            .unwrap()
            .contains("If-Match")
    );

    let foreign_show = post_topology(
        &scenario.app,
        Some(&scenario.token),
        &Uuid::new_v4().to_string(),
        Some(revision),
        request.clone(),
        None,
    )
    .await;
    assert_eq!(foreign_show.status(), StatusCode::NOT_FOUND);
    assert_eq!(json(foreign_show).await["kind"], "not_found");

    for field in ["show_id", "desk_id", "user_id", "session_id"] {
        let mut forged = request.clone();
        forged[field] = serde_json::json!(Uuid::new_v4());
        let response = scenario.action(revision, forged).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{field}");
        assert_eq!(json(response).await["kind"], "invalid", "{field}");
    }
    assert_eq!(scenario.state.application_events.latest_sequence(), cursor);
    scenario.cleanup();
}

#[tokio::test]
async fn same_user_two_desks_and_another_user_share_the_active_show() {
    let scenario = TopologyScenario::new("Playback topology shared show").await;
    let primary_user_id = scenario
        .state
        .sessions
        .read()
        .values()
        .find(|session| session.token == scenario.token)
        .unwrap()
        .user
        .id;
    let (same_user_desk, other_user_desk) = {
        let store = scenario.state.desk.lock();
        let same = store.add_desk("Topology wing", "topology-wing").unwrap();
        store.add_user("Topology guest").unwrap();
        let other = store
            .add_desk("Topology guest desk", "topology-guest")
            .unwrap();
        (same, other)
    };
    let same_user = login_on_desk(&scenario.app, "Operator", Some(same_user_desk.id), None).await;
    let other_user = login_on_desk(
        &scenario.app,
        "Topology guest",
        Some(other_user_desk.id),
        None,
    )
    .await;
    assert_eq!(same_user["user"]["id"], primary_user_id.0.to_string());
    assert_ne!(other_user["user"]["id"], primary_user_id.0.to_string());
    let cursor = scenario.state.application_events.latest_sequence();

    for (token, request_id) in [
        (scenario.token.as_str(), "primary-save"),
        (same_user["token"].as_str().unwrap(), "same-user-wing-save"),
        (other_user["token"].as_str().unwrap(), "other-user-save"),
    ] {
        let response = post_topology(
            &scenario.app,
            Some(token),
            &scenario.show_id,
            Some(scenario.show_revision()),
            save_request(request_id, 0),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{request_id}");
        assert_eq!(json(response).await["status"], "changed", "{request_id}");
    }
    let events = show_events(&scenario.state, cursor);
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| matches!(
        event.payload,
        light_application::ApplicationEvent::Show(light_application::ShowEvent::ObjectsChanged(_))
    )));
    scenario.cleanup();
}

#[tokio::test]
async fn configured_desk_boundary_rejects_missing_or_foreign_credentials() {
    let (mut state, data_dir) = test_state();
    state.desk_token = Some(Arc::from("topology-boundary"));
    let app = router(state.clone());
    let session = login_on_desk(&app, "Operator", None, Some("topology-boundary")).await;
    let token = session["token"].as_str().unwrap();
    let show = create_show_with_boundary(&app, token, "Boundary show", "topology-boundary").await;
    let show_id = show["id"].as_str().unwrap();
    open_topology_show(&app, token, show_id, Some("topology-boundary")).await;
    let revision = state
        .active_show
        .read()
        .as_ref()
        .map(|entry| {
            ShowStore::open(&entry.path)
                .unwrap()
                .portable_revision()
                .unwrap()
                .value()
        })
        .unwrap();

    for boundary in [None, Some("foreign-boundary")] {
        let response = post_topology(
            &app,
            Some(token),
            show_id,
            Some(revision),
            clear_request("boundary-denied", 0, 0),
            boundary,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let allowed = post_topology(
        &app,
        Some(token),
        show_id,
        Some(revision),
        clear_request("boundary-allowed", 0, 0),
        Some("topology-boundary"),
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    assert_eq!(json(allowed).await["status"], "no_change");
    let _ = std::fs::remove_dir_all(data_dir);
}

fn assert_one_topology_event(state: &AppState, after: u64, changed_objects: usize) {
    let events = show_events(state, after);
    assert_eq!(events.len(), 1);
    let light_application::ApplicationEvent::Show(light_application::ShowEvent::ObjectsChanged(
        change,
    )) = &events[0].payload
    else {
        panic!("expected one active-show ObjectsChanged event")
    };
    assert_eq!(change.changes.len(), changed_objects);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    assert!(events[0].correlation_id.is_some());
}
