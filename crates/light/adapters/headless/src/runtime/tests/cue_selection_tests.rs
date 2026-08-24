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
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![
            light_playback::Cue::new(cue("1")),
            light_playback::Cue::new(cue("2")),
            light_playback::Cue::new(cue("3")),
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
        footprint: light_playback::PlaybackFootprint::Normal,
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
        cue_lists: vec![list].into(),
        playbacks: vec![definition(1), definition(2)].into(),
        playback_pages: vec![light_playback::PlaybackPage {
            number: 4,
            name: "Page 4".into(),
            slots: HashMap::from([(6, 1), (7, 2)]),
            virtual_playbacks: HashMap::new(),
        }].into(),
        ..Default::default()
    }
}

#[test]
fn canonical_navigation_uses_current_and_explicit_page_playbacks() {
    let (state, data_dir) = test_state();
    let (user, first_desk, second_desk) = {
        let user = state.installation.users().unwrap().remove(0);
        let first = state.installation.add_desk("Front").unwrap();
        let second = state.installation.add_desk("Wing").unwrap();
        (user, first, second)
    };
    let show_id = light_core::ShowId::new();
    state.active_show.replace_current(Some(ShowEntry {
        id: show_id,
        name: "Selection".into(),
        path: data_dir.join("selection.show").display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    }));
    let list_id = light_core::CueListId::new();
    state
        .output.replace_snapshot(cue_selection_snapshot(list_id))
        .unwrap();
    state
        .installation.set_selected_playback(first_desk.id, show_id, Some(1))
        .unwrap();
    state
        .installation.set_selected_playback(second_desk.id, show_id, Some(2))
        .unwrap();
    state
        .installation.set_desk_page(first_desk.id, show_id, 4)
        .unwrap();
    state
        .installation.set_desk_page(second_desk.id, show_id, 4)
        .unwrap();
    handle_playback_osc(
        &state,
        "/light/front/page-playback/7/select",
        &[OscArgument::Bool(true)],
        None,
    );
    assert_eq!(
        state
            .installation.selected_playback(first_desk.id, show_id)
            .unwrap(),
        Some(2)
    );
    state
        .installation.set_selected_playback(first_desk.id, show_id, Some(1))
        .unwrap();
    let first = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        user: user.clone(),
        token: "first".into(),
        connected: true,
        desk: first_desk,
    };
    let second = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        user,
        token: "second".into(),
        connected: true,
        desk: second_desk,
    };
    execute_cue_and_assert_typed_event(
        &state,
        &first,
        "GO TO PBK 6 CUE 2",
        light_application::ActionSource::Keyboard,
        1,
        Some(cue("2")),
        None,
    );
    execute_programmer_command(&state, &second, "GO TO PBK 7 CUE 3").unwrap();
    execute_cue_and_assert_typed_event(
        &state,
        &first,
        "LOAD PBK 6 CUE 1",
        light_application::ActionSource::Osc,
        1,
        Some(cue("3")),
        Some(cue("1")),
    );
    for number in [1, 2] {
        let runtime = state
            .output
            .playback_runtime_status_at(light_playback::PlaybackIdentity::physical(number).unwrap())
            .unwrap();
        assert_eq!(
            (
                runtime.playback.current_cue_number,
                runtime.playback.loaded_cue_number
            ),
            (Some(cue("3")), Some(cue("1")))
        );
    }
    execute_programmer_command(&state, &first, "GO TO PBK 7 CUE 1").unwrap();
    execute_programmer_command(&state, &first, "LOAD PBK 4 . 7 CUE 2").unwrap();
    for number in [1, 2] {
        let runtime = state
            .output
            .playback_runtime_status_at(light_playback::PlaybackIdentity::physical(number).unwrap())
            .unwrap();
        assert_eq!(
            (
                runtime.playback.current_cue_number,
                runtime.playback.loaded_cue_number
            ),
            (Some(cue("1")), Some(cue("2")))
        );
    }
    assert_eq!(
        state
            .installation.selected_playback(first.desk.id, show_id)
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
    current: Option<light_playback::CueNumber>,
    loaded: Option<light_playback::CueNumber>,
) {
    let context = operator_action_context(session, source);
    let before = state.events.latest_sequence();
    execute_programmer_command_from(state, session, command, &context).unwrap();
    assert!(state.events.latest_sequence() >= before + 2);

    let light_application::EventReplay::Events(events) = state
        .events
        .replay(before, &light_application::EventFilter::default())
    else {
        panic!("expected retained Playback event");
    };
    let event = events
        .iter()
        .find(|event| {
            matches!(
                &event.payload,
                light_application::ApplicationEvent::Playback(
                    light_application::PlaybackEvent::RuntimeChanged(change)
                ) if change.projection.playback_number == Some(playback)
            )
        })
        .expect("the addressed Playback projection must be published");
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
    assert_eq!(runtime.current.as_ref().map(|cue| cue.number.clone()), current);
    assert_eq!(runtime.loaded.as_ref().map(|cue| cue.number.clone()), loaded);
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
fn dmx_address_parser_requires_one_physical_universe_and_slot() {
    assert_eq!(
        parse_dmx_address(&["DMX".into(), "2".into(), ".".into(), "511".into()]).unwrap(),
        (2, 511)
    );
    for tokens in [
        vec!["DMX".into(), "0".into(), ".".into(), "1".into()],
        vec!["DMX".into(), "1".into(), ".".into(), "0".into()],
        vec!["DMX".into(), "1".into(), ".".into(), "513".into()],
        vec!["DMX".into(), "1".into()],
        vec!["DMX".into(), "01".into(), ".".into(), "1".into()],
        vec!["DMX".into(), "1".into(), ".".into(), "001".into()],
        vec!["DMX".into(), "1".into(), ".".into(), "2".into(), "+".into()],
    ] {
        assert!(parse_dmx_address(&tokens).is_err(), "{tokens:?}");
    }
}

#[test]
fn dmx_lookup_resolves_footprints_splits_multipatches_and_all_logical_heads() {
    let mut fixture = schema_v2_direct_fixture().0;
    fixture.universe = None;
    fixture.address = None;
    fixture.definition.profile_snapshot.as_mut().unwrap().modes[0].splits = vec![
        light_fixture::FixtureSplit { number: 1, footprint: 4 },
        light_fixture::FixtureSplit { number: 2, footprint: 2 },
    ];
    fixture.split_patches = vec![
        light_fixture::SplitPatch { split: 1, universe: Some(2), address: Some(100) },
        light_fixture::SplitPatch { split: 2, universe: Some(7), address: Some(300) },
    ];
    fixture.multipatch = vec![light_fixture::MultiPatchInstance {
        id: Uuid::new_v4(),
        name: "Mirror".into(),
        universe: Some(9),
        address: Some(400),
        split_patches: Vec::new(),
        location: Default::default(),
        rotation: Default::default(),
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
    }];
    let first_head = light_core::FixtureId::new();
    let second_head = light_core::FixtureId::new();
    fixture.definition.heads = vec![
        light_fixture::LogicalHead { index: 1, name: "Cell 1".into(), shared: false, parameters: Vec::new() },
        light_fixture::LogicalHead { index: 2, name: "Cell 2".into(), shared: false, parameters: Vec::new() },
    ];
    fixture.logical_heads = vec![
        light_fixture::PatchedHead { profile_head_id: None, head_index: 1, fixture_id: first_head },
        light_fixture::PatchedHead { profile_head_id: None, head_index: 2, fixture_id: second_head },
    ];
    let expected = vec![first_head, second_head];

    for (universe, address) in [(2, 100), (2, 103), (7, 301), (9, 400), (9, 403)] {
        assert_eq!(
            resolve_dmx_fixture_selection(&[fixture.clone()], universe, address).unwrap(),
            expected,
            "DMX {universe}.{address}"
        );
    }
    assert!(resolve_dmx_fixture_selection(&[fixture.clone()], 2, 104).unwrap().is_empty());
    assert!(resolve_dmx_fixture_selection(&[fixture], 7, 302).unwrap().is_empty());
}

#[test]
fn dmx_lookup_ignores_unpatched_visual_only_and_internal_fixtures() {
    let mut unpatched = schema_v2_direct_fixture().0;
    unpatched.universe = None;
    unpatched.address = None;
    let mut visual = schema_v2_direct_fixture().0;
    visual.fixture_id = light_core::FixtureId::new();
    visual.definition.profile_snapshot.as_mut().unwrap().patch_policy = light_fixture::PatchPolicy::VisualOnly;
    let mut internal = schema_v2_direct_fixture().0;
    internal.fixture_id = light_core::FixtureId::new();
    internal.definition.profile_snapshot.as_mut().unwrap().patch_policy = light_fixture::PatchPolicy::Internal;

    assert!(
        resolve_dmx_fixture_selection(&[unpatched, visual, internal], 1, 1)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn fixture_selection_skips_missing_positive_numbers_and_supports_fixture_thru() {
    let base = schema_v2_direct_fixture().0;
    let fixtures = [10_u32, 11, 12]
        .into_iter()
        .map(|number| {
            let mut fixture = base.clone();
            fixture.fixture_id = light_core::FixtureId::new();
            fixture.fixture_number = Some(number);
            fixture
        })
        .collect::<Vec<_>>();
    let expected = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();

    assert_eq!(
        parse_fixture_selection(&fixtures, &["999".into()]).unwrap(),
        Vec::<light_core::FixtureId>::new()
    );
    assert_eq!(
        parse_fixture_selection(
            &fixtures,
            &["1".into(), "THRU".into(), "999".into()]
        )
        .unwrap(),
        expected
    );
    assert_eq!(
        parse_fixture_selection(&fixtures, &["9".into(), "THRU".into(), "13".into()]).unwrap(),
        expected
    );
    assert_eq!(
        parse_fixture_selection(&fixtures, &["THRU".into()]).unwrap(),
        expected
    );
    assert_eq!(
        parse_fixture_selection(&fixtures, &["not-a-number".into()]).unwrap_err(),
        "fixture number is invalid"
    );
}

#[test]
fn fixture_thru_excludes_stage_only_and_internal_objects_but_keeps_unpatched_dmx_fixtures() {
    let mut controllable = schema_v2_direct_fixture().0;
    controllable.fixture_id = light_core::FixtureId::new();
    controllable.fixture_number = Some(1);
    controllable.universe = None;
    controllable.address = None;

    let mut visual_only = controllable.clone();
    visual_only.fixture_id = light_core::FixtureId::new();
    visual_only.fixture_number = None;
    visual_only
        .definition
        .profile_snapshot
        .as_mut()
        .unwrap()
        .patch_policy = light_fixture::PatchPolicy::VisualOnly;

    let mut internal = controllable.clone();
    internal.fixture_id = light_core::FixtureId::new();
    internal.fixture_number = None;
    internal
        .definition
        .profile_snapshot
        .as_mut()
        .unwrap()
        .patch_policy = light_fixture::PatchPolicy::Internal;

    assert_eq!(
        parse_fixture_selection(
            &[visual_only, controllable.clone(), internal],
            &["THRU".into()]
        )
        .unwrap(),
        vec![controllable.fixture_id]
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
    registry
        .action(
                        HighlightAction::On,
            &complete,
            &fixtures,
            &HashMap::new(),
            false,
        )
        .unwrap();
    let first = registry
        .action(
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
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        user: user.clone(),
        token: "multi-head-selection".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());
    let (fixture, children) = highlight_multi_head_fixture();
    let parent = fixture.fixture_id;
    state
        .output.replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();

    let set = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "multi-head-set",
            light_wire::v2::live_action::LiveAction::ProgrammingSelection(
                light_wire::v2::command_line::ProgrammingSelectionActionRequest {
                    request_id: "multi-head-set".into(),
                    action:
                        light_wire::v2::command_line::ProgrammingSelectionAction::Replace {
                            fixtures: vec![parent.0],
                            expected_revision: 0,
                        },
                },
            ),
        ),
    );
    assert!(set.ok, "{:?}", set.error);
    assert_eq!(
        state.programming.get(session.id).unwrap().selected,
        children
    );

    state.programming.select(session.id, []);
    let gesture = dispatch_live_action(
        &state,
        &session,
        live_action_frame(
            &session,
            "multi-head-gesture",
            light_wire::v2::live_action::LiveAction::ProgrammingSelection(
                light_wire::v2::command_line::ProgrammingSelectionActionRequest {
                    request_id: "multi-head-gesture".into(),
                    action:
                        light_wire::v2::command_line::ProgrammingSelectionAction::Gesture {
                            source: light_wire::v2::command_line::ProgrammingSelectionGestureSource::Fixture {
                                fixture_id: parent.0,
                            },
                            remove: false,
                        },
                },
            ),
        ),
    );
    assert!(gesture.ok, "{:?}", gesture.error);
    assert_eq!(
        state.programming.get(session.id).unwrap().selected,
        children
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
