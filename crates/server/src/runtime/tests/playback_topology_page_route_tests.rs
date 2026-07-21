use super::{playback_topology_route_support::*, *};

#[tokio::test]
async fn create_page_returns_one_authority_and_is_replayable_and_idempotent() {
    let scenario = TopologyScenario::new("Create Playback Page").await;
    let revision = scenario.show_revision();
    let cursor = scenario.state.application_events.latest_sequence();
    let mut compatibility = scenario.state.events.subscribe();
    let request = create_page_request("create-page-four", 4, 0, None);

    let response = scenario.action(revision, request.clone()).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_etag(&response, revision + 1);
    let changed = json(response).await;
    assert_eq!(changed["status"], "changed");
    assert_eq!(
        changed["resolution"],
        serde_json::json!({"kind":"page","page":4})
    );
    assert_eq!(changed["objects"].as_array().unwrap().len(), 1);
    assert_eq!(changed["objects"][0]["kind"], "playback_page");
    assert_eq!(changed["objects"][0]["object_id"], "4");
    assert_eq!(
        changed["objects"][0]["body"],
        serde_json::json!({"number":4,"name":"Page 4","slots":{}})
    );
    assert_one_topology_event(&scenario.state, cursor, 1);
    let compatibility_events =
        std::iter::from_fn(|| compatibility.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(compatibility_events.len(), 1);
    assert_eq!(compatibility_events[0].kind, "show_object_changed");

    let replay = scenario.action(revision, request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], changed["event_sequence"]);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );

    let changed_page = scenario
        .action(
            revision,
            create_page_request("create-page-four", 5, 0, None),
        )
        .await;
    assert_eq!(changed_page.status(), StatusCode::CONFLICT);
    assert!(
        json(changed_page).await["error"]
            .as_str()
            .unwrap()
            .contains("request_id was already used")
    );
    assert!(scenario.document().object("playback_page", "5").is_none());
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );

    let page_revision = projection_revision(&changed, "playback_page");
    let no_change = scenario
        .action(
            revision + 1,
            create_page_request("ensure-page-four", 4, page_revision, Some("4")),
        )
        .await;
    assert_eq!(no_change.status(), StatusCode::OK);
    let no_change = json(no_change).await;
    assert_eq!(no_change["status"], "no_change");
    assert!(no_change.get("event_sequence").is_none());
    assert_eq!(no_change["objects"].as_array().unwrap().len(), 1);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );
    scenario.cleanup();
}

