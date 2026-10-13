use super::*;

fn setup() -> (AppState, PathBuf, Session, Session) {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "osc-record-interaction".into(),
        connected: true,
        desk: test_control_desk(),
    };
    let peer = Session {
        id: SessionId::new(),
        token: "same-desk-peer".into(),
        ..session.clone()
    };
    for actor in [&session, &peer] {
        state.programming.start(actor.id);
        attach_session_command_context(&state, actor);
        state.sessions.insert_session(actor.clone());
    }
    let source: SocketAddr = "127.0.0.1:9027".parse().unwrap();
    state.integrations.register_osc_subscriber(
        "record-interaction".into(),
        OscSubscriber {
            capability: light_core::SurfaceCapability::Programming,
            path: "desk".into(),
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
    (state, data_dir, session, peer)
}

fn send(state: &AppState, key: &str, pressed: bool) {
    assert!(handle_programmer_osc(
        state,
        &format!("/light/desk/programmer/{key}"),
        &[OscArgument::Bool(pressed)],
        Some("127.0.0.1:9027")
    ));
}

fn assert_command_event(
    state: &AppState,
    session: &Session,
    after: u64,
    text: &str,
    revision: u64,
) {
    let filter = light_application::EventFilter::for_desk(session.desk.id).with_object(
        light_application::EventObject::programming_command_line(session.desk.id),
    );
    let light_application::EventReplay::Events(events) = state.events.replay(after, &filter) else {
        panic!("expected replayable scoped command event");
    };
    assert_eq!(
        events.len(),
        1,
        "the actual gesture must publish without another key or GUI echo"
    );
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::InteractionChanged(change),
    ) = &events[0].payload
    else {
        panic!("expected typed Programming interaction");
    };
    let command = change.command_line().expect("changed command line");
    assert_eq!(command.visible_text(), text);
    assert_eq!(command.revision, revision);
    assert!(change.selection().is_none());
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Osc)
    );
    let other_desk = Uuid::new_v4();
    let other_filter = light_application::EventFilter::for_desk(other_desk).with_object(
        light_application::EventObject::programming_command_line(other_desk),
    );
    let light_application::EventReplay::Events(other) = state.events.replay(after, &other_filter)
    else {
        panic!("peer desk replay");
    };
    assert!(other.is_empty(), "another desk cannot observe this change");
}

#[test]
fn actual_shift_record_arm_publishes_command_without_gui_echo_or_next_key() {
    let (state, data_dir, session, peer) = setup();
    let revision = state
        .programming
        .command_line_state(session.id)
        .unwrap()
        .revision;
    let sequence = state.events.latest_sequence();
    send(&state, "shift", true);
    send(&state, "record", true);
    send(&state, "record", false);
    assert_eq!(
        state.programming.get(session.id).unwrap().command_line,
        "UPDATE"
    );
    assert_eq!(
        state.programming.get(peer.id).unwrap().command_line,
        "UPDATE"
    );
    assert_command_event(&state, &session, sequence, "UPDATE", revision + 1);
    let after = state.events.latest_sequence();
    send(&state, "record", false);
    assert_eq!(
        state.events.latest_sequence(),
        after,
        "unpaired release is not a replayed arm"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn actual_shift_record_settings_publishes_reset_without_gui_echo() {
    let (state, data_dir, session, peer) = setup();
    send(&state, "shift", true);
    send(&state, "record", true);
    send(&state, "record", false);
    let revision = state
        .programming
        .command_line_state(session.id)
        .unwrap()
        .revision;
    let sequence = state.events.latest_sequence();
    send(&state, "record", true);
    state.integrations.set_osc_record_started(
        "record-interaction",
        Instant::now() - Duration::from_millis(3000),
    );
    send(&state, "record", false);
    assert_eq!(state.programming.get(session.id).unwrap().command_line, "");
    assert_eq!(state.programming.get(peer.id).unwrap().command_line, "");
    assert_command_event(&state, &session, sequence, "FIXTURE", revision + 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn locked_actual_shift_record_does_not_mutate_or_publish_command() {
    let (state, data_dir, session, _) = setup();
    let before = state.programming.command_line_state(session.id).unwrap();
    let sequence = state.events.latest_sequence();
    let configuration = DeskLockConfiguration {
        locked: true,
        ..DeskLockConfiguration::default()
    };
    write_desk_lock(&state, &configuration).unwrap();
    for (key, pressed) in [("shift", true), ("record", true), ("record", false)] {
        assert!(!handle_programmer_osc(
            &state,
            &format!("/light/desk/programmer/{key}"),
            &[OscArgument::Bool(pressed)],
            Some("127.0.0.1:9027")
        ));
    }
    assert_eq!(
        state.programming.command_line_state(session.id).unwrap(),
        before
    );
    assert_eq!(state.events.latest_sequence(), sequence);
    let _ = std::fs::remove_dir_all(data_dir);
}
