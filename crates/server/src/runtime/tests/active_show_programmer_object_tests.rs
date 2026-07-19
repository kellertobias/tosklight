use super::*;

#[tokio::test]
async fn active_group_and_preset_puts_install_the_exact_committed_candidate() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Programmer object boundary").await;
    let show_id = show["id"].as_str().unwrap();
    let show_uuid = Uuid::parse_str(show_id).unwrap();
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(show_uuid))
        .unwrap()
        .unwrap();
    let first = light_core::FixtureId(Uuid::from_u128(101));
    let second = light_core::FixtureId(Uuid::from_u128(102));
    ShowStore::open(&entry.path)
        .unwrap()
        .put_object(
            "group",
            "7",
            &serde_json::json!({
                "id":"old",
                "name":"Before",
                "fixtures":[first],
                "future_server_field":{"retained":true}
            }),
            0,
        )
        .unwrap();
    open_show_for_test(&app, &token, show_id).await;
    let initial_group_revision = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap()
        .object("group", "7")
        .unwrap()
        .revision();

    let updated = put_active_object(
        &app,
        &token,
        show_id,
        "group",
        "7",
        initial_group_revision,
        serde_json::json!({
            "name":"Ordered",
            "fixtures":[second,first],
            "future_client_field":"accepted"
        }),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    assert_eq!(
        updated.headers()[header::ETAG],
        format!("\"{}\"", initial_group_revision + 1)
    );

    let empty = put_active_object(
        &app,
        &token,
        show_id,
        "group",
        "8",
        0,
        serde_json::json!({"name":"Stored empty","fixtures":[]}),
    )
    .await;
    assert_eq!(empty.status(), StatusCode::OK);

    let preset = put_active_object(
        &app,
        &token,
        show_id,
        "preset",
        "2.3",
        0,
        serde_json::json!({
            "name":"Color three",
            "family":"Color",
            "number":3,
            "values":{},
            "group_values":{},
            "future_preset_field":42
        }),
    )
    .await;
    assert_eq!(preset.status(), StatusCode::OK);

    let stored_preset = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/presets/2.3/store"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(
                    serde_json::json!({
                        "mode":"merge",
                        "preset":{
                            "name":"Merged color three",
                            "family":"Color",
                            "number":3,
                            "values":{},
                            "group_values":{},
                            "future_store_field":"accepted"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stored_preset.status(), StatusCode::OK);
    assert_eq!(stored_preset.headers()[header::ETAG], "\"2\"");

    let document = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    let stored = document.object("group", "7").unwrap();
    assert_eq!(stored.revision(), initial_group_revision + 1);
    assert_eq!(stored.body()["id"], "7");
    assert_eq!(
        stored.body()["fixtures"],
        serde_json::json!([second, first])
    );
    assert_eq!(
        stored.body()["future_server_field"],
        serde_json::json!({"retained":true})
    );
    assert_eq!(stored.body()["future_client_field"], "accepted");
    assert_eq!(
        document.object("group", "8").unwrap().body()["fixtures"],
        serde_json::json!([])
    );
    let stored_preset = document.object("preset", "2.3").unwrap();
    assert_eq!(stored_preset.revision(), 2);
    assert_eq!(stored_preset.body()["future_preset_field"], 42);
    assert_eq!(stored_preset.body()["future_store_field"], "accepted");
    let snapshot = state.engine.snapshot();
    assert_eq!(snapshot.revision, document.revision().value());
    assert_eq!(
        snapshot
            .groups
            .iter()
            .find(|group| group.id == "7")
            .unwrap()
            .fixtures,
        vec![second, first]
    );
    assert!(
        snapshot
            .groups
            .iter()
            .any(|group| group.id == "8" && group.fixtures.is_empty())
    );

    let revision_before_failures = document.revision();
    let invalid = put_active_object(
        &app,
        &token,
        show_id,
        "preset",
        "2.4",
        0,
        serde_json::json!({"name":"Wrong","family":"Position","number":4}),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let stale = put_active_object(
        &app,
        &token,
        show_id,
        "group",
        "7",
        initial_group_revision,
        serde_json::json!({"name":"Stale","fixtures":[]}),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let after_failures = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(after_failures.revision(), revision_before_failures);
    assert_eq!(
        state.engine.snapshot().revision,
        revision_before_failures.value()
    );
    assert!(after_failures.object("preset", "2.4").is_none());

    let object_backups = std::fs::read_dir(data_dir.join("backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.contains("-show-object-"))
        })
        .count();
    assert_eq!(object_backups, 4);
    let audit = state.audit_events.lock();
    let changed = audit
        .iter()
        .filter(|event| event.kind == "show_object_changed")
        .count();
    assert_eq!(changed, 3);
    assert_eq!(
        audit
            .iter()
            .filter(|event| event.kind == "preset_stored")
            .count(),
        1
    );
    drop(audit);
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn open_show_for_test(app: &Router, token: &str, show_id: &str) {
    let response = app
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
    assert_eq!(response.status(), StatusCode::OK);
}

async fn put_active_object(
    app: &Router,
    token: &str,
    show_id: &str,
    kind: &str,
    object_id: &str,
    revision: u64,
    body: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::put(format!(
                "/api/v1/shows/{show_id}/objects/{kind}/{object_id}"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_MATCH, revision.to_string())
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap()
}
