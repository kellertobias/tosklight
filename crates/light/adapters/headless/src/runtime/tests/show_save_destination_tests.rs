use super::*;

async fn destination_action(
    app: &Router,
    token: &str,
    request_id: &str,
    action: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/shows")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id":request_id,"action":action}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), json(response).await)
}

#[tokio::test]
async fn folder_save_creates_independent_snapshot_and_replays_without_touching_source_history() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let source = create_show(&app, &token, "Folder source").await;
    let id = source["id"].as_str().unwrap();
    let (status, _) = destination_action(
        &app,
        &token,
        "folder-revision",
        serde_json::json!({"type":"save_revision","show_id":id,"name":"Protected"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let before = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(id).unwrap()))
        .unwrap()
        .unwrap();
    let active_before = state.active_show.current();
    std::fs::create_dir_all(data_dir.join("shows/nested")).unwrap();
    let action = serde_json::json!({"type":"save_copy","source_show_id":id,"name":"Folder source","root_id":"shows","path":"nested","is_base_show":true});
    let (status, saved) = destination_action(&app, &token, "folder-save", action.clone()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let copy = &saved["result"]["show"];
    assert_ne!(copy["id"], id);
    assert_eq!(copy["name"], "Folder source 2");
    assert_eq!(copy["is_base_show"], true);
    assert_eq!(
        copy["path"],
        std::fs::canonicalize(data_dir.join("shows/nested/Folder source.show"))
            .unwrap()
            .display()
            .to_string()
    );
    let store = ShowStore::open(copy["path"].as_str().unwrap()).unwrap();
    assert_eq!(
        store.id().unwrap().0.to_string(),
        copy["id"].as_str().unwrap()
    );
    assert!(store.revision_copy_source().unwrap().is_none());
    drop(store);
    let (status, replay) = destination_action(&app, &token, "folder-save", action.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"]["show"]["id"], copy["id"]);
    let (status, _) = destination_action(&app, &token, "folder-collision", action).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        serde_json::to_value(state.installation.show(before.id).unwrap().unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    assert_eq!(
        serde_json::to_value(state.active_show.current()).unwrap(),
        serde_json::to_value(active_before).unwrap()
    );
    assert_eq!(
        state
            .installation
            .show_revisions(light_core::ShowId(Uuid::parse_str(id).unwrap()))
            .unwrap()
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn folder_save_rejects_escape_missing_root_invalid_bytes_and_preserves_catalog() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let source = create_show(&app, &token, "Confined source").await;
    let before = state.installation.show_library().unwrap().len();
    for (index, (root, path)) in [
        ("shows", "../outside"),
        ("missing", ""),
        ("shows", "/absolute"),
    ]
    .into_iter()
    .enumerate()
    {
        let (status, _) = destination_action(&app, &token, &format!("reject-path-{index}"), serde_json::json!({"type":"save_copy","source_show_id":source["id"],"name":"Copy","root_id":root,"path":path})).await;
        assert!(status.is_client_error());
    }
    let (status, _) = destination_action(&app, &token, "reject-document", serde_json::json!({"type":"save_copy","data_base64":STANDARD.encode(b"not a show"),"name":"Copy","root_id":"shows","path":""})).await;
    assert!(status.is_client_error());
    assert_eq!(state.installation.show_library().unwrap().len(), before);
    assert!(!data_dir.join("shows/Copy.show").exists());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[tokio::test]
async fn folder_save_rejects_symlink_escape_and_readonly_destination() {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let source = create_show(&app, &token, "Readonly source").await;
    std::fs::create_dir_all(data_dir.join("outside")).unwrap();
    symlink(data_dir.join("outside"), data_dir.join("shows/link")).unwrap();
    std::fs::create_dir_all(data_dir.join("shows/locked")).unwrap();
    std::fs::set_permissions(
        data_dir.join("shows/locked"),
        std::fs::Permissions::from_mode(0o555),
    )
    .unwrap();
    for (index, path) in ["link", "locked"].into_iter().enumerate() {
        let (status, _) = destination_action(&app, &token, &format!("reject-confined-{index}"), serde_json::json!({"type":"save_copy","source_show_id":source["id"],"name":"Copy","root_id":"shows","path":path})).await;
        assert!(status.is_client_error());
    }
    std::fs::set_permissions(
        data_dir.join("shows/locked"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert!(!data_dir.join("outside/Copy.show").exists());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mvr_export_writes_the_selected_folder_and_never_overwrites_an_existing_file() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let source = create_show(&app, &token, "MVR source").await;
    std::fs::create_dir_all(data_dir.join("shows/export")).unwrap();
    let action = serde_json::json!({"type":"export_mvr_file","show_id":source["id"],"name":"Rig","root_id":"shows","path":"export"});
    let (status, saved) =
        destination_action(&app, &token, "mvr-folder-export", action.clone()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["result"]["type"], "file_saved");
    assert_eq!(saved["result"]["path"], "export/Rig.mvr");
    let bytes = std::fs::read(data_dir.join("shows/export/Rig.mvr")).unwrap();
    assert!(bytes.starts_with(b"PK"));
    let (status, _) = destination_action(&app, &token, "mvr-folder-collision", action).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        std::fs::read(data_dir.join("shows/export/Rig.mvr")).unwrap(),
        bytes
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
