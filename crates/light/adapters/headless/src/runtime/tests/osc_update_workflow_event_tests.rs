use super::*;

#[test]
fn osc_update_enter_publishes_authoritative_disarm_without_replay_or_rejection_echo() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "osc-update-workflow-events".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    let (cue_list, snapshot) = update_undo_snapshot(fixture);
    state.output.replace_snapshot(snapshot).unwrap();
    state
        .output
        .execute_playback(EnginePlaybackCommand::Playback {
            address: cue_programming_test_address(1, 7),
            action: light_engine::PlaybackAction::Go,
            activation_origin: None,
            control_source: cue_programming_test_source(),
        })
        .unwrap();
    let show_path = data_dir.join("shows/osc-update-workflow.show");
    let show_id = initialise_show(&show_path, "OSC Update workflow").unwrap();
    state.active_show.replace_current(Some(ShowEntry {
        is_base_show: false,
        id: show_id,
        name: "OSC Update workflow".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    }));
    let store = ActiveShowRepository::open(&show_path).unwrap();
    store
        .put_object(
            "cue_list",
            &cue_list.id.0.to_string(),
            &serde_json::to_value(&cue_list).unwrap(),
            0,
        )
        .unwrap();
    state.programming.set(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.8),
    );
    state
        .programming
        .set_command_line(session.id, "UPDATE ALL PBK 1.7".into());
    let before = state.programming.get(session.id).unwrap();
    let event_start = state.events.audit_events().len();
    assert_eq!(
        command_http::route_osc_command_key_outcome(
            &state,
            &session,
            "desk",
            "enter",
            Some("osc-update-enter")
        ),
        Some(true)
    );
    let after = state.programming.get(session.id).unwrap();
    assert!(!command_line_arms_update(&after.command_line));
    assert_eq!(before.values, after.values);
    let stored: light_playback::CueList = serde_json::from_value(
        stored_update_object(&store, "cue_list", &cue_list.id.0.to_string())
            .unwrap()
            .body,
    )
    .unwrap();
    assert_eq!(
        stored.cues[0].changes[0].value,
        Some(light_core::AttributeValue::Normalized(0.8))
    );
    assert_eq!(stored.cues[1..], cue_list.cues[1..]);
    let events = state.events.audit_events();
    let disarmed: Vec<_> = events[event_start..]
        .iter()
        .filter(|event| event.kind == "update_armed" && event.payload["armed"] == false)
        .collect();
    assert_eq!(
        disarmed.len(),
        1,
        "accepted OSC Enter must tell the GUI Update is disarmed"
    );
    assert_eq!(disarmed[0].payload["desk_id"], session.desk.id.to_string());
    assert_eq!(disarmed[0].payload["source"], "osc");
    let after_events = events.len();
    assert_eq!(
        command_http::route_osc_command_key_outcome(
            &state,
            &session,
            "desk",
            "enter",
            Some("osc-update-enter")
        ),
        Some(true)
    );
    assert_eq!(
        state.events.audit_events().len(),
        after_events,
        "replay publishes nothing twice"
    );

    state
        .programming
        .set_command_line(session.id, "UPDATE ALL PBK 1.99".into());
    let rejected_start = state.events.audit_events().len();
    assert_eq!(
        command_http::route_osc_command_key_outcome(
            &state,
            &session,
            "desk",
            "enter",
            Some("osc-update-rejected")
        ),
        Some(true)
    );
    assert!(command_line_arms_update(
        &state.programming.get(session.id).unwrap().command_line
    ));
    let rejected = state.events.audit_events();
    assert!(
        rejected[rejected_start..]
            .iter()
            .any(|event| event.kind == "programmer_command_rejected")
    );
    assert!(
        !rejected[rejected_start..]
            .iter()
            .any(|event| event.kind == "update_armed")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
