fn update_undo_snapshot(
    fixture: light_core::FixtureId,
) -> (light_playback::CueList, EngineSnapshot) {
    let cue_list_id = light_core::CueListId::new();
    let mut first = light_playback::Cue::new(cue("1"));
    first.changes.push(light_playback::CueChange::set(
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.2),
    ));
    let mut second = light_playback::Cue::new(cue("2"));
    second.changes.push(light_playback::CueChange::set(
        fixture,
        light_core::AttributeKey("color.red".into()),
        light_core::AttributeValue::Normalized(0.3),
    ));
    let cue_list = light_playback::CueList {
        id: cue_list_id,
        name: "Update undo".into(),
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
        cues: vec![first, second, light_playback::Cue::new(cue("3"))],
    };
    let playback = light_playback::PlaybackDefinition {
        number: 7,
        name: "Update playback".into(),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [light_playback::PlaybackButtonAction::None; 3],
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
    let snapshot = EngineSnapshot {
        cue_lists: vec![cue_list.clone()].into(),
        playbacks: vec![playback].into(),
        playback_pages: vec![light_playback::PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::from([(7, 7)]),
            virtual_playbacks: HashMap::new(),
        }].into(),
        ..EngineSnapshot::default()
    };
    (cue_list, snapshot)
}

#[test]
fn command_line_update_all_adds_new_group_content_without_using_a_desk_default() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "update-enter-default".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());

    let first = light_core::FixtureId::new();
    let added = light_core::FixtureId::new();
    let group = light_programmer::GroupDefinition {
        id: "981".into(),
        name: "Enter Update".into(),
        fixtures: vec![first],
        ..Default::default()
    };
    state
        .output.replace_snapshot(EngineSnapshot {
            groups: vec![group.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    state.programming.select(session.id, [first, added]);
    let show_path = data_dir.join("shows/update-enter-default.show");
    let show_id = initialise_show(&show_path, "Update Enter default").unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Update Enter default".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    state.active_show.replace_current(Some(entry));
    let store = ActiveShowRepository::open(&show_path).unwrap();
    store
        .put_object("group", "981", &serde_json::to_value(&group).unwrap(), 0)
        .unwrap();

    assert_eq!(
        execute_programmer_command(&state, &session, "UPDATE ALL GROUP 981").unwrap(),
        1
    );
    let updated = serde_json::from_value::<light_programmer::GroupDefinition>(
        stored_update_object(&store, "group", "981").unwrap().body,
    )
    .unwrap();
    assert_eq!(updated.fixtures, vec![first, added]);
    assert_eq!(
        state.programming.get(session.id).unwrap().selected,
        vec![first, added]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn manual_update_grammar_resolves_selected_explicit_physical_virtual_and_preset_targets() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let desk = state
        .installation
        .add_desk("Manual Update", "manual-update")
        .unwrap();
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "manual-update-grammar".into(),
        connected: true,
        desk,
    };
    state.programming.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());

    let fixture = light_core::FixtureId::new();
    let (cue_list, mut snapshot) = update_undo_snapshot(fixture);
    let mut virtual_playback = snapshot.playbacks[0].clone();
    virtual_playback.number = 1001;
    let mut page = snapshot.playback_pages[0].clone();
    page.virtual_playbacks.insert(1001, virtual_playback);
    snapshot.playback_pages = vec![page].into();

    let show_path = data_dir.join("shows/manual-update-grammar.show");
    let show = state
        .installation
        .upsert_show(
            "Manual Update grammar",
            &show_path.display().to_string(),
            false,
        )
        .unwrap();
    let show_id = show.id;
    state.active_show.replace_current(Some(show));
    state
        .installation
        .set_selected_playback(session.desk.id, show_id, Some(7))
        .unwrap();

    let request = |command: &str| {
        let (tokens, _) = tokenize_programmer_command(command).unwrap();
        update_request(&state, &session, &tokens[1..], &snapshot)
    };

    let selected = request("UPDATE CUE 3").unwrap();
    assert_eq!(
        selected.mode,
        update::UpdateMode::Cue(update::CueUpdateMode::ExistingInCurrentCue)
    );
    assert_eq!(selected.target.playback_number, Some(7));
    assert_eq!(selected.target.cue_number.as_deref(), Some("3"));

    let explicit = request("UPDATE TRACKED CUELIST 7 CUE 2").unwrap();
    assert_eq!(
        explicit.mode,
        update::UpdateMode::Cue(update::CueUpdateMode::ExistingOnly)
    );
    assert_eq!(explicit.target.cue_id, Some(cue_list.cues[1].id));

    let physical = request("UPDATE KNOWN PBK 7").unwrap();
    assert_eq!(
        physical.mode,
        update::UpdateMode::Cue(update::CueUpdateMode::AddToCurrentCue)
    );
    assert_eq!(physical.target.playback_number, Some(7));
    assert_eq!(physical.target.cue_id, None);

    let virtual_target = request("UPDATE ALL VPBK 1001").unwrap();
    assert_eq!(
        virtual_target.mode,
        update::UpdateMode::Cue(update::CueUpdateMode::AddNew)
    );
    assert_eq!(virtual_target.target.playback_number, Some(1001));

    let preset = request("UPDATE COLOR PRESET 22").unwrap();
    assert_eq!(preset.target.family, UpdateApiTargetFamily::Preset);
    assert_eq!(preset.target.object_id.as_deref(), Some("2.22"));
    assert_eq!(
        preset.mode,
        update::UpdateMode::ExistingContent(update::ExistingContentMode::UpdateExisting)
    );
    assert_eq!(
        request("UPDATE ALL COLOR PRESET 22").unwrap().mode,
        update::UpdateMode::ExistingContent(update::ExistingContentMode::AddNew)
    );
    assert_eq!(
        request("UPDATE ALL PRESET 3")
            .unwrap()
            .target
            .object_id
            .as_deref(),
        Some("0.3")
    );
    assert_eq!(
        request("UPDATE ALL ALL PRESET 3").unwrap().mode,
        update::UpdateMode::ExistingContent(update::ExistingContentMode::AddNew)
    );

    for obsolete in ["UPDATE SET 1 CUE 2", "UPDATE 2 . 22"] {
        assert!(request(obsolete).is_err(), "{obsolete}");
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn touched_update_target_rejects_a_changed_playback_context_but_explicit_cue_remains_pinned() {
    let cue_list_id = light_core::CueListId::new();
    let first_cue = Uuid::new_v4();
    let second_cue = Uuid::new_v4();
    let active = vec![update::ActiveCueContext {
        playback_number: 7,
        cue_list_id,
        cue_id: second_cue,
        cue_number: cue("2"),
    }];
    let touched = UpdateApiTarget {
        family: UpdateApiTargetFamily::Cue,
        object_id: Some(cue_list_id.0.to_string()),
        playback_number: Some(7),
        cue_id: Some(first_cue),
        cue_number: Some("1".into()),
        validate_active_context: true,
    };
    let error = resolve_update_cue_target(&touched, &active).unwrap_err();
    assert_eq!(error.status, StatusCode::CONFLICT);
    assert!(error.message.contains("context changed"));

    let explicit = UpdateApiTarget {
        validate_active_context: false,
        ..touched
    };
    assert_eq!(
        resolve_update_cue_target(&explicit, &active)
            .unwrap()
            .cue_id,
        first_cue
    );
}

#[test]
fn confirmed_update_rejects_changed_programmer_and_is_one_step_undoable() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "update-confirmation".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());

    let fixture = light_core::FixtureId::new();
    let (cue_list, snapshot) = update_undo_snapshot(fixture);
    let cue_list_id = cue_list.id;
    state
        .output.replace_snapshot(snapshot)
        .unwrap();
    for _ in 0..3 {
        state
            .output.execute_playback(EnginePlaybackCommand::Pool {
                number: 7,
                action: PoolPlaybackAction::Go,
            })
            .unwrap();
    }

    let show_path = data_dir.join("shows/update-confirmation.show");
    let show_id = initialise_show(&show_path, "Update confirmation").unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Update confirmation".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    state.active_show.replace_current(Some(entry.clone()));
    let store = ActiveShowRepository::open(&show_path).unwrap();
    let cue_list_object_id = cue_list_id.0.to_string();
    let stored_revision = store
        .put_object(
            "cue_list",
            &cue_list_object_id,
            &serde_json::to_value(&cue_list).unwrap(),
            0,
        )
        .unwrap();
    let baseline = stored_update_object(&store, "cue_list", &cue_list_object_id)
        .unwrap()
        .body;

    state.programming.set(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.8),
    );
    state.programming.set(
        session.id,
        fixture,
        light_core::AttributeKey("color.red".into()),
        light_core::AttributeValue::Normalized(0.7),
    );
    let target = UpdateApiTarget {
        family: UpdateApiTargetFamily::Cue,
        object_id: Some(cue_list_object_id.clone()),
        playback_number: Some(7),
        cue_id: Some(cue_list.cues[2].id),
        cue_number: Some("3".into()),
        validate_active_context: true,
    };
    let preview_request = UpdateApiRequest {
        target: target.clone(),
        mode: update::UpdateMode::Cue(update::CueUpdateMode::ExistingOnly),
        expected_revision: None,
        expected_programmer_revision: None,
        expected_show_revision: None,
    };
    let preview = preview_update_request(&state, &session, &preview_request).unwrap();
    assert_eq!(preview.revision, stored_revision);
    assert_eq!(preview.preview.changed_count(), 2);

    state.programming.set(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.9),
    );
    let stale = UpdateApiRequest {
        expected_revision: Some(preview.revision),
        expected_programmer_revision: Some(preview.programmer_revision),
        ..preview_request.clone()
    };
    let error = perform_update(&state, &session, &stale).unwrap_err();
    assert_eq!(error.status, StatusCode::CONFLICT);
    assert!(error.message.contains("programmer content changed"));
    assert_eq!(
        stored_update_object(&store, "cue_list", &cue_list_object_id)
            .unwrap()
            .body,
        baseline
    );

    let preview = preview_update_request(&state, &session, &preview_request).unwrap();
    let confirmed = UpdateApiRequest {
        expected_revision: Some(preview.revision),
        expected_programmer_revision: Some(preview.programmer_revision),
        ..preview_request
    };
    let result = perform_update(&state, &session, &confirmed).unwrap();
    assert_eq!(result.changed_cues.len(), 2);
    assert_eq!(result.revision_after, stored_revision + 1);
    assert_eq!(
        store
            .undo_object("cue_list", &cue_list_object_id, result.revision_after)
            .unwrap(),
        result.revision_after + 1
    );
    assert_eq!(
        stored_update_object(&store, "cue_list", &cue_list_object_id)
            .unwrap()
            .body,
        baseline
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
