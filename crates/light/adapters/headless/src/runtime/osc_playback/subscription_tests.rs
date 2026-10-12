use super::*;
use light_core::SurfaceCapability;
use light_playback::{Cue, CueNumber, PlaybackDefinition, PlaybackPage};

fn setup() -> (AppState, std::path::PathBuf, Session) {
    let (state, path) = crate::runtime::tests::test_state();
    let id = light_core::CueListId::new();
    let cue_list = serde_json::from_value::<light_playback::CueList>(serde_json::json!({
        "id":id,"name":"Lease test","priority":0,"mode":"sequence","looped":true,
        "cues":[Cue::new(CueNumber::from(1_u8))]
    }))
    .unwrap();
    let mut definition = PlaybackDefinition::new_cue_list("Lease", id);
    definition.auto_off = false;
    definition.buttons[0] = light_playback::PlaybackButtonAction::Go;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list].into(),
            playback_pages: vec![PlaybackPage {
                number: 1,
                name: "Lease".into(),
                assignments: [(1, definition)].into(),
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    let session = Session {
        capability: SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "lease-unit".into(),
        connected: true,
        desk: state.installation.desk().unwrap(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    (state, path, session)
}
fn subscribe(state: &AppState, session: &Session, capability: SurfaceCapability, path: &str) {
    let source: SocketAddr = "127.0.0.1:19331".parse().unwrap();
    state.integrations.register_osc_subscriber(
        "lease-unit".into(),
        OscSubscriber {
            capability,
            path: path.into(),
            target: source,
            command_source: source,
            session_id: session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
}
fn enabled(state: &AppState) -> bool {
    state
        .output
        .engine()
        .permanent_playback_enabled(light_playback::PlaybackAddress::new(1, 1).unwrap())
}
#[tokio::test]
async fn unknown_and_malformed_surface_sources_cannot_inherit_native_session_or_record() {
    let (state, path, session) = setup();
    state
        .programming
        .set_command_line(session.id, "RECORD".into());
    let before = state.output.snapshot();
    let page = state.installation.command_page(session.desk.id).unwrap();
    for source in ["127.0.0.1:19331", "not-a-socket"] {
        assert!(
            osc_playback_session(&state, Some(source), "main", Some(&session.desk)).is_err(),
            "unknown/malformed surface source cannot inherit native session"
        );
        assert!(!handle_playback_osc(
            &state,
            "/light/main/playback/1/1/button/1",
            &[OscArgument::Int(1)],
            Some(source)
        ));
        assert!(!enabled(&state));
        assert_eq!(
            state.programming.get(session.id).unwrap().command_line,
            "RECORD"
        );
        assert_eq!(state.output.snapshot().cue_lists, before.cue_lists);
        handle_playback_osc(
            &state,
            "/light/main/page",
            &[OscArgument::Int(2)],
            Some(source),
        );
        assert_eq!(
            state.installation.command_page(session.desk.id).unwrap(),
            page
        );
    }
    std::fs::remove_dir_all(path).unwrap();
}
#[tokio::test]
async fn unsubscribed_source_cannot_restart_but_unsubscribe_preserves_permanent_owner() {
    let (state, path, session) = setup();
    subscribe(&state, &session, SurfaceCapability::PlaybackOnly, "remote");
    let route = "/light/remote/playback/1/1/button/1";
    assert!(handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19331")
    ));
    assert!(enabled(&state));
    assert!(crate::runtime::handle_subscription_osc(
        &state,
        "/light/unsubscribe",
        &[OscArgument::String("lease-unit".into())],
        Some("127.0.0.1:19331")
    ));
    assert!(
        enabled(&state),
        "unsubscribe preserves intentional permanent activation"
    );
    state
        .output
        .engine()
        .execute_playback(EnginePlaybackCommand::Playback {
            address: light_playback::PlaybackAddress::new(1, 1).unwrap(),
            action: light_engine::PlaybackAction::Off,
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "internal-unit".into(),
                control_id: 1,
            },
        })
        .unwrap();
    assert!(!enabled(&state));
    assert!(!handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19331")
    ));
    assert!(
        !enabled(&state),
        "old socket cannot restart after unsubscribe"
    );
    std::fs::remove_dir_all(path).unwrap();
}
#[tokio::test]
async fn expired_missing_inactive_and_wrong_path_subscribers_are_rejected() {
    let (state, path, session) = setup();
    for condition in ["expired", "missing-session", "inactive", "wrong-path"] {
        subscribe(&state, &session, SurfaceCapability::Programming, "desk");
        match condition {
            "expired" => state
                .integrations
                .set_osc_last_seen("lease-unit", Instant::now() - Duration::from_secs(21)),
            "missing-session" => {
                let mut subscriber = state.integrations.osc_subscriber("lease-unit").unwrap();
                subscriber.session_id = SessionId::new();
                state
                    .integrations
                    .register_osc_subscriber("lease-unit".into(), subscriber);
            }
            "inactive" => {
                let mut inactive = session.clone();
                inactive.connected = false;
                state.sessions.insert_session(inactive);
            }
            _ => {}
        }
        let alias = if condition == "wrong-path" {
            "remote"
        } else {
            "desk"
        };
        assert!(
            osc_playback_session(&state, Some("127.0.0.1:19331"), alias, Some(&session.desk))
                .is_err(),
            "{condition}"
        );
        assert!(!handle_playback_osc(
            &state,
            &format!("/light/{alias}/playback/1/1/go"),
            &[OscArgument::Int(1)],
            Some("127.0.0.1:19331")
        ));
        assert!(!enabled(&state));
        state.sessions.insert_session(session.clone());
    }
    std::fs::remove_dir_all(path).unwrap();
}
#[tokio::test]
async fn registered_guest_top_button_one_bypasses_native_record_while_private_none_is_retained() {
    let (state, path, session) = setup();
    subscribe(&state, &session, SurfaceCapability::PlaybackOnly, "remote");
    state
        .programming
        .set_command_line(session.id, "RECORD".into());
    assert!(handle_playback_osc(
        &state,
        "/light/remote/playback/1/1/button/1",
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19331")
    ));
    assert!(enabled(&state));
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "RECORD"
    );
    assert!(!osc_source_may_program(
        &state,
        Some("127.0.0.1:19331".parse().unwrap())
    ));
    state
        .programming
        .set_command_line(session.id, String::new());
    assert!(
        osc_playback_session(&state, None, "main", Some(&session.desk)).is_ok_and(|s| s.is_some())
    );
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/1/1/off",
        &[OscArgument::Int(1)],
        None
    ));
    assert!(!enabled(&state));
    subscribe(&state, &session, SurfaceCapability::Programming, "desk");
    assert!(osc_source_may_program(
        &state,
        Some("127.0.0.1:19331".parse().unwrap())
    ));
    assert!(handle_playback_osc(
        &state,
        "/light/desk/playback/1/1/go",
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19331")
    ));
    assert!(enabled(&state));
    std::fs::remove_dir_all(path).unwrap();
}

