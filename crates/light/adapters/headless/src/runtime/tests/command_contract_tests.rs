struct CommandContractScenario {
    state: AppState,
    session: Session,
    data_dir: PathBuf,
    show_path: PathBuf,
    show_id: light_core::ShowId,
}

impl CommandContractScenario {
    fn new() -> Self {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let control_desk = state.installation.add_desk("Commands", "commands").unwrap();
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "test".into(),
        connected: true,
        desk: control_desk,
    };
    state.programming.start(session.id, user.id);
    let show_path = data_dir.join("shows/commands.show");
    let show_id = initialise_show(&show_path, "Commands").unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Commands".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    let store = ActiveShowRepository::open(&show_path).unwrap();
    let group = light_programmer::GroupDefinition {
        id: "1".into(),
        name: "Group 1".into(),
        ..Default::default()
    };
    store
        .put_object("group", "1", &serde_json::to_value(group).unwrap(), 0)
        .unwrap();
    state.active_show.replace_current(Some(entry.clone()));
    state
        .output.replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();
        Self {
            state,
            session,
            data_dir,
            show_path,
            show_id,
        }
    }

    fn verify_group_and_preset_contract(&self) {
    execute_programmer_command(&self.state, &self.session, "GROUP 1 DIV 2 + 1").unwrap();
    execute_programmer_command(&self.state, &self.session, "GROUP 1 AT 50 DELAY 1 TIME 2")
        .unwrap();
    let programmer = self.state.programming.get(self.session.id).unwrap();
    let timed_group = &programmer.group_values["1"][&light_core::AttributeKey::intensity()];
    assert_eq!(timed_group.fade_millis, Some(2_000));
    assert_eq!(timed_group.delay_millis, Some(1_000));

    let preset_fixture = light_core::FixtureId::new();
    self.state.programming.set(
        self.session.id,
        preset_fixture,
        light_core::AttributeKey("pan".into()),
        light_core::AttributeValue::Normalized(0.4),
    );
    execute_programmer_command(&self.state, &self.session, "RECORD 0.1").unwrap();
    execute_programmer_command(&self.state, &self.session, "RECORD 1.1").unwrap();
    let intensity_preset: light_programmer::Preset = serde_json::from_value(
        ActiveShowRepository::open(&self.show_path)
            .unwrap()
            .objects("preset")
            .unwrap()
            .into_iter()
            .find(|object| object.id == "1.1")
            .unwrap()
            .body,
    )
    .unwrap();
    assert_eq!(
        intensity_preset.family,
        light_programmer::PresetFamily::Intensity
    );
    assert!(intensity_preset.values.values().all(|attributes| {
        attributes
            .keys()
            .all(light_core::AttributeKey::is_intensity)
    }));
    execute_programmer_command(&self.state, &self.session, "DELETE 1.1").unwrap();
    execute_programmer_command(&self.state, &self.session, "COPY 0.1 AT 2").unwrap();
    execute_programmer_command(&self.state, &self.session, "MOVE 0.2 AT 3").unwrap();
    execute_programmer_command(&self.state, &self.session, "DELETE 0.1").unwrap();
    let preset_ids = ActiveShowRepository::open(&self.show_path)
        .unwrap()
        .objects("preset")
        .unwrap()
        .into_iter()
        .map(|object| object.id)
        .collect::<Vec<_>>();
    assert_eq!(preset_ids, vec!["0.3"]);
    }

    fn verify_cue_creation_and_timing(&self) {
    execute_programmer_command(&self.state, &self.session, "RECORD SET 25 TIME 3 DELAY 1.5")
        .unwrap();
    execute_programmer_command(&self.state, &self.session, "RECORD SET 25 CUE 2.5").unwrap();
    let snapshot = self.state.output.snapshot();
    let (_, _, cue_list) =
        cue_list_for_playback(&ActiveShowRepository::open(&self.show_path).unwrap(), &snapshot, 25).unwrap();
    assert_eq!(
        cue_list
            .cues
            .iter()
            .map(|cue| cue.number)
            .collect::<Vec<_>>(),
        vec![1.0, 2.5]
    );
    assert_eq!(cue_list.cues[0].fade_millis, 3_000);
    assert_eq!(cue_list.cues[0].delay_millis, 0);
    assert!(matches!(
        cue_list.cues[0].trigger,
        light_playback::CueTrigger::Wait {
            delay_millis: 1_500
        }
    ));
    assert_eq!(cue_list.cues[0].group_changes[0].fade_millis, Some(2_000));
    assert_eq!(cue_list.cues[0].group_changes[0].delay_millis, Some(1_000));
    execute_programmer_command(
        &self.state,
        &self.session,
        "RECORD SET 25 CUE 2.5 DELAY 0",
    )
    .unwrap();
    let (_, _, cue_list) = cue_list_for_playback(
        &ActiveShowRepository::open(&self.show_path).unwrap(),
        &self.state.output.snapshot(),
        25,
    )
    .unwrap();
    assert!(matches!(
        cue_list
            .cues
            .iter()
            .find(|cue| cue.number == 2.5)
            .unwrap()
            .trigger,
        light_playback::CueTrigger::Follow { delay_millis: 0 }
    ));

    self.state
        .installation.set_selected_playback(self.session.desk.id, self.show_id, Some(25))
        .unwrap();
    execute_programmer_command(&self.state, &self.session, "RECORD CUE 7").unwrap();
    let (_, _, selected_list) = cue_list_for_playback(
        &ActiveShowRepository::open(&self.show_path).unwrap(),
        &self.state.output.snapshot(),
        25,
    )
    .unwrap();
    assert!(selected_list.cues.iter().any(|cue| cue.number == 7.0));
    }

    fn verify_record_modes(&self) {
    let color = light_core::AttributeKey("color.emitter.red".into());
    let set_only_color = || {
        let mut programmer = self.state.programming.get(self.session.id).unwrap();
        programmer.values.clear();
        programmer.group_values.clear();
        self.state.programming.restore(programmer);
        assert!(self.state.programming.set_group(
            self.session.id,
            "1".into(),
            color.clone(),
            light_core::AttributeValue::Normalized(0.5),
        ));
    };
    set_only_color();
    execute_programmer_command(&self.state, &self.session, "RECORD + SET 25 CUE 2.5").unwrap();
    let (_, _, cue_list) = cue_list_for_playback(
        &ActiveShowRepository::open(&self.show_path).unwrap(),
        &self.state.output.snapshot(),
        25,
    )
    .unwrap();
    let merged = cue_list.cues.iter().find(|cue| cue.number == 2.5).unwrap();
    assert_eq!(merged.group_changes.len(), 2);

    execute_programmer_command(&self.state, &self.session, "RECORD - SET 25 CUE 2.5").unwrap();
    let (_, _, cue_list) = cue_list_for_playback(
        &ActiveShowRepository::open(&self.show_path).unwrap(),
        &self.state.output.snapshot(),
        25,
    )
    .unwrap();
    let subtracted = cue_list.cues.iter().find(|cue| cue.number == 2.5).unwrap();
    assert_eq!(subtracted.group_changes.len(), 1);
    assert_eq!(
        subtracted.group_changes[0].attribute,
        light_core::AttributeKey::intensity()
    );

    set_only_color();
    execute_programmer_command(&self.state, &self.session, "RECORD SET 25 CUE 2.5").unwrap();
    let (_, _, cue_list) = cue_list_for_playback(
        &ActiveShowRepository::open(&self.show_path).unwrap(),
        &self.state.output.snapshot(),
        25,
    )
    .unwrap();
    let overwritten = cue_list.cues.iter().find(|cue| cue.number == 2.5).unwrap();
    assert_eq!(overwritten.group_changes.len(), 1);
    assert_eq!(overwritten.group_changes[0].attribute, color);

    let mut programmer = self.state.programming.get(self.session.id).unwrap();
    programmer.values.clear();
    programmer.group_values.clear();
    self.state.programming.restore(programmer);
    execute_programmer_command(&self.state, &self.session, "RECORD - SET 25 CUE 2.5").unwrap();
    let (_, _, cue_list) = cue_list_for_playback(
        &ActiveShowRepository::open(&self.show_path).unwrap(),
        &self.state.output.snapshot(),
        25,
    )
    .unwrap();
    assert_eq!(
        cue_list
            .cues
            .iter()
            .map(|cue| cue.number)
            .collect::<Vec<_>>(),
        vec![1.0, 7.0]
    );
    }
}

