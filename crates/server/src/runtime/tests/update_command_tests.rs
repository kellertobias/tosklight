fn update_undo_snapshot(
    fixture: light_core::FixtureId,
) -> (light_playback::CueList, EngineSnapshot) {
    let cue_list_id = light_core::CueListId::new();
    let mut first = light_playback::Cue::new(1.0);
    first.changes.push(light_playback::CueChange::set(
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.2),
    ));
    let mut second = light_playback::Cue::new(2.0);
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
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![first, second, light_playback::Cue::new(3.0)],
    };
    let playback = light_playback::PlaybackDefinition {
        number: 7,
        name: "Update playback".into(),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [light_playback::PlaybackButtonAction::None; 3],
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
    let snapshot = EngineSnapshot {
        cue_lists: vec![cue_list.clone()],
        playbacks: vec![playback],
        playback_pages: vec![light_playback::PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::from([(7, 7)]),
        }],
        ..EngineSnapshot::default()
    };
    (cue_list, snapshot)
}

#[test]
fn command_line_update_enter_applies_the_configured_group_default_directly() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "update-enter-default".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());

    let first = light_core::FixtureId::new();
    let added = light_core::FixtureId::new();
    let group = light_programmer::GroupDefinition {
        id: "981".into(),
        name: "Enter Update".into(),
        fixtures: vec![first],
        ..Default::default()
    };
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![group.clone()],
            ..Default::default()
        })
        .unwrap();
    state.programmers.select(session.id, [first, added]);
    state.configuration.write().update_settings_by_desk.insert(
        session.desk.id,
        update::UpdateSettings {
            group_mode: update::ExistingContentMode::AddNew,
            show_update_modal_on_touch: true,
            ..Default::default()
        },
    );

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
    *state.active_show.write() = Some(entry);
    let store = ShowStore::open(&show_path).unwrap();
    store
        .put_object("group", "981", &serde_json::to_value(&group).unwrap(), 0)
        .unwrap();

    assert_eq!(
        execute_programmer_command(&state, &session, "UPDATE GROUP 981").unwrap(),
        1
    );
    let updated = serde_json::from_value::<light_programmer::GroupDefinition>(
        stored_update_object(&store, "group", "981").unwrap().body,
    )
    .unwrap();
    assert_eq!(updated.fixtures, vec![first, added]);
    assert_eq!(
        state.programmers.get(session.id).unwrap().selected,
        vec![first, added]
    );
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
        cue_number: 2.0,
    }];
    let touched = UpdateApiTarget {
        family: UpdateApiTargetFamily::Cue,
        object_id: Some(cue_list_id.0.to_string()),
        playback_number: Some(7),
        cue_id: Some(first_cue),
        cue_number: Some(1.0),
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
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "update-confirmation".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());

    let fixture = light_core::FixtureId::new();
    let (cue_list, snapshot) = update_undo_snapshot(fixture);
    let cue_list_id = cue_list.id;
    state
        .engine
        .replace_snapshot(snapshot)
        .unwrap();
    for _ in 0..3 {
        state
            .engine
            .execute_playback(EnginePlaybackCommand::Pool {
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
    *state.active_show.write() = Some(entry.clone());
    let store = ShowStore::open(&show_path).unwrap();
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

    state.programmers.set(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.8),
    );
    state.programmers.set(
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
        cue_number: Some(3.0),
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

    state.programmers.set(
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
