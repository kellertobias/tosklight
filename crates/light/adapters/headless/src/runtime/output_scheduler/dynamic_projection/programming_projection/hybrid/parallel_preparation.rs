//! The frame's workers and source forks for a parallel family preparation (TL-639 round 5).
//!
//! `light_dynamics` cuts the controllers by target and replays the workers' records in order
//! (`prepare_dynamic_family_samples_in_parallel`); this supplies the pool, one fork of the
//! cohort's sources per chunk (adopting through a private view of the Color and Focus/Zoom
//! lanes), and takes the forks' changes back. A controller that adopts Position Current needs the
//! Position lane, which stays on the frame's thread: its fork reports it, and the frame prepares
//! it in turn.
use super::super::CurrentResolutionRequirement;
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::family_lanes::{
    FamilyLanesWorker, FamilyStaging,
};
use light_dynamics::{
    CompletedChunk, CompletedDynamicSamples, DeferredTypedSampling,
    DynamicFamilyPreparationRequirement, DynamicRuntimeError, DynamicRuntimeSample,
    DynamicSourceDependency, DynamicSourceOccurrenceId, DynamicValue, PreparationSources,
    PreparationWorkers, PreparedChunk,
};
use std::cell::Cell;

/// Below this many samples a frame prepares them in turn (two in tests, as for the groups).
const MIN_PARALLEL_SAMPLES: usize = if cfg!(test) { 2 } else { 256 };

/// Prepare the cohort's family samples into `scratch`, on the pool when the observer lends its
/// lanes and the cohort is large enough. Lane stagings the forks took come back through
/// `observer`.
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_families<T>(
    samples: &[DynamicRuntimeSample],
    sampling_requirements: &[DynamicFamilyPreparationRequirement],
    typed: &CapturedProgrammingSources<'_, PreparedFamilySources<'_>>,
    frame: HybridFrameContext<'_>,
    native_models: &dyn DynamicNativeModelResolver,
    scratch: &mut DynamicFamilyPreparationScratch,
    observer: &mut impl HybridFrameObserver<T>,
    pool: Option<&light_engine::parallel::OutputPool>,
) -> Result<(), TransitionError> {
    let prepared = prepare_families_on(
        samples,
        sampling_requirements,
        typed,
        frame,
        native_models,
        scratch,
        observer,
        pool,
    );
    // TL-639 round 6: last frame's compiled samples nothing reused are freed on the pool.
    if let Some(pool) = pool {
        pool.drop_later(scratch.take_retired());
    }
    prepared
}

#[allow(clippy::too_many_arguments)]
fn prepare_families_on<T>(
    samples: &[DynamicRuntimeSample],
    sampling_requirements: &[DynamicFamilyPreparationRequirement],
    typed: &CapturedProgrammingSources<'_, PreparedFamilySources<'_>>,
    frame: HybridFrameContext<'_>,
    native_models: &dyn DynamicNativeModelResolver,
    scratch: &mut DynamicFamilyPreparationScratch,
    observer: &mut impl HybridFrameObserver<T>,
    pool: Option<&light_engine::parallel::OutputPool>,
) -> Result<(), TransitionError> {
    let Some(pool) = pool.filter(|_| samples.len() >= MIN_PARALLEL_SAMPLES && typed.forks()) else {
        return prepare_dynamic_family_samples_with_requirements(
            samples,
            sampling_requirements,
            typed,
            Some(native_models),
            scratch,
        )
        .map(drop);
    };
    if !lends_lanes(observer) {
        return prepare_dynamic_family_samples_with_requirements(
            samples,
            sampling_requirements,
            typed,
            Some(native_models),
            scratch,
        )
        .map(drop);
    }
    let mut workers = FrameWorkers::new(typed, frame, &*observer, pool);
    let result = light_dynamics::prepare_dynamic_family_samples_in_parallel(
        samples,
        sampling_requirements,
        typed,
        Some(native_models),
        scratch,
        &mut workers,
    )
    .map(drop);
    let stagings = workers.stagings;
    for staging in stagings {
        observer.merge_parallel(frame.token, staging)?;
    }
    result
}

