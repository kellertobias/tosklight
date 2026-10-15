use super::*;

#[tokio::test]
async fn websocket_programmer_undo_restores_selective_import_history() {
    selective_import_undo_transport(true).await;
}

#[tokio::test]
async fn http_programmer_undo_restores_selective_import_history() {
    selective_import_undo_transport(false).await;
}

async fn selective_import_undo_transport(websocket: bool) {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let source = create_show(&app, &token, "Undo source").await;
    let target = create_show(&app, &token, "Undo destination").await;
    let source_id = source["id"].as_str().unwrap();
    let target_id = target["id"].as_str().unwrap();
    put_group(&state, source_id, "front", "Imported front");
    put_group(&state, target_id, "front", "Original front");
    open_import_target(&app, &token, target_id).await;
    let response = post_import(
        &app,
        &token,
        target_id,
        source_id,
        "apply",
        Some(3),
        serde_json::json!({
            "request_id":"transport-import", "expected_source_revision":2,
            "expected_target_revision":3, "mode":"replace_by_position",
            "selected_objects":[{"kind":"group","id":"front"}],
            "conflict_resolutions":[{"key":{"kind":"group","id":"front"},
                "resolution":"replace_destination"}],
            "profile_conflict_resolutions":[]
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        stored_group_name(&state, target_id, "front"),
        "Imported front"
    );
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    if websocket {
        let response = super::super::super::ws_dispatch::dispatch_live_action_live(
            &state,
            &session,
            live_action_frame(
                &session,
                "undo-import-ws",
                light_wire::v2::live_action::LiveAction::ProgrammerUndo,
            ),
        )
        .await;
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(response.payload.unwrap()["changed"], true);
    } else {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v2/programmer-undo/actions")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header("x-tosk-show", target_id)
                    .header("x-tosk-desk", session.desk.id.to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(json(response).await["changed"], true);
    }
    assert_eq!(
        stored_group_name(&state, target_id, "front"),
        "Original front"
    );
    let store = ShowStore::open(&show_entry(&state, target_id).path).unwrap();
    assert_eq!(store.portable_document().unwrap().revision().value(), 5);
    drop(store);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn http_programmer_undo_preserves_authorization_show_lock_and_noop_guards() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let created = create_show(&app, &token, "Undo guards").await;
    let show = created["id"].as_str().unwrap().to_owned();
    open_import_target(&app, &token, &show).await;
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    let mut before = serde_json::to_value(state.programming.get(session.id)).unwrap();
    let before_activity = before
        .as_object_mut()
        .unwrap()
        .remove("last_activity")
        .unwrap();
    let guest = Session {
        id: SessionId::new(),
        token: "undo-guest".into(),
        connected: true,
        capability: light_core::SurfaceCapability::PlaybackOnly,
        desk: session.desk.clone(),
    };
    state.programming.start(guest.id);
    state.sessions.insert_session(guest.clone());
    for (token, show_id, expected) in [
        ("invalid-token", show.clone(), StatusCode::UNAUTHORIZED),
        (guest.token.as_str(), show.clone(), StatusCode::FORBIDDEN),
        (
            token.as_str(),
            Uuid::new_v4().to_string(),
            StatusCode::CONFLICT,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v2/programmer-undo/actions")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header("x-tosk-show", show_id)
                    .header("x-tosk-desk", session.desk.id.to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{:?}", json(response).await);
    }
    write_desk_lock(
        &state,
        &DeskLockConfiguration {
            locked: true,
            ..Default::default()
        },
    )
    .unwrap();
    let locked = undo_http(&app, &session, &show).await;
    assert_eq!(locked.status(), StatusCode::CONFLICT);
    write_desk_lock(&state, &DeskLockConfiguration::default()).unwrap();
    let noop = undo_http(&app, &session, &show).await;
    assert_eq!(noop.status(), StatusCode::OK);
    assert_eq!(noop.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(json(noop).await["changed"], false);
    let mut after = serde_json::to_value(state.programming.get(session.id)).unwrap();
    let after_activity = after
        .as_object_mut()
        .unwrap()
        .remove("last_activity")
        .unwrap();
    assert!(
        chrono::DateTime::parse_from_rfc3339(after_activity.as_str().unwrap()).unwrap()
            >= chrono::DateTime::parse_from_rfc3339(before_activity.as_str().unwrap()).unwrap()
    );
    assert_eq!(after, before);
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn undo_http(app: &Router, session: &Session, show: &str) -> Response {
    app.clone()
        .oneshot(
            Request::get("/api/v2/programmer-undo/actions")
                .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
                .header("x-tosk-show", show)
                .header("x-tosk-desk", session.desk.id.to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}
