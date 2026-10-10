#[test]
fn fix_at_command_uses_first_class_fat_timing_and_the_final_contribution_path() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock.clone());
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "fix-at-command".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.fixtures = vec![operational_fixture(fixture)].into();
    state.output.replace_snapshot(snapshot).unwrap();
    state.programming.programmers().set_faded(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.2),
    );

    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let unsupported = execute_programmer_command_from(
        &state,
        &session,
        "ATTRIBUTE pan FixAT 40",
        &context,
    )
    .unwrap_err();
    assert!(unsupported.contains("unsupported"));
    assert!(state
        .programming
        .get(session.id)
        .unwrap()
        .dynamic_values
        .is_empty());
    assert_eq!(
        execute_programmer_command_from(&state, &session, "FixAT 40 TIME 2 DELAY 1", &context,)
            .unwrap(),
        1
    );
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(programmer.dynamic_values.len(), 1);
    assert!(matches!(
        programmer.dynamic_values[0].value,
        light_dynamics::DynamicSemanticValue::FixAt {
            value,
            timing: light_dynamics::DynamicValueTiming {
                fade_millis: Some(2_000),
                delay_millis: Some(1_000),
            },
        } if (value - 0.4).abs() < f32::EPSILON
    ));

    clock.advance_millis(3_000);
    let batches = state.output.dynamic_contributions_for_test();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 1);
    let contribution = batches
        .iter()
        .flat_map(|batch| batch.samples())
        .map(|sample| sample.value())
        .find(|value| value.value == light_core::AttributeValue::Normalized(0.4))
        .expect("FixAT contributes its normalized value");
    assert_eq!(contribution.fixture_id, fixture);
    assert_eq!(
        contribution.attribute,
        light_core::AttributeKey::intensity()
    );
    assert_eq!(
        contribution.value,
        light_core::AttributeValue::Normalized(0.4)
    );
    assert_eq!(
        contribution.merge_mode,
        light_core::MergeMode::Htp,
        "a lone Intensity FAT must arbitrate like an ordinary static Programmer value"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn release_command_stores_a_non_contributing_instruction_and_preserves_other_tracks() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "release-command".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let mut snapshot = light_engine::EngineSnapshot::default();
    let mut patched = operational_fixture(fixture);
    let mut color = patched.definition.heads[0].parameters[0].clone();
    color.attribute = light_core::AttributeKey("color.wheel.1".into());
    patched.definition.heads[0].parameters.push(color);
    snapshot.fixtures = vec![patched].into();
    state.output.replace_snapshot(snapshot).unwrap();
    state.programming.programmers().set_faded(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.75),
    );
    state.programming.programmers().set_faded(
        session.id,
        fixture,
        light_core::AttributeKey("color.wheel.1".into()),
        light_core::AttributeValue::Normalized(0.25),
    );
    let controller = Uuid::new_v4();
    assert!(state.programming.programmers().apply_dynamic_values(
        session.id,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: light_core::AttributeKey::intensity(),
            value: light_dynamics::DynamicSemanticValue::DynamicOff {
                instance_link: controller,
                timing: Default::default(),
            },
        }],
        None,
    ));

    let context = operator_action_context(&session, light_application::ActionSource::Http);
    assert_eq!(
        execute_programmer_command_from(&state, &session, "AT RELEASE", &context).unwrap(),
        1
    );
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(programmer.values.len(), 1);
    assert_eq!(&*programmer.values[0].attribute.0, "color.wheel.1");
    assert!(programmer.dynamic_values.iter().any(|value| matches!(
        value.value,
        light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. }
            if instance_link == controller
    )));
    assert!(programmer
        .dynamic_values
        .iter()
        .any(|value| matches!(value.value, light_dynamics::DynamicSemanticValue::Release)));

    assert!(state.programming.programmers().undo(session.id));
    assert_eq!(
        execute_programmer_command_from(&state, &session, "AT RELEASE COLOR", &context).unwrap(),
        1
    );
    let color_release = state.programming.get(session.id).unwrap();
    assert_eq!(color_release.values.len(), 1);
    assert!(color_release.values[0].attribute.is_intensity());
    assert!(color_release.dynamic_values.iter().any(|value| {
        *value.attribute.0 == *"color.wheel.1"
            && matches!(value.value, light_dynamics::DynamicSemanticValue::Release)
    }));
    assert_eq!(
        execute_programmer_command_from(&state, &session, "RELEASE ALL", &context).unwrap(),
        2
    );
    assert!(state.programming.get(session.id).unwrap().values.is_empty());
    assert!(state.output.dynamic_contributions_for_test().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn fix_at_command_accepts_a_named_preset_batch_with_timing() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "fix-at-preset".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.fixtures = vec![operational_fixture(fixture)].into();
    state.output.replace_snapshot(snapshot).unwrap();

    let show_path = data_dir.join("shows/fix-at-preset.show");
    let show_id = default_show::initialise_legacy_test_show(&show_path).unwrap();
    let entry = ShowEntry {
        is_base_show: false,
        id: show_id,
        name: "FixAT Preset".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    };
    state.active_show.replace_current(Some(entry.clone()));
    let preset = light_programmer::Preset {

fixture_replacement_projections: Default::default(),
group_replacement_projections: Default::default(),
instance_id: None,
        name: "Half".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 7,
        values: HashMap::from([(
            fixture,
            HashMap::from([(
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(0.65),
            )]),
        )]),
        group_values: HashMap::new(),
        aim_at_fixture_number: None,
        universal_values: Default::default(),
    };
    ActiveShowRepository::open(&entry.path)
        .unwrap()
        .put_object("preset", "1.7", &serde_json::to_value(preset).unwrap(), 0)
        .unwrap();

    let context = operator_action_context(&session, light_application::ActionSource::Http);
    assert_eq!(
        execute_programmer_command_from(
            &state,
            &session,
            "FixAT INTENSITY PRESET 7 TIME 2 DELAY 1",
            &context,
        )
        .unwrap(),
        1
    );
    assert!(matches!(
        &state.programming.get(session.id).unwrap().dynamic_values[0].value,
        light_dynamics::DynamicSemanticValue::Static {
            value: light_core::AttributeValue::Normalized(value),
            timing: light_dynamics::DynamicValueTiming {
                fade_millis: Some(2_000),
                delay_millis: Some(1_000),
            },
        } if (*value - 0.65).abs() < f32::EPSILON
    ));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn preload_dynamic_start_stays_projected_until_go_then_installs_and_persists_runtime() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "preload-dynamic".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let dynamic_id = uuid::Uuid::new_v4();
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.dynamics = vec![command_test_dynamic(dynamic_id, 1)].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let show = state
        .installation
        .upsert_show(
            "Preload Dynamic",
            &data_dir
                .join("shows/preload-dynamic.show")
                .display()
                .to_string(),
            false,
        )
        .unwrap();
    state.active_show.replace_current(Some(show.clone()));
    assert!(state.programming.arm_preload(session.id, true));

    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let result = state
        .dynamics
        .start(
            &context,
            light_application::DynamicStartCommand {
                dynamic_id,
                targets: vec![fixture],
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: light_dynamics::DynamicValueTiming::default(),
                undo_group: None,
            },
            &super::dynamics_adapter::ServerDynamicsPorts {
                state: &state,
                session: &session,
            },
        )
        .unwrap();
    assert!(result.started);
    assert!(
        state.output.dynamic_runtime_snapshot().instances.is_empty(),
        "blind Preload must not mutate Live Dynamic runtime"
    );
    assert_eq!(
        state
            .programming
            .get(session.id)
            .unwrap()
            .preload_dynamic_pending
            .len(),
        1
    );

    commit_preload(&state, &session).unwrap();

    let runtime = state.output.dynamic_runtime_snapshot();
    assert_eq!(runtime.instances.len(), 1);
    assert_eq!(runtime.instances[0].controllers[0].id, result.controller_id);
    let persisted = state
        .installation
        .setting(&output_runtime_setting(show.id))
        .unwrap()
        .expect("Preload GO persists the reconciled Dynamic runtime");
    assert!(persisted.contains(&result.controller_id.to_string()));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn grouped_dynamic_starts_share_one_programmer_undo_checkpoint() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "grouped-dynamic-starts".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let dynamic_id = uuid::Uuid::new_v4();
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.dynamics = vec![command_test_dynamic(dynamic_id, 1)].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let ports = super::dynamics_adapter::ServerDynamicsPorts {
        state: &state,
        session: &session,
    };

    for fixture in fixtures {
        state
            .dynamics
            .start(
                &context,
                light_application::DynamicStartCommand {
                    dynamic_id,
                    targets: vec![fixture],
                    overrides: light_dynamics::DynamicInstanceOverrides {
                        size: 1.0,
                        speed_multiplier: light_dynamics::Rational::ONE,
                        phase_offset_degrees: 0.0,
                    },
                    timing: light_dynamics::DynamicValueTiming::default(),
                    undo_group: Some("stage-capacity-dynamics".into()),
                },
                &ports,
            )
            .unwrap();
    }

    assert_eq!(state.programming.programmers().undo_depth(session.id), Some(1));
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 2);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn startup_load_restores_persisted_dynamic_runtime_and_programmer_identity() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock);
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "dynamic-restart".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let dynamic_id = Uuid::new_v4();
    let definition = command_test_dynamic(dynamic_id, 7);
    let show_path = data_dir.join("shows/dynamic-restart.show");
    let store = light_show::ShowStore::create(&show_path, "Dynamic restart")
        .unwrap()
        .0;
    store
        .put_object(
            "dynamic",
            &dynamic_id.to_string(),
            &serde_json::to_value(&definition).unwrap(),
            0,
        )
        .unwrap();
    let show = state
        .installation
        .upsert_show(
            "Dynamic restart",
            &show_path.display().to_string(),
            false,
        )
        .unwrap();
    store.set_identity(show.id, &show.name, None).unwrap();
    state.installation.set_active_show(Some(show.id)).unwrap();
    state.active_show.replace_current(Some(show.clone()));
    let mut engine_snapshot = light_engine::EngineSnapshot::default();
    engine_snapshot.dynamics = vec![definition].into();
    state.output.replace_snapshot(engine_snapshot).unwrap();

    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let started = state
        .dynamics
        .start(
            &context,
            light_application::DynamicStartCommand {
                dynamic_id,
                targets: vec![fixture],
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 0.65,
                    speed_multiplier: light_dynamics::Rational {
                        numerator: 3,
                        denominator: 2,
                    },
                    phase_offset_degrees: 45.0,
                },
                timing: light_dynamics::DynamicValueTiming {
                    fade_millis: Some(400),
                    delay_millis: Some(100),
                },
                undo_group: None,
            },
            &super::dynamics_adapter::ServerDynamicsPorts {
                state: &state,
                session: &session,
            },
        )
        .unwrap();
    let expected_runtime = state.output.dynamic_runtime_snapshot();
    assert_eq!(expected_runtime.instances.len(), 1);
    assert_eq!(
        expected_runtime.instances[0].controllers[0].id,
        started.controller_id
    );
    persist_programmer(&state, &session).unwrap();
    persist_output_runtime(&state).unwrap();
    drop(store);
    drop(state);

    let startup = super::startup_state::StartupState::load(startup_options::StartupOptions {
        data_dir: data_dir.clone(),
        show_file: None,
        fixture_package_dir: None,
        extensions_dir: Some(data_dir.join("extensions")),
        bind: "127.0.0.1:0".parse().unwrap(),
        test_bench: true,
        visualizer_preview: false,
        osc_bind_override: None,
        output_bind_override: None,
    })
    .unwrap();
    assert_eq!(startup.engine.snapshot().dynamics.len(), 1);
    let persisted_runtime = startup
        .output_runtime
        .dynamic_runtime
        .clone()
        .expect("startup loads the show-scoped Dynamic runtime checkpoint");
    assert_eq!(persisted_runtime, expected_runtime);
    let restored_programmer = startup
        .programmers
        .get(session.id)
        .expect("the persisted Programmer is restored for reconciliation");
    assert_eq!(restored_programmer.dynamic_values.len(), 1);
    assert!(matches!(
        &restored_programmer.dynamic_values[0].value,
        light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link,
            overrides,
            ..
        } if light_dynamics::programmer_dynamic_controller_id(restored_programmer.id, *instance_link) == started.controller_id
            && (overrides.size - 0.65).abs() < f32::EPSILON
            && (overrides.speed_multiplier.factor() - 1.5).abs() < f64::EPSILON
            && (overrides.phase_offset_degrees - 45.0).abs() < f32::EPSILON
    ));

    let mut restored_runtime = light_dynamics::DynamicRuntime::default();
    restored_runtime
        .install_definitions(startup.engine.snapshot().dynamics.iter().cloned())
        .unwrap();
    restored_runtime.restore_snapshot(persisted_runtime).unwrap();
    assert_eq!(restored_runtime.snapshot(), expected_runtime);

    drop(startup);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn startup_loads_legacy_dynamic_phase_spread_as_uniform() {
    let (state, data_dir) = test_state();
    let dynamic_id = Uuid::new_v4();
    let mut legacy = serde_json::to_value(command_test_dynamic(dynamic_id, 8)).unwrap();
    assert!(
        legacy
            .as_object_mut()
            .unwrap()
            .remove("phase_mode")
            .is_some(),
        "the current definition fixture must serialize the compatibility field"
    );
    assert!(
        legacy["lanes"][0]
            .as_object_mut()
            .unwrap()
            .remove("phase")
            .is_some(),
        "the current lane fixture must serialize the compatibility field"
    );
    let show_path = data_dir.join("shows/legacy-dynamic-phase-spread.show");
    let store = light_show::ShowStore::create(&show_path, "Legacy Dynamic phase spread")
        .unwrap()
        .0;
    store
        .put_object("dynamic", &dynamic_id.to_string(), &legacy, 0)
        .unwrap();
    let show = state
        .installation
        .upsert_show(
            "Legacy Dynamic phase spread",
            &show_path.display().to_string(),
            false,
        )
        .unwrap();
    store.set_identity(show.id, &show.name, None).unwrap();
    state.installation.set_active_show(Some(show.id)).unwrap();
    drop(store);
    drop(state);

    let startup = super::startup_state::StartupState::load(startup_options::StartupOptions {
        data_dir: data_dir.clone(),
        show_file: None,
        fixture_package_dir: None,
        extensions_dir: Some(data_dir.join("extensions")),
        bind: "127.0.0.1:0".parse().unwrap(),
        test_bench: true,
        visualizer_preview: false,
        osc_bind_override: None,
        output_bind_override: None,
    })
    .unwrap();
    let restored = startup.engine.snapshot();
    assert_eq!(restored.dynamics.len(), 1);
    assert_eq!(
        restored.dynamics[0].phase_spread_mode,
        light_dynamics::DynamicPhaseSpreadMode::Uniform
    );
    assert_eq!(restored.dynamics[0].lanes[0].phase, None);

    drop(startup);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn malformed_show_runtime_is_rejected_without_discarding_valid_dynamic_state() {
    let (state, data_dir) = test_state();
    let dynamic_id = Uuid::new_v4();
    let definition = command_test_dynamic(dynamic_id, 8);
    let fixture = light_core::FixtureId::new();
    let mut engine_snapshot = light_engine::EngineSnapshot::default();
    engine_snapshot.dynamics = vec![definition].into();
    state.output.replace_snapshot(engine_snapshot).unwrap();
    let show = state
        .installation
        .upsert_show(
            "Malformed Dynamic runtime",
            &data_dir
                .join("shows/malformed-dynamic-runtime.show")
                .display()
                .to_string(),
            false,
        )
        .unwrap();
    state.active_show.replace_current(Some(show.clone()));
    state
        .output
        .start_dynamic(light_dynamics::DynamicStartRequest {
            definition_id: dynamic_id,
            controller: light_dynamics::DynamicController {
                id: Uuid::new_v4(),
                source: light_dynamics::DynamicControllerSource::Programmer {
                    programmer_id: Uuid::new_v4(),
                    instance_link: None,
                },
                priority: 0,
                activated_at_millis: 1_000,
                size: 1.0,
                speed_multiplier: 1.0,
                phase_offset_degrees: 0.0,
                paused: false,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: vec![fixture],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 1_000,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    let original = state.output.dynamic_runtime_snapshot();
    let mut malformed = original.clone();
    malformed.instances[0].controllers.clear();

    let result = restore_output_runtime_for_show(
        &state,
        show.id,
        PersistedOutputRuntime {
            dynamic_runtime: Some(malformed),
            ..Default::default()
        },
    );

    assert!(result.is_err());
    assert_eq!(state.output.dynamic_runtime_snapshot(), original);
    assert_eq!(
        state.output.snapshot().dynamics.len(),
        1,
        "runtime recovery must not discard the valid portable Dynamic definition"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn websocket_dynamic_toggle_matches_the_authoritative_target_scope() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "dynamic-toggle".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let fixture_a = light_core::FixtureId::new();
    let fixture_b = light_core::FixtureId::new();
    let dynamic_id = uuid::Uuid::new_v4();
    let mut snapshot = light_engine::EngineSnapshot::default();
    let mut second_fixture = operational_fixture(fixture_b);
    second_fixture.address = Some(2);
    snapshot.fixtures = vec![operational_fixture(fixture_a), second_fixture].into();
    snapshot.dynamics = vec![command_test_dynamic(dynamic_id, 1)].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let show = state
        .installation
        .upsert_show(
            "Dynamic Toggle",
            &data_dir.join("shows/dynamic-toggle.show").display().to_string(),
            false,
        )
        .unwrap();
    state.active_show.replace_current(Some(show));

    let toggle = |request_id: &str, fixture: light_core::FixtureId| {
        dispatch_live_action(
            &state,
            &session,
            live_action_frame(
                &session,
                request_id,
                light_wire::v2::live_action::LiveAction::DynamicToggle(
                    light_wire::v2::dynamics::DynamicStartLiveActionRequest {
                        dynamic_id,
                        request: light_wire::v2::dynamics::DynamicStartActionRequest {
                            request_id: request_id.into(),
                            targets: vec![fixture.0],
                            overrides: light_wire::v2::dynamics::DynamicInstanceOverridesProjection {
                                size: 1.0,
                                speed_multiplier:
                                    light_wire::v2::dynamics::DynamicRationalProjection {
                                        numerator: 1,
                                        denominator: 1,
                                    },
                                phase_offset_degrees: 0.0,
                            },
                            timing:
                                light_wire::v2::dynamics::DynamicValueTimingProjection::default(),
                            undo_group: None,
                        },
                    },
                ),
            ),
        )
    };

    let first = toggle("toggle-a-on", fixture_a);
    assert!(first.ok, "{:?}", first.error);
    assert_eq!(first.payload.as_ref().unwrap()["started"], true);
    let second = toggle("toggle-b-on", fixture_b);
    assert!(second.ok, "{:?}", second.error);
    assert_eq!(second.payload.as_ref().unwrap()["started"], true);
    let third = toggle("toggle-a-off", fixture_a);
    assert!(third.ok, "{:?}", third.error);
    assert_eq!(third.payload.as_ref().unwrap()["started"], false);

    let runtime = state.output.dynamic_runtime_snapshot();
    assert_eq!(runtime.instances.len(), 1);
    assert_eq!(runtime.instances[0].targets, vec![fixture_b]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn production_dynamic_action_waits_for_active_show_contention() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "dynamic-contention".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    let dynamic_id = uuid::Uuid::new_v4();
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.fixtures = vec![operational_fixture(fixture)].into();
    snapshot.dynamics = vec![command_test_dynamic(dynamic_id, 1)].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let show = state
        .installation
        .upsert_show(
            "Dynamic Contention",
            &data_dir
                .join("shows/dynamic-contention.show")
                .display()
                .to_string(),
            false,
        )
        .unwrap();
    state.active_show.replace_current(Some(show));
    let request_id = "dynamic-contention-toggle";
    let frame = live_action_frame(
        &session,
        request_id,
        light_wire::v2::live_action::LiveAction::DynamicToggle(
            light_wire::v2::dynamics::DynamicStartLiveActionRequest {
                dynamic_id,
                request: light_wire::v2::dynamics::DynamicStartActionRequest {
                    request_id: request_id.into(),
                    targets: vec![fixture.0],
                    overrides: light_wire::v2::dynamics::DynamicInstanceOverridesProjection {
                        size: 1.0,
                        speed_multiplier:
                            light_wire::v2::dynamics::DynamicRationalProjection {
                                numerator: 1,
                                denominator: 1,
                            },
                        phase_offset_degrees: 0.0,
                    },
                    timing: light_wire::v2::dynamics::DynamicValueTimingProjection::default(),
                    undo_group: None,
                },
            },
        ),
    );
    let contention = state.active_show.acquire().await;
    let dispatch_state = state.clone();
    let dispatch_session = session.clone();
    let action = tokio::spawn(async move {
        dispatch_live_action_live(&dispatch_state, &dispatch_session, frame).await
    });
    tokio::task::yield_now().await;
    assert!(
        !action.is_finished(),
        "the Dynamic action should wait while the active-show operation is held"
    );

    drop(contention);
    let response = action.await.unwrap();
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(response.payload.as_ref().unwrap()["started"], true);
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 1);

    let changed_request_id = "dynamic-contention-show-change";
    let changed_frame = live_action_frame(
        &session,
        changed_request_id,
        light_wire::v2::live_action::LiveAction::DynamicToggle(
            light_wire::v2::dynamics::DynamicStartLiveActionRequest {
                dynamic_id,
                request: light_wire::v2::dynamics::DynamicStartActionRequest {
                    request_id: changed_request_id.into(),
                    targets: vec![fixture.0],
                    overrides: light_wire::v2::dynamics::DynamicInstanceOverridesProjection {
                        size: 1.0,
                        speed_multiplier:
                            light_wire::v2::dynamics::DynamicRationalProjection {
                                numerator: 1,
                                denominator: 1,
                            },
                        phase_offset_degrees: 0.0,
                    },
                    timing: light_wire::v2::dynamics::DynamicValueTimingProjection::default(),
                    undo_group: None,
                },
            },
        ),
    );
    let contention = state.active_show.acquire().await;
    let dispatch_state = state.clone();
    let dispatch_session = session.clone();
    let changed_action = tokio::spawn(async move {
        dispatch_live_action_live(&dispatch_state, &dispatch_session, changed_frame).await
    });
    tokio::task::yield_now().await;
    let replacement = state
        .installation
        .upsert_show(
            "Replacement Show",
            &data_dir
                .join("shows/replacement.show")
                .display()
                .to_string(),
            false,
        )
        .unwrap();
    state.active_show.replace_current(Some(replacement));
    drop(contention);
    let changed = changed_action.await.unwrap();
    assert!(!changed.ok);
    assert_eq!(
        changed.error.as_deref(),
        Some(
            "The active show changed before the Dynamic action could run. Tap the Dynamic again."
        )
    );
    assert_eq!(
        state.output.dynamic_runtime_snapshot().instances.len(),
        1,
        "a rejected action must not toggle the existing Dynamic instance"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn live_and_preload_visualization_resolve_different_dynamic_layers_authoritatively() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "dynamic-visualization".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let dynamic_id = uuid::Uuid::new_v4();
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.dynamics = vec![command_test_dynamic(dynamic_id, 1)].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let ports = super::dynamics_adapter::ServerDynamicsPorts {
        state: &state,
        session: &session,
    };
    state
        .dynamics
        .start(
            &context,
            light_application::DynamicStartCommand {
                dynamic_id,
                targets: vec![fixture],
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: light_dynamics::DynamicValueTiming::default(),
                undo_group: None,
            },
            &ports,
        )
        .unwrap();
    assert!(state.programming.arm_preload(session.id, true));
    state
        .dynamics
        .off_matching(
            &context,
            light_application::DynamicStartCommand {
                dynamic_id,
                targets: vec![fixture],
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: light_dynamics::DynamicValueTiming::default(),
                undo_group: None,
            },
            &ports,
        )
        .unwrap()
        .expect("the staged Off matches the Live Programmer controller");

    let live = state.output.visualization_dynamic_values(&[], false);
    assert!(
        live.contains_key(&(fixture, light_core::AttributeKey::intensity())),
        "Live visualization remains on the uncommitted Live layer"
    );
    let programmer = state.programming.get(session.id).unwrap();
    let projected_values = programmer
        .preload_dynamic_pending
        .iter()
        .cloned()
        .map(|value| (programmer.id.0, programmer.priority, value))
        .collect::<Vec<_>>();
    let preload = state
        .output
        .visualization_dynamic_values(&projected_values, true);
    assert!(
        !preload.contains_key(&(fixture, light_core::AttributeKey::intensity())),
        "Preload visualization applies staged Dynamic Off without changing Live"
    );
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_command_line_routes_start_parameters_and_off_through_one_controller() {
    let (state, data_dir) = test_state();
    state.installation.update_configuration(|configuration| {
        configuration.command_line_at_uses_programmer_fade = true;
    });
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "dynamic-command".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let dynamic_id = uuid::Uuid::new_v4();
    let definition = command_test_dynamic(dynamic_id, 12);
    let show_path = data_dir.join("shows/dynamic-command.show");
    light_show::ShowStore::create(&show_path, "Dynamic command")
        .unwrap()
        .0
        .put_object(
            "dynamic",
            &dynamic_id.to_string(),
            &serde_json::to_value(&definition).unwrap(),
            0,
        )
        .unwrap();
    let show = state
        .installation
        .upsert_show("Dynamic command", &show_path.display().to_string(), false)
        .unwrap();
    light_show::ShowStore::open(&show_path)
        .unwrap()
        .set_identity(show.id, &show.name, None)
        .unwrap();
    state.active_show.replace_current(Some(show));
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.dynamics = vec![definition].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Keyboard);

    assert_eq!(
        execute_programmer_command_from(&state, &session, "DYNAMIC 12", &context).unwrap(),
        1
    );
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 1);
    assert_eq!(
        execute_programmer_command_from(&state, &session, "DYNAMIC 12", &context).unwrap(),
        1
    );
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 1);
    assert!(state
        .programming
        .get(session.id)
        .unwrap()
        .dynamic_values
        .iter()
        .all(|stored| matches!(stored.value, light_dynamics::DynamicSemanticValue::DynamicOn { .. })));
    assert_eq!(
        execute_programmer_command_from(&state, &session, "DYNAMIC 12 SIZE AT 50", &context,)
            .unwrap(),
        1
    );
    let programmer = state.programming.get(session.id).unwrap();
    assert!(programmer.dynamic_values.iter().all(|stored| matches!(
        &stored.value,
        light_dynamics::DynamicSemanticValue::DynamicOn { overrides, .. }
            if (overrides.size - 0.5).abs() < f32::EPSILON
    )));
    assert_eq!(
        execute_programmer_command_from(&state, &session, "DYNAMIC 12 SPEED AT 4 DIV 2", &context,)
            .unwrap(),
        1
    );
    assert!(
        state
            .programming
            .get(session.id)
            .unwrap()
            .dynamic_values
            .iter()
            .all(|stored| matches!(
                &stored.value,
                light_dynamics::DynamicSemanticValue::DynamicOn { overrides, .. }
                    if (overrides.speed_multiplier.factor() - 2.0).abs() < f64::EPSILON
            ))
    );
    execute_programmer_command_from(
        &state,
        &session,
        "DYNAMIC 12 PHASE AT 0 THRU 360 THRU 0",
        &context,
    )
    .unwrap();
    execute_programmer_command_from(&state, &session, "DYNAMIC 12 BLOCKS AT 4", &context).unwrap();
    execute_programmer_command_from(&state, &session, "DYNAMIC 12 REPEATS AT 2", &context).unwrap();
    execute_programmer_command_from(&state, &session, "DYNAMIC 12 WINGS AT ON", &context).unwrap();
    let (_, stored) = ActiveShowRepository::open(&show_path)
        .unwrap()
        .object_with_portable_revision("dynamic", &dynamic_id.to_string())
        .unwrap();
    let stored: light_dynamics::DynamicDefinition =
        serde_json::from_value(stored.unwrap().body).unwrap();
    assert_eq!(stored.phase.anchors_degrees, [0.0, 360.0, 0.0]);
    assert_eq!(stored.phase.block_size, 4);
    assert_eq!(stored.phase.repeats, 2);
    assert!(stored.phase.wings);
    assert_eq!(
        execute_programmer_command_from(&state, &session, "DYNAMIC 12 OFF", &context).unwrap(),
        1
    );
    assert!(
        state
            .programming
            .get(session.id)
            .unwrap()
            .dynamic_values
            .iter()
            .all(|stored| matches!(
                stored.value,
                light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            ))
    );
    let light_application::EventReplay::Events(events) = state
        .events
        .replay(0, &light_application::EventFilter::default())
    else {
        panic!("Dynamic command events must remain lossless");
    };
    let kinds = events
        .iter()
        .filter_map(|event| match &event.payload {
            light_application::ApplicationEvent::Output(
                light_application::OutputEvent::DynamicRuntimeChanged(change),
            ) => Some(change.kind),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            light_application::DynamicRuntimeEventKind::InstanceStarted,
            light_application::DynamicRuntimeEventKind::InstanceActive,
            light_application::DynamicRuntimeEventKind::ControllerWinnerChanged,
            light_application::DynamicRuntimeEventKind::ControllerUpdated,
            light_application::DynamicRuntimeEventKind::ControllerUpdated,
            light_application::DynamicRuntimeEventKind::InstanceRelease,
        ]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn ambiguous_targetless_dynamic_command_retains_typed_exact_instance_choice() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "dynamic-instance-choice".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let first_fixture = light_core::FixtureId::new();
    let second_fixture = light_core::FixtureId::new();
    let dynamic_id = uuid::Uuid::new_v4();
    let definition = command_test_dynamic(dynamic_id, 12);
    let show_path = data_dir.join("shows/dynamic-instance-choice.show");
    light_show::ShowStore::create(&show_path, "Dynamic instance choice")
        .unwrap()
        .0
        .put_object(
            "dynamic",
            &dynamic_id.to_string(),
            &serde_json::to_value(&definition).unwrap(),
            0,
        )
        .unwrap();
    let show = state
        .installation
        .upsert_show(
            "Dynamic instance choice",
            &show_path.display().to_string(),
            false,
        )
        .unwrap();
    light_show::ShowStore::open(&show_path)
        .unwrap()
        .set_identity(show.id, &show.name, None)
        .unwrap();
    state.active_show.replace_current(Some(show));
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.dynamics = vec![definition].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Keyboard);

    state.programming.select(session.id, [first_fixture]);
    execute_programmer_command_from(&state, &session, "DYNAMIC 12", &context).unwrap();
    state.programming.select(session.id, [second_fixture]);
    execute_programmer_command_from(&state, &session, "DYNAMIC 12", &context).unwrap();
    state
        .programming
        .select(session.id, std::iter::empty::<light_core::FixtureId>());

    let choice = match execute_programmer_command_effect_from(
        &state,
        &session,
        "DYNAMIC 12 SIZE AT 50 TIME 2",
        &context,
    )
    .unwrap()
    {
        ProgrammerCommandExecution::ChoiceRequired(choice) => choice,
        ProgrammerCommandExecution::Applied(_) => {
            panic!("an ambiguous targetless command must not choose by storage order")
        }
    };
    assert_eq!(choice.dynamic_id, dynamic_id);
    assert_eq!(choice.pool_number, 12);
    assert_eq!(choice.options.len(), 2);
    assert!(choice.options.iter().all(|option| {
        option.command.contains(&format!(
            "DYNAMIC 12 INSTANCE {} SIZE AT 50 TIME 2",
            option.controller_id.simple()
        ))
    }));

    let chosen = choice.options[0].clone();
    let result = execute_programmer_command_effect_from(
        &state,
        &session,
        &chosen.command,
        &context.clone().with_request_id("choose-exact-dynamic"),
    )
    .unwrap();
    assert!(matches!(result, ProgrammerCommandExecution::Applied(0)));
    let programmer = state.programming.get(session.id).unwrap();
    let sizes = programmer
        .dynamic_values
        .iter()
        .filter_map(|stored| match &stored.value {
            light_dynamics::DynamicSemanticValue::DynamicOn {
                instance_link,
                overrides,
                ..
            } => Some((light_dynamics::programmer_dynamic_controller_id(programmer.id, *instance_link), overrides.size)),
            _ => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(sizes.get(&chosen.controller_id), Some(&0.5));
    assert_eq!(
        sizes.values().filter(|size| (**size - 1.0).abs() < f32::EPSILON).count(),
        1
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn preload_controller_edit_updates_projection_without_touching_live_runtime() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "preload-dynamic-edit".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let fixture = light_core::FixtureId::new();
    state.programming.select(session.id, [fixture]);
    let dynamic_id = uuid::Uuid::new_v4();
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.dynamics = vec![command_test_dynamic(dynamic_id, 1)].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let ports = super::dynamics_adapter::ServerDynamicsPorts {
        state: &state,
        session: &session,
    };
    let started = state
        .dynamics
        .start(
            &context,
            light_application::DynamicStartCommand {
                dynamic_id,
                targets: vec![fixture],
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: light_dynamics::DynamicValueTiming::default(),
                undo_group: None,
            },
            &ports,
        )
        .unwrap();
    assert!(state.programming.arm_preload(session.id, true));

    state
        .dynamics
        .update_controller(
            &context,
            light_application::DynamicControllerUpdate {
                controller_id: started.controller_id,
                size: Some(0.4),
                speed_multiplier: None,
                phase_offset_degrees: None,
                undo_group: Some("preload-size".into()),
            },
            &ports,
        )
        .unwrap();

    let runtime = state.output.dynamic_runtime_snapshot();
    assert_eq!(runtime.instances[0].controllers[0].size, 1.0);
    let programmer = state.programming.get(session.id).unwrap();
    assert!(
        programmer
            .preload_dynamic_pending
            .iter()
            .all(|stored| matches!(
                &stored.value,
                light_dynamics::DynamicSemanticValue::DynamicOn { overrides, .. }
                    if (overrides.size - 0.4).abs() < f32::EPSILON
            ))
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn target_bound_dynamic_command_uses_its_stored_targets_over_the_current_selection() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "target-bound-dynamic-command".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let selected_fixture = light_core::FixtureId::new();
    let stored_target = light_core::FixtureId::new();
    state.programming.select(session.id, [selected_fixture]);

    let dynamic_id = uuid::Uuid::new_v4();
    let mut definition = command_test_dynamic(dynamic_id, 13);
    definition.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![stored_target],
    };
    let mut stored_fixture = operational_fixture(stored_target);
    stored_fixture.address = Some(2);
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.fixtures = vec![operational_fixture(selected_fixture), stored_fixture].into();
    snapshot.dynamics = vec![definition].into();
    state.output.replace_snapshot(snapshot).unwrap();

    let context = operator_action_context(&session, light_application::ActionSource::Keyboard);
    assert_eq!(
        execute_programmer_command_from(&state, &session, "DYNAMIC 13", &context).unwrap(),
        1
    );

    let runtime = state.output.dynamic_runtime_snapshot();
    assert_eq!(runtime.instances.len(), 1);
    assert_eq!(runtime.instances[0].targets, vec![stored_target]);
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(programmer.selected, vec![selected_fixture]);
    assert_eq!(programmer.dynamic_values.len(), 1);
    assert_eq!(programmer.dynamic_values[0].fixture_id, stored_target);

    let _ = std::fs::remove_dir_all(data_dir);
}

fn command_test_dynamic(id: uuid::Uuid, pool_number: u16) -> light_dynamics::DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "pool_number": pool_number,
        "revision": 1,
        "name": "Command test wave",
        "color": null,
        "icon": null,
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": uuid::Uuid::new_v4(),
            "attribute": "intensity",
            "mode": "keyframes",
            "keyframes": {
                "points": [
                    {"position": 0.0, "source": {"type": "value", "value": 0.25}, "interpolation": "linear"},
                    {"position": 0.5, "source": {"type": "value", "value": 0.75}, "interpolation": "linear"}
                ],
                "size": 1.0
            },
            "max_min": {
                "minimum": {"type": "value", "value": 0.25},
                "maximum": {"type": "value", "value": 0.75},
                "function": "sinus",
                "size": 1.0,
                "pwm": {
                    "attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                    "attack_interpolation": "linear", "decay_interpolation": "linear"
                }
            },
            "middle_amplitude": {
                "middle": {"type": "current"},
                "amplitude": 0.25,
                "function": "sinus",
                "size": 1.0,
                "pwm": {
                    "attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                    "attack_interpolation": "linear", "decay_interpolation": "linear"
                }
            },
            "speed_multiplier": {"numerator": 1, "denominator": 1},
            "width": 1.0,
            "random_group_id": null
        }],
        "random_groups": [],
        "phase": {
            "ordering": {"type": "selection"},
            "offset_degrees": 0.0,
            "span_degrees": 360.0,
            "block_size": 1,
            "repeats": 1,
            "wings": false,
            "anchors_degrees": []
        },
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}

#[test]
fn empty_selection_commands_are_quiet_noops_but_malformed_commands_still_fail() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(), token: "quiet-empty-selection".into(),
        connected: true, desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    for blind in [false, true] {
        state.programming.set_modes(session.id, Some(blind), None, None, None);
        let before = state.programming.get(session.id).unwrap();
        for command in ["AT 50", "AT + 10", "AT 0 THRU 50 THRU 100", "FixAT 50", "ATTRIBUTE pan FixAT 50", "RELEASE", "RELEASE COLOR"] {
            assert_eq!(execute_programmer_command_from(&state, &session, command, &context), Ok(0), "{command}");
        }
        for command in ["AT invalid", "AT 101", "AT 1 2", "AT 0 THRU 500", "FixAT nope", "FixAT 101", "RELEASE invalid"] {
            assert!(execute_programmer_command_from(&state, &session, command, &context).is_err(), "{command}");
        }
        let after = state.programming.get(session.id).unwrap();
        assert_eq!(serde_json::to_value(&after).unwrap(), serde_json::to_value(&before).unwrap());
        assert_eq!(after.undo.len(), before.undo.len());
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn quiet_command_guards_preserve_empty_live_groups_and_explicit_noops() {
    // These are scalar production-contract guards. Semantic Position retains requested intent
    // even on an unsupported fixture; its separate contract1 route tests assert that behavior.
    let (state, data_dir) = test_state_with_programming_contract(ProgrammerRegistry::default(), None, 0);
    let session = Session { capability: light_core::SurfaceCapability::Programming, id: SessionId::new(), token: "quiet-group-and-aim".into(), connected: true, desk: test_control_desk() };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let mut first = operational_fixture(light_core::FixtureId::new());
    first.fixture_number = Some(1);
    let mut target = operational_fixture(light_core::FixtureId::new());
    target.fixture_number = Some(2);
    target.address = Some(2);
    let snapshot = light_engine::EngineSnapshot {
        fixtures: vec![first, target].into(),
        groups: vec![light_programmer::GroupDefinition { id: "1".into(), ..Default::default() }].into(),
        ..Default::default()
    };
    state.output.replace_snapshot(snapshot).unwrap();
    let show_path = data_dir.join("shows/quiet-groups.show");
    let show_id = default_show::initialise_legacy_test_show(&show_path).unwrap();
    let entry = ShowEntry { is_base_show: false, id: show_id, name: "Quiet group".into(), path: show_path.display().to_string(), revision: 0, updated_at: String::new(), created_at: None, last_loaded_at: None, revision_copy: None };
    state.active_show.replace_current(Some(entry.clone()));
    let preset = light_programmer::Preset {

fixture_replacement_projections: Default::default(),
group_replacement_projections: Default::default(),
family: light_programmer::PresetFamily::Intensity, number: 7,
        group_values: HashMap::from([("1".into(), HashMap::from([(light_core::AttributeKey::intensity(), light_core::AttributeValue::Normalized(0.8))]))]),
        ..Default::default()
    };
    ActiveShowRepository::open(&entry.path).unwrap().put_object("preset", "1.7", &serde_json::to_value(preset).unwrap(), 0).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    for blind in [false, true] {
        state.programming.select(session.id, []);
        state.programming.set_modes(session.id, Some(blind), None, None, None);
        let before = state.programming.get(session.id).unwrap();
        for command in ["AT 1.7", "FixAT INTENSITY PRESET 7", "FIXTURE 1 AT 1.7", "FIXTURE 1 RELEASE COLOR", "FIXTURE 1 AT FIXTURE 2"] {
            assert_eq!(execute_programmer_command_from(&state, &session, command, &context), Ok(0), "{command}");
        }
        for command in ["FIXTURE 1 AT FIXTURE 999", "AT 1.999", "FixAT INTENSITY PRESET 999"] {
            assert!(execute_programmer_command_from(&state, &session, command, &context).is_err(), "{command}");
        }
        let after = state.programming.get(session.id).unwrap();
        assert_eq!(serde_json::to_value(&after).unwrap(), serde_json::to_value(&before).unwrap());
        assert_eq!(after.undo.len(), before.undo.len());
        execute_programmer_command_from(&state, &session, "GROUP 1", &context).unwrap();
        execute_programmer_command_from(&state, &session, "AT 50", &context).unwrap();
        let current = state.programming.get(session.id).unwrap();
        let groups = if blind { &current.preload_group_pending } else { &current.group_values };
        assert_eq!(groups["1"][&light_core::AttributeKey::intensity()].value, light_core::AttributeValue::Normalized(0.5));
        execute_programmer_command_from(&state, &session, "AT 1.7", &context).unwrap();
        let current = state.programming.get(session.id).unwrap();
        let groups = if blind { &current.preload_group_pending } else { &current.group_values };
        assert_eq!(groups["1"][&light_core::AttributeKey::intensity()].value, light_core::AttributeValue::Normalized(0.8));
        execute_programmer_command_from(&state, &session, "RELEASE", &context).unwrap();
        let current = state.programming.get(session.id).unwrap();
        let groups = if blind { &current.preload_group_pending } else { &current.group_values };
        assert!(groups.get("1").is_none_or(|attributes| attributes.is_empty()));
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

fn fat_scope_fixture(
    virtual_dimmer: bool,
) -> (light_fixture::PatchedFixture, [light_core::FixtureId; 3]) {
    use crate::runtime::output_scheduler::physical_adapters::color::profiles::{patched, rgbw};
    let mut profile = rgbw();
    let mode = &mut profile.modes[0];
    mode.heads[0].master_shared = false;
    let intensity = mode
        .channels
        .iter()
        .find(|channel| channel.attribute.is_intensity())
        .unwrap()
        .clone();
    if virtual_dimmer {
        mode.channels
            .retain(|channel| !channel.attribute.is_intensity());
        mode.default_virtual_dimmer_reactions();
    }
    for (index, attribute) in [(1, "intensity"), (2, "pan")] {
        let head = uuid::Uuid::new_v4();
        mode.heads.push(light_fixture::FixtureHead {
            id: head,
            name: format!("Head {index}"),
            master_shared: false,
        });
        let mut channel = intensity.clone();
        channel.id = uuid::Uuid::new_v4();
        channel.head_id = head;
        channel.attribute = light_core::AttributeKey(attribute.into());
        channel.fixture_attribute = channel.attribute.clone();
        channel.functions[0].attribute = channel.attribute.clone();
        if index == 1 {
            channel.functions[0].behavior = light_fixture::ChannelFunctionBehavior::Fixed {
                semantic_id: "closed".into(),
                label: "Closed".into(),
                raw_value: 0,
            };
        }
        mode.channels.push(channel);
    }
    mode.splits[0].footprint = mode
        .channels
        .iter()
        .map(|channel| channel.resolution.bytes() as u16)
        .sum();
    let root = light_core::FixtureId::new();
    let targets = std::array::from_fn(|_| light_core::FixtureId::new());
    let mut fixture = patched(&profile, root, 1);
    fixture.fixture_number = Some(9004);
    fixture.logical_heads = targets
        .iter()
        .enumerate()
        .map(|(index, id)| light_fixture::PatchedHead {
            fixture_id: *id,
            head_index: index as u16,
            profile_head_id: Some(profile.modes[0].heads[index].id),
        })
        .collect();
    (fixture, targets)
}

#[test]
fn fat_logical_head_accepts_physical_and_projected_virtual_intensity() {
    for virtual_dimmer in [false, true] {
        let (state, data_dir) = test_state();
        let session = Session {
            capability: light_core::SurfaceCapability::Programming,
            id: SessionId::new(),
            token: "fat-head-positive".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programming.start(session.id);
        let (fixture, targets) = fat_scope_fixture(virtual_dimmer);
        let parameter = fixture.definition.heads[0]
            .parameters
            .iter()
            .find(|parameter| parameter.attribute.is_intensity())
            .unwrap();
        assert_eq!(parameter.virtual_dimmer, virtual_dimmer);
        assert_eq!(parameter.components.is_empty(), virtual_dimmer);
        state
            .output
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![fixture].into(),
                ..Default::default()
            })
            .unwrap();
        state.programming.select(session.id, [targets[0]]);
        let context = operator_action_context(&session, light_application::ActionSource::Http);
        assert_eq!(
            execute_programmer_command_from(
                &state,
                &session,
                "ATTRIBUTE intensity FIXAT 50 TIME 2 DELAY 1",
                &context
            )
            .unwrap(),
            1
        );
        let programmer = state.programming.get(session.id).unwrap();
        assert!(programmer.values.is_empty());
        assert_eq!(programmer.dynamic_values.len(), 1);
        assert_eq!(programmer.dynamic_values[0].fixture_id, targets[0]);
        assert!(
            matches!(programmer.dynamic_values[0].value, light_dynamics::DynamicSemanticValue::FixAt { value, timing: light_dynamics::DynamicValueTiming { fade_millis: Some(2000), delay_millis: Some(1000) } } if value == 0.5)
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[test]
fn fat_logical_head_rejections_are_atomic_and_do_not_borrow_other_heads() {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "fat-head-negative".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let (fixture, targets) = fat_scope_fixture(false);
    let root = fixture.fixture_id;
    state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..Default::default()
        })
        .unwrap();
    for (selection, expected) in [
        (vec![targets[1]], "discrete"),
        (vec![targets[0], targets[1]], "discrete"),
        (vec![targets[2]], "unsupported"),
        (vec![targets[0], targets[2]], "unsupported"),
        (
            vec![targets[0], light_core::FixtureId::new()],
            "unsupported",
        ),
        (vec![root], "discrete"),
    ] {
        state.programming.select(session.id, selection);
        let before = state.programming.get(session.id).unwrap();
        let context = operator_action_context(&session, light_application::ActionSource::Http);
        let error = execute_programmer_command_from(
            &state,
            &session,
            "ATTRIBUTE intensity FIXAT 50",
            &context,
        )
        .unwrap_err();
        assert!(error.contains(expected), "{error}");
        let after = state.programming.get(session.id).unwrap();
        assert_eq!(
            serde_json::to_value(after).unwrap(),
            serde_json::to_value(before).unwrap()
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}
