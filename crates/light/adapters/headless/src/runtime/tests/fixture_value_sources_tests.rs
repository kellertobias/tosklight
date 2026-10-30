#[tokio::test]
async fn fixture_value_sources_reports_actual_playback_programmer_and_freeze() {
    let (state, data_dir) = test_state();
    let fixture_id = light_core::FixtureId::new();
    let list_id = light_core::CueListId::new();
    let mut fixture = operational_fixture(fixture_id);
    fixture.universe = None;
    fixture.address = None;
    let mut list = operational_cue_list(list_id, fixture_id);
    list.cues[0].fade_millis = 0;
    state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![list].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 2,
                name: "Test page".into(),
                assignments: [(
                    7,
                    light_playback::PlaybackDefinition::new_cue_list("Test", list_id),
                )]
                .into(),
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = state.sessions.session_for_token(&token).unwrap();
    let read = || async {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v2/output/value-sources?fixture_id={}",
                    fixture_id.0
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        json(response).await
    };
    assert_eq!(read().await["values"], serde_json::json!([]));
    state
        .output
        .execute_playback(light_engine::EnginePlaybackCommand::Playback {
            address: light_playback::PlaybackAddress::new(2, 7).unwrap(),
            action: light_engine::PlaybackAction::On,
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "inspection".into(),
                control_id: 1,
            },
        })
        .unwrap();
    assert!(
        read().await["values"][0]["sources"]
            .to_string()
            .contains("Playback 2.7 · Main")
    );
    state.programming.set_many_immediate_with_delay(
        session.id,
        [(
            fixture_id,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(1.0),
        )],
        Some(0),
    );
    assert!(
        read().await["values"][0]["sources"]
            .to_string()
            .contains("Programmer"),
        "same numeric value still changes its source"
    );
    state.programming.clear_normal_values(session.id);
    assert!(
        read().await["values"][0]["sources"]
            .to_string()
            .contains("Playback 2.7")
    );
    let mut snapshot = state.output.snapshot().as_ref().clone();
    Arc::make_mut(&mut snapshot.fixtures)[0]
        .freeze
        .targets
        .insert(
            fixture_id,
            light_fixture::FrozenFixtureTarget {
                full: true,
                values: [(
                    light_core::AttributeKey::intensity(),
                    light_core::AttributeValue::Normalized(0.4),
                )]
                .into(),
                ..Default::default()
            },
        );
    snapshot.revision += 1;
    state.output.replace_snapshot(snapshot).unwrap();
    assert_eq!(
        read().await["values"][0]["sources"],
        serde_json::json!(["Freeze (held output)"])
    );
    for (query, authenticated, expected) in [
        (fixture_id.0.to_string(), false, StatusCode::UNAUTHORIZED),
        (Uuid::new_v4().to_string(), true, StatusCode::NOT_FOUND),
        ("invalid".into(), true, StatusCode::BAD_REQUEST),
    ] {
        let mut request = Request::get(format!("/api/v2/output/value-sources?fixture_id={query}"));
        if authenticated {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            expected
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}
