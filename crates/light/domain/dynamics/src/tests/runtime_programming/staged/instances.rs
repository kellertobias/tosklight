//! TL-639 round 7: instances pinned and completed on workers equal the single-threaded frame,
//! including the journal a failed frame restores.
use super::*;
use crate::{
    CompletedChunk, InstanceWorkers, PreparationSources, PreparationWorkers, PreparedChunk,
};
use light_core::programming::TransitionError;

/// Runs every index on its own thread, started in reverse order.
struct ReverseThreads;

/// Instances run by [`ReverseThreads`] (pinning and completion each count every instance).
static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl InstanceWorkers for ReverseThreads {
    fn run_indexed(&self, count: usize, run: &(dyn Fn(usize) + Sync)) {
        RUN.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
        std::thread::scope(|scope| {
            for index in (0..count).rev() {
                scope.spawn(move || run(index));
            }
        });
    }
}

/// Typed Current, shareable between threads.
struct Typed(DynamicValue);

impl DynamicValueSourceResolver for Typed {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        Some(self.0.clone())
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}

/// Completion chunks evaluated in turn against forks answering as the frame's sources.
struct InlineWorkers<'a>(&'a Typed);
struct InlineFork<'a>(&'a Typed);

impl DynamicValueSourceResolver for InlineFork<'_> {
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.0.current(target, address)
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}

impl PreparationSources for InlineFork<'_> {
    fn finish_controller(&self) -> Option<(usize, usize)> {
        Some((0, 0))
    }
}

impl PreparationWorkers for InlineWorkers<'_> {
    fn chunks(&self) -> usize {
        3
    }
    fn run(
        &mut self,
        _: &(dyn Fn(usize, &dyn PreparationSources) -> PreparedChunk + Sync),
    ) -> Result<Vec<PreparedChunk>, TransitionError> {
        unreachable!("completion only")
    }
    fn run_completion(
        &mut self,
        complete: &(dyn Fn(usize, &dyn PreparationSources) -> CompletedChunk + Sync),
    ) -> Result<Vec<CompletedChunk>, TransitionError> {
        Ok((0..3)
            .map(|chunk| complete(chunk, &InlineFork(self.0)))
            .collect())
    }
    fn append_logs(&mut self, _: usize, _: (usize, usize), _: (usize, usize)) {}
}

fn random_lane() -> (DynamicLane, DynamicRandomGroup) {
    let mut lane = lane();
    lane.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group_id = Uuid::from_u128(1702);
    lane.random_group_id = Some(group_id);
    let group = DynamicRandomGroup {
        id: group_id,
        seed: 17,
        range: DynamicRandomRange::LegacyScalar {
            low: source(0.0),
            high: source(1.0),
        },
        decision_interval_millis: 100,
        start_probability: 0.7,
        mean_duration_millis: 250,
        duration_spread_millis: 30,
        attack_ratio: 0.1,
        decay_ratio: 0.5,
    };
    (lane, group)
}

/// Three Dynamics (Keyframes with Current, a Random lane, and one the caller pauses and
/// resumes under JoinSyncNow; with `typed`, the first and last add a typed Focus lane), built
/// once so two runtimes share lane ids.
fn definitions(typed: bool) -> Vec<DynamicDefinition> {
    let mut keyframes = definition(lane());
    keyframes.id = Uuid::from_u128(11);
    let (random_lane, group) = random_lane();
    let mut random = definition(random_lane);
    random.id = Uuid::from_u128(12);
    random.random_groups.push(group);
    let mut held = definition(lane());
    held.id = Uuid::from_u128(13);
    if typed {
        keyframes.lanes.push(focus_lane());
        held.lanes.push(focus_lane());
    }
    vec![keyframes, random, held]
}

/// The Dynamics started on three targets each, with derived instance identities so two
/// runtimes order their instances alike; the held instance and its controller.
fn runtime(definitions: &[DynamicDefinition]) -> (DynamicRuntime, Uuid, DynamicController) {
    let targets = (1..=3)
        .map(|target| FixtureId(Uuid::from_u128(target)))
        .collect::<Vec<_>>();
    let mut runtime = DynamicRuntime::default();
    runtime.derive_instance_ids_from(Uuid::from_u128(99));
    runtime
        .install_definitions(definitions.iter().cloned())
        .unwrap();
    let mut held = None;
    for (index, definition) in definitions.iter().enumerate() {
        let control = controller(101 + index as u128, 1, false);
        let mut request = start_request(definition.id, control.clone(), targets[0], 0, false);
        request.target_scope.ordered_targets = targets.clone();
        if index == 2 {
            request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
            request.activation_duration_millis = 400;
        }
        let instance = runtime.start(request).unwrap();
        if index == 2 {
            held = Some((instance, control));
        }
    }
    let (instance, control) = held.unwrap();
    (runtime, instance, control)
}