async fn queued_lease_loss(condition: &'static str) {
    let (state, path, session) = setup();
    subscribe(&state, &session, SurfaceCapability::Programming, "desk");
    state
        .programming
        .set_command_line(session.id, "RECORD".into());
    let before = state.output.snapshot();
    let writer = state.active_show.acquire().await;
    let worker_state = state.clone();
    let worker_session = session.clone();
    let (ready, started) = std::sync::mpsc::channel();
    let worker = tokio::task::spawn_blocking(move || {
        let admission = OscInputAdmission::capture(&worker_state, None);
        let lease = OscPlaybackLease {
            source: Some("127.0.0.1:19331".into()),
            alias: "desk".into(),
            session_id: Some(worker_session.id),
            may_program: true,
        };
        ready.send(()).unwrap();
        let result = dispatch_osc_input(OscDispatch {
            state: &worker_state,
            session: Some(&worker_session),
            desk: Some(&worker_session.desk),
            address: PlaybackAddress::Playback(light_playback::PlaybackAddress::new(1, 1).unwrap()),
            action: "button",
            input: &mut PoolPlaybackInput {
                pressed: Some(true),
                button: Some(1),
                ..Default::default()
            },
            control_source: light_playback::PlaybackControlSource {
                surface_id: "lease-test".into(),
                control_id: 1,
            },
            suppression_input: None,
            record_target: true,
            may_program: true,
            captured_release: false,
            lease: Some(&lease),
            admission: &admission,
        });
        matches!(result, OscDispatchResult::Rejected)
    });
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    match condition {
        "removed" => {
            state.integrations.unregister_osc_subscriber("lease-unit");
        }
        "expired" => state
            .integrations
            .set_osc_last_seen("lease-unit", Instant::now() - Duration::from_secs(21)),
        "replacement" => {
            let mut replacement = session.clone();
            replacement.id = SessionId::new();
            state.sessions.insert_session(replacement.clone());
            subscribe(&state, &replacement, SurfaceCapability::Programming, "desk");
        }
        "capability" => subscribe(&state, &session, SurfaceCapability::PlaybackOnly, "desk"),
        _ => unreachable!(),
    }
    drop(writer);
    assert!(
        worker.await.unwrap(),
        "queued {condition} lease must be rejected before Record interception"
    );
    assert!(!enabled(&state));
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "RECORD"
    );
    assert_eq!(state.output.snapshot().cue_lists, before.cue_lists);
    std::fs::remove_dir_all(path).unwrap();
}
#[tokio::test]
async fn queued_unsubscribe_cannot_intercept_record() {
    queued_lease_loss("removed").await;
}
#[tokio::test]
async fn queued_expiry_cannot_intercept_record() {
    queued_lease_loss("expired").await;
}
#[tokio::test]
async fn queued_session_replacement_cannot_intercept_record() {
    queued_lease_loss("replacement").await;
}
#[tokio::test]
async fn queued_guest_replacement_cannot_inherit_programming() {
    queued_lease_loss("capability").await;
}

