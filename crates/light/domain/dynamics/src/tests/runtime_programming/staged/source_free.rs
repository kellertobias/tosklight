//! Pure completion must remain the ordinary sampler, with fewer empty fork closes only.
use super::*;
use crate::{CompletedChunk, PreparationSources, PreparationWorkers, PreparedChunk};
use light_core::programming::{ColorComponent, TransitionError};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Captured {
    epoch: u128,
    current: f32,
}
impl DynamicValueSourceResolver for Captured {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        Some(DynamicValue::Scalar(self.current))
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        Some(DynamicValue::Scalar(self.current * 0.75))
    }
    fn authored_occurrence(
        &self,
        _: Uuid,
        _: Uuid,
        target: FixtureId,
        lane: Uuid,
    ) -> Option<DynamicSourceOccurrenceId> {
        Some(
            DynamicSourceOccurrenceId::new(Uuid::from_u128(
                self.epoch + target.0.as_u128() + lane.as_u128(),
            ))
            .unwrap(),
        )
    }
    fn current_dependency(
        &self,
        _: FixtureId,
        address: &DynamicValueAddress,
    ) -> DynamicSourceDependency {
        DynamicSourceDependency::compatible(
            Some(DynamicSourceOccurrenceId::new(Uuid::from_u128(self.epoch)).unwrap()),
            address,
        )
    }
}
struct Workers<'a> {
    source: &'a Captured,
    opt_in: bool,
    stop: bool,
    closes: AtomicUsize,
    logs: Vec<(usize, (usize, usize), (usize, usize))>,
}
struct Fork<'a> {
    workers: &'a Workers<'a>,
    reads: Cell<usize>,
    missed: Cell<bool>,
}
impl DynamicValueSourceResolver for Fork<'_> {
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.reads.set(self.reads.get() + 1);
        if self.workers.stop
            && address.component == Some(ProgrammingComponent::Color(ColorComponent::Blue))
        {
            self.missed.set(true);
            None
        } else {
            self.workers.source.current(target, address)
        }
    }
    fn preset(
        &self,
        source: &DynamicPresetSourceBinding,
        instance: Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue> {
        self.reads.set(self.reads.get() + 1);
        self.workers.source.preset(source, instance, target)
    }
    fn current_dependency(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> DynamicSourceDependency {
        self.workers.source.current_dependency(target, address)
    }
}
impl PreparationSources for Fork<'_> {
    fn source_free_completion(&self) -> bool {
        self.workers.opt_in
    }
    fn finish_controller(&self) -> Option<(usize, usize)> {
        self.workers.closes.fetch_add(1, Ordering::Relaxed);
        (!self.missed.get()).then(|| (self.reads.get(), 0))
    }
}
impl PreparationWorkers for Workers<'_> {
    fn chunks(&self) -> usize {
        4
    }
    fn run(
        &mut self,
        _: &(dyn Fn(usize, &dyn PreparationSources) -> PreparedChunk + Sync),
    ) -> Result<Vec<PreparedChunk>, TransitionError> {
        unreachable!("completion only")
    }
    fn run_completion(
        &mut self,
        run: &(dyn Fn(usize, &dyn PreparationSources) -> CompletedChunk + Sync),
    ) -> Result<Vec<CompletedChunk>, TransitionError> {
        let workers = &*self;
        Ok(std::thread::scope(|scope| {
            let handles = (0..4)
                .map(|chunk| {
                    scope.spawn(move || {
                        run(
                            chunk,
                            &Fork {
                                workers,
                                reads: Cell::new(0),
                                missed: Cell::new(false),
                            },
                        )
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        }))
    }
    fn append_logs(&mut self, chunk: usize, from: (usize, usize), to: (usize, usize)) {
        self.logs.push((chunk, from, to));
    }
}
fn color(component: ColorComponent, low: DynamicValueSource) -> DynamicLane {
    let mut lane = ramp(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            },
            component: Some(ProgrammingComponent::Color(component)),
        },
        low,
        value(DynamicValue::Scalar(0.9)),
    );
    lane.id = Uuid::from_u128(match component {
        ColorComponent::Red => 10,
        ColorComponent::Green => 20,
        _ => 30,
    });
    lane
}
fn literal() -> DynamicValueSource {
    value(DynamicValue::Scalar(0.1))
}
fn setup(definition: &DynamicDefinition) -> (DynamicRuntime, Uuid) {
    let mut runtime = DynamicRuntime::default();
    runtime.derive_instance_ids_from(Uuid::from_u128(99));
    runtime.install_definitions([definition.clone()]).unwrap();
    let mut request = start_request(
        definition.id,
        controller(101, 7, false),
        FixtureId(Uuid::from_u128(1)),
        0,
        false,
    );
    // Targets cover every one of the four shard chunks, with several lanes per target.
    request.target_scope.ordered_targets = [1, 17, 33, 49]
        .map(|id| FixtureId(Uuid::from_u128(id)))
        .to_vec();
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 400;
    let id = runtime.start(request).unwrap();
    (runtime, id)
}
fn frame(
    runtime: &mut DynamicRuntime,
    at: u64,
    source: &Captured,
    mut workers: Option<&mut Workers<'_>>,
    fail: bool,
) -> Result<
    (
        Vec<DynamicRuntimeSample>,
        Vec<DynamicFamilyPreparationRequirement>,
    ),
    DynamicRuntimeError,
> {
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut scratch = DynamicSamplingScratch::default();
    runtime.with_output_frame_transaction(&mut transaction, |runtime| {
        runtime.sample_all_programming_staged(
            at,
            10,
            &transports(at),
            &Sources { current: 0.3 },
            source,
            None,
            &mut scratch,
            |_, deferred| {
                let completed = match workers.as_deref_mut() {
                    Some(workers) => deferred.complete_in_parallel(source, workers)?,
                    None => deferred.complete(source)?,
                };
                let samples = (
                    completed.samples().to_vec(),
                    completed.requirements().to_vec(),
                );
                if fail {
                    return Err(DynamicRuntimeError::InvalidSample(
                        "rollback after completion".into(),
                    ));
                }
                Ok((completed, samples))
            },
        )
    })
}
fn workers(source: &Captured, opt_in: bool, stop: bool) -> Workers<'_> {
    Workers {
        source,
        opt_in,
        stop,
        closes: AtomicUsize::new(0),
        logs: Vec::new(),
    }
}
#[test]
fn literal_completion_omits_empty_closes_but_preserves_full_samples_and_logs() {
    // Logged Current, pure literal, then a Current the fork cannot answer: later lanes
    // must replay on the frame with their original provenance and dependency evidence.
    for stop in [false, true] {
        let mut definition = definition(color(ColorComponent::Red, DynamicValueSource::Current));
        definition.lanes.extend([
            color(ColorComponent::Green, literal()),
            color(ColorComponent::Blue, DynamicValueSource::Current),
        ]);
        let (mut serial, _) = setup(&definition);
        let (mut ordinary, _) = setup(&definition);
        let (mut optimized, _) = setup(&definition);
        let source = Captured {
            epoch: 1000,
            current: 0.37,
        };
        let mut generic = workers(&source, false, stop);
        let mut pure = workers(&source, true, stop);
        let expected = frame(&mut serial, 211, &source, None, false).unwrap();
        assert_eq!(
            frame(&mut ordinary, 211, &source, Some(&mut generic), false).unwrap(),
            expected
        );
        assert_eq!(
            frame(&mut optimized, 211, &source, Some(&mut pure), false).unwrap(),
            expected
        );
        assert_eq!(
            pure.logs, generic.logs,
            "all ordered ranges including empty closes"
        );
        assert_eq!(
            pure.closes.load(Ordering::Relaxed) + 4,
            generic.closes.load(Ordering::Relaxed)
        );
        assert_eq!(optimized.snapshot(), serial.snapshot());
        assert_eq!(ordinary.snapshot(), serial.snapshot());
        assert!(expected.0.iter().all(|sample| sample.priority == 7
            && sample.activated_at_millis == 101
            && sample.activation_mix > 0.0));
    }
}
#[test]
fn hot_edits_size_pause_resume_and_rollback_remain_identical_to_generic_completion() {
    let mut definition = definition(color(ColorComponent::Green, literal()));
    let (mut serial, instance) = setup(&definition);
    let (mut parallel, _) = setup(&definition);
    for step in 0..12 {
        let at = 100 + step * 111;
        if [2, 3, 4, 5].contains(&step) {
            let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
                unreachable!()
            };
            let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
                unreachable!()
            };
            config.minimum = match step {
                2 => DynamicValueSource::Current,
                4 => DynamicValueSource::Preset {
                    preset_id: "color".into(),
                    address: body.address.clone(),
                    last_valid_by_target: vec![],
                    retained: None,
                },
                _ => value(DynamicValue::Scalar(0.2 + step as f32 * 0.03)),
            };
            definition.revision += 1;
            for runtime in [&mut serial, &mut parallel] {
                runtime.install_definitions([definition.clone()]).unwrap();
            }
        }
        for runtime in [&mut serial, &mut parallel] {
            if step == 6 || step == 7 {
                runtime
                    .update_controller(
                        Uuid::from_u128(101),
                        Some(if step == 6 { 0.4 } else { 1.0 }),
                        None,
                        None,
                    )
                    .unwrap();
            }
            if step == 7 || step == 9 {
                runtime
                    .set_controller_paused_with_resume(
                        instance,
                        Uuid::from_u128(101),
                        step == 7,
                        at,
                        Some(ActivationPolicy::JoinSyncNow),
                    )
                    .unwrap();
            }
            if step == 10 {
                runtime
                    .update_controller_rank(Uuid::from_u128(101), 9, at - 1, at)
                    .unwrap();
            }
        }
        let source = Captured {
            epoch: 1000 + step as u128 * 100,
            current: 0.25 + step as f32 * 0.01,
        };
        let mut pure = workers(&source, true, false);
        if step == 11 {
            let before = parallel.snapshot();
            assert!(frame(&mut parallel, at, &source, Some(&mut pure), true).is_err());
            assert_eq!(
                parallel.snapshot(),
                before,
                "failure restores last samples and runtime journal"
            );
        }
        let expected = frame(&mut serial, at, &source, None, false).unwrap();
        let actual = frame(&mut parallel, at, &source, Some(&mut pure), false).unwrap();
        assert_eq!(
            actual, expected,
            "step {step}: full expression, occurrence, dependency, rank and activation"
        );
        assert_eq!(
            parallel.snapshot(),
            serial.snapshot(),
            "step {step}: retained samples and runtime"
        );
        if step == 0 || step == 1 || step == 3 || step == 5 {
            assert_eq!(
                pure.closes.load(Ordering::Relaxed),
                0,
                "pure literals have no fork bookkeeping"
            );
        }
        if step == 2 || step == 6 {
            assert!(
                pure.closes.load(Ordering::Relaxed) > 0,
                "Current and non-unit Size must use generic path"
            );
        }
    }
}
