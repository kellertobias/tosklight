use super::*;

#[tokio::test]
async fn usb_endpoint_api_authenticates_replays_guards_revision_and_persists() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        user: user.clone(),
        token: "usb-endpoint-writer".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    state.sessions.insert_session(session.clone());
    let app = router(state.clone());
    let unauthorized = app
        .clone()
        .oneshot(
            Request::get("/api/v2/usb-dmx/endpoints")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let body = serde_json::json!({
        "request_id": "install-front-usb",
        "expected_revision": 0,
        "future_field": "is tolerated",
        "action": {
            "action": "upsert",
            "endpoint": {
                "endpoint_id": "front",
                "driver": "enttec_usb_pro_v144",
                "identity": {
                    "vendor_id": 1027,
                    "product_id": 24577,
                    "manufacturer": "ENTTEC",
                    "product": "DMX USB PRO",
                    "usb_serial": "PRO-1"
                },
                "enabled": true
            }
        }
    });
    let request = || {
        Request::post("/api/v2/usb-dmx/endpoints/update")
            .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let first = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first = json(first).await;
    assert_eq!(first["document"]["revision"], 1);
    assert_eq!(
        first["diagnostics"][0]["code"],
        "platform_adapter_unavailable"
    );
    assert_eq!(first["replayed"], false);

    let replay = json(app.clone().oneshot(request()).await.unwrap()).await;
    assert_eq!(replay["document"]["revision"], 1);
    assert_eq!(replay["replayed"], true);

    let conflict = app
        .clone()
        .oneshot(
            Request::post("/api/v2/usb-dmx/endpoints/update")
                .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "stale-edit",
                        "expected_revision": 0,
                        "action": {"action": "remove", "endpoint_id": "front"}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);

    let stored = state
        .installation
        .setting(crate::runtime::usb_output::USB_ENDPOINTS_SETTING)
        .unwrap()
        .unwrap();
    let stored: light_output::UsbEndpointDocument = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored.revision, 1);
    assert_eq!(stored.endpoints[0].endpoint_id, "front");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn malformed_endpoint_setting_is_reported_and_requires_explicit_reset() {
    let (state, data_dir) = test_state();
    state
        .installation
        .set_setting(crate::runtime::usb_output::USB_ENDPOINTS_SETTING, "{broken")
        .unwrap();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        user,
        token: "usb-recovery".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, session.user.id);
    state.sessions.insert_session(session.clone());
    let app = router(state.clone());
    let snapshot = app
        .clone()
        .oneshot(
            Request::get("/api/v2/usb-dmx/endpoints")
                .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    assert!(
        json(snapshot).await["configuration_error"]
            .as_str()
            .unwrap()
            .contains("invalid JSON")
    );
    assert_eq!(
        state
            .installation
            .setting(crate::runtime::usb_output::USB_ENDPOINTS_SETTING)
            .unwrap()
            .as_deref(),
        Some("{broken")
    );

    let reset = app
        .oneshot(
            Request::post("/api/v2/usb-dmx/endpoints/update")
                .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "repair-usb-setting",
                        "expected_revision": 0,
                        "action": {"action": "reset_malformed"}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reset.status(), StatusCode::OK);
    assert_eq!(json(reset).await["document"]["revision"], 1);
    let _ = std::fs::remove_dir_all(data_dir);
}
