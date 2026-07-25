use super::*;

async fn post_output_route_action(
    app: &Router,
    token: &str,
    show_id: &str,
    body: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v2/output-routes/actions")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn typed_route_update_and_delete_share_the_prepared_application_boundary() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Route boundary").await;
    let show_id = show["id"].as_str().unwrap();
    let show_uuid = Uuid::parse_str(show_id).unwrap();
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(show_uuid))
        .unwrap()
        .unwrap();
    ShowStore::open(&entry.path)
        .unwrap()
        .put_object(
            "route",
            "main",
            &serde_json::json!({
                "protocol": "art_net",
                "logical_universe": 1,
                "destination_universe": 1,
                "delivery_mode": "broadcast",
                "destination": null,
                "enabled": true,
                "minimum_slots": 512,
                "future_server_field": {"kept": true}
            }),
            0,
        )
        .unwrap();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let before_events = state.application_events.latest_sequence();
    let updated_route = serde_json::json!({
        "protocol": "art_net",
        "logical_universe": 1,
        "destination_universe": 2,
        "delivery_mode": "broadcast",
        "destination": null,
        "enabled": true,
        "minimum_slots": 128,
        "future_client_field": "accepted"
    });

    let updated = post_output_route_action(
        &app,
        &token,
        show_id,
        serde_json::json!({
            "request_id": "active-route-update",
            "action": {
                "type": "update",
                "route_id": "main",
                "expected_revision": 1,
                "patch": updated_route,
            }
        }),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);

    let document = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    let stored = document.object("route", "main").unwrap();
    assert_eq!(stored.revision(), 2);
    assert_eq!(
        stored.body()["future_server_field"],
        serde_json::json!({"kept": true})
    );
    assert!(stored.body()["future_client_field"].is_null());
    assert_eq!(
        state.engine.snapshot().revision,
        document.revision().value()
    );
    assert_eq!(state.engine.snapshot().routes[0].destination_universe, 2);
    assert_eq!(
        state.application_events.latest_sequence(),
        before_events + 1
    );
    assert_output_route_event(&state, before_events, "main", false);

    let stale = post_output_route_action(
        &app,
        &token,
        show_id,
        serde_json::json!({
            "request_id": "active-route-stale",
            "action": {
                "type": "update",
                "route_id": "main",
                "expected_revision": 1,
                "patch": {"enabled": false},
            }
        }),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        state.application_events.latest_sequence(),
        before_events + 1
    );
    assert_eq!(
        ShowStore::open(&entry.path)
            .unwrap()
            .portable_document()
            .unwrap()
            .object("route", "main")
            .unwrap()
            .revision(),
        2
    );

    let deleted = post_output_route_action(
        &app,
        &token,
        show_id,
        serde_json::json!({
            "request_id": "active-route-delete",
            "action": {
                "type": "delete",
                "route_id": "main",
                "expected_revision": 2,
            }
        }),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);
    let document = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert!(document.object("route", "main").is_none());
    assert!(state.engine.snapshot().routes.is_empty());
    assert_eq!(
        state.engine.snapshot().revision,
        document.revision().value()
    );
    assert_output_route_event(&state, before_events + 1, "main", true);

    let route_backups = std::fs::read_dir(data_dir.join("backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.contains("-output-route-"))
        })
        .count();
    // Put and delete share one interval-gated recovery checkpoint.
    assert_eq!(route_backups, 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn assert_output_route_event(state: &AppState, after: u64, route_id: &str, deleted: bool) {
    let filter = light_application::EventFilter::default()
        .with_capability(light_application::EventCapability::Output);
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(after, &filter)
    else {
        panic!("expected retained output-route event");
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        light_application::ApplicationEvent::Show(
            light_application::ShowEvent::OutputRouteChanged(change)
        ) if change.route_id == route_id && change.deleted == deleted
    ));
}
