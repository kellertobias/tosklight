use super::*;

#[test]
fn prepared_output_snapshot_keeps_controls_and_pause_changed_after_preparation() {
    for release_playback in [false, true] {
        let clock = Arc::new(ManualClock::new(fixed_test_time()));
        let (state, data_dir) = test_state_with_clock(clock.clone());
        let definition = command_test_dynamic(Uuid::new_v4(), 1);
        let controller_id = Uuid::new_v4();
        let now = clock.now().timestamp_millis() as u64;
        state
            .output
            .replace_snapshot(EngineSnapshot {
                dynamics: vec![definition.clone()].into(),
                ..Default::default()
            })
            .unwrap();
        state
            .output
            .start_dynamic(light_dynamics::DynamicStartRequest {
                definition_id: definition.id,
                controller: light_dynamics::DynamicController {
                    id: controller_id,
                    source: light_dynamics::DynamicControllerSource::Programmer {
                        programmer_id: Uuid::new_v4(),
                        instance_link: None,
                    },
                    priority: 0,
                    activated_at_millis: now,
                    size: 1.0,
                    speed_multiplier: 1.0,
                    phase_offset_degrees: 0.0,
                    paused: false,
                },
                target_scope: light_dynamics::DynamicTargetScope {
                    ordered_targets: vec![light_core::FixtureId::new()],
                },
                stage_positions: HashMap::new(),
                inherited_spatial_mapping: None,
                now_millis: now,
                activation_delay_millis: 0,
                activation_duration_millis: 0,
                activation_policy_override: None,
                reuse_matching_targetless: false,
            })
            .unwrap();
        let original = state.output.snapshot();
        let mut next = original.as_ref().clone();
        next.revision += 1;
        let prepared = state.output.prepare_snapshot(next).unwrap();
        assert!(Arc::ptr_eq(&original, &state.output.snapshot()));

        clock.advance_millis(325);
        state
            .output
            .update_dynamic_controller(controller_id, Some(0.4), Some(1.5), Some(35.0))
            .unwrap();
        state.output.set_dynamic_runtime_paused(true);
        let latest = state.output.dynamic_runtime_snapshot();
        assert!(latest.global_paused);
        assert_eq!(latest.instances[0].paused_at_millis, Some(now + 325));
        if release_playback {
            state
                .output
                .install_prepared_snapshot_releasing_playback(prepared);
        } else {
            state.output.install_prepared_snapshot(prepared);
        }
        assert_eq!(state.output.snapshot().revision, original.revision + 1);
        assert_eq!(state.output.dynamic_runtime_snapshot(), latest);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[test]
fn rejected_output_snapshot_leaves_engine_and_dynamic_state_unchanged() {
    let (state, data_dir) = test_state();
    let original = state.output.snapshot();
    let runtime = state.output.dynamic_runtime_snapshot();
    let mut definition = command_test_dynamic(Uuid::new_v4(), 1);
    definition.lanes[0].speed_multiplier.denominator = 0;
    let mut next = original.as_ref().clone();
    next.revision += 1;
    next.dynamics = vec![definition].into();
    assert!(state.output.prepare_snapshot(next).is_err());
    assert!(Arc::ptr_eq(&original, &state.output.snapshot()));
    assert_eq!(state.output.dynamic_runtime_snapshot(), runtime);
    let _ = std::fs::remove_dir_all(data_dir);
}
