use super::*;

#[tokio::test]
async fn active_route_put_and_delete_share_the_prepared_application_boundary() {
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
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/open"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
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

    let updated = app
        .clone()
        .oneshot(
            Request::put(format!("/api/v1/shows/{show_id}/objects/route/main"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(updated_route.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    assert_eq!(updated.headers()[header::ETAG], "\"2\"");

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
    assert_eq!(stored.body()["future_client_field"], "accepted");
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

    let stale = app
        .clone()
        .oneshot(
            Request::put(format!("/api/v1/shows/{show_id}/objects/route/main"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(updated_route.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
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

    let deleted = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v1/shows/{show_id}/objects/route/main"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
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
    assert_eq!(route_backups, 2);
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
