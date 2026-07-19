#[tokio::test]
async fn legacy_programmer_set_closes_a_gesture_with_one_authoritative_selection_event() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let fixture = light_core::FixtureId::new();
    assert!(state.programmers.apply_selection_gesture(
        session.id,
        vec![light_programmer::SelectionReference::Fixture {
            fixture_id: fixture,
        }],
        &HashMap::new(),
    ));
    let before = state.application_events.latest_sequence();

    let response = legacy_programmer_set_request(&app, &token, fixture, 0.5).await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let events = programming_selection_events(&state, session.desk.id, before);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::InteractionChanged(change),
    ) = &events[0].payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    let selection = change.selection().unwrap();
    assert_eq!(selection.selected, vec![fixture]);
    assert!(!selection.gesture_open);

    let cursor = state.application_events.latest_sequence();
    let response = legacy_programmer_set_request(&app, &token, fixture, 0.75).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(programming_selection_events(&state, session.desk.id, cursor).is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn legacy_programmer_set_request(
    app: &Router,
    token: &str,
    fixture: light_core::FixtureId,
    value: f32,
) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v1/programmer/set")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({
                        "fixture_id": fixture,
                        "attribute": "intensity",
                        "value": value,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

fn programming_selection_events(
    state: &AppState,
    desk_id: Uuid,
    after: u64,
) -> Vec<Arc<light_application::EventEnvelope>> {
    let filter = light_application::EventFilter::for_desk(desk_id).with_object(
        light_application::EventObject::programming_selection(desk_id),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(after, &filter)
    else {
        panic!("selection events should remain replayable")
    };
    events
}
use super::*;
