// TL-549/550/551 UI foundation: the family encoder page route and the per-desk Color
// presentation setting.

async fn family_pages_get(
    app: &Router,
    token: Option<&str>,
    show: Option<&str>,
    uri: &str,
) -> Response {
    let mut request = Request::get(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(show) = show {
        request = request.header("x-tosk-show", show);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn update_color_presentation(app: &Router, token: &str, request_id: &str, value: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/configuration/update")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({
                        "request_id": request_id,
                        "patch": {"color_presentation": value}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["configuration"]["color_presentation"], value);
}

#[tokio::test]
async fn family_encoder_pages_are_authenticated_show_guarded_typed_and_no_store() {
    use light_wire::v2::family_encoders::{
        ColorEncoderPresentation, FamilyEncoderFamily, FamilyEncoderPagesSnapshot,
    };
    let (state, data_dir) = test_state();
    let fixture = crate::runtime::family_encoder_pages::tests::spot((8., 48.));
    let id = fixture.fixture_id;
    state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 3,
            ..Default::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let uri = format!(
        "/api/v2/programming/family-encoder-pages?fixture_ids={0},{0}&future=1",
        id.0
    );
    assert_eq!(
        family_pages_get(&app, None, None, &uri).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let other_show = Uuid::new_v4().to_string();
    assert_eq!(
        family_pages_get(&app, Some(&token), Some(&other_show), &uri)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        family_pages_get(&app, Some(&token), Some("not-a-uuid"), &uri)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        family_pages_get(
            &app,
            Some(&token),
            None,
            "/api/v2/programming/family-encoder-pages?fixture_ids=nope"
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let response = family_pages_get(&app, Some(&token), None, &uri).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let pages: FamilyEncoderPagesSnapshot =
        serde_json::from_slice(&bytes).expect("the response is the typed page snapshot");
    let supported = state.output.supported_programming_contract();
    assert_eq!(pages.supported_programming_contract, supported);
    assert_eq!(
        pages.semantic,
        supported >= light_core::programming::PROGRAMMING_CONTRACT_VERSION,
        "the semantic flag mirrors the runtime contract"
    );
    assert_eq!(pages.fixture_ids, vec![id.0, id.0]);
    assert_eq!(pages.show_revision, 3);
    assert_eq!(pages.color_presentation, ColorEncoderPresentation::EasyRgbw);
    let color = pages
        .families
        .iter()
        .find(|group| group.family == FamilyEncoderFamily::Color)
        .unwrap();
    assert_eq!(color.pages.len(), 1);

    update_color_presentation(&app, &token, "color-presentation-1", "advanced").await;
    let response = family_pages_get(&app, Some(&token), None, &uri).await;
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let pages: FamilyEncoderPagesSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(pages.color_presentation, ColorEncoderPresentation::Advanced);
    let color = pages
        .families
        .iter()
        .find(|group| group.family == FamilyEncoderFamily::Color)
        .unwrap();
    assert_eq!(color.pages.len(), 2, "the desk setting drives the Color layout");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn color_presentation_round_trips_per_desk_and_stays_out_of_the_show_file() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Color presentation stays on the desk").await;
    let response = app
        .clone()
        .oneshot(open_show_request(&token, show["id"].as_str().unwrap()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        state.installation.configuration().color_presentation,
        light_wire::v2::family_encoders::ColorEncoderPresentation::EasyRgbw,
        "an installation without the setting reads as Easy RGBW"
    );
    update_color_presentation(&app, &token, "color-presentation-a", "easy_rgbwauv").await;
    update_color_presentation(&app, &token, "color-presentation-b", "advanced").await;

    let persisted = state
        .installation
        .setting("server_configuration")
        .unwrap()
        .unwrap();
    let persisted: serde_json::Value = serde_json::from_str(&persisted).unwrap();
    assert_eq!(persisted["color_presentation"], "advanced");

    let snapshot = app
        .clone()
        .oneshot(
            Request::get("/api/v2/configuration")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    assert_eq!(
        json(snapshot).await["configuration"]["color_presentation"],
        "advanced"
    );

    let rejected = app
        .clone()
        .oneshot(
            Request::post("/api/v2/configuration/update")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    r#"{"request_id":"color-presentation-c","patch":{"color_presentation":"sideways"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(rejected.status().is_client_error(), "{}", rejected.status());
    assert_eq!(
        state.installation.configuration().color_presentation,
        light_wire::v2::family_encoders::ColorEncoderPresentation::Advanced
    );

    // The portable show file never carries a desk presentation preference.
    let show = std::path::PathBuf::from(state.active_show.current().unwrap().path);
    assert!(show.exists(), "the active show file exists");
    for path in [show.clone(), PathBuf::from(format!("{}-wal", show.display()))] {
        if let Ok(bytes) = std::fs::read(&path) {
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                !text.contains("color_presentation") && !text.contains("easy_rgbwauv"),
                "{} carries the desk Color presentation",
                path.display()
            );
        }
    }
    let _ = std::fs::remove_dir_all(data_dir);
}
