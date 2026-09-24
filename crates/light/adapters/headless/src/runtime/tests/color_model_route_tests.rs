//! The show's colour programming model: stored in the show, defaulted by the desk for new shows
//! only, installed into the engine, and switched only with an acknowledged impact.

use super::*;

async fn send(
    app: &Router,
    token: &str,
    request: Request<Body>,
) -> (StatusCode, serde_json::Value) {
    let _ = token;
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or_else(
        |_| serde_json::json!({"raw": String::from_utf8_lossy(&bytes).into_owned()}),
    );
    (status, body)
}

fn get(token: &str, show_id: &str, uri: &str) -> Request<Body> {
    Request::get(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("x-tosk-show", show_id)
        .body(Body::empty())
        .unwrap()
}

fn post(token: &str, show_id: Option<&str>, uri: &str, body: serde_json::Value) -> Request<Body> {
    let mut request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    if let Some(show_id) = show_id {
        request = request.header("x-tosk-show", show_id);
    }
    request.body(Body::from(body.to_string())).unwrap()
}

async fn open(app: &Router, token: &str, show_id: &str) {
    let (status, body) = send(app, token, open_show_request(token, show_id)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

async fn snapshot(app: &Router, token: &str, show_id: &str) -> serde_json::Value {
    let (status, body) = send(
        app,
        token,
        get(token, show_id, "/api/v2/attribute-configuration"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn switch(
    app: &Router,
    token: &str,
    show_id: &str,
    model: &str,
    acknowledge: bool,
) -> (StatusCode, serde_json::Value) {
    let current = snapshot(app, token, show_id).await;
    send(
        app,
        token,
        post(
            token,
            Some(show_id),
            "/api/v2/attribute-configuration/update",
            serde_json::json!({
                "request_id": Uuid::new_v4().to_string(),
                "expected_show_revision": current["show_revision"],
                "expected_object_revision": current["object_revision"],
                "patch": { "color_model": model },
                "acknowledge_color_model_impact": acknowledge,
            }),
        ),
    )
    .await
}

async fn set_desk_default(app: &Router, token: &str, model: &str) {
    let (status, body) = send(
        app,
        token,
        post(
            token,
            None,
            "/api/v2/configuration/update",
            serde_json::json!({
                "request_id": Uuid::new_v4().to_string(),
                "patch": { "color_programming_model_default": model },
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

fn stored_object(path: &str) -> Option<serde_json::Value> {
    ActiveShowRepository::open(path)
        .unwrap()
        .portable_document()
        .unwrap()
        .object("attribute_configuration", "default")
        .map(|object| object.body().clone())
}

#[tokio::test]
async fn a_show_without_the_setting_stays_direct_and_an_empty_show_switches_freely() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Legacy colour").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let path = show["path"].as_str().unwrap().to_owned();
    assert!(
        stored_object(&path).is_none(),
        "a Direct desk writes no setting"
    );
    open(&app, &token, &show_id).await;

    let initial = snapshot(&app, &token, &show_id).await;
    assert_eq!(initial["configuration"]["color_model"], "direct");
    assert_eq!(
        state.output.engine().color_model(),
        light_core::ColorProgrammingModel::Direct
    );

    let (status, impact) = send(
        &app,
        &token,
        get(
            &token,
            &show_id,
            "/api/v2/attribute-configuration/color-model-impact?model=intent",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{impact}");
    assert_eq!(impact["lossy"], false);
    assert_eq!(impact["items"], serde_json::json!([]));

    let (status, saved) = switch(&app, &token, &show_id, "intent", false).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["snapshot"]["configuration"]["color_model"], "intent");
    assert_eq!(
        state.output.engine().color_model(),
        light_core::ColorProgrammingModel::Intent
    );
    assert_eq!(stored_object(&path).unwrap()["color_model"], "intent");

    let (status, saved) = switch(&app, &token, &show_id, "direct", false).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert!(
        stored_object(&path).unwrap().get("color_model").is_none(),
        "Direct is stored as the absence of the setting"
    );
    assert_eq!(
        state.output.engine().color_model(),
        light_core::ColorProgrammingModel::Direct
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn switching_a_programmed_show_is_refused_until_its_lossy_impact_is_acknowledged() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Programmed colour").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let path = show["path"].as_str().unwrap().to_owned();
    let fixture = Uuid::new_v4();
    // A half-level red and one fixture-native channel value, stored the way Direct records them.
    let dim_red = light_fixture::srgb_to_xyz(0.5, 0.0, 0.0);
    ActiveShowRepository::open(&path)
        .unwrap()
        .put_object(
            "preset",
            "2.1",
            &serde_json::json!({
                "name": "Dim red",
                "family": "Color",
                "number": 1,
                "values": {
                    fixture.to_string(): {
                        "color": {"kind": "color_xyz", "value": dim_red},
                        "color.white": {"kind": "normalized", "value": 0.5},
                    }
                },
            }),
            0,
        )
        .unwrap();
    open(&app, &token, &show_id).await;

    let (status, impact) = send(
        &app,
        &token,
        get(
            &token,
            &show_id,
            "/api/v2/attribute-configuration/color-model-impact?model=intent",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{impact}");
    assert_eq!(impact["lossy"], true, "{impact}");
    let kinds = impact["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["kind"].as_str().unwrap().to_owned(),
                item["count"].clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            ("native_color_values".to_owned(), serde_json::json!(1)),
            ("dimmed_whole_colors".to_owned(), serde_json::json!(1)),
        ]
    );

    let (status, refused) = switch(&app, &token, &show_id, "intent", false).await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert!(refused.to_string().contains("full brightness"), "{refused}");
    assert_eq!(
        state.output.engine().color_model(),
        light_core::ColorProgrammingModel::Direct,
        "a refused switch changes nothing"
    );

    let (status, saved) = switch(&app, &token, &show_id, "intent", true).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["snapshot"]["configuration"]["color_model"], "intent");
    let preset = ActiveShowRepository::open(&path)
        .unwrap()
        .portable_document()
        .unwrap()
        .object("preset", "2.1")
        .unwrap()
        .body()
        .clone();
    assert_eq!(
        preset["values"][fixture.to_string()]["color"]["value"]["x"],
        serde_json::json!(dim_red.x),
        "the switch rewrites no stored value"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_desk_default_is_copied_into_new_shows_only() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let before = create_show(&app, &token, "Before the default").await;

    set_desk_default(&app, &token, "intent").await;
    let intent = create_show(&app, &token, "Intent by default").await;
    assert_eq!(
        stored_object(intent["path"].as_str().unwrap()).unwrap()["color_model"],
        "intent"
    );
    assert!(
        stored_object(before["path"].as_str().unwrap()).is_none(),
        "an existing show is not reinterpreted"
    );

    let intent_id = intent["id"].as_str().unwrap().to_owned();
    open(&app, &token, &intent_id).await;
    assert_eq!(
        state.output.engine().color_model(),
        light_core::ColorProgrammingModel::Intent
    );
    assert_eq!(
        snapshot(&app, &token, &intent_id).await["configuration"]["color_model"],
        "intent"
    );

    set_desk_default(&app, &token, "direct").await;
    assert_eq!(
        stored_object(intent["path"].as_str().unwrap()).unwrap()["color_model"],
        "intent",
        "changing the desk default later keeps the show's own model"
    );
    let before_id = before["id"].as_str().unwrap().to_owned();
    open(&app, &token, &before_id).await;
    assert_eq!(
        state.output.engine().color_model(),
        light_core::ColorProgrammingModel::Direct,
        "opening another show installs that show's model"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
