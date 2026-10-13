use super::*;

fn setup() -> (AppState, PathBuf, Session) {
    let (state, path) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "renewal-native".into(),
        connected: true,
        desk: state.installation.desk().unwrap(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());
    subscribe(&state, "renewal", "desk", "127.0.0.1:19541", 19542);
    (state, path, session)
}

fn subscribe(state: &AppState, client: &str, path: &str, source: &str, port: i32) {
    assert!(handle_subscription_osc(
        state,
        "/light/subscribe",
        &[
            OscArgument::String(client.into()),
            OscArgument::String(path.into()),
            OscArgument::Int(port),
        ],
        Some(source)
    ));
}

fn flash(state: &AppState, position: u16) -> bool {
    state
        .output
        .engine()
        .playback_runtime_status_at(light_playback::PlaybackAddress::new(1, position).unwrap())
        .is_some_and(|status| status.playback.flash)
}

fn seed_owned_controls(state: &AppState, session: &Session) {
    let ids = [
        light_core::CueListId::new(),
        light_core::CueListId::new(),
        light_core::CueListId::new(),
    ];
    let cue_lists =
        ids.iter()
            .map(|id| {
                serde_json::from_value::<light_playback::CueList>(serde_json::json!({
        "id":id,"name":"Renewal control scope","priority":0,"mode":"sequence","looped":true,
        "cues":[light_playback::Cue::new(light_playback::CueNumber::from(1_u8))]
    })).unwrap()
            })
            .collect::<Vec<_>>();
    let assignments = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut definition = light_playback::PlaybackDefinition::new_cue_list("Renewal", *id);
            definition.auto_off = false;
            definition.buttons[0] = if index == 1 {
                light_playback::PlaybackButtonAction::Go
            } else {
                light_playback::PlaybackButtonAction::Flash
            };
            (index as u16 + 1, definition)
        })
        .collect();
    let definition = schema_v2_direct_fixture().0;
    let fixture = definition.fixture_id;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![definition].into(),
            cue_lists: cue_lists.into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Renewal".into(),
                assignments,
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    state.programming.set(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.7),
    );
    subscribe(state, "peer", "desk", "127.0.0.1:19545", 19546);
    for (position, source) in [
        (1, "127.0.0.1:19541"),
        (2, "127.0.0.1:19541"),
        (3, "127.0.0.1:19545"),
    ] {
        assert!(handle_playback_osc(
            state,
            &format!("/light/desk/playback/1/{position}/button/1"),
            &[OscArgument::Int(1)],
            Some(source)
        ));
    }
    assert!(flash(state, 1) && flash(state, 3));
    assert!(
        state
            .output
            .engine()
            .permanent_playback_enabled(light_playback::PlaybackAddress::new(1, 2).unwrap())
    );
}

fn hold(state: &AppState) {
    for key in ["shift", "record"] {
        assert!(handle_programmer_osc(
            state,
            &format!("/light/desk/programmer/{key}"),
            &[OscArgument::Int(1)],
            Some("127.0.0.1:19541")
        ));
    }
}

