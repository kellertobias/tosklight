use super::*;
use light_dynamics::{
    DynamicController, DynamicControllerSource, DynamicDefinition, DynamicRuntime,
    DynamicStartRequest, DynamicTargetScope,
};
use std::sync::mpsc;

pub(super) fn output() -> OutputResource {
    output_with_programmers(light_programmer::ProgrammerRegistry::default())
}

pub(super) fn output_with_programmers(
    programmers: light_programmer::ProgrammerRegistry,
) -> OutputResource {
    let events = EventBus::default();
    let engine = Arc::new(Engine::new(programmers));
    let publication = Arc::new(DynamicSnapshotPublication::new(engine.snapshot()));
    OutputResource::new(
        OutputRuntimeService::new(events.clone()),
        SpeedGroupService::new(events),
        engine,
        Arc::new(std::sync::Mutex::new(OutputHealth::default())),
        Arc::new(AtomicU16::new(40)),
        OutputControlCapability::new(Arc::new(Mutex::new(OutputControl::default()))),
        Arc::new(Mutex::new(TimecodeRouter::default())),
        None,
        Arc::new(light_output::UsbOutputFanout::new(Arc::new(
            light_output::UnavailableUsbDriverFactory,
        ))),
        Arc::default(),
        None,
        Arc::new(Mutex::new(std::array::from_fn(|_| {
            SpeedGroupController::new(120.0, Default::default()).unwrap()
        }))),
        Arc::new(Mutex::new(DynamicRuntime::default())),
        publication,
        SharedDynamicSourceOrigins::default(),
        Arc::default(),
        Arc::new(crate::runtime::visualization_frame::VisualizationFrameHub::default()),
    )
}

fn definition() -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Publisher A",
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"},
                {"position": 0.5, "source": {"type": "value", "value": 1.0}, "interpolation": "linear"}
            ]},
            "max_min": {"minimum": {"type": "value", "value": 0.0},
                "maximum": {"type": "value", "value": 1.0}, "function": "sinus"},
            "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.5, "function": "sinus"},
            "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
        }],
        "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0,
            "span_degrees": 0.0, "block_size": 1, "repeats": 1,
            "wings": false, "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}

