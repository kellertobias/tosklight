use super::*;

async fn get_configuration(
    app: &Router,
    token: &str,
    show_id: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/attribute-configuration")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn update_configuration(
    app: &Router,
    token: &str,
    show_id: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/attribute-configuration/update")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or_else(
        |_| serde_json::json!({"raw": String::from_utf8_lossy(&bytes).into_owned()}),
    );
    (status, body)
}

#[tokio::test]
async fn attribute_configuration_defaults_persist_and_replay_without_eager_show_mutation() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Attribute configuration").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);

    let (status, initial) = get_configuration(&app, &token, &show_id).await;
    assert_eq!(status, StatusCode::OK, "{initial}");
    assert_eq!(initial["object_revision"], 0);
    assert!(initial["validation_error"].is_null());
    assert_eq!(initial["configuration"]["version"], 1);
    assert_eq!(
        initial["descriptors"].as_array().unwrap().len(),
        light_core::ATTRIBUTE_REGISTRY.len()
    );
    let prism_rotation = initial["descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|descriptor| descriptor["id"] == "prism.1.rotation")
        .unwrap();
    assert_eq!(prism_rotation["push_turn_of"], "prism.1");

    let request = serde_json::json!({
        "request_id": "attribute-configuration-1",
        "expected_show_revision": initial["show_revision"],
        "expected_object_revision": 0,
        "patch": {
            "activation_groups": initial["configuration"]["activation_groups"],
        },
    });
    let (status, saved) = update_configuration(&app, &token, &show_id, request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["replayed"], false);
    assert_eq!(saved["snapshot"]["object_revision"], 1);
    assert_eq!(saved["snapshot"]["configuration"], initial["configuration"]);

    let (status, replayed) = update_configuration(&app, &token, &show_id, request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["snapshot"], saved["snapshot"]);

    let mut reused = request;
    reused["expected_object_revision"] = serde_json::json!(1);
    let (status, conflict) = update_configuration(&app, &token, &show_id, reused).await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");

    let (status, stale) = update_configuration(
        &app,
        &token,
        &show_id,
        serde_json::json!({
            "request_id": "attribute-configuration-stale",
            "expected_show_revision": initial["show_revision"],
            "expected_object_revision": 1,
            "patch": {
                "activation_groups": saved["snapshot"]["configuration"]["activation_groups"],
            },
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");

    let mut cross_group = saved["snapshot"]["configuration"]["activation_groups"].clone();
    cross_group[0]["members"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("pan"));
    let (status, invalid) = update_configuration(
        &app,
        &token,
        &show_id,
        serde_json::json!({
            "request_id": "attribute-configuration-cross-group",
            "expected_show_revision": saved["snapshot"]["show_revision"],
            "expected_object_revision": 1,
            "patch": {"activation_groups": cross_group},
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");

    let entry = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(&show_id).unwrap()))
        .unwrap()
        .unwrap();
    let persisted = ShowStore::open(entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert!(
        persisted
            .object(
                attribute_configuration::ATTRIBUTE_CONFIGURATION_KIND,
                attribute_configuration::ATTRIBUTE_CONFIGURATION_ID,
            )
            .is_some()
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn saved_configuration_from_an_older_catalog_is_upgraded_without_rewriting_the_show() {
    let (_state, data_dir) = test_state();
    let show_path = data_dir.join("legacy-attribute-catalog.show");
    let (store, _) = ShowStore::create(&show_path, "Legacy Attribute catalog").unwrap();
    let mut legacy = light_core::AttributeConfiguration::recommended();
    for placement in &mut legacy.placements {
        placement.push_turn_of = None;
    }
    legacy
        .placements
        .retain(|placement| !placement.attribute.0.starts_with("media."));
    for group in &mut legacy.activation_groups {
        group
            .members
            .retain(|member| !member.0.starts_with("media."));
    }
    legacy
        .activation_groups
        .retain(|group| !group.members.is_empty());
    store
        .put_object(
            attribute_configuration::ATTRIBUTE_CONFIGURATION_KIND,
            attribute_configuration::ATTRIBUTE_CONFIGURATION_ID,
            &serde_json::to_value(&legacy).unwrap(),
            0,
        )
        .unwrap();
    let document = store.portable_document().unwrap();

    let installed =
        attribute_configuration::InstalledAttributeConfiguration::for_document(&document);
    assert_eq!(installed.validation_error, None);
    installed.configuration.validate().unwrap();
    assert_eq!(
        installed
            .configuration
            .placement_for(&light_core::AttributeKey("media.folder".into())),
        Some(light_core::EncoderPlacement::new(
            light_core::EncoderGroup::Media,
            1,
            1
        ))
    );
    assert_eq!(
        installed
            .configuration
            .attribute_placement_for(&light_core::AttributeKey("prism.1.rotation".into()))
            .and_then(|placement| placement.push_turn_of.as_ref()),
        Some(&light_core::AttributeKey("prism.1".into()))
    );
    assert!(
        document
            .object(
                attribute_configuration::ATTRIBUTE_CONFIGURATION_KIND,
                attribute_configuration::ATTRIBUTE_CONFIGURATION_ID,
            )
            .unwrap()
            .body()["placements"]
            .as_array()
            .unwrap()
            .iter()
            .all(|placement| placement.get("push_turn_of").is_none()),
        "runtime compatibility must not eagerly persist new compound metadata"
    );
    assert!(
        document
            .object(
                attribute_configuration::ATTRIBUTE_CONFIGURATION_KIND,
                attribute_configuration::ATTRIBUTE_CONFIGURATION_ID,
            )
            .unwrap()
            .body()["placements"]
            .as_array()
            .unwrap()
            .len()
            < installed.configuration.placements.len(),
        "runtime compatibility must not eagerly rewrite the portable show object"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
