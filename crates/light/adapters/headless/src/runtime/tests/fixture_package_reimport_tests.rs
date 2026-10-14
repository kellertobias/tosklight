// Actual HTTP package import must recognize an unchanged installed legacy profile before mapping.
fn legacy_reimport_profile() -> light_fixture::FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/robe--robin-dls-profile.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(profile.gobos.len(), 7);
    assert!(profile.gobos.iter().all(|gobo| {
        gobo.artwork_asset
            .as_ref()
            .is_some_and(|asset| asset.starts_with("data:image/png;base64,"))
    }));
    profile
}

async fn legacy_reimport_call(
    app: &Router,
    token: Option<&str>,
    request_id: &str,
    package: &[u8],
    mappings: serde_json::Value,
) -> axum::response::Response {
    let mut request =
        Request::post("/api/v2/fixture-library").header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    app.clone().oneshot(request.body(Body::from(serde_json::json!({
        "request_id":request_id,"action":{"type":"import_package","package_base64":STANDARD.encode(package),"attribute_mappings":mappings}
    }).to_string())).unwrap()).await.unwrap()
}

#[tokio::test]
async fn fixture_package_reimport_exact_legacy_http_noop_preserves_assets_events_and_replay() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let profile = legacy_reimport_profile();
    let id = profile.id;
    let stored = state.installation.save_fixture_profile(profile, 0).unwrap();
    assert_eq!(stored.revision, 1);
    assert_eq!(
        fixture_api::unknown_canonical_attributes(&state, &stored),
        vec![(
            "gobo.rotating_gobo_selection_speed".into(),
            light_core::AttributeValueType::Continuous
        )]
    );
    let exported = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/fixture-library/profiles/{}/revisions/1/package",
                id.0
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exported.status(), StatusCode::OK);
    let bytes = exported.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::to_value(light_fixture::read_fixture_package(&bytes).unwrap()).unwrap(),
        serde_json::to_value(&stored).unwrap()
    );
    let readonly = json(
        app.clone()
            .oneshot(
                Request::post("/api/v2/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({"username":"Operator","role":"visualizer"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let profiles_before =
        serde_json::to_value(state.installation.fixture_profiles().unwrap()).unwrap();
    let events_before = state.events.latest_sequence();
    assert_eq!(
        legacy_reimport_call(&app, None, "unauth-import", &bytes, serde_json::json!([]))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        legacy_reimport_call(
            &app,
            readonly["token"].as_str(),
            "readonly-import",
            &bytes,
            serde_json::json!([])
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = legacy_reimport_call(
        &app,
        Some(&token),
        "legacy-noop",
        &bytes,
        serde_json::json!([]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(
        outcome["result"],
        serde_json::json!({"type":"profile","profile_id":id.0,"revision":1}),
        "an exact existing package must not require descriptor mapping"
    );
    assert_eq!(outcome["replayed"], false);
    let replay = json(
        legacy_reimport_call(
            &app,
            Some(&token),
            "legacy-noop",
            &bytes,
            serde_json::json!([]),
        )
        .await,
    )
    .await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"], outcome["result"]);
    let (a, b) = tokio::join!(
        legacy_reimport_call(
            &app,
            Some(&token),
            "concurrent-noop",
            &bytes,
            serde_json::json!([])
        ),
        legacy_reimport_call(
            &app,
            Some(&token),
            "concurrent-noop",
            &bytes,
            serde_json::json!([])
        )
    );
    assert_eq!(a.status(), StatusCode::OK);
    assert_eq!(b.status(), StatusCode::OK);
    let a = json(a).await;
    let b = json(b).await;
    assert_eq!(a["result"], outcome["result"]);
    assert_eq!(b["result"], outcome["result"]);
    assert_ne!(
        a["replayed"], b["replayed"],
        "same identity has one original outcome and one replay"
    );
    let mut changed = stored.clone();
    changed.notes.push_str("changed content");
    let changed_bytes = light_fixture::write_fixture_package(&changed).unwrap();
    assert_eq!(
        legacy_reimport_call(
            &app,
            Some(&token),
            "legacy-noop",
            &changed_bytes,
            serde_json::json!([])
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        state
            .installation
            .fixture_profile_revisions(id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        serde_json::to_value(state.installation.fixture_profiles().unwrap()).unwrap(),
        profiles_before
    );
    assert_eq!(
        state
            .installation
            .export_fixture_package(id, 1)
            .unwrap()
            .unwrap()
            .1,
        bytes
    );
    assert_eq!(
        state.events.latest_sequence(),
        events_before,
        "no import, rejection, or replay may publish an event for unchanged content"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn fixture_package_reimport_changed_new_and_explicit_mapping_keep_validation() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let stored = state
        .installation
        .save_fixture_profile(legacy_reimport_profile(), 0)
        .unwrap();
    let before = serde_json::to_value(state.installation.fixture_profiles().unwrap()).unwrap();
    let events = state.events.latest_sequence();
    let mut changed = stored.clone();
    changed.notes.push_str("new authored optics");
    let mut new = stored.clone();
    new.id = light_core::FixtureId::new();
    for (request, profile) in [("changed-package", &changed), ("new-package", &new)] {
        let package = light_fixture::write_fixture_package(profile).unwrap();
        let response =
            legacy_reimport_call(&app, Some(&token), request, &package, serde_json::json!([]))
                .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json(response).await["result"],
            serde_json::json!({"type":"import_required","unknown_attributes":[{"attribute":"gobo.rotating_gobo_selection_speed","value_type":"continuous"}]})
        );
    }
    let exact = light_fixture::write_fixture_package(&stored).unwrap();
    let bad_mapping=legacy_reimport_call(&app,Some(&token),"explicit-incompatible-mapping",&exact,serde_json::json!([{"source_attribute":"gobo.rotating_gobo_selection_speed","target_attribute":"gobo.1"}])).await;
    assert_eq!(bad_mapping.status(), StatusCode::BAD_REQUEST);
    assert!(json(bad_mapping).await.to_string().contains("incompatible"));
    let mut wrong_family = stored.clone();
    wrong_family.manufacturer = "Different family".into();
    let wrong = legacy_reimport_call(
        &app,
        Some(&token),
        "wrong-family",
        &light_fixture::write_fixture_package(&wrong_family).unwrap(),
        serde_json::json!([]),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);
    assert!(json(wrong).await.to_string().contains("belongs to"));
    assert_eq!(
        serde_json::to_value(state.installation.fixture_profiles().unwrap()).unwrap(),
        before
    );
    assert_eq!(state.events.latest_sequence(), events);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn fixture_package_reimport_historical_match_cannot_bypass_latest_content_mapping() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let older = state
        .installation
        .save_fixture_profile(legacy_reimport_profile(), 0)
        .unwrap();
    let package = light_fixture::write_fixture_package(&older).unwrap();
    let mut latest = older.clone();
    latest.notes.push_str("operator revision");
    let latest = state.installation.save_fixture_profile(latest, 1).unwrap();
    assert_eq!(latest.revision, 2);
    let before = serde_json::to_value(state.installation.fixture_profiles().unwrap()).unwrap();
    let events = state.events.latest_sequence();
    let response = legacy_reimport_call(
        &app,
        Some(&token),
        "old-revision",
        &package,
        serde_json::json!([]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json(response).await["result"]["type"],
        "import_required",
        "an exact historical revision still differs from the installed latest content"
    );
    assert_eq!(
        serde_json::to_value(state.installation.fixture_profiles().unwrap()).unwrap(),
        before
    );
    assert_eq!(
        state
            .installation
            .fixture_profile_revisions(older.id)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(state.events.latest_sequence(), events);
    let _ = std::fs::remove_dir_all(data_dir);
}
