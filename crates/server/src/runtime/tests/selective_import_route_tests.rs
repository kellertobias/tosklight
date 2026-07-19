use super::*;

#[tokio::test]
async fn v2_selective_import_previews_conflicts_and_applies_one_atomic_revision() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let source = create_show(&app, &token, "Import Source").await;
    let target = create_show(&app, &token, "Import Target").await;
    let source_id = source["id"].as_str().unwrap();
    let target_id = target["id"].as_str().unwrap();
    put_group(&state, source_id, "front", "Source Front");
    put_group(&state, target_id, "front", "Target Front");
    open_import_target(&app, &token, target_id).await;

    let denied = app
        .clone()
        .oneshot(
            Request::get(import_path(target_id, source_id, "catalog"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json(denied).await["retryable"], false);

    let catalog = app
        .clone()
        .oneshot(
            Request::get(import_path(target_id, source_id, "catalog"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    assert_eq!(catalog.headers()[header::ETAG], "\"2\"");
    let catalog = json(catalog).await;
    assert_eq!(catalog["source_revision"], 2);
    assert_eq!(catalog["objects"][0]["display_name"], "Source Front");

    let blocked = post_import(
        &app,
        &token,
        target_id,
        source_id,
        "preview",
        None,
        selection(None),
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::OK);
    assert_eq!(blocked.headers()[header::ETAG], "\"2\"");
    let blocked = json(blocked).await;
    assert_eq!(blocked["can_apply"], false);
    assert_eq!(blocked["conflicts"][0]["key"]["id"], "front");
    assert!(
        blocked["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|blocker| { blocker["type"] == "object_conflict" })
    );

    let ready = post_import(
        &app,
        &token,
        target_id,
        source_id,
        "preview",
        None,
        selection(Some("replace_destination")),
    )
    .await;
    assert_eq!(ready.status(), StatusCode::OK);
    let ready = json(ready).await;
    assert_eq!(ready["can_apply"], true);
    assert_eq!(ready["source_revision"], 2);
    assert_eq!(ready["target_revision"], 2);

    let applied = post_import(
        &app,
        &token,
        target_id,
        source_id,
        "apply",
        Some(2),
        serde_json::json!({
            "request_id":"replace-front",
            "expected_source_revision":2,
            "expected_target_revision":2,
            "selected_objects":[{"kind":"group","id":"front"}],
            "conflict_resolutions":[{
                "key":{"kind":"group","id":"front"},
                "resolution":"replace_destination"
            }],
            "profile_conflict_resolutions":[]
        }),
    )
    .await;
    assert_eq!(applied.status(), StatusCode::OK);
    assert_eq!(applied.headers()[header::ETAG], "\"3\"");
    let applied = json(applied).await;
    assert_eq!(applied["changed"], true);
    assert_eq!(applied["show_revision"], 3);
    assert_eq!(applied["event_sequence"], 1);
    assert_eq!(applied["objects"].as_array().unwrap().len(), 1);
    assert_eq!(
        stored_group_name(&state, target_id, "front"),
        "Source Front"
    );
    assert_eq!(state.application_events.latest_sequence(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_selective_import_rejects_stale_target_and_source_previews() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let source = create_show(&app, &token, "Stale Source").await;
    let target = create_show(&app, &token, "Stale Target").await;
    let source_id = source["id"].as_str().unwrap();
    let target_id = target["id"].as_str().unwrap();
    put_group(&state, source_id, "front", "Source Front");
    open_import_target(&app, &token, target_id).await;

    let preview = post_import(
        &app,
        &token,
        target_id,
        source_id,
        "preview",
        None,
        selection(None),
    )
    .await;
    let preview = json(preview).await;
    assert_eq!(preview["source_revision"], 2);
    assert_eq!(preview["target_revision"], 1);

    put_group(&state, source_id, "later", "Later Source Change");
    let stale_source = apply_one(&app, &token, target_id, source_id, 1, 2, "stale-source").await;
    assert_eq!(stale_source.status(), StatusCode::CONFLICT);
    assert_eq!(stale_source.headers()[header::ETAG], "\"3\"");
    let stale_source = json(stale_source).await;
    assert_eq!(stale_source["error"], "source show changed after preview");
    assert_eq!(stale_source["current_revision"], 3);
    assert!(
        ShowStore::open(&show_entry(&state, target_id).path)
            .unwrap()
            .portable_document()
            .unwrap()
            .object("group", "front")
            .is_none()
    );

    put_group(&state, target_id, "target-change", "Target Change");
    let stale_target = apply_one(&app, &token, target_id, source_id, 1, 3, "stale-target").await;
    assert_eq!(stale_target.status(), StatusCode::CONFLICT);
    assert_eq!(stale_target.headers()[header::ETAG], "\"2\"");
    let stale_target = json(stale_target).await;
    assert_eq!(stale_target["error"], "active show changed after preview");
    assert_eq!(stale_target["current_revision"], 2);
    assert_eq!(state.application_events.latest_sequence(), 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn selection(resolution: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "selected_objects":[{"kind":"group","id":"front"}],
        "conflict_resolutions":resolution.map(|resolution| vec![serde_json::json!({
            "key":{"kind":"group","id":"front"},"resolution":resolution
        })]).unwrap_or_default(),
        "profile_conflict_resolutions":[]
    })
}

async fn apply_one(
    app: &Router,
    token: &str,
    target: &str,
    source: &str,
    target_revision: u64,
    source_revision: u64,
    request_id: &str,
) -> Response {
    post_import(
        app,
        token,
        target,
        source,
        "apply",
        Some(target_revision),
        serde_json::json!({
            "request_id":request_id,
            "expected_source_revision":source_revision,
            "expected_target_revision":target_revision,
            "selected_objects":[{"kind":"group","id":"front"}],
            "conflict_resolutions":[],
            "profile_conflict_resolutions":[]
        }),
    )
    .await
}

async fn post_import(
    app: &Router,
    token: &str,
    target: &str,
    source: &str,
    operation: &str,
    revision: Option<u64>,
    body: serde_json::Value,
) -> Response {
    let mut request = Request::post(import_path(target, source, operation))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(revision) = revision {
        request = request.header(header::IF_MATCH, revision);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

fn import_path(target: &str, source: &str, operation: &str) -> String {
    format!("/api/v2/shows/{target}/selective-imports/{source}/{operation}")
}

async fn open_import_target(app: &Router, token: &str, target: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{target}/open"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

fn put_group(state: &AppState, show_id: &str, group_id: &str, name: &str) {
    let entry = show_entry(state, show_id);
    let group = light_programmer::GroupDefinition {
        id: group_id.into(),
        name: name.into(),
        ..Default::default()
    };
    ShowStore::open(entry.path)
        .unwrap()
        .put_object("group", group_id, &serde_json::to_value(group).unwrap(), 0)
        .unwrap();
}

fn stored_group_name(state: &AppState, show_id: &str, group_id: &str) -> String {
    let document = ShowStore::open(show_entry(state, show_id).path)
        .unwrap()
        .portable_document()
        .unwrap();
    document.object("group", group_id).unwrap().body()["name"]
        .as_str()
        .unwrap()
        .into()
}

fn show_entry(state: &AppState, show_id: &str) -> ShowEntry {
    state
        .desk
        .lock()
        .show(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
        .unwrap()
        .unwrap()
}