#[tokio::test]
async fn rename_page_is_lossless_no_change_replayable_and_revision_checked() {
    let scenario = TopologyScenario::new("Rename Playback Page").await;
    scenario.seed("playback", "8", &numbered_grand_master_playback_body(8));
    scenario.seed(
        "playback_page",
        "legacy-page-three",
        &serde_json::json!({
            "number":3,"name":"Before","slots":{"2":8},
            "future_layout":{"columns":8}
        }),
    );
    let revision = scenario.show_revision();
    let cursor = scenario.state.application_events.latest_sequence();
    let request = rename_page_request(
        "rename-page-three",
        3,
        "Act One",
        1,
        Some("legacy-page-three"),
    );

    let response = scenario.action(revision, request.clone()).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_etag(&response, revision + 1);
    let changed = json(response).await;
    assert_eq!(changed["status"], "changed");
    assert_eq!(
        changed["resolution"],
        serde_json::json!({"kind":"page","page":3})
    );
    assert_eq!(changed["objects"].as_array().unwrap().len(), 1);
    let page = &changed["objects"][0];
    assert_eq!(page["object_id"], "legacy-page-three");
    assert_eq!(page["body"]["name"], "Act One");
    assert_eq!(page["body"]["slots"], serde_json::json!({"2":8}));
    assert_eq!(page["body"]["future_layout"]["columns"], 8);
    assert_one_topology_event(&scenario.state, cursor, 1);

    let replay = scenario.action(revision, request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);
    let page_revision = projection_revision(&changed, "playback_page");

    let wrong_identity = scenario
        .action(
            revision + 1,
            rename_page_request(
                "rename-page-three-wrong-id",
                3,
                "Act Two",
                page_revision,
                Some("3"),
            ),
        )
        .await;
    assert_eq!(wrong_identity.status(), StatusCode::CONFLICT);
    let wrong_identity = json(wrong_identity).await;
    assert_eq!(wrong_identity["current_revision"], revision + 1);
    assert_eq!(wrong_identity["current_related_revision"], page_revision);

    let changed_name = scenario
        .action(
            revision,
            rename_page_request(
                "rename-page-three",
                3,
                "Act Two",
                1,
                Some("legacy-page-three"),
            ),
        )
        .await;
    assert_eq!(changed_name.status(), StatusCode::CONFLICT);
    assert!(
        json(changed_name).await["error"]
            .as_str()
            .unwrap()
            .contains("request_id was already used")
    );
    assert_eq!(
        scenario
            .document()
            .object("playback_page", "legacy-page-three")
            .unwrap()
            .body()["name"],
        "Act One"
    );

    let no_change = scenario
        .action(
            revision + 1,
            rename_page_request(
                "rename-page-three-no-change",
                3,
                "Act One",
                page_revision,
                Some("legacy-page-three"),
            ),
        )
        .await;
    assert_eq!(json(no_change).await["status"], "no_change");

    let stale = scenario
        .action(
            revision + 1,
            rename_page_request(
                "rename-page-three-stale",
                3,
                "Act Two",
                1,
                Some("legacy-page-three"),
            ),
        )
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale = json(stale).await;
    assert_eq!(stale["current_revision"], revision + 1);
    assert_eq!(stale["current_related_revision"], page_revision);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );
    scenario.cleanup();
}

#[tokio::test]
async fn strict_desk_page_selects_one_existing_page_without_creating_another() {
    let scenario = TopologyScenario::new("Strict desk Page selection").await;
    let configured = scenario
        .action(
            scenario.show_revision(),
            configure_request("configure-page-one", 0, 0),
        )
        .await;
    assert_eq!(configured.status(), StatusCode::OK);
    let created = scenario
        .action(
            scenario.show_revision(),
            create_page_request("create-page-two", 2, 0, None),
        )
        .await;
    assert_eq!(created.status(), StatusCode::OK);
    let desk_id = scenario_desk_id(&scenario);
    let show_id = scenario_show_id(&scenario);
    let cursor = scenario.state.application_events.latest_sequence();
    let mut compatibility = scenario.state.events.subscribe();

    let response = put_desk_page(&scenario, desk_id, 2, Some(true)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["desk_id"], desk_id.to_string());
    assert_eq!(body["page"], 2);
    assert_eq!(body["event_sequence"], cursor + 1);
    assert!(body["page_creation_event_sequence"].is_null());
    assert_eq!(desk_page(&scenario, desk_id, show_id), 2);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );
    let retained = show_events(&scenario.state, cursor);
    assert_eq!(retained.len(), 1);
    let light_application::ApplicationEvent::Desk(
        light_application::DeskEvent::PlaybackViewChanged(projection),
    ) = &retained[0].payload
    else {
        panic!("expected one authoritative Playback desk event")
    };
    assert_eq!(projection.desk_id, desk_id);
    assert_eq!(projection.active_page, 2);
    let events = std::iter::from_fn(|| compatibility.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "playback_page_changed");
    scenario.cleanup();
}