#[tokio::test]
async fn reserved_mappings_require_a_lease_but_ordinary_mappings_remain_supported() {
    let (state, path, session) = setup();
    let reserved = "/light/remote/playback/1/1/go";
    let mut snapshot = (*state.output.snapshot()).clone();
    snapshot.control_mappings = vec![
        light_control::ControlMapping {
            name: "reserved".into(),
            enabled: true,
            trigger: light_control::ControlTrigger::Osc {
                address: reserved.into(),
            },
            action: ControlAction::GrandMaster { level: 0.75 },
        },
        light_control::ControlMapping {
            name: "ordinary".into(),
            enabled: true,
            trigger: light_control::ControlTrigger::Osc {
                address: "/custom/level".into(),
            },
            action: ControlAction::GrandMaster { level: 0.35 },
        },
    ]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();
    let send = |address: &str| {
        crate::runtime::handle_control_event(
            &state,
            ControlEvent::Osc {
                address: address.into(),
                arguments: vec![OscArgument::Int(1)],
                source: Some("127.0.0.1:19331".into()),
            },
        )
    };
    send(reserved);
    assert_eq!(state.output.control_projection().grand_master, 1.0);
    assert!(!enabled(&state));
    send("/custom/level");
    assert_eq!(state.output.control_projection().grand_master, 0.35);
    subscribe(&state, &session, SurfaceCapability::PlaybackOnly, "remote");
    send(reserved);
    assert_eq!(state.output.control_projection().grand_master, 0.75);
    assert!(enabled(&state));
    std::fs::remove_dir_all(path).unwrap();
}

