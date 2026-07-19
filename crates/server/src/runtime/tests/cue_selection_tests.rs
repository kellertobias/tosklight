fn cue_selection_snapshot(list_id: light_core::CueListId) -> EngineSnapshot {
    let list = light_playback::CueList {
        id: list_id,
        name: "Shared".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![
            light_playback::Cue::new(1.0),
            light_playback::Cue::new(2.0),
            light_playback::Cue::new(3.0),
        ],
    };
    let definition = |number| light_playback::PlaybackDefinition {
        number,
        name: format!("Playback {number}"),
        target: light_playback::PlaybackTarget::CueList {
            cue_list_id: list_id,
        },
        buttons: [
            light_playback::PlaybackButtonAction::GoMinus,
            light_playback::PlaybackButtonAction::Go,
            light_playback::PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    EngineSnapshot {
        cue_lists: vec![list],
        playbacks: vec![definition(1), definition(2)],
        playback_pages: vec![light_playback::PlaybackPage {
            number: 4,
            name: "Page 4".into(),
            slots: HashMap::from([(7, 2)]),
        }],
        ..Default::default()
    }
}

#[test]
fn cue_commands_use_the_desk_selected_concrete_playback() {
    let (state, data_dir) = test_state();
    let (user, first_desk, second_desk) = {
        let store = state.desk.lock();
        let user = store.users().unwrap().remove(0);
        let first = store.add_desk("Front", "front").unwrap();
        let second = store.add_desk("Wing", "wing").unwrap();
        (user, first, second)
    };
    let show_id = light_core::ShowId::new();
    *state.active_show.write() = Some(ShowEntry {
        id: show_id,
        name: "Selection".into(),
        path: data_dir.join("selection.show").display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    });
    let list_id = light_core::CueListId::new();
    state
        .engine
        .replace_snapshot(cue_selection_snapshot(list_id))
        .unwrap();
    state
        .desk
        .lock()
        .set_selected_playback(first_desk.id, show_id, Some(1))
        .unwrap();
    state
        .desk
        .lock()
        .set_selected_playback(second_desk.id, show_id, Some(2))
        .unwrap();
    state
        .desk
        .lock()
        .set_desk_page(first_desk.id, show_id, 4)
        .unwrap();
    handle_playback_osc(
        &state,
        "/light/front/page-playback/7/select",
        &[OscArgument::Bool(true)],
        None,
    );
    assert_eq!(
        state
            .desk
            .lock()
            .selected_playback(first_desk.id, show_id)
            .unwrap(),
        Some(2)
    );
    state
        .desk
        .lock()
        .set_selected_playback(first_desk.id, show_id, Some(1))
        .unwrap();
    let first = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "first".into(),
        connected: true,
        desk: first_desk,
    };
    let second = Session {
        id: SessionId::new(),
        user,
        token: "second".into(),
        connected: true,
        desk: second_desk,
    };
    execute_cue_and_assert_typed_event(
        &state,
        &first,
        "CUE 2",
        light_application::ActionSource::Keyboard,
        1,
        Some(2.0),
        None,
    );
    execute_programmer_command(&state, &second, "CUE 3").unwrap();
    execute_cue_and_assert_typed_event(
        &state,
        &first,
        "CUE CUE 1",
        light_application::ActionSource::Osc,
        1,
        Some(2.0),
        Some(1.0),
    );
    let runtime = state.engine.playback_runtime();
    let first_runtime = runtime
        .iter()
        .find(|item| item.playback_number == Some(1))
        .unwrap();
    let second_runtime = runtime
        .iter()
        .find(|item| item.playback_number == Some(2))
        .unwrap();
    assert_eq!(
        (
            first_runtime.current_cue_number,
            first_runtime.loaded_cue_number
        ),
        (Some(2.0), Some(1.0))
    );
    assert_eq!(
        (
            second_runtime.current_cue_number,
            second_runtime.loaded_cue_number
        ),
        (Some(3.0), None)
    );
    execute_programmer_command(&state, &first, "CUE SET 2 CUE 1").unwrap();
    execute_programmer_command(&state, &first, "CUE CUE SET 4 . 7 CUE 2").unwrap();
    let second_runtime = state
        .engine
        .playback_runtime()
        .into_iter()
        .find(|item| item.playback_number == Some(2))
        .unwrap();
    assert_eq!(
        (
            second_runtime.current_cue_number,
            second_runtime.loaded_cue_number
        ),
        (Some(1.0), Some(2.0))
    );
    assert_eq!(
        state
            .desk
            .lock()
            .selected_playback(first.desk.id, show_id)
            .unwrap(),
        Some(1)
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

fn execute_cue_and_assert_typed_event(
    state: &AppState,
    session: &Session,
    command: &str,
    source: light_application::ActionSource,
    playback: u16,
    current: Option<f64>,
    loaded: Option<f64>,
) {
    let context = operator_action_context(session, source);
    let before = state.application_events.latest_sequence();
    execute_programmer_command_from(state, session, command, &context).unwrap();
    assert_eq!(state.application_events.latest_sequence(), before + 1);

    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(before, &light_application::EventFilter::default())
    else {
        panic!("expected retained Playback event");
    };
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.source, light_application::EventSource::Action(source));
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    let light_application::ApplicationEvent::Playback(
        light_application::PlaybackEvent::RuntimeChanged(change),
    ) = &event.payload
    else {
        panic!("expected typed Playback runtime event");
    };
    assert_eq!(change.projection.playback_number, Some(playback));
    let runtime = change.projection.cue_list_runtime().unwrap();
    assert_eq!(runtime.current.as_ref().map(|cue| cue.number), current);
    assert_eq!(runtime.loaded.as_ref().map(|cue| cue.number), loaded);
}

#[test]
fn fixture_selection_accepts_minus_before_subsetting() {
    let tokens = ["1", "THRU", "10", "-", "5", "DIV", "2"].map(String::from);
    assert!(parse_fixture_selection(&[], &tokens).is_ok());
    let malformed = ["-", "5"].map(String::from);
    assert_eq!(
        parse_fixture_selection(&[], &malformed).unwrap_err(),
        "- requires fixture selections on both sides"
    );
}

#[test]
fn bare_multi_head_selection_expands_to_children_and_steps_without_parent_identity() {
    let mut fixture = schema_v2_direct_fixture().0;
    fixture.fixture_number = Some(1);
    let parent = fixture.fixture_id;
    let first_head = light_core::FixtureId::new();
    let second_head = light_core::FixtureId::new();
    fixture.definition.heads = vec![
        light_fixture::LogicalHead {
            index: 0,
            name: "Master".into(),
            shared: true,
            parameters: Vec::new(),
        },
        light_fixture::LogicalHead {
            index: 1,
            name: "Cell 1".into(),
            shared: false,
            parameters: Vec::new(),
        },
        light_fixture::LogicalHead {
            index: 2,
            name: "Cell 2".into(),
            shared: false,
            parameters: Vec::new(),
        },
    ];
    fixture.logical_heads = vec![
        light_fixture::PatchedHead {
            profile_head_id: None,
            head_index: 1,
            fixture_id: first_head,
        },
        light_fixture::PatchedHead {
            profile_head_id: None,
            head_index: 2,
            fixture_id: second_head,
        },
    ];

    let expanded = parse_fixture_selection(&[fixture.clone()], &["1".into()]).unwrap();
    assert_eq!(expanded, vec![first_head, second_head]);
    assert_eq!(
        parse_fixture_selection(&[fixture.clone()], &["1".into(), ".".into(), "0".into()]).unwrap(),
        vec![parent],
        "only an explicit .0 address selects the master identity"
    );

    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let user = light_core::UserId::new();
    let fixtures = highlight_fixture_summaries(&[fixture]);
    let complete = light_programmer::ProgrammerSelection {
        selected: expanded,
        expression: Some(light_programmer::SelectionExpression::Static),
        revision: 1,
        gesture_open: false,
    };
    let first = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Next,
            &complete,
            &fixtures,
            &HashMap::new(),
            false,
        )
        .unwrap();
    assert_eq!(
        first.working_selection.as_ref().unwrap().selected,
        vec![first_head]
    );
    let stepped = light_programmer::ProgrammerSelection {
        selected: vec![first_head],
        expression: Some(light_programmer::SelectionExpression::Static),
        revision: 2,
        gesture_open: false,
    };
    registry.acknowledge_internal_selection(desk, user, &stepped);
    let second = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Next,
            &stepped,
            &fixtures,
            &HashMap::new(),
            false,
        )
        .unwrap();
    assert_eq!(
        second.working_selection.as_ref().unwrap().selected,
        vec![second_head]
    );
    assert!(
        !second
            .state
            .remembered
            .iter()
            .any(|item| item.fixture_id == parent)
    );
}

#[test]
fn authoritative_selection_surfaces_expand_a_multi_head_parent_to_child_rows() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "multi-head-selection".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());
    let (fixture, children) = highlight_multi_head_fixture();
    let parent = fixture.fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            ..EngineSnapshot::default()
        })
        .unwrap();

    let set = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "multi-head-set".into(),
            session_id: session.id,
            expected_revision: None,
            command: "selection.set".into(),
            payload: serde_json::json!({"fixtures":[parent]}),
        },
    );
    assert!(set.ok, "{:?}", set.error);
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        children
    );

    state.programmers.select(session.id, []);
    let gesture = dispatch_ws_command(
        &state,
        &session,
        WsCommand {
            protocol_version: 1,
            request_id: "multi-head-gesture".into(),
            session_id: session.id,
            expected_revision: None,
            command: "selection.gesture".into(),
            payload: serde_json::json!({
                "source":{"type":"fixture","fixture_id":parent}
            }),
        },
    );
    assert!(gesture.ok, "{:?}", gesture.error);
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        children
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
