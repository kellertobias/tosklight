use super::*;

fn transports() -> [DynamicSpeedTransport; 5] {
    [DynamicSpeedTransport {
        effective_bpm: 60.,
        phase_origin_millis: 0,
        phase_reference_millis: 0,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5]
}

fn rig() -> (DynamicRuntime, Uuid) {
    let definition = definition(lane());
    let id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let instance = runtime
        .start(start_request(
            id,
            controller(901, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    (runtime, instance)
}

fn sample(runtime: &mut DynamicRuntime, at: u64) {
    assert!(
        !runtime
            .sample_all_addressed(at, 10, &transports(), &Sources { current: 0.3 }, None)
            .is_empty()
    );
}

#[test]
fn sample_boundaries_distinguish_same_time_and_do_not_advance_on_controls() {
    let (mut runtime, _) = rig();
    assert_eq!(runtime.committed_sample_boundary(), None);
    sample(&mut runtime, 100);
    let first = runtime.committed_sample_boundary().unwrap();
    assert_eq!(first.sampled_at_millis(), 100);
    assert_eq!(first.scope(), DynamicSampleScope::WholeRuntime);
    runtime.set_global_paused(true, 101);
    assert_eq!(runtime.committed_sample_boundary(), Some(first));
    sample(&mut runtime, 100);
    let second = runtime.committed_sample_boundary().unwrap();
    assert_eq!(second.sampled_at_millis(), first.sampled_at_millis());
    assert_ne!(
        second, first,
        "time alone cannot identify the captured Current inputs"
    );
}

#[test]
fn sample_boundaries_remain_provisional_until_output_acceptance_and_rollback_with_history() {
    let (mut runtime, _) = rig();
    sample(&mut runtime, 100);
    let original = runtime.committed_sample_boundary();
    let history = runtime.snapshot();
    let mut transaction = DynamicOutputFrameScratch::default();
    for accept in [false, true] {
        let result: Result<(), &'static str> =
            runtime.with_output_frame_transaction(&mut transaction, |runtime| {
                sample(runtime, 200);
                assert_eq!(runtime.committed_sample_boundary(), original);
                assert_eq!(
                    runtime
                        .fork_for_pending_preview()
                        .committed_sample_boundary(),
                    None,
                    "a fork of provisional history cannot inherit an accepted anchor"
                );
                if accept {
                    Ok(())
                } else {
                    Err("final output rejected")
                }
            });
        if accept {
            result.unwrap();
            assert_ne!(runtime.committed_sample_boundary(), original);
            assert_eq!(
                runtime
                    .committed_sample_boundary()
                    .unwrap()
                    .sampled_at_millis(),
                200
            );
        } else {
            assert!(result.is_err());
            assert_eq!(runtime.committed_sample_boundary(), original);
            assert_eq!(runtime.snapshot(), history);
        }
    }
}

#[test]
fn staged_sample_boundary_is_committed_only_after_completion_and_outer_success() {
    let (mut runtime, _) = rig();
    sample(&mut runtime, 100);
    let original = runtime.committed_sample_boundary();
    let history = runtime.snapshot();
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut sampling = DynamicSamplingScratch::default();
    let sources = Sources { current: 0.3 };
    let typed = crate::programming::UnavailableProgrammingSources;
    for fail in [1, 2, 0] {
        let result: Result<(), DynamicRuntimeError> =
            runtime.with_output_frame_transaction(&mut transaction, |runtime| {
                runtime.sample_all_programming_staged(
                    200,
                    10,
                    &transports(),
                    &sources,
                    &typed,
                    None,
                    &mut sampling,
                    |_, deferred| {
                        let completed = deferred.complete(&typed)?;
                        if fail == 1 {
                            return Err(DynamicRuntimeError::InvalidSample(
                                "composition failed".into(),
                            ));
                        }
                        Ok((completed, ()))
                    },
                )?;
                assert_eq!(runtime.committed_sample_boundary(), original);
                if fail == 2 {
                    return Err(DynamicRuntimeError::InvalidSample(
                        "publication rejected".into(),
                    ));
                }
                Ok(())
            });
        if fail == 0 {
            result.unwrap();
            let accepted = runtime.committed_sample_boundary().unwrap();
            assert_ne!(Some(accepted), original);
            assert_eq!(accepted.scope(), DynamicSampleScope::WholeRuntime);
        } else {
            assert!(result.is_err());
            assert_eq!(runtime.committed_sample_boundary(), original);
            assert_eq!(runtime.snapshot(), history);
        }
    }
}

#[test]
fn forks_share_only_the_seed_boundary_and_restore_cannot_certify_a_capture() {
    let (mut runtime, _) = rig();
    sample(&mut runtime, 100);
    let original = runtime.committed_sample_boundary();
    let mut preview = runtime.fork_for_pending_preview();
    assert_eq!(preview.committed_sample_boundary(), original);
    sample(&mut preview, 100);
    assert_ne!(preview.committed_sample_boundary(), original);
    assert_eq!(runtime.committed_sample_boundary(), original);
    let mut invalid = runtime.snapshot();
    invalid.instances[0].controllers[0].size = f32::NAN;
    assert!(runtime.restore_snapshot(invalid).is_err());
    assert_eq!(runtime.committed_sample_boundary(), original);
    runtime.restore_snapshot(runtime.snapshot()).unwrap();
    assert_eq!(runtime.committed_sample_boundary(), None);
}

#[test]
fn partial_and_failed_untransactional_samples_cannot_claim_a_complete_boundary() {
    let (mut runtime, instance) = rig();
    sample(&mut runtime, 100);
    runtime
        .sample(instance, 200, 1000, 10, &Sources { current: 0.3 })
        .unwrap();
    assert_eq!(
        runtime.committed_sample_boundary().unwrap().scope(),
        DynamicSampleScope::Instance(instance)
    );
    assert!(
        runtime
            .sample(Uuid::new_v4(), 300, 1000, 10, &Sources { current: 0.3 })
            .is_err()
    );
    assert_eq!(runtime.committed_sample_boundary(), None);
}

#[test]
fn unwind_restores_sample_boundary_with_history() {
    let (mut runtime, _) = rig();
    sample(&mut runtime, 100);
    let original = runtime.committed_sample_boundary();
    let history = runtime.snapshot();
    let mut transaction = DynamicOutputFrameScratch::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.with_output_frame_transaction(&mut transaction, |runtime| -> Result<(), ()> {
            sample(runtime, 200);
            panic!("abort before output acceptance");
        })
    }));
    assert!(result.is_err());
    assert_eq!(runtime.committed_sample_boundary(), original);
    assert_eq!(runtime.snapshot(), history);
}
