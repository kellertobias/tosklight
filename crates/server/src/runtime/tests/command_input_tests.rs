#[test]
fn command_backspace_removes_words_as_tokens_and_numbers_as_characters() {
    let mut value = "GROUP 1 THRU 6 AT 88".to_string();
    for expected in [
        "GROUP 1 THRU 6 AT 8",
        "GROUP 1 THRU 6 AT",
        "GROUP 1 THRU 6",
        "GROUP 1 THRU",
        "GROUP 1",
        "GROUP",
        "",
    ] {
        value = light_programmer::command_line::remove_command_token(&value);
        assert_eq!(value, expected);
    }
}

#[test]
fn osc_keypad_uses_the_same_scoped_selection_edits_as_the_ui() {
    use light_programmer::command_line::{CommandKeyIntent, CommandKeyPhase, command_key_intent};
    use light_programmer::{CommandLineState, CommandTarget};

    fn press(state: &mut CommandLineState, action: &str) {
        let key = command_http::osc_command_key(action).expect("known OSC keypad action");
        let CommandKeyIntent::Edit(edit) = command_key_intent(state, key, CommandKeyPhase::Press)
        else {
            panic!("expected OSC edit")
        };
        state.text = edit.text;
        state.target = edit.target;
        state.pristine = edit.pristine;
    }

    let mut state = CommandLineState::default();
    for action in ["grp", "digit-7", "plus", "digit-8"] {
        press(&mut state, action);
    }
    assert_eq!(state.visible_text(), "G7 + F8");

    let mut override_scope = CommandLineState {
        text: "G7 +".into(),
        target: CommandTarget::Fixture,
        pristine: false,
        revision: 0,
    };
    press(&mut override_scope, "grp");
    press(&mut override_scope, "digit-8");
    assert_eq!(override_scope.visible_text(), "G7 + G8");

    let mut group_scope = CommandLineState {
        text: "G7 +".into(),
        target: CommandTarget::Group,
        pristine: false,
        revision: 0,
    };
    press(&mut group_scope, "digit-8");
    assert_eq!(group_scope.visible_text(), "G7 + G8");
}

