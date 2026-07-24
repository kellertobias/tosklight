use super::*;

async fn post_action(
    app: &Router,
    token: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/shows")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, json(response).await)
}

#[tokio::test]
async fn show_library_v2_is_typed_tolerant_and_replay_safe() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let create = serde_json::json!({
        "request_id": "create-tour",
        "action": {
            "type": "create",
            "name": "Tour",
            "data_base64": null,
            "overwrite": false,
            "future_create_hint": true
        },
        "future_root_hint": true
    });

    let (status, first) = post_action(&app, &token, create.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["replayed"], false);
    assert_eq!(first["result"]["type"], "show");
    assert_eq!(first["result"]["show"]["name"], "Tour");
    let show_id = first["result"]["show"]["id"].as_str().unwrap();

    let (status, replay) = post_action(&app, &token, create).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"]["show"]["id"], show_id);

    let (status, conflict) = post_action(
        &app,
        &token,
        serde_json::json!({
            "request_id": "create-tour",
            "action": {
                "type": "create",
                "name": "Different",
                "data_base64": null,
                "overwrite": false
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(conflict["error"].as_str().unwrap().contains("request_id"));

    let snapshot = app
        .clone()
        .oneshot(
            Request::get("/api/v2/shows")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot = json(snapshot).await;
    assert_eq!(
        snapshot["shows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["id"] == show_id)
            .count(),
        1
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn show_library_v2_revision_retry_does_not_create_a_second_revision() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let (_, created) = post_action(
        &app,
        &token,
        serde_json::json!({
            "request_id": "create-revision-source",
            "action": {
                "type": "create",
                "name": "Revision Source",
                "data_base64": null,
                "overwrite": false
            }
        }),
    )
    .await;
    let show_id = created["result"]["show"]["id"].as_str().unwrap();
    let save = serde_json::json!({
        "request_id": "save-revision-once",
        "action": {
            "type": "save_revision",
            "show_id": show_id,
            "name": "Before experiment"
        }
    });

    let (status, first) = post_action(&app, &token, save.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["result"]["revision"]["revision"], 1);
    assert_eq!(first["replayed"], false);
    let (status, replay) = post_action(&app, &token, save).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["result"]["revision"]["revision"], 1);
    assert_eq!(replay["replayed"], true);

    let snapshot = app
        .clone()
        .oneshot(
            Request::get("/api/v2/shows")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let snapshot = json(snapshot).await;
    let show = snapshot["shows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == show_id)
        .unwrap();
    assert_eq!(show["revisions"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn retired_v1_show_library_and_mvr_routes_are_absent_but_object_routes_remain() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let id = Uuid::nil();
    for (method, path) in [
        (Method::GET, "/api/v1/shows".to_owned()),
        (Method::POST, "/api/v1/shows".to_owned()),
        (Method::POST, "/api/v1/shows/default/open".to_owned()),
        (Method::POST, "/api/v1/shows/rollback".to_owned()),
        (Method::POST, format!("/api/v1/shows/{id}/open")),
        (Method::PUT, format!("/api/v1/shows/{id}/rename")),
        (Method::GET, format!("/api/v1/shows/{id}/download")),
        (Method::POST, format!("/api/v1/shows/{id}/overwrite/{id}")),
        (Method::GET, format!("/api/v1/shows/{id}/revisions")),
        (Method::POST, format!("/api/v1/shows/{id}/revisions")),
        (Method::POST, format!("/api/v1/shows/{id}/revisions/1/open")),
        (Method::POST, "/api/v1/mvr/imports/preview".to_owned()),
        (Method::POST, format!("/api/v1/mvr/imports/{id}/apply")),
        (Method::GET, format!("/api/v1/shows/{id}/mvr/preview")),
        (Method::GET, format!("/api/v1/shows/{id}/mvr")),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Object route remains").await;
    let object_route = app
        .oneshot(
            Request::get(format!(
                "/api/v1/shows/{}/objects/group",
                show["id"].as_str().unwrap()
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(object_route.status(), StatusCode::OK);
    let _ = std::fs::remove_dir_all(data_dir);
}
