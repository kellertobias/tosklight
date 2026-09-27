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
async fn architect_document_save_preserves_identity_and_refuses_newer_desk_work() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let (_, created) = post_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"architect-source", "action":{"type":"create","name":"Architect source"}
        }),
    )
    .await;
    let id = created["result"]["show"]["id"].as_str().unwrap();
    let (status, opened) = post_action(&app, &token, serde_json::json!({
        "request_id":"architect-open", "action":{"type":"open","show_id":id,"transition":"hold_current"}
    })).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let (status, saved) = post_action(&app, &token, serde_json::json!({
        "request_id":"architect-revision", "action":{"type":"save_revision","show_id":id,"name":"Before Architect"}
    })).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let entry = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(id).unwrap()))
        .unwrap()
        .unwrap();
    let store = ShowStore::open(&entry.path).unwrap();
    let expected = store.portable_revision().unwrap().value();
    let staged = data_dir.join("architect-edited.show");
    store.backup_to(&staged).unwrap();
    drop(store);
    ShowStore::open(&staged)
        .unwrap()
        .set_metadata_values(&[("architect.venue", "Edited venue")])
        .unwrap();
    ShowStore::open(&staged)
        .unwrap()
        .put_object(
            "group",
            "42",
            &serde_json::to_value(light_programmer::GroupDefinition {
                id: "42".into(),
                name: "Architect stored empty group".into(),
                ..Default::default()
            })
            .unwrap(),
            0,
        )
        .unwrap();
    let request = serde_json::json!({"request_id":"architect-save", "action":{
        "type":"update_document","destination_show_id":id,"expected_revision":expected,
        "data_base64":base64::engine::general_purpose::STANDARD.encode(std::fs::read(&staged).unwrap())
    }});
    let (status, first) = post_action(&app, &token, request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["result"]["show"]["id"], id);
    assert_eq!(first["result"]["show"]["name"], "Architect source");
    assert!(
        state
            .output
            .snapshot()
            .groups
            .iter()
            .any(|group| group.id == "42" && group.name == "Architect stored empty group")
    );
    assert_eq!(state.active_show.current().as_ref().unwrap().id, entry.id);
    assert_eq!(
        ShowStore::open(&entry.path)
            .unwrap()
            .metadata_value("architect.venue")
            .unwrap()
            .as_deref(),
        Some("Edited venue")
    );
    assert!(first["result"]["document_revision"].as_u64().unwrap() > expected);
    let (status, replay) = post_action(&app, &token, request.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["replayed"], true);
    assert_eq!(
        state.installation.show_revisions(entry.id).unwrap().len(),
        1
    );
    let guard = first["result"]["document_revision"].as_u64().unwrap();
    let (status, _) = post_action(&app, &token, serde_json::json!({"request_id":"invalid-upload", "action":{
        "type":"update_document","destination_show_id":id,"expected_revision":guard,"data_base64":"broken"
    }})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        ShowStore::open(&entry.path)
            .unwrap()
            .portable_revision()
            .unwrap()
            .value(),
        guard
    );
    let (status, _) = post_action(&app, &token, serde_json::json!({"request_id":"missing-destination", "action":{
        "type":"update_document","destination_show_id":Uuid::new_v4(),"expected_revision":0,"data_base64":""
    }})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mut stale = request;
    stale["request_id"] = serde_json::json!("architect-stale-save");
    let store = ShowStore::open(&entry.path).unwrap();
    store
        .set_metadata_values(&[("architect.venue", "Newer desk edit")])
        .unwrap();
    let (status, _) = post_action(&app, &token, stale).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn architect_document_save_refuses_a_busy_wal_reader_without_replacing_the_show() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let (_, created) = post_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"wal-source", "action":{"type":"create","name":"WAL source"}
        }),
    )
    .await;
    let id = created["result"]["show"]["id"].as_str().unwrap();
    let entry = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(id).unwrap()))
        .unwrap()
        .unwrap();
    let staged = data_dir.join("wal-edited.show");
    let store = ShowStore::open(&entry.path).unwrap();
    store.backup_to(&staged).unwrap();
    let reader = rusqlite::Connection::open(&entry.path).unwrap();
    reader
        .execute_batch("BEGIN; SELECT * FROM metadata")
        .unwrap();
    store
        .set_metadata_values(&[("architect.venue", "Protected desk work")])
        .unwrap();
    let expected = store.portable_revision().unwrap().value();
    drop(store);
    let (status, outcome) = post_action(&app, &token, serde_json::json!({"request_id":"wal-save", "action":{
        "type":"update_document","destination_show_id":id,"expected_revision":expected,
        "data_base64":base64::engine::general_purpose::STANDARD.encode(std::fs::read(&staged).unwrap())
    }})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{outcome}");
    reader.execute_batch("ROLLBACK").unwrap();
    drop(reader);
    assert_eq!(
        ShowStore::open(&entry.path)
            .unwrap()
            .metadata_value("architect.venue")
            .unwrap()
            .as_deref(),
        Some("Protected desk work")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn base_show_starts_an_independent_copy_and_preserves_source_history() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let (_, created) = post_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"base-create", "action":{"type":"create","name":"Tour Base"}
        }),
    )
    .await;
    let id = created["result"]["show"]["id"].as_str().unwrap();
    let (_, _) = post_action(&app, &token, serde_json::json!({
        "request_id":"base-history", "action":{"type":"save_revision","show_id":id,"name":"Approved Base"}
    })).await;
    let mark = serde_json::json!({"request_id":"base-mark","action":{"type":"set_base_show","show_id":id,"is_base_show":true}});
    let (status, marked) = post_action(&app, &token, mark.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(marked["result"]["show"]["is_base_show"], true);
    let (_, replay) = post_action(&app, &token, mark).await;
    assert_eq!(replay["replayed"], true);
    let (status, copy) = post_action(&app, &token, serde_json::json!({
        "request_id":"base-copy", "action":{"type":"create_from_base","show_id":id,"name":"Friday"}
    })).await;
    assert_eq!(status, StatusCode::OK, "{copy}");
    let copy_id = copy["result"]["show"]["id"].as_str().unwrap();
    assert_ne!(copy_id, id);
    assert_eq!(copy["result"]["show"]["is_base_show"], false);
    assert!(copy["result"]["show"]["revision_copy"].is_null());
    let source = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(id).unwrap()))
        .unwrap()
        .unwrap();
    let dest = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(copy_id).unwrap()))
        .unwrap()
        .unwrap();
    ShowStore::open(&dest.path)
        .unwrap()
        .set_metadata_values(&[("architect.venue", "Friday venue")])
        .unwrap();
    assert_ne!(
        ShowStore::open(&source.path)
            .unwrap()
            .metadata_value("architect.venue")
            .unwrap(),
        Some("Friday venue".into())
    );
    assert_eq!(
        state.installation.show_revisions(source.id).unwrap().len(),
        1
    );
    assert!(
        state
            .installation
            .show_revisions(dest.id)
            .unwrap()
            .is_empty()
    );
    let (status, _) = post_action(&app, &token, serde_json::json!({
        "request_id":"not-base-copy", "action":{"type":"create_from_base","show_id":copy_id,"name":"Wrong"}
    })).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn network_import_name_collisions_create_a_new_copy_without_changing_existing_shows() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut originals = Vec::new();
    for (index, name) in ["Tour", "tOuR 2", "TOUR 3"].into_iter().enumerate() {
        let (status, created) = post_action(
            &app,
            &token,
            serde_json::json!({
                "request_id":format!("collision-original-{index}"),
                "action":{"type":"create","name":name}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{created}");
        let id = light_core::ShowId(
            Uuid::parse_str(created["result"]["show"]["id"].as_str().unwrap()).unwrap(),
        );
        originals.push(state.installation.show(id).unwrap().unwrap());
    }
    let selected = super::super::show_library_v2::unique_import_name(&state, "Tour").unwrap();
    assert_eq!(selected, "Tour 4");
    let snapshot = data_dir.join("network-import-collision.show");
    ShowStore::open(&originals[0].path)
        .unwrap()
        .backup_to(&snapshot)
        .unwrap();
    let (status, imported) = post_action(&app, &token, serde_json::json!({
        "request_id":"collision-network-copy",
        "action":{"type":"create","name":selected,"data_base64":
            base64::engine::general_purpose::STANDARD.encode(std::fs::read(&snapshot).unwrap()),"overwrite":false}
    })).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["result"]["show"]["name"], "Tour 4");
    let imported_id = imported["result"]["show"]["id"].as_str().unwrap();
    for original in originals {
        assert_ne!(original.id.0.to_string(), imported_id);
        let current = state.installation.show(original.id).unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(&current).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        assert_eq!(
            ShowStore::open(&current.path).unwrap().id().unwrap(),
            original.id
        );
    }
    let imported_path = imported["result"]["show"]["path"].as_str().unwrap();
    assert_eq!(
        ShowStore::open(imported_path)
            .unwrap()
            .id()
            .unwrap()
            .0
            .to_string(),
        imported_id
    );
    assert_eq!(
        super::super::show_library_v2::unique_import_name(&state, "Tour").unwrap(),
        "Tour 5"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
