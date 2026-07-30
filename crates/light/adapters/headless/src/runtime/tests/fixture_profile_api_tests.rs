#[tokio::test]
async fn fixture_profile_api_rejects_invalid_discrete_wheel_before_storing_revision() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let (fixture, _, channel_ids) = schema_v2_direct_fixture();
    let mut profile = *fixture.definition.profile_snapshot.unwrap();
    let profile_id = profile.id;
    let head_id = profile.modes[0].heads[0].id;
    profile.modes[0].color_systems = vec![light_fixture::HeadColorSystem {
        head_id,
        correction_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        system: light_fixture::ColorSystem::DiscreteWheel {
            channel_id: channel_ids[0],
            slots: vec![
                light_fixture::ColorWheelSlot {
                    semantic_id: "red".into(),
                    label: "Red".into(),
                    dmx_from: 0,
                    dmx_to: 100,
                    measured_xyz: None,
                },
                light_fixture::ColorWheelSlot {
                    semantic_id: "blue".into(),
                    label: "Blue".into(),
                    dmx_from: 100,
                    dmx_to: 120,
                    measured_xyz: None,
                },
            ],
        },
    }];

    let response = app
        .oneshot(
            Request::post("/api/v2/fixture-library")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "invalid-wheel",
                        "action": {
                            "type": "save_profile",
                            "profile": profile,
                            "expected_revision": 0
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        state
            .installation
            .fixture_profile(profile_id, 1)
            .unwrap()
            .is_none()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn new_fixture_import_pauses_until_unknown_canonical_id_is_configured() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let (fixture, _, _) = schema_v2_direct_fixture();
    let mut profile = *fixture.definition.profile_snapshot.unwrap();
    let unknown = light_core::AttributeKey("vendor.test.feature".into());
    profile.modes[0].channels[0].fixture_attribute = unknown.clone();
    profile.modes[0].channels[0].attribute = unknown.clone();
    for function in &mut profile.modes[0].channels[0].functions {
        function.attribute = unknown.clone();
    }
    profile.modes[0].channels[1].attribute = light_core::AttributeKey("control".into());
    let new_profile = app
        .clone()
        .oneshot(
            Request::post("/api/v2/fixture-library")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "unknown-new-profile",
                        "action": {
                            "type": "save_profile",
                            "profile": profile.clone(),
                            "expected_revision": 0
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(new_profile.status(), StatusCode::BAD_REQUEST);

    let package = light_fixture::write_fixture_package(&profile).unwrap();
    let request = |request_id: &str, attribute_mappings: serde_json::Value| {
        Request::post("/api/v2/fixture-library")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "request_id": request_id,
                    "action": {
                        "type": "import_package",
                        "package_base64": STANDARD.encode(&package),
                        "attribute_mappings": attribute_mappings
                    }
                })
                .to_string(),
            ))
            .unwrap()
    };

    let paused = app
        .clone()
        .oneshot(request("unknown-package", serde_json::json!([])))
        .await
        .unwrap();
    assert_eq!(paused.status(), StatusCode::OK);
    let paused = json(paused).await;
    assert_eq!(paused["result"]["type"], "import_required");
    assert_eq!(
        paused["result"]["unknown_attributes"],
        serde_json::json!([{
            "attribute": "vendor.test.feature",
            "value_type": "indexed"
        }])
    );

    let mapped = app
        .clone()
        .oneshot(request(
            "mapped-package",
            serde_json::json!([{
                "source_attribute": "vendor.test.feature",
                "target_attribute": "shutter"
            }]),
        ))
        .await
        .unwrap();
    assert_eq!(mapped.status(), StatusCode::OK);
    let mapped = json(mapped).await;
    assert_eq!(mapped["result"]["type"], "profile");
    let mapped_profile = state
        .installation
        .fixture_profile(profile.id, mapped["result"]["revision"].as_u64().unwrap() as u32)
        .unwrap()
        .unwrap();
    assert_eq!(
        mapped_profile.modes[0].channels[0].fixture_attribute,
        light_core::AttributeKey("vendor.test.feature".into())
    );
    assert_eq!(
        mapped_profile.modes[0].channels[0].attribute,
        light_core::AttributeKey("shutter".into())
    );

    {
        let mut installed = state.attributes.snapshot();
        installed.configuration.custom_attributes.push(
            light_core::CustomAttributeDescriptor {
                id: unknown.clone(),
                label: "Vendor feature".into(),
                value_type: light_core::AttributeValueType::Indexed,
                display_unit: None,
                physical_unit: None,
                normalized_bounds: None,
                domain_bounds: None,
                cyclic: false,
                recordable: true,
                lifecycle: light_core::CustomAttributeLifecycle::Active,
            },
        );
        installed
            .configuration
            .placements
            .push(light_core::AttributePlacement {
                attribute: unknown.clone(),
                encoder: light_core::EncoderPlacement::new(
                    light_core::EncoderGroup::Beam,
                    99,
                    1,
                ),
            });
        installed.configuration.activation_groups.push(
            light_core::AttributeActivationGroup {
                id: "vendor-test-feature".into(),
                label: "Vendor feature".into(),
                members: vec![unknown],
            },
        );
        installed.configuration.validate().unwrap();
        state.attributes.replace_installed(installed);
    }
    let imported = app
        .oneshot(request("configured-package", serde_json::json!([])))
        .await
        .unwrap();
    let imported_status = imported.status();
    let imported_body = String::from_utf8(
        imported
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(imported_status, StatusCode::OK, "{imported_body}");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn inactive_show_rejects_invalid_schema_v2_patch_before_persistence() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Inactive patch preflight").await;
    let show_id = light_core::ShowId(Uuid::parse_str(show["id"].as_str().unwrap()).unwrap());
    let entry = state.installation.show(show_id).unwrap().unwrap();
    assert!(state.active_show.current().is_none());

    let (fixture, _, _) = schema_v2_direct_fixture();
    let object_id = fixture.fixture_id.0.to_string();
    let mut inconsistent_identity = fixture.clone();
    inconsistent_identity.definition.profile_id = Some(light_core::FixtureId::new());

    let mut unknown_split = fixture.clone();
    unknown_split.split_patches = vec![light_fixture::SplitPatch {
        split: 99,
        universe: Some(1),
        address: Some(1),
    }];

    let mut overlapping_multipatch = fixture;
    overlapping_multipatch.split_patches = vec![light_fixture::SplitPatch {
        split: 1,
        universe: Some(1),
        address: Some(1),
    }];
    overlapping_multipatch.multipatch = vec![light_fixture::MultiPatchInstance {
        id: Uuid::new_v4(),
        name: "Overlapping instance".into(),
        universe: None,
        address: None,
        split_patches: vec![light_fixture::SplitPatch {
            split: 1,
            universe: Some(1),
            address: Some(2),
        }],
        location: Default::default(),
        rotation: Default::default(),
        invert_pan: false,
        invert_tilt: false,
    }];

    for invalid in [inconsistent_identity, unknown_split, overlapping_multipatch] {
        let response = put_show_object(
            &state,
            &token,
            &show_id.0.to_string(),
            "patched_fixture",
            &object_id,
            serde_json::to_value(invalid).unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            ShowStore::open(&entry.path)
                .unwrap()
                .objects("patched_fixture")
                .unwrap()
                .iter()
                .all(|object| object.id != object_id)
        );
    }

    let (mut multi_split, _, _) = schema_v2_direct_fixture();
    let mut profile = *multi_split.definition.profile_snapshot.take().unwrap();
    let mode_id = profile.modes[0].id;
    profile.modes[0].splits.push(light_fixture::FixtureSplit {
        number: 2,
        footprint: 1,
    });
    profile.modes[0].heads.push(light_fixture::FixtureHead {
        id: Uuid::new_v4(),
        name: "Second".into(),
        master_shared: false,
    });
    multi_split.definition = profile.resolved_definition(mode_id).unwrap();
    multi_split.split_patches = vec![
        light_fixture::SplitPatch {
            split: 1,
            universe: Some(1),
            address: Some(1),
        },
        light_fixture::SplitPatch {
            split: 2,
            universe: None,
            address: None,
        },
    ];
    multi_split.multipatch = vec![light_fixture::MultiPatchInstance {
        id: Uuid::new_v4(),
        name: "Second body".into(),
        universe: None,
        address: None,
        split_patches: multi_split.split_patches.clone(),
        location: Default::default(),
        rotation: Default::default(),
        invert_pan: false,
        invert_tilt: false,
    }];

    let mut missing_parent = multi_split.clone();
    missing_parent.split_patches.pop();
    let mut duplicate_parent = multi_split.clone();
    duplicate_parent.split_patches[1].split = 1;
    let mut partial_parent = multi_split.clone();
    partial_parent.split_patches[1].universe = Some(2);
    let mut missing_multipatch = multi_split;
    missing_multipatch.multipatch[0].split_patches.clear();

    for invalid in [
        missing_parent,
        duplicate_parent,
        partial_parent,
        missing_multipatch,
    ] {
        let response = put_show_object(
            &state,
            &token,
            &show_id.0.to_string(),
            "patched_fixture",
            &object_id,
            serde_json::to_value(invalid).unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            ShowStore::open(&entry.path)
                .unwrap()
                .objects("patched_fixture")
                .unwrap()
                .iter()
                .all(|object| object.id != object_id)
        );
    }

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn fixture_library_v2_is_replay_safe_and_preserves_package_and_gdtf_bytes() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut profile = light_fixture::FixtureProfile::blank();
    profile.manufacturer = "Acme".into();
    profile.name = "Orbit".into();
    profile.short_name = "Orbit".into();
    let profile_id = profile.id;

    let created = app
        .clone()
        .oneshot(
            Request::post("/api/v2/fixture-library")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "create-orbit",
                        "action": {
                            "type": "save_profile",
                            "profile": profile,
                            "expected_revision": 0,
                            "future_hint": true
                        },
                        "future_root": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["result"]["revision"], 1);
    let created_profile = serde_json::to_value(
        state
            .installation
            .fixture_profile(profile_id, 1)
            .unwrap()
            .unwrap(),
    )
    .unwrap();

    let exported = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/fixture-library/profiles/{}/revisions/1/package",
                profile_id.0
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exported.status(), StatusCode::OK);
    assert_eq!(
        exported.headers()[header::CONTENT_TYPE],
        light_fixture::FIXTURE_PACKAGE_MIME_TYPE
    );
    let package = exported.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        light_fixture::read_fixture_package(&package).unwrap().id,
        profile_id
    );
    let imported = app
        .clone()
        .oneshot(
            Request::post("/api/v2/fixture-library")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "import-orbit",
                        "action": {
                            "type": "import_package",
                            "package_base64": STANDARD.encode(&package)
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported.status(), StatusCode::OK);
    let imported = json(imported).await;
    assert_eq!(imported["result"]["revision"], 1);
    assert!(!imported["replayed"].as_bool().unwrap());

    let replayed = app
        .clone()
        .oneshot(
            Request::post("/api/v2/fixture-library")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "import-orbit",
                        "action": {
                            "type": "import_package",
                            "package_base64": STANDARD.encode(&package)
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(json(replayed).await["replayed"].as_bool().unwrap());

    let stale = app
        .clone()
        .oneshot(
            Request::post("/api/v2/fixture-library")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "stale-orbit",
                        "action": {
                            "type": "save_profile",
                            "profile": created_profile,
                            "expected_revision": 0
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);

    let source = b"PK\x03\x04retained-gdtf";
    let retained = app
        .clone()
        .oneshot(
            Request::post("/api/v2/fixture-library")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "request_id": "attach-orbit-gdtf",
                    "action": {
                        "type": "attach_gdtf",
                        "profile_id": profile_id.0,
                        "revision": 1,
                        "source_base64": STANDARD.encode(source)
                    }
                })
                .to_string(),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retained.status(), StatusCode::OK);
    assert_eq!(
        state
            .installation
            .fixture_profile_source_gdtf(profile_id, 1)
            .unwrap()
            .as_deref(),
        Some(source.as_slice())
    );

    let revisions = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/fixture-library/profiles/{}/revisions",
                profile_id.0
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revisions.status(), StatusCode::OK);
    assert_eq!(
        json(revisions).await["profiles"].as_array().unwrap().len(),
        1
    );

    for (path, field) in [
        ("/api/v2/fixture-library/definitions", "definitions"),
        ("/api/v2/fixture-library/profiles", "profiles"),
        ("/api/v2/fixture-library/warnings", "warnings"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get(path)
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(json(response).await[field].is_array());
    }

    let _ = std::fs::remove_dir_all(data_dir);
}