#[tokio::test]
async fn strict_missing_page_is_side_effect_free_while_legacy_advance_still_creates() {
    let scenario = TopologyScenario::new("Strict missing desk Page").await;
    let configured = scenario
        .action(
            scenario.show_revision(),
            configure_request("occupy-page-one", 0, 0),
        )
        .await;
    assert_eq!(configured.status(), StatusCode::OK);
    let desk_id = scenario_desk_id(&scenario);
    let show_id = scenario_show_id(&scenario);
    let revision = scenario.show_revision();
    let cursor = scenario.state.application_events.latest_sequence();
    let mut compatibility = scenario.state.events.subscribe();

    let strict = put_desk_page(&scenario, desk_id, 2, Some(true)).await;

    assert_eq!(strict.status(), StatusCode::BAD_REQUEST);
    assert_eq!(scenario.show_revision(), revision);
    assert!(scenario.document().object("playback_page", "2").is_none());
    assert!(
        !scenario
            .state
            .engine
            .snapshot()
            .playback_pages
            .iter()
            .any(|page| page.number == 2)
    );
    assert_eq!(desk_page(&scenario, desk_id, show_id), 1);
    assert_eq!(scenario.state.application_events.latest_sequence(), cursor);
    assert!(compatibility.try_recv().is_err());

    let legacy = put_desk_page(&scenario, desk_id, 2, None).await;

    assert_eq!(legacy.status(), StatusCode::OK);
    let legacy = json(legacy).await;
    assert_eq!(legacy["page_creation_event_sequence"], cursor + 1);
    assert_eq!(legacy["event_sequence"], cursor + 2);
    assert_eq!(desk_page(&scenario, desk_id, show_id), 2);
    assert!(scenario.document().object("playback_page", "2").is_some());
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 2
    );
    let events = std::iter::from_fn(|| compatibility.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, "show_object_changed");
    assert_eq!(events[1].kind, "playback_page_changed");
    scenario.cleanup();
}

#[tokio::test]
async fn page_route_rejects_unauthenticated_unsafe_and_canonical_collision_requests() {
    let scenario = TopologyScenario::new("Playback Page validation").await;
    scenario.seed(
        "playback_page",
        "6",
        &serde_json::json!({"number":9,"name":"Occupied","slots":{}}),
    );
    let revision = scenario.show_revision();
    let cursor = scenario.state.application_events.latest_sequence();
    let request = create_page_request("create-page-six", 6, 0, None);

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

    let collision = scenario.action(revision, request).await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);
    assert_etag(&collision, revision);
    let collision = json(collision).await;
    assert_eq!(collision["current_revision"], revision);
    assert_eq!(collision["current_related_revision"], 1);

    let mut unsafe_request = rename_page_request("unsafe-page", 9, "Nine", 1, Some("6"));
    unsafe_request["action"]["expected_page_revision"] =
        serde_json::json!(9_007_199_254_740_992_u64);
    let unsafe_response = scenario.action(revision, unsafe_request).await;
    assert_eq!(unsafe_response.status(), StatusCode::BAD_REQUEST);
    assert!(
        json(unsafe_response).await["error"]
            .as_str()
            .unwrap()
            .contains("safe integer")
    );
    assert_eq!(scenario.state.application_events.latest_sequence(), cursor);
    scenario.cleanup();
}

async fn put_desk_page(
    scenario: &TopologyScenario,
    desk_id: Uuid,
    page: u8,
    existing_only: Option<bool>,
) -> Response {
    let mut body = serde_json::json!({"page":page});
    if let Some(existing_only) = existing_only {
        body["existing_only"] = serde_json::json!(existing_only);
    }
    scenario
        .app
        .clone()
        .oneshot(
            Request::put(format!("/api/v1/control-desks/{desk_id}/page"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

fn scenario_desk_id(scenario: &TopologyScenario) -> Uuid {
    scenario
        .state
        .sessions
        .read()
        .values()
        .next()
        .unwrap()
        .desk
        .id
}

fn scenario_show_id(scenario: &TopologyScenario) -> light_core::ShowId {
    light_core::ShowId(Uuid::parse_str(&scenario.show_id).unwrap())
}

fn desk_page(scenario: &TopologyScenario, desk_id: Uuid, show_id: light_core::ShowId) -> u8 {
    scenario
        .state
        .desk
        .lock()
        .desk_page(desk_id, show_id)
        .unwrap()
}