/// Whether `observer` lends its lanes to parallel sections.
fn lends_lanes<T>(observer: &impl HybridFrameObserver<T>) -> bool {
    let mut lends = false;
    observer.with_parallel_lanes(&mut |lanes| lends = lanes.is_some());
    lends
}

/// Complete the frame's deferred typed lanes, on the pool when the observer lends its lanes and
/// the frame has enough of them (TL-639 round 6). Lane stagings the forks took come back
/// through `observer`.
pub(super) fn complete_deferred<'frame, T>(
    deferred: DeferredTypedSampling<'frame>,
    typed: &CapturedProgrammingSources<'_, PreparedFamilySources<'_>>,
    frame: HybridFrameContext<'_>,
    observer: &mut impl HybridFrameObserver<T>,
    pool: Option<&light_engine::parallel::OutputPool>,
) -> Result<CompletedDynamicSamples<'frame>, DynamicRuntimeError> {
    let Some(pool) =
        pool.filter(|_| deferred.typed_lane_count() >= MIN_PARALLEL_SAMPLES && typed.forks())
    else {
        return deferred.complete(typed);
    };
    if !lends_lanes(observer) {
        return deferred.complete(typed);
    }
    let mut workers = FrameWorkers::new(typed, frame, &*observer, pool);
    let result = deferred.complete_in_parallel(typed, &mut workers);
    let stagings = workers.stagings;
    for staging in stagings {
        observer
            .merge_parallel(frame.token, staging)
            .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
    }
    result
}

/// The pool and forks one parallel preparation or completion runs on. The observer lends its
/// lanes for each parallel section only (TL-639 round 6), so whatever the frame prepares or
/// completes itself between sections reaches the lanes as it would without workers.
struct FrameWorkers<'a, 't, O, T> {
    typed: &'a CapturedProgrammingSources<'t, PreparedFamilySources<'t>>,
    frame: HybridFrameContext<'a>,
    observer: &'a O,
    sidecar: std::marker::PhantomData<fn() -> T>,
    pool: &'a light_engine::parallel::OutputPool,
    /// Each chunk's ordered fork logs: requirements and failures.
    #[allow(clippy::type_complexity)]
    logs: Vec<(Vec<CurrentResolutionRequirement>, Vec<TransitionError>)>,
    /// The (empty) lane stagings the forks took, to give back.
    stagings: Vec<FamilyStaging>,
}

impl<'a, 't, O: HybridFrameObserver<T>, T> FrameWorkers<'a, 't, O, T> {
    fn new(
        typed: &'a CapturedProgrammingSources<'t, PreparedFamilySources<'t>>,
        frame: HybridFrameContext<'a>,
        observer: &'a O,
        pool: &'a light_engine::parallel::OutputPool,
    ) -> Self {
        Self {
            typed,
            frame,
            observer,
            sidecar: std::marker::PhantomData,
            pool,
            logs: Vec::new(),
            stagings: Vec::new(),
        }
    }
}

impl<O: HybridFrameObserver<T>, T> PreparationWorkers for FrameWorkers<'_, '_, O, T> {
    fn chunks(&self) -> usize {
        light_engine::parallel::chunk_count(light_dynamics::TARGET_SHARDS, self.pool.workers(), 1)
    }

    fn run(
        &mut self,
        prepare: &(dyn Fn(usize, &dyn PreparationSources) -> PreparedChunk + Sync),
    ) -> Result<Vec<PreparedChunk>, TransitionError> {
        self.run_forked(prepare)
    }

    fn run_completion(
        &mut self,
        complete: &(dyn Fn(usize, &dyn PreparationSources) -> CompletedChunk + Sync),
    ) -> Result<Vec<CompletedChunk>, TransitionError> {
        self.run_forked(complete)
    }

    fn append_logs(&mut self, chunk: usize, from: (usize, usize), to: (usize, usize)) {
        let (requirements, failures) = &self.logs[chunk];
        self.typed
            .append_fork_logs(&requirements[from.0..to.0], &failures[from.1..to.1]);
    }

    fn sort_controller_keys(&self, keys: &mut [light_dynamics::ControllerSortKey]) {
        light_engine::parallel::sort_unstable_by_key(Some(self.pool), keys, |key| *key);
    }

