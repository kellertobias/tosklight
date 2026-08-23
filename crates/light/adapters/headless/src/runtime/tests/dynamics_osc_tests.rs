#[test]
fn dynamics_osc_actions_and_feedback_share_exact_runtime_identity() {
    let (state, data_dir) = test_state();
    let user = state.installation.users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "dynamics-osc".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let dynamic_id = Uuid::new_v4();
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.dynamics = vec![command_test_dynamic(dynamic_id, 12)].into();
    state.output.replace_snapshot(snapshot).unwrap();

    let source: SocketAddr = "127.0.0.1:19121".parse().unwrap();
    let target: SocketAddr = "127.0.0.1:19122".parse().unwrap();
    let subscriber = OscSubscriber {
        capability: light_core::SurfaceCapability::Programming,
        desk_alias: session.desk.osc_alias.clone(),
        target,
        command_source: source,
        session_id: session.id,
        last_seen: Instant::now(),
        shifted: false,
        shift_held: false,
        update_record_started: None,
        update_first_release: None,
        last_highlight_action: None,
    };
    state
        .integrations
        .register_osc_subscriber("dynamics-osc".into(), subscriber.clone());
    let address = format!("/light/{}/dynamic/12/toggle", session.desk.osc_alias);
    handle_dynamics_osc(
        &state,
        &address,
        &[OscArgument::Bool(true)],
        Some(&source.to_string()),
    );

    let runtime = state.output.dynamic_runtime_snapshot();
    assert_eq!(runtime.instances.len(), 1);
    let instance_id = runtime.instances[0].id;
    let controller_id = runtime.instances[0].controllers[0].id;
    for (field, value) in [("size", 0.4), ("speed", 2.0), ("phase", 90.0)] {
        handle_dynamics_osc(
            &state,
            &format!(
                "/light/{}/dynamic/instance/{instance_id}/{field}",
                session.desk.osc_alias
            ),
            &[OscArgument::Float(value)],
            Some(&source.to_string()),
        );
    }
    let runtime = state.output.dynamic_runtime_snapshot();
    let controller = &runtime.instances[0].controllers[0];
    assert_eq!(controller.id, controller_id);
    assert!((controller.size - 0.4).abs() < f32::EPSILON);
    assert!((controller.speed_multiplier - 2.0).abs() < f32::EPSILON);
    assert!((controller.phase_offset_degrees - 90.0).abs() < f32::EPSILON);

    send_programmer_osc_feedback(&state, &subscriber, &session.desk, 1, &[], &HashMap::new());
    let feedback = state.integrations.captured_osc_feedback();
    let prefix = format!("/light/{}/feedback/dynamic", session.desk.osc_alias);
    assert!(feedback.iter().any(|(sent_to, address, arguments)| {
        *sent_to == target
            && address == &format!("{prefix}/runtime/{instance_id}/winning-controller")
            && arguments == &[OscArgument::String(controller_id.to_string())]
    }));
    assert!(feedback.iter().any(|(sent_to, address, arguments)| {
        *sent_to == target
            && address == &format!("{prefix}/controller/{controller_id}/runtime-instance")
            && arguments == &[OscArgument::String(instance_id.to_string())]
    }));
    assert!(feedback.iter().any(|(sent_to, address, arguments)| {
        *sent_to == target
            && address == &format!("{prefix}/controller/{controller_id}/size")
            && arguments == &[OscArgument::Float(0.4)]
    }));

    let rejected_address = format!(
        "/light/{}/dynamic/instance/not-a-uuid/size",
        session.desk.osc_alias
    );
    handle_dynamics_osc(
        &state,
        &rejected_address,
        &[OscArgument::Float(0.5)],
        Some(&source.to_string()),
    );
    let feedback = state.integrations.captured_osc_feedback();
    assert!(feedback.iter().any(|(sent_to, address, arguments)| {
        *sent_to == target
            && address == &format!("{prefix}/error")
            && arguments
                == &[
                    OscArgument::String(rejected_address.clone()),
                    OscArgument::String("invalid Dynamic instance UUID".into()),
                ]
    }));

    let off_address = format!("/light/{}/dynamic/12/off", session.desk.osc_alias);
    handle_dynamics_osc(
        &state,
        &off_address,
        &[OscArgument::Bool(true)],
        Some(&source.to_string()),
    );
    assert!(
        state.output.dynamic_runtime_snapshot().instances.is_empty(),
        "pool Off releases the exact Programmer-owned runtime instance"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
