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

    let mut double_group = CommandLineState::default();
    press(&mut double_group, "grp");
    press(&mut double_group, "grp");
    assert_eq!(double_group.visible_text(), "DEGROUP");
    press(&mut double_group, "digit-7");
    assert_eq!(double_group.visible_text(), "DEGROUP 7");

    let mut override_scope = CommandLineState {
        text: "G7 +".into(),
        target: CommandTarget::Fixture,
        pristine: false,
        revision: 0,
        pending_choice: None,
    };
    press(&mut override_scope, "grp");
    press(&mut override_scope, "digit-8");
    assert_eq!(override_scope.visible_text(), "G7 + G8");

    let mut group_scope = CommandLineState {
        text: "G7 +".into(),
        target: CommandTarget::Group,
        pristine: false,
        revision: 0,
        pending_choice: None,
    };
    press(&mut group_scope, "digit-8");
    assert_eq!(group_scope.visible_text(), "G7 + G8");
}

#[test]
fn osc_and_ui_share_the_desks_one_command_line() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let front = state.installation.add_desk("Front", "front").unwrap();
    let wing = state.installation.add_desk("Wing", "wing").unwrap();
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
        state.programming.start(session.id, session.user.id);
        attach_session_command_context(&state, session);
        state.sessions.insert_session(session.clone());
    }
    state.programming.set_command_line(ui.id, "GROUP".into());
    state.programming.set_command_target(ui.id, "GROUP".into());

    write_desk_lock(&state, &DeskLockConfiguration {
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
    assert_eq!(state.programming.get(ui.id).unwrap().command_line, "GROUP");

    write_desk_lock(&state, &DeskLockConfiguration::default()).unwrap();
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/front/programmer/digit-7".into(),
            arguments: vec![OscArgument::Bool(true)],
            source: Some(source.into()),
        },
    );
    // One desk, one command line: every surface shows what was typed on any of them, including
    // one that logged in on a legacy second desk record.
    for session in [&ui, &second_front, &wing_ui] {
        assert_eq!(state.programming.get(session.id).unwrap().command_line, "G7");
        assert_eq!(state.programming.command_target(session.id), "GROUP");
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn osc_keypad_continues_the_shared_desk_command_line_and_lands_the_spread_once() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let front = state.installation.add_desk("Front", "front").unwrap();
    let ui = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "front-ui".into(),
        connected: true,
        desk: front.clone(),
    };
    let second_ui = Session {
        id: SessionId::new(),
        user,
        token: "front-second".into(),
        connected: true,
        desk: front.clone(),
    };
    for session in [&ui, &second_ui] {
        state.programming.start(session.id, session.user.id);
        attach_session_command_context(&state, session);
        state.sessions.insert_session(session.clone());
    }

    // Five patched fixtures, deliberately stored in reverse order: the spread must follow the
    // fixture-number selection order 1..5, not the engine snapshot's storage order.
    let template = schema_v2_direct_fixture().0;
    let mut fixtures = Vec::new();
    let mut ids = HashMap::new();
    for number in (1..=5_u32).rev() {
        let mut fixture = template.clone();
        fixture.fixture_id = light_core::FixtureId::new();
        fixture.fixture_number = Some(number);
        fixture.address = Some(1 + (number as u16 - 1) * 8);
        ids.insert(number, fixture.fixture_id);
        fixtures.push(fixture);
    }
    state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    let source = "127.0.0.1:19020";
    handle_control_event(
        &state,
        ControlEvent::Osc {
            address: "/light/subscribe".into(),
            arguments: vec![
                OscArgument::String("front-hardware".into()),
                OscArgument::String("front".into()),
                OscArgument::Int(19021),
            ],
            source: Some(source.into()),
        },
    );
    let press = |action: &str| {
        handle_control_event(
            &state,
            ControlEvent::Osc {
                address: format!("/light/front/programmer/{action}"),
                arguments: vec![OscArgument::Bool(true)],
                source: Some(source.into()),
            },
        );
    };
    for action in [
        "digit-1", "thru", "digit-5", "at", "digit-1", "digit-0", "digit-0", "thru", "digit-0",
        "thru", "digit-1", "digit-0", "digit-0",
    ] {
        press(action);
    }
    // The physical keypad continues the one shared desk command line: every UI session on the
    // same desk sees the identical text a software keypad would have produced.
    let typed = "F1 THRU 5 AT 100 THRU 0 THRU 100";
    assert_eq!(state.programming.get(ui.id).unwrap().command_line, typed);
    assert_eq!(
        state.programming.get(second_ui.id).unwrap().command_line,
        typed
    );

    press("enter");
    assert!(state.programming.get(ui.id).unwrap().command_line.is_empty());
    assert!(
        state
            .programming
            .get(second_ui.id)
            .unwrap()
            .command_line
            .is_empty()
    );

    // Normative resolve_spread result for `100 THRU 0 THRU 100` over five fixtures.
    let programmer = state.programming.get(ui.id).unwrap();
    for (number, level) in [(1, 1.0), (2, 0.5), (3, 0.0), (4, 0.5), (5, 1.0)] {
        let fixture_id = ids[&number];
        let values = programmer
            .values
            .iter()
            .filter(|value| {
                value.fixture_id == fixture_id
                    && value.attribute == light_core::AttributeKey::intensity()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            values.len(),
            1,
            "fixture {number} must hold exactly one programmer intensity"
        );
        assert_eq!(
            values[0].value.normalized(),
            Some(level),
            "fixture {number} must land the normative spread level"
        );
    }

    // The mutation landed exactly once: one values event and one accepted history entry.
    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_values(ui.user.id.0));
    let light_application::EventReplay::Events(events) = state.events.replay(0, &filter)
    else {
        panic!("the values event history should remain replayable")
    };
    assert_eq!(events.len(), 1);
    let executed = state
        .programming
        .command_history(front.id)
        .into_iter()
        .filter(|entry| entry.command == typed)
        .collect::<Vec<_>>();
    assert_eq!(
        executed.len(),
        1,
        "the OSC keypad command must be recorded exactly once"
    );
    assert_eq!(executed[0].status, "accepted");
    assert_eq!(executed[0].source, "osc");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn file_input_context_follows_the_desk_not_the_shared_programmer_session() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
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
    state
        .sessions
        .try_claim_file_input_context(
            file_manager::FileInputContext {
                instance_id: "front-files".into(),
                action: file_manager::FileInputAction::Copy,
                session_id: owner.id,
                desk_id: owner.desk.id,
                expires_at: Instant::now() + Duration::from_secs(60),
            },
            || Ok(()),
        )
        .unwrap();

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
            .sessions
            .file_input_context(owner.desk.id)
            .is_some()
    );
    assert!(file_manager::route_osc_input(
        &state,
        &same_desk_hardware,
        "escape"
    ));
    assert_eq!(state.sessions.file_input_context_count(), 0);
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
    assert_eq!(state.sessions.file_input_context_count(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn synthetic_osc_sessions_publish_start_and_removal_on_unsubscribe_and_timeout() {
    let (state, data_dir) = test_state();
    state
        .installation.add_desk("OSC lifecycle main", "osc-lifecycle-main")
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
            .integrations
            .osc_subscriber(client)
            .unwrap_or_else(|| panic!("subscriber {client} was not retained"))
            .session_id
    };

    let first = subscribe("lifecycle-unsubscribe");
    assert!(state.sessions.contains_session(first));
    assert!(handle_subscription_osc(
        &state,
        "/light/unsubscribe",
        &[OscArgument::String("lifecycle-unsubscribe".into())],
        Some("127.0.0.1:19010"),
    ));
    assert!(!state.sessions.contains_session(first));
    assert!(state.programming.active_for_sessions().is_empty());

    let second = subscribe("lifecycle-timeout");
    state.integrations.set_osc_last_seen(
        "lifecycle-timeout",
        Instant::now() - Duration::from_secs(21),
    );
    send_osc_feedback(&state, false);
    assert!(!state.sessions.contains_session(second));
    assert!(state
        .integrations
        .osc_subscriber("lifecycle-timeout")
        .is_none());

    let filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_lifecycle());
    let light_application::EventReplay::Events(events) =
        state.events.replay(0, &filter)
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
        .installation.add_desk("OSC lifecycle main", "osc-lifecycle-main")
        .unwrap();
    let second = state
        .installation.add_desk("OSC lifecycle second", "osc-lifecycle-second")
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
        state
            .integrations
            .osc_subscriber("lifecycle-replace")
            .unwrap()
            .session_id
    };

    let session_id = subscribe("main");
    let before = state.events.latest_sequence();
    assert_eq!(subscribe(&second.osc_alias), session_id);
    let lifecycle_filter = light_application::EventFilter::default()
        .with_object(light_application::EventObject::programming_lifecycle());
    let light_application::EventReplay::Events(lifecycle_events) =
        state.events.replay(before, &lifecycle_filter)
    else {
        panic!("the focused Programmer lifecycle history must remain replayable")
    };
    assert!(lifecycle_events.is_empty());
    assert_eq!(state.sessions.session_count(), 1);
    assert_eq!(state.programming.active_for_sessions().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}
