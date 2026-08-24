use super::{playback_topology_route_support::open_topology_show, *};

#[tokio::test]
async fn show_level_route_is_cross_desk_and_publishes_one_replay_safe_event() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Scoped zones").await;
    let show_id = show["id"].as_str().unwrap();
    open_topology_show(&app, &token, show_id, None).await;
    let empty = get_zones(&app, &token).await;
    assert_eq!(empty.status(), StatusCode::OK);
    assert_eq!(
        json(empty).await,
        serde_json::json!({"show_id":show_id,"revision":0,"zones":[]})
    );

    let cursor = state.events.latest_sequence();
    let saved = put_zones(&app, &token, show_id, "save-zones", 0).await;
    assert_eq!(saved.status(), StatusCode::OK);
    assert_eq!(
        json(saved).await,
        serde_json::json!({
            "show_id": show_id,
            "revision": 1,
            "zones": zones(),
            "request_id": "save-zones",
            "replayed": false,
            "changed": true,
        })
    );
    let active = state.active_show.current().unwrap();
    let portable = ShowStore::open(&active.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(
        portable
            .object(
                VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_KIND,
                VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_ID,
            )
            .unwrap()
            .body(),
        &serde_json::json!({"revision":1,"zones":zones()})
    );
    assert!(
        state
            .installation
            .setting(&format!("virtual_playback_exclusion_zones:{show_id}"))
            .unwrap()
            .is_none()
    );

    let replay = put_zones(&app, &token, show_id, "save-zones", 0).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["changed"], true);
    assert_eq!(state.events.latest_sequence(), cursor + 1);
    let light_application::EventReplay::Events(events) = state.events.replay(
        cursor,
        &light_application::EventFilter::default()
            .with_capability(light_application::EventCapability::Show),
    ) else {
        panic!("zone invalidation event should remain replayable")
    };
    assert_eq!(events.len(), 1);
    let light_application::ApplicationEvent::Show(
        light_application::ShowEvent::VirtualPlaybackExclusionZonesChanged(change),
    ) = &events[0].payload
    else {
        panic!("expected typed exclusion-zone invalidation event")
    };
    assert_eq!(change.show_id.0.to_string(), show_id);
    assert_eq!(change.revision, 1);

    let no_change = json(put_zones(&app, &token, show_id, "same-zones-new-request", 1).await).await;
    assert_eq!(no_change["replayed"], false);
    assert_eq!(no_change["changed"], false);
    assert_eq!(state.events.latest_sequence(), cursor + 1);

    let stale_revision = put_zones(&app, &token, show_id, "stale-revision", 0).await;
    assert_eq!(stale_revision.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(stale_revision).await["error"],
        "Virtual Playback exclusion-zone revision conflict: expected 0, actual 1"
    );

    let second_desk = state.installation.add_desk("Zone wing").unwrap();
    let second_token = login_playback_user_on_desk(&app, "Operator", second_desk.id).await;
    let second = put_zones(&app, &second_token, show_id, "save-second-zones", 1).await;
    assert_eq!(second.status(), StatusCode::OK);
    let second = json(second).await;
    assert_eq!(second["changed"], false);
    assert_eq!(second["revision"], 1);
    let snapshot = json(get_zones(&app, &token).await).await;
    assert_eq!(snapshot["revision"], 1);
    assert_eq!(snapshot["zones"], zones());

    let foreign_show =
        put_zones(&app, &token, &Uuid::new_v4().to_string(), "foreign-show", 1).await;
    assert_eq!(foreign_show.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(foreign_show).await["error"],
        "X-Tosk-Show does not match the active show"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn incompatible_legacy_zone_shape_is_rejected_instead_of_silently_dropped() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Legacy zone schema").await;
    let show_id = show["id"].as_str().unwrap();
    open_topology_show(&app, &token, show_id, None).await;
    state.active_show.clear_document_cache();
    let active = state.active_show.current().unwrap();
    ShowStore::open(&active.path)
        .unwrap()
        .put_object(
            VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_KIND,
            VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_ID,
            &serde_json::json!({
                "revision": 1,
                "zones": [{"id":"legacy","name":"Legacy","slots":[1,2]}]
            }),
            0,
        )
        .unwrap();

    let response = get_zones(&app, &token).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        json(response).await["error"]
            .as_str()
            .unwrap()
            .starts_with("incompatible Virtual Playback exclusion-zone schema:")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn captured_show_scope_is_rejected_after_active_show_replacement() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let first = create_show(&app, &token, "First zone show").await;
    let first_id = first["id"].as_str().unwrap().to_owned();
    open_topology_show(&app, &token, &first_id, None).await;
    let second = create_show(&app, &token, "Replacement zone show").await;
    let second_id = second["id"].as_str().unwrap().to_owned();
    open_topology_show(&app, &token, &second_id, None).await;

    let stale = put_zones(&app, &token, &first_id, "stale-show", 0).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(stale).await["error"],
        "X-Tosk-Show does not match the active show"
    );
    let current = put_zones(&app, &token, &second_id, "current-show", 0).await;
    assert_eq!(current.status(), StatusCode::OK);
    assert_eq!(json(current).await["show_id"], second_id);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn zones() -> serde_json::Value {
    serde_json::json!([{"id":"paired","name":"Paired","playback_numbers":[1001,1002]}])
}

async fn get_zones(app: &Router, token: &str) -> Response {
    app.clone()
        .oneshot(
            Request::get("/api/v2/virtual-playback-exclusion-zones")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn put_zones(
    app: &Router,
    token: &str,
    show_id: &str,
    request_id: &str,
    expected_revision: u64,
) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v2/virtual-playback-exclusion-zones/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": request_id,
                        "expected_revision": expected_revision,
                        "zones": zones(),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
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
