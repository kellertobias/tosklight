use super::{playback_topology_route_support::open_topology_show, *};

const SURFACE_ID: &str = "surface-a";

#[tokio::test]
async fn scoped_routes_prove_show_and_authenticated_desk_authority() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Scoped zones").await;
    let show_id = show["id"].as_str().unwrap();
    open_topology_show(&app, &token, show_id, None).await;
    let desk_id = authenticated_desk_id(&state, &token);

    let empty = get_zones(&app, &token, show_id, desk_id).await;
    assert_eq!(empty.status(), StatusCode::OK);
    assert_eq!(
        json(empty).await,
        serde_json::json!({"show_id":show_id,"desk_id":desk_id,"surfaces":{}})
    );

    let saved = put_zones(&app, &token, show_id, desk_id, SURFACE_ID).await;
    assert_eq!(saved.status(), StatusCode::OK);
    assert_eq!(
        json(saved).await,
        serde_json::json!({
            "show_id": show_id,
            "desk_id": desk_id,
            "surface_id": SURFACE_ID,
            "zones": zones(),
        })
    );

    let foreign_desk = put_zones(&app, &token, show_id, Uuid::new_v4(), SURFACE_ID).await;
    assert_eq!(foreign_desk.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        json(foreign_desk).await["error"],
        "session is not authorized for this desk"
    );
    let foreign_show = put_zones(
        &app,
        &token,
        &Uuid::new_v4().to_string(),
        desk_id,
        SURFACE_ID,
    )
    .await;
    assert_eq!(foreign_show.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(foreign_show).await["error"],
        "requested show is no longer active"
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
    let desk_id = authenticated_desk_id(&state, &token);

    let second = create_show(&app, &token, "Replacement zone show").await;
    let second_id = second["id"].as_str().unwrap().to_owned();
    open_topology_show(&app, &token, &second_id, None).await;

    let stale = put_zones(&app, &token, &first_id, desk_id, SURFACE_ID).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(stale).await["error"],
        "requested show is no longer active"
    );
    {
        let desk = state.desk.lock();
        let first_store = read_virtual_playback_exclusion_store(
            &desk,
            light_core::ShowId(Uuid::parse_str(&first_id).unwrap()),
        );
        let second_store = read_virtual_playback_exclusion_store(
            &desk,
            light_core::ShowId(Uuid::parse_str(&second_id).unwrap()),
        );
        assert!(first_store.is_empty());
        assert!(second_store.is_empty());
    }

    let current = put_zones(&app, &token, &second_id, desk_id, SURFACE_ID).await;
    assert_eq!(current.status(), StatusCode::OK);
    assert_eq!(json(current).await["show_id"], second_id);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn authenticated_desk_id(state: &AppState, token: &str) -> Uuid {
    state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .unwrap()
        .desk
        .id
}

fn zones() -> serde_json::Value {
    serde_json::json!([{"id":"paired","name":"Paired","slots":[1,2]}])
}

async fn get_zones(app: &Router, token: &str, show_id: &str, desk_id: Uuid) -> Response {
    app.clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/shows/{show_id}/desks/{desk_id}/virtual-playback-exclusion-zones"
            ))
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
    desk_id: Uuid,
    surface_id: &str,
) -> Response {
    app.clone()
        .oneshot(
            Request::put(format!(
                "/api/v2/shows/{show_id}/desks/{desk_id}/virtual-playback-exclusion-zones/{surface_id}"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::json!({"zones":zones()}).to_string()))
            .unwrap(),
        )
        .await
        .unwrap()
}