    fn sort_families(&self, families: &mut [light_dynamics::DynamicFamilySampleGroup]) {
        light_engine::parallel::sort_unstable_by_key(
            Some(self.pool),
            families,
            light_dynamics::family_order,
        );
    }
}

impl<O: HybridFrameObserver<T>, T> FrameWorkers<'_, '_, O, T> {
    /// Run `section(chunk, fork)` for every chunk on the pool, each against its own fork of the
    /// frame's sources, and take the forks' keyed changes back (their logs wait in `logs`).
    fn run_forked<R: Send>(
        &mut self,
        section: &(dyn Fn(usize, &dyn PreparationSources) -> R + Sync),
    ) -> Result<Vec<R>, TransitionError> {
        self.logs.clear();
        let chunks = self.chunks();
        let (typed, frame, pool) = (self.typed, self.frame, self.pool);
        let mut slots = vec![(); pool.workers()];
        let frozen = typed.freeze();
        let mut outputs = None;
        self.observer.with_parallel_lanes(&mut |lanes| {
            let Some((shared, _, _)) = lanes else {
                return;
            };
            outputs = Some(typed.with_fork_base(&frozen, |base| {
                light_engine::parallel::run_ordered(Some(pool), &mut slots, chunks, |_, chunk| {
                    let missed = Cell::new(false);
                    let lanes = FamilyLanesWorker::new(shared, &missed);
                    let adopt =
                        |target, original: &AttributeValue, address: &DynamicValueAddress| {
                            lanes.adopt(frame, target, original, address)
                        };
                    let fork = base.fork(&adopt);
                    let prepared = section(
                        chunk,
                        &PreparationFork {
                            typed: &fork,
                            missed: &missed,
                        },
                    );
                    (prepared, fork.into_changes(), lanes.into_staging())
                })
            }));
        });
        typed.thaw(frozen);
        let outputs = outputs.ok_or_else(|| {
            TransitionError::from(IntentError(
                "the frame observer stopped lending its lanes".into(),
            ))
        })?;
        let mut prepared = Vec::with_capacity(outputs.len());
        let mut failed = None;
        for (chunk, changes, staging) in outputs {
            match typed.apply_fork(changes) {
                Ok(logs) => self.logs.push(logs),
                Err(error) => {
                    failed.get_or_insert(error);
                    self.logs.push(Default::default());
                }
            }
            self.stagings.push(staging);
            prepared.push(chunk);
        }
        match failed {
            Some(error) => Err(error.into()),
            None => Ok(prepared),
        }
    }
}

/// One worker's fork, closing controllers for the preparation.
struct PreparationFork<'a, 'f> {
    typed: &'a CapturedProgrammingSources<'f, PreparedFamilySources<'f>>,
    missed: &'a Cell<bool>,
}

impl PreparationSources for PreparationFork<'_, '_> {
    fn finish_controller(&self) -> Option<(usize, usize)> {
        if self.missed.get() {
            self.typed.abort_group();
            return None;
        }
        self.typed.commit_group();
        Some(self.typed.log_lengths())
    }
}

/// Every source query answered by the fork, as the frame's sources answer it.
impl DynamicValueSourceResolver for PreparationFork<'_, '_> {
    fn try_position_current_family(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.typed.try_position_current_family(target, address)
    }

    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.typed.current(target, address)
    }

    fn try_current(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<DynamicValue>, TransitionError> {
        self.typed.try_current(target, address)
    }

    fn authored_occurrence(
        &self,
        instance_id: Uuid,
        controller_id: Uuid,
        target: FixtureId,
        lane_id: Uuid,
    ) -> Option<DynamicSourceOccurrenceId> {
        self.typed
            .authored_occurrence(instance_id, controller_id, target, lane_id)
    }

    fn current_family_occurrence(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<DynamicSourceOccurrenceId> {
        self.typed.current_family_occurrence(target, address)
    }

    fn current_dependency(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> DynamicSourceDependency {
        self.typed.current_dependency(target, address)
    }

    fn current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<AttributeValue> {
        self.typed.current_family_base(target, address)
    }

    fn try_current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.typed.try_current_family_base(target, address)
    }

    fn preset(
        &self,
        source: &light_dynamics::DynamicPresetSourceBinding,
        instance: Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue> {
        self.typed.preset(source, instance, target)
    }
}