#[test]
fn rejected_preload_install_keeps_pins_and_success_retires_obsolete_owner() {
    let output = output();
    let original = definition();
    output
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![original.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    output
        .start_dynamic(DynamicStartRequest {
            definition_id: original.id,
            controller: DynamicController {
                id: Uuid::new_v4(),
                source: DynamicControllerSource::Programmer {
                    programmer_id: Uuid::new_v4(),
                    instance_link: None,
                },
                priority: 1,
                activated_at_millis: 100,
                size: 1.,
                speed_multiplier: 1.,
                phase_offset_degrees: 25.,
                paused: true,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: vec![light_core::FixtureId::new()],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 100,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    output.set_dynamic_definitions_pinned(true);
    let mut latest = original.clone();
    latest.revision += 1;
    latest.name = "Edited during Blind".into();
    output
        .replace_snapshot(EngineSnapshot {
            revision: 2,
            dynamics: vec![latest.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    let stale = output
        .prepare_playback_batch(&[], output.engine.application_time(), 0)
        .unwrap();
    let mut changed = (*output.snapshot()).clone();
    changed.revision += 1;
    output.replace_snapshot(changed).unwrap();
    let snapshot = output.snapshot();
    let before = output.dynamic_runtime_snapshot();
    assert_eq!(before.instances[0].definition, original);
    let (cold, controls) = output
        .dynamic_snapshot
        .begin_retained_history(
            &mut output.dynamics.lock(),
            &snapshot,
            std::num::NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
    let input = output.dynamic_snapshot.input_capture_cursor();
    assert!(
        output
            .install_preload_playback_batch(stale, output.engine.application_time(), |_, _, _| Ok(
                ()
            ))
            .is_err()
    );
    assert_eq!(output.dynamic_runtime_snapshot(), before);
    assert_eq!(output.dynamics.lock().control_cursor(), Some(controls));
    assert_eq!(output.dynamic_snapshot.input_capture_cursor(), input);
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(cold)
            .unwrap()
            .is_empty()
    );

    let fresh = output
        .prepare_playback_batch(&[], output.engine.application_time(), 0)
        .unwrap();
    output
        .install_preload_playback_batch(fresh, output.engine.application_time(), |_, _, _| Ok(()))
        .unwrap();
    // The deliberately orphaned Programmer controller must not survive final owner reconciliation.
    assert!(output.dynamic_runtime_snapshot().instances.is_empty());
    assert_ne!(output.dynamic_snapshot.input_capture_cursor(), input);
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(cold)
            .is_err()
    );
}

#[test]
fn output_control_entry_points_publish_replayable_requests_with_application_time() {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(100_000).unwrap(),
    ));
    let output = output_with_programmers(light_programmer::ProgrammerRegistry::with_clock(
        clock.clone(),
    ));
    let definition = definition();
    output
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    let mut pending = output.dynamics.lock().fork_for_pending_preview();
    let mut cursor = output
        .dynamics
        .lock()
        .begin_control_recording(std::num::NonZeroUsize::new(32).unwrap());
    let beginning = cursor;
    let controller_id = Uuid::new_v4();
    let id = output
        .start_dynamic(DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: controller_id,
                source: DynamicControllerSource::Programmer {
                    programmer_id: Uuid::new_v4(),
                    instance_link: None,
                },
                priority: 1,
                activated_at_millis: 100_000,
                size: 1.,
                speed_multiplier: 1.,
                phase_offset_degrees: 0.,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: vec![light_core::FixtureId::new()],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 100_000,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    clock.advance_millis(25);
    output
        .update_dynamic_controller(controller_id, Some(0.5), Some(2.), None)
        .unwrap();
    let valid_cursor = output.dynamics.lock().control_cursor();
    assert!(
        output
            .update_dynamic_controller(controller_id, Some(f32::NAN), None, None)
            .is_err()
    );
    assert_eq!(output.dynamics.lock().control_cursor(), valid_cursor);
    output.set_dynamic_runtime_paused(true);
    clock.advance_millis(25);
    assert_eq!(
        output
            .off_dynamic_controller(controller_id, 100_050, 10, 20)
            .unwrap(),
        (id, false)
    );
    let batch = output
        .dynamics
        .lock()
        .controls_since(beginning)
        .unwrap()
        .unwrap();
    assert_eq!(
        batch.operation_times().collect::<Vec<_>>(),
        vec![100_000, 100_025, 100_025, 100_050]
    );
    light_dynamics::replay_dynamic_controls(
        &mut pending,
        &mut Default::default(),
        &mut cursor,
        &batch,
        None,
    )
    .unwrap();
    assert_eq!(pending.snapshot(), output.dynamic_runtime_snapshot());
    assert_eq!(pending.controller(controller_id).unwrap().0, id);
}

fn install(output: &OutputResource, prepared: PreparedOutputSnapshot, release: bool) {
    if release {
        output.install_prepared_snapshot_releasing_playback(prepared);
    } else {
        output.install_prepared_snapshot(prepared);
    }
}

fn wait_for_snapshot(engine: &Engine, expected: &Arc<EngineSnapshot>) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if Arc::ptr_eq(&engine.snapshot(), expected) {
            return true;
        }
        std::thread::yield_now();
    }
    false
}

#[test]
fn overlapping_preserve_and_release_publishers_serialize_the_complete_registry_pair() {
    for first_releases in [false, true] {
        let output = output();
        let first_definition = definition();
        let mut second_definition = first_definition.clone();
        second_definition.name = "Publisher B".into();
        second_definition.speed = light_dynamics::DynamicSpeed::Fixed {
            duration_millis: 2000,
        };
        let prepare = |definition: &DynamicDefinition| {
            output
                .prepare_snapshot(EngineSnapshot {
                    revision: 17,
                    dynamics: vec![definition.clone()].into(),
                    ..Default::default()
                })
                .unwrap()
        };
        let first = prepare(&first_definition);
        let second = prepare(&second_definition);
        let first_snapshot = first.engine.snapshot_arc();
        let second_snapshot = second.engine.snapshot_arc();
        assert_eq!(first_snapshot.revision, second_snapshot.revision);
        assert_eq!(first_definition.id, second_definition.id);
        assert_eq!(first_definition.revision, second_definition.revision);
        let initial_snapshot = output.snapshot();

        // A must publish its Engine snapshot, then stop at the real Dynamics mutex. This
        // forces the publication gap without a production test hook or a scheduling sleep.
        let runtime = output.dynamics.lock();
        let (first_done_tx, first_done_rx) = mpsc::channel();
        let first_output = output.clone();
        let first_thread = std::thread::spawn(move || {
            install(&first_output, first, first_releases);
            first_done_tx.send(()).unwrap();
        });
        let first_published = wait_for_snapshot(output.engine(), &first_snapshot);
        if !first_published {
            drop(runtime);
            first_done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            first_thread.join().unwrap();
            panic!("first publisher did not reach its Engine publication");
        }
        assert!(output.dynamic_snapshot.matches(&initial_snapshot));

        let (second_entered_tx, second_entered_rx) = mpsc::channel();
        let (second_done_tx, second_done_rx) = mpsc::channel();
        let second_output = output.clone();
        let second_thread = std::thread::spawn(move || {
            second_entered_tx.send(()).unwrap();
            install(&second_output, second, !first_releases);
            second_done_tx.send(()).unwrap();
        });
        second_entered_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        // The held Dynamics guard makes completion impossible. Give B a bounded chance to
        // attempt its installer, then inspect whether it improperly overtook A's Engine.
        let second_completed_early = second_done_rx.recv_timeout(Duration::from_millis(100));
        let engine_while_blocked = output.snapshot();
        let stamp_while_blocked = output.dynamic_snapshot.matches(&initial_snapshot);
        drop(runtime);

        first_done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if second_completed_early.is_err() {
            second_done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        first_thread.join().unwrap();
        second_thread.join().unwrap();

        assert!(
            Arc::ptr_eq(&engine_while_blocked, &first_snapshot),
            "second installer published Engine while the first registry was still pending"
        );
        assert!(stamp_while_blocked);
        assert!(matches!(
            second_completed_early,
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(Arc::ptr_eq(&output.snapshot(), &second_snapshot));
        let mut runtime = output.dynamics.lock();
        assert!(output.dynamic_snapshot.matches(&second_snapshot));
        let instance = runtime
            .start(DynamicStartRequest {
                definition_id: second_definition.id,
                controller: DynamicController {
                    id: Uuid::new_v4(),
                    source: DynamicControllerSource::physical_playback(1),
                    priority: 100,
                    activated_at_millis: 0,
                    size: 1.0,
                    speed_multiplier: 1.0,
                    phase_offset_degrees: 0.0,
                    paused: false,
                },
                target_scope: DynamicTargetScope {
                    ordered_targets: vec![light_core::FixtureId::new()],
                },
                stage_positions: HashMap::new(),
                inherited_spatial_mapping: None,
                now_millis: 0,
                activation_delay_millis: 0,
                activation_duration_millis: 0,
                activation_policy_override: None,
                reuse_matching_targetless: false,
            })
            .unwrap();
        assert_eq!(
            runtime.instance_definition(instance).unwrap().as_ref(),
            &second_definition,
            "a real start must use B's registry, not only B's publication stamp"
        );
    }
}
