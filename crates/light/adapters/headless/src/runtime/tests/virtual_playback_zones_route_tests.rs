use super::{playback_topology_route_support::open_topology_show, *};

const SURFACE_ID: &str = "surface-a";

#[tokio::test]
async fn show_level_route_returns_all_desks_and_publishes_one_replay_safe_event() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Scoped zones").await;
    let show_id = show["id"].as_str().unwrap();
    open_topology_show(&app, &token, show_id, None).await;
    let desk_id = authenticated_desk_id(&state, &token);

    let empty = get_zones(&app, &token).await;
    assert_eq!(empty.status(), StatusCode::OK);
    assert_eq!(
        json(empty).await,
        serde_json::json!({"show_id":show_id,"desks":{}})
    );

    let cursor = state.events.latest_sequence();
    let saved = put_zones(&app, &token, show_id, SURFACE_ID, "save-zones").await;
    assert_eq!(saved.status(), StatusCode::OK);
    assert_eq!(
        json(saved).await,
        serde_json::json!({
            "show_id": show_id,
            "desk_id": desk_id,
            "surface_id": SURFACE_ID,
            "zones": zones(),
            "request_id": "save-zones",
            "replayed": false,
            "changed": true,
        })
    );

    let replay = put_zones(&app, &token, show_id, SURFACE_ID, "save-zones").await;
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
    assert_eq!(change.desk_id, desk_id);
    assert_eq!(change.surface_id, SURFACE_ID);

    let no_change =
        json(put_zones(&app, &token, show_id, SURFACE_ID, "same-zones-new-request").await).await;
    assert_eq!(no_change["replayed"], false);
    assert_eq!(no_change["changed"], false);
    assert_eq!(state.events.latest_sequence(), cursor + 1);

    let second_desk = state
        .installation
        .add_desk("Zone wing", "zone-wing")
        .unwrap();
    let second_token = login_playback_user_on_desk(&app, "Operator", second_desk.id).await;
    let second = put_zones(
        &app,
        &second_token,
        show_id,
        "surface-b",
        "save-second-zones",
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let snapshot = json(get_zones(&app, &token).await).await;
    assert_eq!(snapshot["desks"][desk_id.to_string()][SURFACE_ID], zones());
    assert_eq!(
        snapshot["desks"][second_desk.id.to_string()]["surface-b"],
        zones()
    );

    let foreign_show = put_zones(
        &app,
        &token,
        &Uuid::new_v4().to_string(),
        SURFACE_ID,
        "foreign-show",
    )
    .await;
    assert_eq!(foreign_show.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(foreign_show).await["error"],
        "X-Tosk-Show does not match the active show"
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

    let stale = put_zones(&app, &token, &first_id, SURFACE_ID, "stale-show").await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(stale).await["error"],
        "X-Tosk-Show does not match the active show"
    );
    let first_store = state
        .installation
        .virtual_playback_exclusions(light_core::ShowId(Uuid::parse_str(&first_id).unwrap()));
    let second_store = state
        .installation
        .virtual_playback_exclusions(light_core::ShowId(Uuid::parse_str(&second_id).unwrap()));
    assert!(first_store.is_empty());
    assert!(second_store.is_empty());

    let current = put_zones(&app, &token, &second_id, SURFACE_ID, "current-show").await;
    assert_eq!(current.status(), StatusCode::OK);
    assert_eq!(json(current).await["show_id"], second_id);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn authenticated_desk_id(state: &AppState, token: &str) -> Uuid {
    state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap()
        .desk
        .id
}

fn zones() -> serde_json::Value {
    serde_json::json!([{"id":"paired","name":"Paired","slots":[1,2]}])
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
    surface_id: &str,
    request_id: &str,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!(
                "/api/v2/virtual-playback-exclusion-zones/{surface_id}/update"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header("x-tosk-show", show_id)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"request_id":request_id,"zones":zones()}).to_string(),
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
