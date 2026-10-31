//! Offline copies and merging them back into the open show (TL-858).
use super::*;
use light_show::ShowStore;

async fn library_action(
    app: &Router,
    token: &str,
    action: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/shows")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": Uuid::new_v4().to_string(), "action": action})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), json(response).await)
}

async fn merge(
    app: &Router,
    token: &str,
    target: &str,
    copy: &str,
    operation: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v2/show-merges/{copy}/{operation}"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", target)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), json(response).await)
}

async fn open(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

fn store(state: &AppState, show_id: &str) -> ShowStore {
    let entry = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(show_id).unwrap()))
        .unwrap()
        .unwrap();
    ShowStore::open(entry.path).unwrap()
}

/// Rewrites a stored Group, keeping its identity and object revision chain.
fn edit_group(store: &ShowStore, id: &str, name: &str) {
    let current = store.object("group", id).unwrap();
    let (mut body, revision) = current
        .map(|object| (object.body, object.revision))
        .unwrap_or_else(|| (serde_json::json!({"id": id, "fixtures": []}), 0));
    body["name"] = serde_json::json!(name);
    store.put_object("group", id, &body, revision).unwrap();
}

fn group_name(store: &ShowStore, id: &str) -> Option<String> {
    store
        .object("group", id)
        .unwrap()
        .and_then(|object| object.body["name"].as_str().map(str::to_owned))
}

#[tokio::test]
async fn an_offline_copy_merges_back_with_conflicts_decided_and_a_number_collision_kept() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let main = create_show(&app, &token, "Main").await;
    let main_id = main["id"].as_str().unwrap().to_owned();
    let other = create_show(&app, &token, "Other").await;
    let other_id = other["id"].as_str().unwrap().to_owned();
    edit_group(&store(&state, &main_id), "1", "Front");
    open(&app, &token, &main_id).await;

    std::fs::create_dir_all(data_dir.join("shows")).unwrap();
    let (status, taken) = library_action(
        &app,
        &token,
        serde_json::json!({"type": "take_offline_copy", "name": "Programmer B", "root_id": "shows", "path": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{taken}");
    let copy_id = taken["result"]["show"]["id"].as_str().unwrap().to_owned();
    let copy = store(&state, &copy_id);
    let lineage = copy
        .merge_lineage()
        .unwrap()
        .expect("the copy records its base");
    assert_eq!(lineage.source_show_id.0.to_string(), main_id);
    // The open show and its copy agree on the Group's identity.
    let main_uid = store(&state, &main_id)
        .object("group", "1")
        .unwrap()
        .unwrap()
        .body["uid"]
        .clone();
    assert!(main_uid.is_string());
    assert_eq!(
        copy.object("group", "1").unwrap().unwrap().body["uid"],
        main_uid
    );
    let revisions = state
        .installation
        .show_revisions(light_core::ShowId(Uuid::parse_str(&main_id).unwrap()))
        .unwrap();
    assert!(
        revisions
            .iter()
            .any(|revision| revision.name == "Offline copy: Programmer B")
    );

    // Both sides program while apart.
    open(&app, &token, &other_id).await;
    let main_store = store(&state, &main_id);
    edit_group(&main_store, "1", "Front (desk)");
    edit_group(&main_store, "7", "Desk seven");
    edit_group(&copy, "1", "Front (copy)");
    edit_group(&copy, "7", "Copy seven");
    edit_group(&copy, "2", "Back");
    drop((main_store, copy));
    open(&app, &token, &main_id).await;

    let (status, refused) = merge(
        &app,
        &token,
        &main_id,
        &other_id,
        "preview",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(refused.to_string().contains("Partial Load"), "{refused}");

    let (status, preview) = merge(
        &app,
        &token,
        &main_id,
        &copy_id,
        "preview",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["lineage"]["source_show_name"], "Main");
    let conflicts = preview["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1, "{preview}");
    assert_eq!(conflicts[0]["path"], serde_json::json!(["name"]));
    assert_eq!(conflicts[0]["mine"], "Front (desk)");
    assert_eq!(conflicts[0]["theirs"], "Front (copy)");
    assert_eq!(
        preview["renumbers"],
        serde_json::json!([{"kind": "group", "from": "7", "to": "8", "name": "Copy seven"}])
    );
    let revision = preview["target_revision"].as_u64().unwrap();

    let (status, undecided) = merge(
        &app,
        &token,
        &main_id,
        &copy_id,
        "apply",
        serde_json::json!({"request_id": "merge-undecided", "expected_target_revision": revision}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{undecided}");

    let resolutions = serde_json::json!([{"conflict_id": conflicts[0]["id"], "choice": "theirs"}]);
    let (status, applied) = merge(
        &app,
        &token,
        &main_id,
        &copy_id,
        "apply",
        serde_json::json!({"request_id": "merge-apply", "expected_target_revision": revision, "resolutions": resolutions}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(applied["changed"], true);
    let merged = store(&state, &main_id);
    assert_eq!(group_name(&merged, "1").as_deref(), Some("Front (copy)"));
    assert_eq!(group_name(&merged, "2").as_deref(), Some("Back"));
    assert_eq!(group_name(&merged, "7").as_deref(), Some("Desk seven"));
    assert_eq!(group_name(&merged, "8").as_deref(), Some("Copy seven"));
    let revisions = state
        .installation
        .show_revisions(light_core::ShowId(Uuid::parse_str(&main_id).unwrap()))
        .unwrap();
    let safety = revisions
        .iter()
        .find(|revision| revision.name == "Before merging Programmer B")
        .expect("a revision is saved before the merge");
    assert_eq!(applied["safety_revision"], safety.revision);
    // The safety revision holds the show as it was.
    assert_eq!(
        group_name(&ShowStore::open(&safety.path).unwrap(), "1").as_deref(),
        Some("Front (desk)")
    );
}
