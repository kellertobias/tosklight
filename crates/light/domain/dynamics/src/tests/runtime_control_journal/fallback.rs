use super::*;

fn fallback(definition: &DynamicDefinition) -> TimedDynamicControl {
    timed(
        200,
        DynamicControl::InstallFallbackDefinition(Box::new(definition.clone())),
    )
}

#[test]
fn fallback_precedes_start_and_replay_preserves_pending_definition() {
    let definition = definition(lane());
    let owner = controller(2500, 1, false);
    let request = start_request(definition.id, owner.clone(), FixtureId::new(), 100, false);
    let mut live = DynamicRuntime::default();
    let mut missing = live.fork_for_pending_preview();
    let mut edited = live.fork_for_pending_preview();
    let mut current = definition.clone();
    current.name = "Pending edit remains authoritative".into();
    edited.install_definitions([current.clone()]).unwrap();
    let start = live.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let mut scratch = DynamicOutputFrameScratch::default();
    let id = live
        .with_output_frame_transaction(&mut scratch, |runtime| {
            assert!(
                runtime
                    .apply_recorded_control(fallback(&definition))?
                    .changed
            );
            assert!(
                !runtime
                    .apply_recorded_control(fallback(&definition))?
                    .changed
            );
            Ok::<_, DynamicRuntimeError>(
                runtime
                    .apply_recorded_control(timed(200, DynamicControl::Start(Box::new(request))))?
                    .instance_id
                    .unwrap(),
            )
        })
        .unwrap();
    let batch = live.controls_since(start).unwrap().unwrap();
    assert_eq!(batch.len(), 2);
    assert_eq!(batch.operation_times().collect::<Vec<_>>(), [200, 200]);
    for runtime in [&mut missing, &mut edited] {
        let mut cursor = start;
        replay_dynamic_controls(runtime, &mut scratch, &mut cursor, &batch, None).unwrap();
        assert_eq!(cursor, batch.to());
        assert_eq!(runtime.controller(owner.id).unwrap().0, id);
    }
    assert_eq!(missing.snapshot(), live.snapshot());
    assert_eq!(edited.instance_definition(id).unwrap().as_ref(), &current);
    assert_eq!(edited.snapshot().instances[0].started_at_millis, 100);
}

#[test]
fn fallback_and_following_controls_rollback_together_and_retry() {
    let definition = definition(lane());
    let mut live = DynamicRuntime::default();
    let mut pending = live.fork_for_pending_preview();
    let start = live.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), DynamicRuntimeError> =
        live.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.apply_recorded_control(fallback(&definition))?;
            Err(DynamicRuntimeError::MissingController)
        });
    assert!(failed.is_err());
    assert_eq!(live.control_cursor(), Some(start));
    assert!(live.controls_since(start).unwrap().unwrap().is_empty());
    let request = start_request(
        definition.id,
        controller(2501, 1, false),
        FixtureId::new(),
        200,
        false,
    );
    assert!(
        live.start(request.clone()).is_err(),
        "rolled-back fallback must be absent"
    );
    assert!(
        live.apply_recorded_control(fallback(&definition))
            .unwrap()
            .changed
    );
    // Replay installs the fallback, then fails because this branch already occupies the
    // recorded non-reusable instance identity. Both installation and cursor must roll back.
    let occupied_id = live
        .apply_recorded_control(timed(200, DynamicControl::Start(Box::new(request.clone()))))
        .unwrap()
        .instance_id
        .unwrap();
    let batch = live.controls_since(start).unwrap().unwrap();
    pending.install_definitions([definition.clone()]).unwrap();
    pending
        .start_with_instance_identity(request, occupied_id)
        .unwrap();
    pending.install_definitions([]).unwrap();
    let before = pending.snapshot();
    let mut cursor = start;
    assert!(
        replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &batch, None).is_err()
    );
    assert_eq!(cursor, start);
    assert_eq!(pending.snapshot(), before);
    assert!(
        pending
            .apply_recorded_control(fallback(&definition))
            .unwrap()
            .changed,
        "failed replay must not retain the fallback"
    );
}