#[test]
fn actual_subscription_heartbeat_preserves_long_shift_record_settings_gesture() {
    let (state, path, session) = setup();
    state
        .programming
        .set_command_line(session.id, "UPDATE".into());
    hold(&state);
    std::thread::sleep(Duration::from_millis(1300));
    subscribe(&state, "renewal", "desk", "127.0.0.1:19541", 19542);
    std::thread::sleep(Duration::from_millis(1300));
    assert!(handle_programmer_osc(
        &state,
        "/light/desk/programmer/record",
        &[OscArgument::Int(0)],
        Some("127.0.0.1:19541")
    ));
    let events = state.events.audit_events();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.kind == "update_settings_requested")
            .count(),
        1,
        "the real timed held gesture must survive a production subscription heartbeat"
    );
    assert_eq!(state.programming.get(session.id).unwrap().command_line, "");
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn same_live_subscription_refreshes_target_without_erasing_held_and_dedupe_state() {
    let (state, path, _) = setup();
    hold(&state);
    let mut before = state.integrations.osc_subscriber("renewal").unwrap();
    before.shifted = true;
    before.last_highlight_action = Some(("next".into(), Instant::now()));
    state
        .integrations
        .register_osc_subscriber("renewal".into(), before.clone());
    subscribe(&state, "renewal", "DESK", "127.0.0.1:19541", 19543);
    let after = state.integrations.osc_subscriber("renewal").unwrap();
    assert!(after.shift_held && after.shifted);
    assert_eq!(after.update_record_started, before.update_record_started);
    assert_eq!(after.update_first_release, before.update_first_release);
    assert_eq!(after.last_highlight_action, before.last_highlight_action);
    assert_eq!(after.session_id, before.session_id);
    assert_eq!(after.target.port(), 19543);
    assert!(after.last_seen >= before.last_seen);
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn replacement_unsubscribe_expiry_and_invalid_sessions_never_inherit_gesture_state() {
    for case in [
        "source",
        "path",
        "client",
        "expired",
        "unsubscribed",
        "missing-session",
        "inactive-session",
        "different-session",
        "wrong-desk-session",
        "capability",
    ] {
        let (state, path, session) = setup();
        hold(&state);
        let mut source = "127.0.0.1:19541";
        let mut alias = "desk";
        let mut client = "renewal";
        match case {
            "source" => source = "127.0.0.1:19544",
            "path" => alias = "remote",
            "client" => client = "new-client",
            "expired" => state
                .integrations
                .set_osc_last_seen("renewal", Instant::now() - Duration::from_secs(21)),
            "unsubscribed" => {
                assert!(handle_subscription_osc(
                    &state,
                    "/light/unsubscribe",
                    &[OscArgument::String("renewal".into())],
                    Some(source)
                ));
            }
            "missing-session" => {
                state.sessions.remove_session(session.id);
            }
            "inactive-session" => {
                let mut inactive = session.clone();
                inactive.connected = false;
                state.sessions.insert_session(inactive);
            }
            "wrong-desk-session" => {
                let mut other = session.clone();
                other.desk.id = Uuid::new_v4();
                state.sessions.insert_session(other);
            }
            "capability" => {
                let mut old = state.integrations.osc_subscriber("renewal").unwrap();
                old.capability = light_core::SurfaceCapability::PlaybackOnly;
                state
                    .integrations
                    .register_osc_subscriber("renewal".into(), old);
            }
            "different-session" => {
                let mut old = state.integrations.osc_subscriber("renewal").unwrap();
                old.session_id = SessionId::new();
                state
                    .integrations
                    .register_osc_subscriber("renewal".into(), old);
            }
            _ => unreachable!(),
        }
        subscribe(&state, client, alias, source, 19542);
        let fresh = state.integrations.osc_subscriber(client).unwrap();
        assert!(!fresh.shift_held && !fresh.shifted, "{case}");
        assert!(
            fresh.update_record_started.is_none() && fresh.update_first_release.is_none(),
            "{case}"
        );
        assert!(fresh.last_highlight_action.is_none(), "{case}");
        let _ = std::fs::remove_dir_all(path);
    }
}

#[test]
fn new_client_on_same_socket_cannot_release_old_held_gesture_or_erase_another_socket() {
    let (state, path, session) = setup();
    seed_owned_controls(&state, &session);
    let values = state.programming.get(session.id).unwrap().values;
    state
        .programming
        .set_command_line(session.id, "UPDATE".into());
    subscribe(&state, "peer", "desk", "127.0.0.1:19545", 19546);
    assert!(handle_programmer_osc(
        &state,
        "/light/desk/programmer/shift",
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19545")
    ));
    let peer = state.integrations.osc_subscriber("peer").unwrap();
    hold(&state);
    std::thread::sleep(Duration::from_millis(2600));
    subscribe(&state, "new-client", "desk", "127.0.0.1:19541", 19542);
    handle_programmer_osc(
        &state,
        "/light/desk/programmer/record",
        &[OscArgument::Int(0)],
        Some("127.0.0.1:19541"),
    );
    assert!(
        state.integrations.osc_subscriber("renewal").is_none(),
        "old source identity must be retired atomically"
    );
    assert_eq!(
        state
            .events
            .audit_events()
            .iter()
            .filter(|e| e.kind == "update_settings_requested")
            .count(),
        0
    );
    let fresh = state.integrations.osc_subscriber("new-client").unwrap();
    assert!(!fresh.shift_held && fresh.update_record_started.is_none());
    let after_peer = state.integrations.osc_subscriber("peer").unwrap();
    assert_eq!(after_peer.shift_held, peer.shift_held);
    assert_eq!(after_peer.last_seen, peer.last_seen);
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "UPDATE"
    );
    assert_eq!(state.programming.get(session.id).unwrap().values, values);
    assert!(
        !flash(&state, 1),
        "retired client loses only its held Flash"
    );
    assert!(
        flash(&state, 3),
        "another command socket keeps its held Flash"
    );
    assert!(
        state
            .output
            .engine()
            .permanent_playback_enabled(light_playback::PlaybackAddress::new(1, 2).unwrap()),
        "retirement cannot stop permanent Go"
    );
    let _ = std::fs::remove_dir_all(path);
}