#[test]
fn osc_and_ui_share_the_unlocked_desk_command_context_not_the_user_session() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let (front, wing) = {
        let store = state.desk.lock();
        (
            store.add_desk("Front", "front").unwrap(),
            store.add_desk("Wing", "wing").unwrap(),
        )
    };
    let ui = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "front-ui".into(),
        connected: true,
        desk: front.clone(),
    };
    let second_front = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "front-second".into(),
        connected: true,
        desk: front.clone(),
    };
    let wing_ui = Session {
        id: SessionId::new(),
        user,
        token: "wing-ui".into(),
        connected: true,
        desk: wing,
    };
    for session in [&ui, &second_front, &wing_ui] {
        state.programmers.start(session.id, session.user.id);
        attach_session_command_context(&state, session);
        state.sessions.write().insert(session.id, session.clone());
    }
    state.programmers.set_command_line(ui.id, "GROUP".into());
    state.programmers.set_command_target(ui.id, "GROUP".into());

    write_desk_lock(
        &state,
        front.id,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();
    let source = "127.0.0.1:19010";
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/subscribe".into(),
            arguments: vec![
                OscArgument::String("front-hardware".into()),
                OscArgument::String("front".into()),
                OscArgument::Int(19011),
            ],
            source: Some(source.into()),
        },
    );
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/front/programmer/digit-7".into(),
            arguments: vec![OscArgument::Bool(true)],
            source: Some(source.into()),
        },
    );
    assert_eq!(state.programmers.get(ui.id).unwrap().command_line, "GROUP");

    write_desk_lock(&state, front.id, &DeskLockConfiguration::default()).unwrap();
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/front/programmer/digit-7".into(),
            arguments: vec![OscArgument::Bool(true)],
            source: Some(source.into()),
        },
    );
    assert_eq!(state.programmers.get(ui.id).unwrap().command_line, "G7");
    assert_eq!(
        state.programmers.get(second_front.id).unwrap().command_line,
        "G7"
    );
    assert!(
        state
            .programmers
            .get(wing_ui.id)
            .unwrap()
            .command_line
            .is_empty()
    );
    assert_eq!(state.programmers.command_target(wing_ui.id), "FIXTURE");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn file_input_context_follows_the_desk_not_the_shared_programmer_session() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let mut front = test_control_desk();
    front.id = Uuid::new_v4();
    front.osc_alias = "front".into();
    let mut wing = test_control_desk();
    wing.id = Uuid::new_v4();
    wing.osc_alias = "wing".into();
    let owner = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "owner".into(),
        connected: true,
        desk: front.clone(),
    };
    let same_desk_hardware = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "hardware".into(),
        connected: true,
        desk: front,
    };
    let different_desk = Session {
        id: SessionId::new(),
        user,
        token: "wing".into(),
        connected: true,
        desk: wing,
    };
    state.file_input_contexts.lock().insert(
        owner.desk.id,
        file_manager::FileInputContext {
            instance_id: "front-files".into(),
            action: file_manager::FileInputAction::Copy,
            session_id: owner.id,
            desk_id: owner.desk.id,
            expires_at: Instant::now() + Duration::from_secs(60),
        },
    );

    assert!(file_manager::route_osc_input(
        &state,
        &same_desk_hardware,
        "enter"
    ));
    assert!(!file_manager::route_osc_input(
        &state,
        &different_desk,
        "enter"
    ));
    assert!(
        state
            .file_input_contexts
            .lock()
            .contains_key(&owner.desk.id)
    );
    assert!(file_manager::route_osc_input(
        &state,
        &same_desk_hardware,
        "escape"
    ));
    assert!(state.file_input_contexts.lock().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn competing_file_input_context_claims_are_atomic() {
    let (state, data_dir) = test_state();
    let desk_id = Uuid::new_v4();
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for instance_id in ["files-left", "files-right"] {
            let state = state.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(scope.spawn(move || {
                let context = file_manager::FileInputContext {
                    instance_id: instance_id.into(),
                    action: file_manager::FileInputAction::Copy,
                    session_id: SessionId::new(),
                    desk_id,
                    expires_at: Instant::now() + Duration::from_secs(60),
                };
                barrier.wait();
                file_manager::try_claim_input_context(&state, context, || Ok(())).is_ok()
            }));
        }
        barrier.wait();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });

    assert_eq!(results.iter().filter(|claimed| **claimed).count(), 1);
    assert_eq!(state.file_input_contexts.lock().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn synthetic_osc_sessions_publish_start_and_removal_on_unsubscribe_and_timeout() {
    let (state, data_dir) = test_state();
    state
        .desk
        .lock()
        .add_desk("OSC lifecycle main", "osc-lifecycle-main")
        .unwrap();
    let subscribe = |client: &str| {
        assert!(handle_subscription_osc(
            &state,
            "/light/subscribe",
            &[
                OscArgument::String(client.into()),
                OscArgument::String("main".into()),
                OscArgument::Int(19_011),
            ],
            Some("127.0.0.1:19010"),
        ));
        state
            .osc_subscribers
            .lock()
            .get(client)
            .unwrap_or_else(|| panic!("subscriber {client} was not retained"))
            .session_id
    };

    let first = subscribe("lifecycle-unsubscribe");
    assert!(state.sessions.read().contains_key(&first));
    assert!(handle_subscription_osc(
        &state,
        "/light/unsubscribe",
        &[OscArgument::String("lifecycle-unsubscribe".into())],
        Some("127.0.0.1:19010"),
    ));
    assert!(!state.sessions.read().contains_key(&first));
    assert!(state.programmers.active_for_sessions().is_empty());

    let second = subscribe("lifecycle-timeout");
    state
        .osc_subscribers
        .lock()
        .get_mut("lifecycle-timeout")
        .unwrap()
        .last_seen = Instant::now() - Duration::from_secs(21);
    send_osc_feedback(&state, false);
    assert!(!state.sessions.read().contains_key(&second));
    assert!(!state
        .osc_subscribers
        .lock()
        .contains_key("lifecycle-timeout"));

    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_lifecycle());
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(0, &filter)
    else {
        panic!("synthetic session lifecycle events should remain replayable")
    };
    assert_eq!(events.len(), 4);
    for index in [1, 3] {
        assert!(matches!(
            events[index].payload,
            light_application::ApplicationEvent::Programming(
                light_application::ProgrammingEvent::LifecycleChanged(
                    light_application::ProgrammingLifecycleChange {
                        delta: light_application::ProgrammingLifecycleDelta::Remove { .. },
                        ..
                    }
                )
            )
        ));
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn synthetic_osc_resubscribe_reuses_an_orphan_session_without_transient_lifecycle_rows() {
    let (state, data_dir) = test_state();
    state
        .desk
        .lock()
        .add_desk("OSC lifecycle main", "osc-lifecycle-main")
        .unwrap();
    let second = state
        .desk
        .lock()
        .add_desk("OSC lifecycle second", "osc-lifecycle-second")
        .unwrap();
    let subscribe = |desk: &str| {
        assert!(handle_subscription_osc(
            &state,
            "/light/subscribe",
            &[
                OscArgument::String("lifecycle-replace".into()),
                OscArgument::String(desk.into()),
                OscArgument::Int(19_011),
            ],
            Some("127.0.0.1:19010"),
        ));
        state.osc_subscribers.lock()["lifecycle-replace"].session_id
    };

    let session_id = subscribe("main");
    let before = state.application_events.latest_sequence();
    assert_eq!(subscribe(&second.osc_alias), session_id);
    assert_eq!(state.application_events.latest_sequence(), before);
    assert_eq!(state.sessions.read().len(), 1);
    assert_eq!(state.programmers.active_for_sessions().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}
