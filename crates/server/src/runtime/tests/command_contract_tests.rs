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
    let user = state.desk.lock().users().unwrap().remove(0);
    let control_desk = state.desk.lock().add_desk("Commands", "commands").unwrap();
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "test".into(),
        connected: true,
        desk: control_desk,
    };
    state.programmers.start(session.id, user.id);
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
    let store = ShowStore::open(&show_path).unwrap();
    let group = light_programmer::GroupDefinition {
        id: "1".into(),
        name: "Group 1".into(),
        ..Default::default()
    };
    store
        .put_object("group", "1", &serde_json::to_value(group).unwrap(), 0)
        .unwrap();
    *state.active_show.write() = Some(entry.clone());
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
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
    let programmer = self.state.programmers.get(self.session.id).unwrap();
    let timed_group = &programmer.group_values["1"][&light_core::AttributeKey::intensity()];
    assert_eq!(timed_group.fade_millis, Some(2_000));
    assert_eq!(timed_group.delay_millis, Some(1_000));

    let preset_fixture = light_core::FixtureId::new();
    self.state.programmers.set(
        self.session.id,
        preset_fixture,
        light_core::AttributeKey("pan".into()),
        light_core::AttributeValue::Normalized(0.4),
    );
    execute_programmer_command(&self.state, &self.session, "RECORD 0.1").unwrap();
    execute_programmer_command(&self.state, &self.session, "RECORD 1.1").unwrap();
    let intensity_preset: light_programmer::Preset = serde_json::from_value(
        ShowStore::open(&self.show_path)
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
    let preset_ids = ShowStore::open(&self.show_path)
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
    let snapshot = self.state.engine.snapshot();
    let (_, _, cue_list) =
        cue_list_for_playback(&ShowStore::open(&self.show_path).unwrap(), &snapshot, 25).unwrap();
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
        &ShowStore::open(&self.show_path).unwrap(),
        &self.state.engine.snapshot(),
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
        .desk
        .lock()
        .set_selected_playback(self.session.desk.id, self.show_id, Some(25))
        .unwrap();
    execute_programmer_command(&self.state, &self.session, "RECORD CUE 7").unwrap();
    let (_, _, selected_list) = cue_list_for_playback(
        &ShowStore::open(&self.show_path).unwrap(),
        &self.state.engine.snapshot(),
        25,
    )
    .unwrap();
    assert!(selected_list.cues.iter().any(|cue| cue.number == 7.0));
    }

    fn verify_record_modes(&self) {
    let color = light_core::AttributeKey("color.emitter.red".into());
    let set_only_color = || {
        let mut programmer = self.state.programmers.get(self.session.id).unwrap();
        programmer.values.clear();
        programmer.group_values.clear();
        self.state.programmers.restore(programmer);
        assert!(self.state.programmers.set_group(
            self.session.id,
            "1".into(),
            color.clone(),
            light_core::AttributeValue::Normalized(0.5),
        ));
    };
    set_only_color();
    execute_programmer_command(&self.state, &self.session, "RECORD + SET 25 CUE 2.5").unwrap();
    let (_, _, cue_list) = cue_list_for_playback(
        &ShowStore::open(&self.show_path).unwrap(),
        &self.state.engine.snapshot(),
        25,
    )
    .unwrap();
    let merged = cue_list.cues.iter().find(|cue| cue.number == 2.5).unwrap();
    assert_eq!(merged.group_changes.len(), 2);

    execute_programmer_command(&self.state, &self.session, "RECORD - SET 25 CUE 2.5").unwrap();
    let (_, _, cue_list) = cue_list_for_playback(
        &ShowStore::open(&self.show_path).unwrap(),
        &self.state.engine.snapshot(),
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
        &ShowStore::open(&self.show_path).unwrap(),
        &self.state.engine.snapshot(),
        25,
    )
    .unwrap();
    let overwritten = cue_list.cues.iter().find(|cue| cue.number == 2.5).unwrap();
    assert_eq!(overwritten.group_changes.len(), 1);
    assert_eq!(overwritten.group_changes[0].attribute, color);

    let mut programmer = self.state.programmers.get(self.session.id).unwrap();
    programmer.values.clear();
    programmer.group_values.clear();
    self.state.programmers.restore(programmer);
    execute_programmer_command(&self.state, &self.session, "RECORD - SET 25 CUE 2.5").unwrap();
    let (_, _, cue_list) = cue_list_for_playback(
        &ShowStore::open(&self.show_path).unwrap(),
        &self.state.engine.snapshot(),
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
fn spd_grp_commands_preserve_precision_mapping_relative_changes_and_phase_links() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
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
        state.configuration.read().speed_groups_bpm,
        [120.0, 127.5, 130.0, 140.0, 150.0]
    );

    execute_programmer_command(&state, &session, "SPD GRP 1 AT + 5").unwrap();
    assert_eq!(state.configuration.read().speed_groups_bpm[0], 125.0);
    execute_programmer_command(&state, &session, "SPD GRP 1 AT - 5").unwrap();
    assert_eq!(state.configuration.read().speed_groups_bpm[0], 120.0);
    assert_eq!(state.configuration.read().speed_groups_bpm[1], 127.5);

    execute_programmer_command(&state, &session, "SPD GRP 1 AT SPD GRP 3").unwrap();
    {
        let controllers = state.speed_groups.lock();
        assert_eq!(controllers[0].manual_bpm(), 120.0);
        assert_eq!(controllers[2].manual_bpm(), 120.0);
        assert_eq!(controllers[0].synchronized_with(), Some(3));
        assert_eq!(controllers[2].synchronized_with(), Some(1));
        let now = application_millis(&state).saturating_add(18_750);
        let source = controllers[0].snapshot(now);
        let target = controllers[2].snapshot(now);
        assert_eq!(source.phase_origin_millis, target.phase_origin_millis);
        assert!((source.beat_phase - target.beat_phase).abs() < f64::EPSILON);
    }

    execute_programmer_command(&state, &session, "SPD GRP 3 AT 90").unwrap();
    {
        let controllers = state.speed_groups.lock();
        assert_eq!(controllers[0].manual_bpm(), 120.0);
        assert_eq!(controllers[2].manual_bpm(), 90.0);
        assert_eq!(controllers[0].synchronized_with(), None);
        assert_eq!(controllers[2].synchronized_with(), None);
    }

    execute_programmer_command(&state, &session, "SPD GRP 1 AT SPD GRP 3").unwrap();
    let tap_start = application_millis(&state).saturating_add(1_000);
    {
        let mut controllers = state.speed_groups.lock();
        let retained_peer_bpm = controllers[2].manual_bpm();
        unlink_speed_group(&mut controllers, 0, tap_start);
        assert!(matches!(
            controllers[0].tap_learn(tap_start),
            light_control::speed::LearnResult::Armed
        ));
        assert!(matches!(
            controllers[0].tap_learn(tap_start + 400),
            light_control::speed::LearnResult::Learned { .. }
        ));
        assert_eq!(controllers[0].manual_bpm(), 150.0);
        assert_eq!(controllers[2].manual_bpm(), retained_peer_bpm);
        assert_eq!(controllers[0].synchronized_with(), None);
        assert_eq!(controllers[2].synchronized_with(), None);
        copy_speed_group_runtime_to_configuration(&state, &controllers, &[0]);
    }
    assert_eq!(state.configuration.read().speed_groups_bpm[0], 150.0);
    assert_eq!(state.configuration.read().speed_groups_bpm[2], 120.0);
    assert!(execute_programmer_command(&state, &session, "SPD GRP 0 AT 120").is_err());
    assert!(execute_programmer_command(&state, &session, "SPD GRP 6 AT 120").is_err());
    let _ = std::fs::remove_dir_all(data_dir);
}