async fn open_page_show(state: &AppState, session: &Session) {
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let app = router(state.clone());
    let request = |action: serde_json::Value| {
        Request::post("/api/v2/shows")
            .header("authorization", format!("Bearer {}", session.token))
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"request_id":Uuid::new_v4(),"action":action}).to_string(),
            ))
            .unwrap()
    };
    let created=app.clone().oneshot(request(serde_json::json!({"type":"create","name":"Queued page lease","data_base64":null,"overwrite":false}))).await.unwrap();
    assert_eq!(created.status(), axum::http::StatusCode::OK);
    let created: serde_json::Value =
        serde_json::from_slice(&created.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let id = created["result"]["show"]["id"].as_str().unwrap();
    let opened=app.clone().oneshot(request(serde_json::json!({"type":"open","show_id":id,"transition":"hold_current","transition_millis":null}))).await.unwrap();
    assert_eq!(opened.status(), axum::http::StatusCode::OK);
    let collection = app
        .clone()
        .oneshot(
            Request::get("/api/v2/objects/playback_page")
                .header("authorization", format!("Bearer {}", session.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(collection.status(), axum::http::StatusCode::OK);
    let collection: serde_json::Value =
        serde_json::from_slice(&collection.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    let configured=app.oneshot(Request::post("/api/v2/playback-topology/actions")
        .header("authorization",format!("Bearer {}",session.token)).header("content-type","application/json")
        .header("if-match",collection["show_revision"].as_u64().unwrap().to_string())
        .body(Body::from(serde_json::json!({"request_id":Uuid::new_v4(),"action":{"type":"assign_playback","address":{"page":1,"position":4},"expected_page_revision":0,"expected_page_object_id":null,"definition":{"name":"Page occupied","target":{"type":"grand_master"},"buttons":["blackout","pause_dynamics","flash"],"button_count":3,"fader":"master","has_fader":true,"go_activates":true,"auto_off":true,"xfade_millis":0,"color":"#20c997","flash_release":"release_all","protect_from_swap":false,"presentation_icon":null,"presentation_image":null}}}).to_string())).unwrap()).await.unwrap();
    let status = configured.status();
    let body = configured.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "canonical page setup: {}",
        String::from_utf8_lossy(&body)
    );
}
#[tokio::test]
async fn queued_page_unsubscribe_cannot_change_page_or_create_page_object() {
    let (state, path, session) = setup();
    open_page_show(&state, &session).await;
    subscribe(&state, &session, SurfaceCapability::Programming, "desk");
    handle_playback_osc(
        &state,
        "/light/desk/page",
        &[OscArgument::Int(2)],
        Some("127.0.0.1:19331"),
    );
    assert_eq!(
        state.installation.command_page(session.desk.id).unwrap(),
        2,
        "positive page route reaches real show authority"
    );
    let before = state.output.snapshot();
    let lease =
        capture_playback_source(&state, "/light/desk/page", Some("127.0.0.1:19331")).unwrap();
    let writer = state.active_show.acquire().await;
    let worker_state = state.clone();
    let desk = session.desk.clone();
    let (ready, started) = std::sync::mpsc::channel();
    let worker = tokio::task::spawn_blocking(move || {
        let admission = OscInputAdmission::capture(&worker_state, None);
        ready.send(()).unwrap();
        admit_playback_route(
            &worker_state,
            "/light/desk/page",
            &["light", "desk", "page"],
            &[OscArgument::Int(1)],
            &admission,
            &lease,
            Some(&desk),
        );
    });
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    state.integrations.unregister_osc_subscriber("lease-unit");
    drop(writer);
    worker.await.unwrap();
    assert_eq!(state.installation.command_page(session.desk.id).unwrap(), 2);
    assert_eq!(
        state.output.snapshot().playback_pages,
        before.playback_pages
    );
    std::fs::remove_dir_all(path).unwrap();
}
#[tokio::test]
async fn queued_reserved_mapping_cannot_transfer_to_a_replacement_session() {
    let (state, path, session) = setup();
    subscribe(&state, &session, SurfaceCapability::Programming, "desk");
    let route = "/light/desk/playback/1/1/go";
    let mut snapshot = (*state.output.snapshot()).clone();
    snapshot.control_mappings = vec![light_control::ControlMapping {
        name: "reserved queued".into(),
        enabled: true,
        trigger: light_control::ControlTrigger::Osc {
            address: route.into(),
        },
        action: ControlAction::GrandMaster { level: 0.35 },
    }]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();
    let lease = capture_playback_source(&state, route, Some("127.0.0.1:19331")).unwrap();
    let writer = state.active_show.acquire().await;
    let worker_state = state.clone();
    let (ready, started) = std::sync::mpsc::channel();
    let worker = tokio::task::spawn_blocking(move || {
        let admission = OscInputAdmission::capture(&worker_state, None);
        ready.send(()).unwrap();
        crate::runtime::control_inputs::apply_admitted_control_mappings(
            &worker_state,
            ControlEvent::Osc {
                address: route.into(),
                arguments: vec![OscArgument::Int(1)],
                source: Some("127.0.0.1:19331".into()),
            },
            &admission,
            Some(&lease),
        );
    });
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut replacement = session.clone();
    replacement.id = SessionId::new();
    state.sessions.insert_session(replacement.clone());
    subscribe(&state, &replacement, SurfaceCapability::Programming, "desk");
    drop(writer);
    worker.await.unwrap();
    assert_eq!(state.output.control_projection().grand_master, 1.0);
    std::fs::remove_dir_all(path).unwrap();
}
