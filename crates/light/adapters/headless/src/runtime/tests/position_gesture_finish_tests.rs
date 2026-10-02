//! Actual canonical HTTP/WS terminal cleanup; no autonomous server motion is introduced.
use super::*;

fn finish_body(_desk: &PositionDesk, id: &str, gesture: &str) -> serde_json::Value {
    serde_json::json!({
        "request_id":id, "expected_revision":u64::MAX,
        "expected_capture_mode_revision":u64::MAX,
        "action":{"type":"finish_gesture", "attribute":"position", "undo_group":gesture}
    })
}

#[tokio::test]
async fn canonical_finish_ends_neutral_capture_then_fresh_http_edit_adopts_new_fitted_pose() {
    let mut desk = PositionDesk::new().await;
    desk.target("finish-target").await;
    let first = desk.publish(0.5);
    let depth = desk.depth();
    desk.http(desk.turn("finish-neutral", 0., "first-touch"))
        .await;
    let second = desk.publish(0.55);
    assert!((second.pan_degrees - first.pan_degrees).abs() > 1.);
    let revision = desk.revision();
    let events = desk.scenario.state.events.latest_sequence();
    let request = finish_body(&desk, "http-finish", "first-touch");
    let end = desk.http(request.clone()).await;
    assert_eq!(end["status"], "no_change");
    assert_eq!(end["revision"], revision);
    assert_eq!(
        end["capture_mode_revision"],
        desk.scenario.state.programming.capture_mode_revision()
    );
    assert_eq!(desk.revision(), revision);
    assert_eq!(desk.depth(), depth);
    assert_eq!(desk.stored(), desk.original);
    assert_eq!(desk.scenario.state.events.latest_sequence(), events);
    // This deliberately reuses the caller ID to prove capture retirement itself, rather than
    // relying on a fresh ID which already invalidates the retained key.
    desk.http(desk.turn("new-neutral-after-end", 0., "first-touch"))
        .await;
    let replay = desk.http(request).await;
    assert_eq!(replay["replayed"], true);
    let still_revision = desk.revision();
    let latest = desk.publish(0.6);
    assert!((latest.pan_degrees - second.pan_degrees).abs() > 1.);
    let moved = desk
        .http(desk.turn("http-after-end", 5., "first-touch"))
        .await;
    assert_eq!(moved["status"], "changed");
    assert!(desk.revision() > still_revision);
    assert_eq!(
        desk.stored(),
        physical::angles(second.pan_degrees + 5., second.tilt_degrees)
    );
    assert_eq!(desk.depth(), depth + 1);
    let _ = std::fs::remove_dir_all(&desk.scenario.data_dir);
}

#[tokio::test]
async fn canonical_ws_finish_preserves_values_and_inactive_preload_finish_is_quiet() {
    let mut desk = PositionDesk::new().await;
    desk.target("ws-finish-target").await;
    desk.publish(0.5);
    desk.http(desk.turn("ws-touch-turn", 5., "ws-touch")).await;
    let value = desk.stored();
    let revision = desk.revision();
    let depth = desk.depth();
    let events = desk.scenario.state.events.latest_sequence();
    let request = finish_body(&desk, "ws-finish", "ws-touch");
    let action =
        serde_json::from_value(serde_json::json!({"type":"programming_values", "request":request}))
            .unwrap();
    let frame = live_action_frame(&desk.scenario.session, "ws-finish", action);
    let result = dispatch_live_action(&desk.scenario.state, &desk.scenario.session, frame.clone());
    assert!(result.ok, "{:?}", result.error);
    let result = result.payload.unwrap();
    assert_eq!(result["status"], "no_change");
    assert_eq!(result["revision"], revision);
    let replay = dispatch_live_action(&desk.scenario.state, &desk.scenario.session, frame);
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["replayed"], true);
    let response = desk
        .scenario
        .preload_values_action(finish_body(&desk, "inactive-preload-http-end", "ws-touch"))
        .await;
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["status"], "no_change");
    let request = finish_body(&desk, "inactive-preload-ws-end", "ws-touch");
    let action = serde_json::from_value(
        serde_json::json!({"type":"programmer_preload_values", "request":request}),
    )
    .unwrap();
    let result = dispatch_live_action(
        &desk.scenario.state,
        &desk.scenario.session,
        live_action_frame(&desk.scenario.session, "inactive-preload-ws-end", action),
    );
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(result.payload.unwrap()["status"], "no_change");
    assert_eq!(desk.stored(), value);
    assert_eq!(desk.revision(), revision);
    assert_eq!(desk.depth(), depth);
    assert_eq!(desk.scenario.state.events.latest_sequence(), events);
    let _ = std::fs::remove_dir_all(&desk.scenario.data_dir);
}
