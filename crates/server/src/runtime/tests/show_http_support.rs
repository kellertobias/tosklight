async fn create_show(app: &Router, token: &str, name: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(show_action_request(
            token,
            serde_json::json!({
                "type": "create",
                "name": name,
                "data_base64": null,
                "overwrite": false
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    show_action_result(json(response).await, "show")
}

fn show_action_request(token: &str, action: serde_json::Value) -> Request<Body> {
    Request::post("/api/v2/shows")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(
            serde_json::json!({
                "request_id": Uuid::new_v4().to_string(),
                "action": action
            })
            .to_string(),
        ))
        .unwrap()
}

fn show_snapshot_request(token: &str) -> Request<Body> {
    Request::get("/api/v2/shows")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn with_desk_boundary(mut request: Request<Body>, boundary: &str) -> Request<Body> {
    request.headers_mut().insert(
        "x-light-desk-token",
        boundary.parse().expect("valid desk boundary header"),
    );
    request
}

fn open_show_request(token: &str, show_id: impl ToString) -> Request<Body> {
    open_show_with_transition_request(token, show_id, "hold_current", None)
}

fn open_show_with_transition_request(
    token: &str,
    show_id: impl ToString,
    transition: &str,
    transition_millis: Option<u64>,
) -> Request<Body> {
    show_action_request(
        token,
        serde_json::json!({
            "type": "open",
            "show_id": show_id.to_string(),
            "transition": transition,
            "transition_millis": transition_millis
        }),
    )
}

fn open_default_show_request(token: &str) -> Request<Body> {
    show_action_request(
        token,
        serde_json::json!({
            "type": "open_default",
            "transition": "safe_blackout",
            "transition_millis": null
        }),
    )
}

fn save_show_revision_request(
    token: &str,
    show_id: impl ToString,
    name: &str,
) -> Request<Body> {
    show_action_request(
        token,
        serde_json::json!({
            "type": "save_revision",
            "show_id": show_id.to_string(),
            "name": name
        }),
    )
}

fn open_show_revision_request(
    token: &str,
    show_id: impl ToString,
    revision: u64,
) -> Request<Body> {
    show_action_request(
        token,
        serde_json::json!({
            "type": "open_revision",
            "show_id": show_id.to_string(),
            "revision": revision,
            "transition": "hold_current",
            "transition_millis": null
        }),
    )
}

fn rename_show_request(token: &str, show_id: impl ToString, name: &str) -> Request<Body> {
    show_action_request(
        token,
        serde_json::json!({
            "type": "rename",
            "show_id": show_id.to_string(),
            "name": name
        }),
    )
}

fn overwrite_show_request(
    token: &str,
    source_show_id: impl ToString,
    destination_show_id: impl ToString,
) -> Request<Body> {
    show_action_request(
        token,
        serde_json::json!({
            "type": "overwrite",
            "source_show_id": source_show_id.to_string(),
            "destination_show_id": destination_show_id.to_string()
        }),
    )
}

fn rollback_show_request(token: &str) -> Request<Body> {
    show_action_request(
        token,
        serde_json::json!({
            "type": "rollback",
            "transition": "hold_current",
            "transition_millis": null
        }),
    )
}

async fn show_revision_snapshot(app: &Router, token: &str, show_id: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(show_snapshot_request(token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await["shows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|show| show["id"] == show_id)
        .expect("show must exist in library snapshot")["revisions"]
        .clone()
}

fn show_action_result(mut outcome: serde_json::Value, expected_type: &str) -> serde_json::Value {
    let result = outcome
        .get_mut("result")
        .expect("show action outcome must contain a result");
    assert_eq!(result["type"], expected_type);
    result
        .get_mut(expected_type)
        .expect("show action result must contain its typed payload")
        .take()
}
