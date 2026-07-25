use super::*;

async fn post_route_action(
    app: &Router,
    token: &str,
    show_id: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/output-routes/actions")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, json(response).await)
}

async fn open_show(app: &Router, token: &str, name: &str) -> String {
    let show = create_show(app, token, name).await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(open_show_request(token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    show_id
}

#[tokio::test]
async fn show_object_v2_snapshots_require_authentication_and_active_show_scope() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let unauthenticated = app
        .clone()
        .oneshot(
            Request::get("/api/v2/objects/group")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let (token, _) = login(&app, "Operator").await;
    let show_id = open_show(&app, &token, "Object snapshots").await;
    let snapshot = app
        .clone()
        .oneshot(
            Request::get("/api/v2/objects/group")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", &show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot = json(snapshot).await;
    assert_eq!(snapshot["show_id"], show_id);
    assert_eq!(snapshot["kind"], "group");
    assert!(snapshot["show_revision"].is_number());
    assert!(snapshot["objects"].is_array());

    let foreign = app
        .oneshot(
            Request::get("/api/v2/objects/group")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", Uuid::new_v4().to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(foreign).await["error"],
        "X-Tosk-Show does not match the active show"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn output_route_v2_actions_are_partial_tolerant_and_replay_safe() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show_id = open_show(&app, &token, "Output route intents").await;
    let before_events = state.application_events.latest_sequence();
    let create = serde_json::json!({
        "request_id": "create-main-route",
        "action": {
            "type": "create",
            "route_id": "main",
            "route": {
                "protocol": "art_net",
                "logical_universe": 1,
                "destination_universe": 2,
                "delivery_mode": "broadcast",
                "destination": null,
                "enabled": true,
                "minimum_slots": 128,
                "future_route_field": true
            },
            "future_action_field": true
        },
        "future_request_field": true
    });

    let (status, created) = post_route_action(&app, &token, &show_id, create.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["replayed"], false);
    assert_eq!(created["change"]["route_id"], "main");
    assert_eq!(created["change"]["object_revision"], 1);
    assert_eq!(created["change"]["deleted"], false);
    assert_eq!(
        state.application_events.latest_sequence(),
        before_events + 1
    );

    let (status, replay) = post_route_action(&app, &token, &show_id, create).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["change"], created["change"]);
    assert_eq!(
        state.application_events.latest_sequence(),
        before_events + 1
    );

    let update = serde_json::json!({
        "request_id": "disable-main-route",
        "action": {
            "type": "update",
            "route_id": "main",
            "expected_revision": 1,
            "patch": {
                "enabled": false,
                "future_patch_field": "ignored"
            }
        }
    });
    let (status, updated) = post_route_action(&app, &token, &show_id, update).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["change"]["object_revision"], 2);
    assert_eq!(updated["change"]["route"]["enabled"], false);
    assert_eq!(updated["change"]["route"]["destination_universe"], 2);
    assert_eq!(updated["change"]["route"]["minimum_slots"], 128);

    let exact = app
        .clone()
        .oneshot(
            Request::get("/api/v2/objects/route/main")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", &show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exact.status(), StatusCode::OK);
    let exact = json(exact).await;
    assert_eq!(exact["object"]["revision"], 2);
    assert_eq!(exact["object"]["body"]["enabled"], false);
    assert_eq!(exact["object"]["body"]["destination_universe"], 2);

    let (status, conflict) = post_route_action(
        &app,
        &token,
        &show_id,
        serde_json::json!({
            "request_id": "disable-main-route",
            "action": {
                "type": "delete",
                "route_id": "main",
                "expected_revision": 2
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(conflict["error"].as_str().unwrap().contains("request_id"));

    let delete = serde_json::json!({
        "request_id": "delete-main-route",
        "action": {
            "type": "delete",
            "route_id": "main",
            "expected_revision": 2
        }
    });
    let (status, deleted) = post_route_action(&app, &token, &show_id, delete.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["change"]["deleted"], true);
    assert_eq!(deleted["change"]["route"], serde_json::Value::Null);
    let (status, replayed_delete) = post_route_action(&app, &token, &show_id, delete).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed_delete["replayed"], true);
    assert_eq!(replayed_delete["change"], deleted["change"]);
    assert_eq!(
        state.application_events.latest_sequence(),
        before_events + 3
    );

    let _ = std::fs::remove_dir_all(data_dir);
}
