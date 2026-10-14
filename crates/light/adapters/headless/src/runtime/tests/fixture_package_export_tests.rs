// Confined export of an exact immutable fixture package, through the real HTTP boundary.
fn package_export_body(request: &str, path: &str, name: &str) -> serde_json::Value {
    serde_json::json!({"request_id": request, "root_id": "shows", "path": path, "name": name})
}

async fn package_export_call(
    app: &Router,
    token: Option<&str>,
    profile: light_core::FixtureId,
    body: serde_json::Value,
) -> axum::response::Response {
    let mut request = Request::post(format!(
        "/api/v2/fixture-library/profiles/{}/revisions/1/export",
        profile.0
    ))
    .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn fixture_package_export_is_exact_exclusive_and_replay_safe() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let profile_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/robe--robin-dls-profile.toskfixture");
    let profile =
        light_fixture::read_fixture_package(&std::fs::read(profile_path).unwrap()).unwrap();
    let id = profile.id;
    state.installation.save_fixture_profile(profile, 0).unwrap();
    let (before_profile, original) = state
        .installation
        .export_fixture_package(id, 1)
        .unwrap()
        .unwrap();
    std::fs::create_dir_all(data_dir.join("shows/exports")).unwrap();
    let body = package_export_body("exact-export", "exports", "dls.toskfixture");
    assert_eq!(
        package_export_call(&app, None, id, body.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let readonly_session = json(
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
    assert_eq!(
        package_export_call(&app, readonly_session["token"].as_str(), id, body.clone())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = package_export_call(&app, Some(&token), id, body.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let first = json(response).await;
    assert_eq!(first["path"], "exports/dls.toskfixture");
    assert_eq!(first["replayed"], false);
    let destination = data_dir.join("shows/exports/dls.toskfixture");
    assert_eq!(std::fs::read(&destination).unwrap(), original);
    let reimported = state
        .installation
        .import_fixture_package(&std::fs::read(&destination).unwrap())
        .unwrap();
    assert_eq!(reimported.id, id);
    assert_eq!(reimported.revision, 1, "identical reimport is a no-op");
    let (_, after) = state
        .installation
        .export_fixture_package(id, 1)
        .unwrap()
        .unwrap();
    assert_eq!(after, original);
    assert_eq!(
        serde_json::to_value(light_fixture::read_fixture_package(&after).unwrap()).unwrap(),
        serde_json::to_value(before_profile).unwrap()
    );
    let replay = json(package_export_call(&app, Some(&token), id, body).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["path"], first["path"]);
    let changed = package_export_call(
        &app,
        Some(&token),
        id,
        package_export_body("exact-export", "exports", "other"),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::CONFLICT);
    assert!(!data_dir.join("shows/exports/other.toskfixture").exists());
    let duplicate = package_export_call(
        &app,
        Some(&token),
        id,
        package_export_body("new-export", "exports", "dls"),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    assert_eq!(std::fs::read(destination).unwrap(), original);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn fixture_package_export_rejects_unknown_roots_profiles_and_path_escapes() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let (fixture, _, _) = schema_v2_direct_fixture();
    let profile = std::sync::Arc::unwrap_or_clone(fixture.definition.profile_snapshot.unwrap());
    let id = profile.id;
    state.installation.save_fixture_profile(profile, 0).unwrap();
    for (request, path, name) in [
        ("parent", "..", "export"),
        ("absolute", "/", "export"),
        ("name", "", "../export"),
    ] {
        let response = package_export_call(
            &app,
            Some(&token),
            id,
            package_export_body(request, path, name),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let mut unknown_root = package_export_body("unknown-root", "", "export");
    unknown_root["root_id"] = "missing".into();
    assert_eq!(
        package_export_call(&app, Some(&token), id, unknown_root)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        package_export_call(
            &app,
            Some(&token),
            light_core::FixtureId::new(),
            package_export_body("missing-profile", "", "export")
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    #[cfg(unix)]
    {
        let outside = data_dir.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, data_dir.join("shows/escape")).unwrap();
        assert_eq!(
            package_export_call(
                &app,
                Some(&token),
                id,
                package_export_body("symlink", "escape", "export")
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert!(!outside.join("export.toskfixture").exists());
        use std::os::unix::fs::PermissionsExt;
        let readonly = data_dir.join("shows/readonly");
        std::fs::create_dir_all(&readonly).unwrap();
        std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert_eq!(
            package_export_call(
                &app,
                Some(&token),
                id,
                package_export_body("readonly", "readonly", "export")
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        std::fs::set_permissions(readonly, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let _ = std::fs::remove_dir_all(data_dir);
}