#[test]
fn command_line_contract_supports_subsets_preset_lifecycle_and_cue_list_creation() {
    let scenario = CommandContractScenario::new();
    scenario.verify_group_and_preset_contract();
    scenario.verify_cue_creation_and_timing();
    scenario.verify_record_modes();
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn command_line_at_timing_setting_preserves_default_and_explicit_time_semantics() {
    let scenario = CommandContractScenario::new();
    assert!(scenario
        .state
        .installation.configuration()
        .command_line_at_uses_programmer_fade);

    scenario.state.installation.update_configuration(|configuration| {
        configuration.programmer_fade_millis = 5_000;
        configuration.command_line_at_uses_programmer_fade = false;
    });
    execute_programmer_command(&scenario.state, &scenario.session, "GROUP 1 AT 50 DELAY 1")
        .unwrap();
    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    let immediate = &programmer.group_values["1"][&light_core::AttributeKey::intensity()];
    assert!(!immediate.fade);
    assert_eq!(immediate.fade_millis, None);
    assert_eq!(immediate.delay_millis, Some(1_000));

    execute_programmer_command(&scenario.state, &scenario.session, "GROUP 1 AT 75 TIME 2").unwrap();
    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    let explicit = &programmer.group_values["1"][&light_core::AttributeKey::intensity()];
    assert!(explicit.fade);
    assert_eq!(explicit.fade_millis, Some(2_000));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn new_cuelist_and_playback_record_is_one_active_show_batch() {
    let scenario = CommandContractScenario::new();
    execute_programmer_command(&scenario.state, &scenario.session, "GROUP 1 AT 50").unwrap();
    let before = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .portable_document()
        .unwrap();
    let before_runtime = scenario.state.output.snapshot();
    let before_events = scenario.state.events.latest_sequence();
    let before_backups = command_show_object_backup_count(&scenario.data_dir);

    execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "RECORD SET 25 CUE 1",
    )
    .unwrap();

    let after = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(after.revision().value(), before.revision().value() + 1);
    assert_eq!(after.objects_of_kind("cue_list").count(), 1);
    assert!(after.object("playback", "25").is_some());
    assert_eq!(
        command_show_object_backup_count(&scenario.data_dir),
        // One interval-gated recovery checkpoint per show (api-rules §8).
        before_backups.max(1)
    );
    assert_eq!(
        scenario.state.events.latest_sequence(),
        before_events + 1
    );
    let runtime = scenario.state.output.snapshot();
    assert_eq!(runtime.revision, after.revision().value());
    assert!(!Arc::ptr_eq(&runtime, &before_runtime));
    let light_application::EventReplay::Events(events) = scenario.state.events.replay(
        before_events,
        &light_application::EventFilter::default(),
    ) else {
        panic!("expected one active-show Record event");
    };
    assert_eq!(events.len(), 1);
    let light_application::ApplicationEvent::Show(
        light_application::ShowEvent::ObjectsChanged(change),
    ) = &events[0].payload
    else {
        panic!("expected one typed object batch");
    };
    assert_eq!(change.changes.len(), 2);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn set_cuelist_page_assignment_is_one_lossless_active_show_batch() {
    let scenario = CommandContractScenario::new();
    execute_programmer_command(&scenario.state, &scenario.session, "GROUP 1 AT 50").unwrap();
    execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "RECORD SET 25 CUE 1",
    )
    .unwrap();
    let store = ActiveShowRepository::open(&scenario.show_path).unwrap();
    store
        .put_object(
            "playback_page",
            "1",
            &serde_json::json!({
                "number": 1,
                "name": "Main",
                "slots": {},
                "virtual_playbacks": {},
                "future_layout": {"columns": 10}
            }),
            0,
        )
        .unwrap();
    let before = store.portable_document().unwrap();
    let before_runtime = scenario.state.output.snapshot();
    let before_events = scenario.state.events.latest_sequence();
    let before_backups = command_show_object_backup_count(&scenario.data_dir);

    assert_eq!(
        execute_programmer_command(&scenario.state, &scenario.session, "SET 25 AT 1.1").unwrap(),
        1
    );

    let after = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .portable_document()
        .unwrap();
    let page = after.object("playback_page", "1").unwrap();
    assert_eq!(after.revision().value(), before.revision().value() + 1);
    assert_eq!(page.body()["slots"]["1"], 25);
    assert_eq!(page.body()["future_layout"]["columns"], 10);
    assert_eq!(
        command_show_object_backup_count(&scenario.data_dir),
        // One interval-gated recovery checkpoint per show (api-rules §8).
        before_backups.max(1)
    );
    assert_eq!(
        scenario.state.events.latest_sequence(),
        before_events + 1
    );
    assert!(!Arc::ptr_eq(
        &scenario.state.output.snapshot(),
        &before_runtime
    ));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn new_cuelist_and_playback_record_conflict_cannot_leave_a_partial_cuelist() {
    let scenario = CommandContractScenario::new();
    execute_programmer_command(&scenario.state, &scenario.session, "GROUP 1 AT 50").unwrap();
    let conflicting = light_playback::PlaybackDefinition {
        number: 25,
        name: "Concurrent playback".into(),
        target: light_playback::PlaybackTarget::GrandMaster,
        buttons: light_playback::PlaybackDefinition::default_buttons(
            &light_playback::PlaybackTarget::GrandMaster,
        ),
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    let store = ActiveShowRepository::open(&scenario.show_path).unwrap();
    store
        .put_object(
            "playback",
            "25",
            &serde_json::to_value(conflicting).unwrap(),
            0,
        )
        .unwrap();
    let before = store.portable_document().unwrap();
    let runtime = scenario.state.output.snapshot();
    let event_sequence = scenario.state.events.latest_sequence();
    let backups = command_show_object_backup_count(&scenario.data_dir);

    let error = execute_programmer_command(
        &scenario.state,
        &scenario.session,
        "RECORD SET 25 CUE 1",
    )
    .unwrap_err();
    assert!(error.contains("stale playback 25 revision"));

    let after = ActiveShowRepository::open(&scenario.show_path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(after.revision(), before.revision());
    assert_eq!(after.objects_of_kind("cue_list").count(), 0);
    assert_eq!(after.object("playback", "25").unwrap().revision(), 1);
    assert!(Arc::ptr_eq(&scenario.state.output.snapshot(), &runtime));
    assert_eq!(
        scenario.state.events.latest_sequence(),
        event_sequence
    );
    assert_eq!(
        command_show_object_backup_count(&scenario.data_dir),
        backups
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn command_show_object_backup_count(data_dir: &std::path::Path) -> usize {
    std::fs::read_dir(data_dir.join("backups"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains("show-object"))
        .count()
}

#[test]
fn spd_grp_commands_preserve_precision_mapping_relative_changes_and_phase_links() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user,
        token: "speed-command".into(),
        connected: true,
        desk: test_control_desk(),
    };

    execute_programmer_command(&state, &session, "SPD GRP 1 AT 120").unwrap();
    execute_programmer_command(&state, &session, "SPD GRP 2 AT 127,5").unwrap();
    execute_programmer_command(&state, &session, "SPD GRP 3 AT 130").unwrap();
    execute_programmer_command(&state, &session, "SPD GRP 4 AT 140").unwrap();
    execute_programmer_command(&state, &session, "SPD GRP 5 AT 150").unwrap();
    assert_eq!(
        state.installation.configuration().speed_groups_bpm,
        [120.0, 127.5, 130.0, 140.0, 150.0]
    );

    execute_programmer_command(&state, &session, "SPD GRP 1 AT + 5").unwrap();
    assert_eq!(state.installation.configuration().speed_groups_bpm[0], 125.0);
    execute_programmer_command(&state, &session, "SPD GRP 1 AT - 5").unwrap();
    assert_eq!(state.installation.configuration().speed_groups_bpm[0], 120.0);
    assert_eq!(state.installation.configuration().speed_groups_bpm[1], 127.5);

    execute_programmer_command(&state, &session, "SPD GRP 1 AT SPD GRP 3").unwrap();
    {
        let source_controller = state.output.speed_group_controller(0);
        let target_controller = state.output.speed_group_controller(2);
        assert_eq!(source_controller.manual_bpm(), 120.0);
        assert_eq!(target_controller.manual_bpm(), 120.0);
        assert_eq!(source_controller.synchronized_with(), Some(3));
        assert_eq!(target_controller.synchronized_with(), Some(1));
        let now = application_millis(&state).saturating_add(18_750);
        let source = source_controller.snapshot(now);
        let target = target_controller.snapshot(now);
        assert_eq!(source.phase_origin_millis, target.phase_origin_millis);
        assert!((source.beat_phase - target.beat_phase).abs() < f64::EPSILON);
    }

    execute_programmer_command(&state, &session, "SPD GRP 3 AT 90").unwrap();
    {
        let source = state.output.speed_group_controller(0);
        let target = state.output.speed_group_controller(2);
        assert_eq!(source.manual_bpm(), 120.0);
        assert_eq!(target.manual_bpm(), 90.0);
        assert_eq!(source.synchronized_with(), None);
        assert_eq!(target.synchronized_with(), None);
    }

    execute_programmer_command(&state, &session, "SPD GRP 1 AT SPD GRP 3").unwrap();
    let tap_start = application_millis(&state).saturating_add(1_000);
    {
        let retained_peer_bpm = state.output.speed_group_manual_bpm(2);
        assert!(matches!(
            state.output.tap_speed_group(0, tap_start),
            light_control::speed::LearnResult::Armed
        ));
        assert!(matches!(
            state.output.tap_speed_group(0, tap_start + 400),
            light_control::speed::LearnResult::Learned { .. }
        ));
        assert_eq!(state.output.speed_group_manual_bpm(0), 150.0);
        assert_eq!(state.output.speed_group_manual_bpm(2), retained_peer_bpm);
        assert_eq!(state.output.speed_group_controller(0).synchronized_with(), None);
        assert_eq!(state.output.speed_group_controller(2).synchronized_with(), None);
        copy_speed_group_runtime_to_configuration(&state, &[0]);
    }
    assert_eq!(state.installation.configuration().speed_groups_bpm[0], 150.0);
    assert_eq!(state.installation.configuration().speed_groups_bpm[2], 120.0);
    assert!(execute_programmer_command(&state, &session, "SPD GRP 0 AT 120").is_err());
    assert!(execute_programmer_command(&state, &session, "SPD GRP 6 AT 120").is_err());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn typed_speed_execution_resets_the_authoritative_command_line() {
    let scenario = CommandContractScenario::new();
    assert!(scenario.state.programming.set_command_line(
        scenario.session.id,
        "SPD GRP 1 AT 120".into()
    ));

    let response = dispatch_live_action(
        &scenario.state,
        &scenario.session,
        live_action_frame(
            &scenario.session,
            "speed-reset",
            light_wire::v2::live_action::LiveAction::CommandLineExecute(
                light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
                    value: "SPD GRP 1 AT 120".into(),
                },
            ),
        ),
    );

    assert!(response.ok, "{:?}", response.error);
    let command = scenario
        .state
        .programming
        .command_line_state(scenario.session.id)
        .unwrap();
    assert_eq!(command.visible_text(), "FIXTURE");
    assert!(command.pristine);
    assert_eq!(scenario.state.installation.configuration().speed_groups_bpm[0], 120.0);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
