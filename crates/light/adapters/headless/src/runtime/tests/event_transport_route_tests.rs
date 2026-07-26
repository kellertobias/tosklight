//! Router-level authentication coverage for the v2 event transport.

use super::*;

#[tokio::test]
async fn v2_socket_protocol_uses_live_auth_and_the_broad_snapshot_is_removed() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut protocols = HeaderMap::new();
    protocols.insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("light.events.v2, light.token.{token}")
            .parse()
            .unwrap(),
    );
    let session = event_transport::authenticate_protocols(&state, &protocols).unwrap();
    assert_eq!(session.token, token);
    assert!(event_transport::authenticate_protocols(&state, &HeaderMap::new()).is_err());

    let removed = app
        .oneshot(
            Request::get("/api/v2/events/playback-snapshot")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn malformed_correlated_action_returns_v2_failure_without_mutating_programmer_state() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut protocols = HeaderMap::new();
    protocols.insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("light.events.v2, light.token.{token}")
            .parse()
            .unwrap(),
    );
    let session = event_transport::authenticate_protocols(&state, &protocols).unwrap();
    let before = serde_json::to_value(state.programming.get(session.id)).unwrap();
    let stream = event_transport::EventStream::subscribe(
        &state.events,
        &session,
        Ok(light_wire::v2::events::EventClientMessage::Subscribe {
            filter: Default::default(),
            after_sequence: None,
            capacity: Some(8),
            rate_limits: Vec::new(),
        }),
    )
    .unwrap();
    let response = event_transport::client_response(
        &stream,
        &state,
        &session,
        event_transport::ClientMessage::Invalid {
            request_id: Some("malformed-action".into()),
            error: "invalid action frame: session_id is not a UUID".into(),
        },
    );
    let event_transport::ServerMessage::Command(response) = response else {
        panic!("correlated malformed actions must return an action response");
    };
    assert_eq!(response.protocol_version, 2);
    assert_eq!(response.request_id, "malformed-action");
    assert!(!response.ok);
    assert_eq!(
        serde_json::to_value(state.programming.get(session.id)).unwrap(),
        before
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_subscription_dispatches_an_action_and_keeps_delivering_events() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut protocols = HeaderMap::new();
    protocols.insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("light.events.v2, light.token.{token}")
            .parse()
            .unwrap(),
    );
    let session = event_transport::authenticate_protocols(&state, &protocols).unwrap();
    let mut stream = event_transport::EventStream::subscribe(
        &state.events,
        &session,
        Ok(light_wire::v2::events::EventClientMessage::Subscribe {
            filter: light_wire::v2::events::EventSubscriptionFilter::default(),
            after_sequence: Some(state.events.latest_sequence()),
            capacity: Some(32),
            rate_limits: Vec::new(),
        }),
    )
    .unwrap();

    let response = event_transport::client_response(
        &stream,
        &state,
        &session,
        event_transport::ClientMessage::Action(light_wire::v2::live_action::LiveActionFrame {
            message_type: light_wire::v2::live_action::LiveActionMessageType::Action,
            protocol_version: 2,
            request_id: "v2-multiplex-command".into(),
            session_id: session.id.0,
            action: light_wire::v2::live_action::LiveAction::ProgrammerUndo,
        }),
    );
    let event_transport::ServerMessage::Command(response) = response else {
        panic!("command frames should return command responses");
    };
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(response.request_id, "v2-multiplex-command");

    state.events.publish(
        light_application::EventDraft::virtual_playback_exclusion_zones_changed(
            light_application::VirtualPlaybackExclusionZonesChange {
                show_id: light_core::ShowId(Uuid::from_u128(41)),
                desk_id: session.desk.id,
                surface_id: "multiplex-test".into(),
            },
        ),
    );
    let Some(light_wire::v2::events::EventServerMessage::Event { event }) = stream.next().await
    else {
        panic!("filtered event delivery should remain active after a command");
    };
    assert!(matches!(
        event.payload,
        light_wire::v2::events::EventPayload::VirtualPlaybackExclusionZonesChanged { .. }
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_subscription_delivers_typed_fixture_library_events_from_the_emit_boundary() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut protocols = HeaderMap::new();
    protocols.insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        format!("light.events.v2, light.token.{token}")
            .parse()
            .unwrap(),
    );
    let session = event_transport::authenticate_protocols(&state, &protocols).unwrap();
    let mut stream = event_transport::EventStream::subscribe(
        &state.events,
        &session,
        Ok(light_wire::v2::events::EventClientMessage::Subscribe {
            filter: light_wire::v2::events::EventSubscriptionFilter {
                capabilities: vec![light_wire::v2::events::EventCapability::Show],
                ..Default::default()
            },
            after_sequence: Some(state.events.latest_sequence()),
            capacity: Some(32),
            rate_limits: Vec::new(),
        }),
    )
    .unwrap();

    emit(
        &state,
        "fixture_profile_changed",
        serde_json::json!({"fixture_id": 42}),
    );

    let Some(light_wire::v2::events::EventServerMessage::Event { event }) = stream.next().await
    else {
        panic!("expected the fixture-library event on the v2 event stream");
    };
    let light_wire::v2::events::EventPayload::FixtureLibraryChanged { change } = event.payload
    else {
        panic!("expected a fixture-library payload");
    };
    assert_eq!(
        change.kind,
        light_wire::v2::events::FixtureLibraryNotificationKind::Profile
    );
    assert_eq!(
        change.revision,
        state.events.audit_events().last().unwrap().revision
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
