//! Portable revision authority is independent of the retained accepted output generation.
use super::*;
use crate::runtime::visualization_frame::RenderedSemanticFrame;
use light_wire::v2::visualization::VisualizationScope;

#[tokio::test]
async fn layout_save_preload_go_uses_portable_authority_without_invalidating_accepted_frame() {
    let scenario = CommandHttpScenario::new().await;
    let (show_id, before_revision, fixture) =
        install_lifecycle_show(&scenario, "TL755 layout authority").await;
    scenario.state.output.set_transition_hold(false);
    let rendered = RenderedSemanticFrame::untraced(
        scenario
            .state
            .output
            .engine()
            .render(Default::default())
            .unwrap(),
        Default::default(),
    );
    scenario.state.output.render_frames_and_publish(
        &rendered,
        VisualizationScope {
            show_id: Some(show_id),
        },
    );
    let accepted = scenario.state.output.latest_visualization_frame().unwrap();
    let engine = scenario.state.output.snapshot();
    assert_eq!(
        scenario
            .preload_lifecycle_action(lifecycle_request(
                "tl755-enter",
                0,
                0,
                0,
                0,
                serde_json::json!({"type":"enter"})
            ))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .preload_values_action(preload_fixture_request(
                "tl755-pending",
                0,
                1,
                fixture.0,
                0.6
            ))
            .await
            .status(),
        StatusCode::OK
    );
    let captured = scenario
        .playback_action_for(
            &scenario.token,
            scenario.session.desk.id,
            playback_request("tl755-queue", 1, "go"),
        )
        .await;
    assert_eq!(captured.status(), StatusCode::OK);
    assert_eq!(json(captured).await["outcome"]["status"], "captured");
    let saved = scenario.app.clone().oneshot(Request::post("/api/v2/user-layouts/desk/update")
        .header(header::AUTHORIZATION,format!("Bearer {}",scenario.token))
        .header("x-tosk-desk",scenario.session.desk.id.to_string())
        .header("x-tosk-show",show_id.to_string())
        .header(header::CONTENT_TYPE,"application/json")
        .body(Body::from(serde_json::json!({"request_id":"tl755-layout","action":{"type":"update","expected_revision":0,"patch":{"desks":[],"active_desk_id":"main"}}}).to_string())).unwrap()).await.unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    let saved = json(saved).await;
    let portable = saved["show_revision"].as_u64().unwrap();
    assert!(portable > before_revision);
    assert!(Arc::ptr_eq(&engine, &scenario.state.output.snapshot()));
    assert!(Arc::ptr_eq(
        &accepted,
        &scenario.state.output.latest_visualization_frame().unwrap()
    ));
    let cursor = scenario.state.events.latest_sequence();
    let pending_values = json(scenario.preload_values_snapshot().await).await["projection"].clone();
    for (request_id, requested_show, revision) in [
        ("tl755-stale", show_id, before_revision),
        ("tl755-foreign", Uuid::new_v4(), portable),
    ] {
        let rejected = scenario
            .preload_lifecycle_action(lifecycle_go(
                request_id,
                1,
                1,
                1,
                0,
                requested_show,
                revision,
                cursor,
            ))
            .await;
        assert_eq!(rejected.status(), StatusCode::CONFLICT);
        assert_eq!(
            scenario
                .state
                .programming
                .preload_playback_actions(scenario.session.id)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(scenario.state.events.latest_sequence(), cursor);
        assert_eq!(
            json(scenario.preload_values_snapshot().await).await["projection"],
            pending_values
        );
        assert!(Arc::ptr_eq(&engine, &scenario.state.output.snapshot()));
    }
    let go = lifecycle_go("tl755-current", 1, 1, 1, 0, show_id, portable, cursor);
    let response = scenario.preload_lifecycle_action(go.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let committed = json(response).await;
    assert_eq!(committed["commit"]["show_revision"], portable);
    assert_eq!(committed["commit"]["executed_playback_actions"], 1);
    assert!(
        scenario
            .state
            .programming
            .preload_playback_actions(scenario.session.id)
            .unwrap()
            .is_empty()
    );
    let after_cursor = scenario.state.events.latest_sequence();
    let replayed = json(scenario.preload_lifecycle_action(go).await).await;
    assert_eq!(replayed["replayed"], true);
    assert_eq!(scenario.state.events.latest_sequence(), after_cursor);
}
