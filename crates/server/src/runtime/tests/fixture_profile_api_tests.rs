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
            Request::put("/api/v1/fixture-profiles")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "0")
                .body(Body::from(serde_json::to_vec(&profile).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        state
            .fixture_library
            .lock()
            .profile(profile_id, 1)
            .unwrap()
            .is_none()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn inactive_show_rejects_invalid_schema_v2_patch_before_persistence() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Inactive patch preflight").await;
    let show_id = light_core::ShowId(Uuid::parse_str(show["id"].as_str().unwrap()).unwrap());
    let entry = state.desk.lock().show(show_id).unwrap().unwrap();
    assert!(state.active_show.read().is_none());

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
    }];

    for invalid in [inconsistent_identity, unknown_split, overlapping_multipatch] {
        let response = put_show_object(
            &app,
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
            &app,
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
async fn fixture_profile_api_assigns_atomic_revisions_retains_gdtf_and_rejects_stale_edits() {
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
            Request::put("/api/v1/fixture-profiles")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "0")
                .body(Body::from(serde_json::to_vec(&profile).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["revision"], 1);

    let exported = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v1/fixture-profiles/{}/1/package",
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
            Request::post("/api/v1/fixture-packages/import")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(
                    header::CONTENT_TYPE,
                    light_fixture::FIXTURE_PACKAGE_MIME_TYPE,
                )
                .body(Body::from(package))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported.status(), StatusCode::OK);
    assert_eq!(json(imported).await["revision"], 1);

    let stale = app
        .clone()
        .oneshot(
            Request::put("/api/v1/fixture-profiles")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "0")
                .body(Body::from(created.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);

    let source = b"PK\x03\x04retained-gdtf";
    let retained = app
        .clone()
        .oneshot(
            Request::put(format!(
                "/api/v1/fixture-profiles/{}/1/source-gdtf",
                profile_id.0
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(source.as_slice()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retained.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        state
            .fixture_library
            .lock()
            .profile_source_gdtf(profile_id, 1)
            .unwrap()
            .as_deref(),
        Some(source.as_slice())
    );

    let revisions = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v1/fixture-profiles/{}/revisions",
                profile_id.0
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revisions.status(), StatusCode::OK);
    assert_eq!(json(revisions).await.as_array().unwrap().len(), 1);

    let warnings = app
        .oneshot(
            Request::get("/api/v1/fixture-profiles/warnings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(warnings.status(), StatusCode::OK);
    assert!(json(warnings).await.as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}