type Frame = (Vec<DynamicRuntimeSample>, Vec<DynamicRuntimeSample>);

fn complete<'frame>(
    scalar: &'frame [DynamicRuntimeSample],
    deferred: DeferredTypedSampling<'frame>,
    typed: &Typed,
    workers: bool,
    fail: bool,
) -> Result<(CompletedDynamicSamples<'frame>, Frame), DynamicRuntimeError> {
    let scalar = scalar.to_vec();
    // Without typed lanes the plain completion applies on the scratch's workers.
    let completed = if workers && deferred.typed_lane_count() > 0 {
        deferred.complete_in_parallel(typed, &mut InlineWorkers(typed))?
    } else {
        deferred.complete(typed)?
    };
    if fail {
        return Err(DynamicRuntimeError::InvalidSample("rolled back".into()));
    }
    let samples = completed.samples().to_vec();
    Ok((completed, (scalar, samples)))
}

/// One staged frame, single-threaded or with every instance on its own thread; `fail` rolls
/// the frame back after completing it.
fn frame(
    runtime: &mut DynamicRuntime,
    scratch: &mut DynamicSamplingScratch,
    at: u64,
    workers: bool,
    fail: bool,
) -> Result<Frame, DynamicRuntimeError> {
    let typed = Typed(DynamicValue::Scalar(0.4));
    let mut transaction = DynamicOutputFrameScratch::default();
    runtime.with_output_frame_transaction(&mut transaction, |runtime| {
        if workers {
            scratch.set_instance_workers(Some(Arc::new(ReverseThreads)));
            runtime.sample_all_programming_staged_shared(
                at,
                10,
                &transports(at),
                &Sources { current: 0.3 },
                &typed,
                None,
                scratch,
                |scalar, deferred| complete(scalar, deferred, &typed, true, fail),
            )
        } else {
            runtime.sample_all_programming_staged(
                at,
                10,
                &transports(at),
                &Sources { current: 0.3 },
                &typed,
                None,
                scratch,
                |scalar, deferred| complete(scalar, deferred, &typed, false, fail),
            )
        }
    })
}

#[test]
fn instances_pinned_and_completed_on_workers_equal_the_single_threaded_frame() {
    for typed in [true, false] {
        compare_frames(&definitions(typed));
    }
    // Both sections ran on the threads: three instances pinned and completed per frame.
    assert_eq!(
        RUN.load(std::sync::atomic::Ordering::Relaxed),
        2 * (0..2400).step_by(37).count() * 6
    );
}

fn compare_frames(definitions: &[DynamicDefinition]) {
    let (mut single, held, control) = runtime(definitions);
    let (mut parallel, ..) = runtime(definitions);
    let mut single_scratch = DynamicSamplingScratch::default();
    let mut parallel_scratch = DynamicSamplingScratch::default();
    for at in (0..2400).step_by(37) {
        if at == 444 || at == 1332 {
            for runtime in [&mut single, &mut parallel] {
                runtime
                    .set_controller_paused_with_resume(
                        held,
                        control.id,
                        at == 444,
                        at,
                        Some(ActivationPolicy::JoinSyncNow),
                    )
                    .unwrap();
            }
        }
        // Every fifth frame fails after completing: both journals restore the same runtime.
        let fail = at % 5 == 0;
        let before = single.snapshot();
        let expected = frame(&mut single, &mut single_scratch, at, false, fail);
        let actual = frame(&mut parallel, &mut parallel_scratch, at, true, fail);
        match (expected, actual) {
            (Ok(expected), Ok(actual)) => {
                assert!(!expected.1.is_empty(), "frame {at} samples");
                assert_eq!(actual, expected, "frame {at} samples, in order");
            }
            (Err(_), Err(_)) => assert_eq!(single.snapshot(), before, "frame {at} rolled back"),
            (expected, actual) => panic!("frame {at}: {expected:?} against {actual:?}"),
        }
        assert_eq!(parallel.snapshot(), single.snapshot(), "frame {at} runtime");
    }
}
