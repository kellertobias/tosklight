//! Output calculations may fail after sampling. Keep semantic mutation reversible until the
//! caller commits the complete frame; immutable compiler/address memoization may stay warm.
use super::*;
use std::collections::HashSet;

type SampleKey = (Uuid, FixtureId, Uuid);
type SampleValues = super::SampleValueMap;
type RandomKey = (Uuid, FixtureId);

/// Reuse one workspace for each independently committed Live or Preload output lane.
#[derive(Default)]
pub struct DynamicOutputFrameScratch {
    undo: OutputFrameUndo,
}

#[derive(Default)]
pub(super) struct OutputFrameUndo {
    pub(super) recorded_controls: Vec<Arc<super::control_batch::RecordedControl>>,
    cold: HashMap<Uuid, Option<DynamicInstance>>,
    warm: HashMap<Uuid, SamplingUndo>,
    bound: HashMap<Uuid, Option<Uuid>>,
    fallback_definitions: HashSet<Uuid>,
    global_paused: Option<bool>,
    pub(super) sample_boundary: Option<Option<DynamicSampleBoundary>>,
    spare_last: HashMap<Uuid, SampleValues>,
    spare_sampling: HashMap<Uuid, SamplingUndo>,
}

struct SamplingUndo {
    clock: SamplingClock,
    previous_last: Option<SampleValues>,
    random: HashMap<RandomKey, Option<RandomStreamState>>,
    unavailable: HashMap<SampleKey, Option<Vec<NativeColorModelUnavailable>>>,
    held_before: Option<(SampleValues, HashSet<SampleKey>)>,
}

#[derive(Clone, Copy)]
struct SamplingClock {
    pending_until_millis: Option<u64>,
    speed_paused_at_millis: Option<u64>,
    speed_paused_elapsed_millis: u64,
    completed: bool,
    synchronized_hold_elapsed_millis: Option<u64>,
    synchronized_hold_captured: bool,
    last_synchronized_elapsed_millis: Option<u64>,
    synchronized_resume_transition: Option<DynamicSynchronizedResumeTransitionSnapshot>,
}
impl SamplingClock {
    fn capture(instance: &DynamicInstance) -> Self {
        Self {
            pending_until_millis: instance.pending_until_millis,
            speed_paused_at_millis: instance.speed_paused_at_millis,
            speed_paused_elapsed_millis: instance.speed_paused_elapsed_millis,
            completed: instance.completed,
            synchronized_hold_elapsed_millis: instance.synchronized_hold_elapsed_millis,
            synchronized_hold_captured: instance.synchronized_hold_captured,
            last_synchronized_elapsed_millis: instance.last_synchronized_elapsed_millis,
            synchronized_resume_transition: instance.synchronized_resume_transition,
        }
    }
    fn restore(self, instance: &mut DynamicInstance) {
        instance.pending_until_millis = self.pending_until_millis;
        instance.speed_paused_at_millis = self.speed_paused_at_millis;
        instance.speed_paused_elapsed_millis = self.speed_paused_elapsed_millis;
        instance.completed = self.completed;
        instance.synchronized_hold_elapsed_millis = self.synchronized_hold_elapsed_millis;
        instance.synchronized_hold_captured = self.synchronized_hold_captured;
        instance.last_synchronized_elapsed_millis = self.last_synchronized_elapsed_millis;
        instance.synchronized_resume_transition = self.synchronized_resume_transition;
    }
}
impl SamplingUndo {
    fn restore(&mut self, instance: &mut DynamicInstance) {
        self.clock.restore(instance);
        if let Some(previous) = &mut self.previous_last {
            std::mem::swap(previous, &mut instance.last_sample_values);
        }
        for (key, previous) in self.random.drain() {
            match previous {
                Some(previous) => {
                    instance.random_streams.insert(key, previous);
                }
                None => {
                    instance.random_streams.remove(&key);
                }
            }
        }
        for (key, previous) in self.unavailable.drain() {
            match previous {
                Some(previous) => {
                    instance.unavailable_samples.insert(key, previous);
                }
                None => {
                    instance.unavailable_samples.remove(&key);
                }
            }
        }
        if let Some((values, sources)) = &mut self.held_before {
            std::mem::swap(values, &mut instance.synchronized_hold_values);
            std::mem::swap(sources, &mut instance.synchronized_hold_angle_sources);
        }
    }
}
impl OutputFrameUndo {
    pub(super) fn record_instance(&mut self, id: Uuid, current: Option<&DynamicInstance>) {
        if self.cold.contains_key(&id) {
            return;
        }
        let mut previous = current.cloned();
        if let Some(mut warm) = self.warm.remove(&id) {
            if let Some(previous) = &mut previous {
                warm.restore(previous);
            }
            self.recycle_warm(id, warm);
        }
        self.cold.insert(id, previous);
    }
    pub(super) fn record_bound(&mut self, id: Uuid, previous: Option<Uuid>) {
        self.bound.entry(id).or_insert(previous);
    }
    pub(super) fn record_fallback(&mut self, id: Uuid) {
        self.fallback_definitions.insert(id);
    }
    pub(super) fn record_global_pause(&mut self, paused: bool) {
        self.global_paused.get_or_insert(paused);
    }
    pub(super) fn sampling(&mut self, instance: &DynamicInstance) {
        if self.cold.contains_key(&instance.id) || self.warm.contains_key(&instance.id) {
            return;
        }
        let mut state = self
            .spare_sampling
            .remove(&instance.id)
            .unwrap_or_else(|| SamplingUndo {
                clock: SamplingClock::capture(instance),
                previous_last: None,
                random: HashMap::new(),
                unavailable: HashMap::new(),
                held_before: None,
            });
        state.clock = SamplingClock::capture(instance);
        self.warm.insert(instance.id, state);
    }
    pub(super) fn begin_samples(&mut self, instance: &mut DynamicInstance) {
        self.sampling(instance);
        let Some(warm) = self.warm.get_mut(&instance.id) else {
            return;
        };
        if warm.previous_last.is_none() {
            let mut spare = self.spare_last.remove(&instance.id).unwrap_or_default();
            spare.clear();
            warm.previous_last = Some(std::mem::replace(&mut instance.last_sample_values, spare));
        }
    }
    pub(super) fn random(&mut self, instance: &DynamicInstance, key: RandomKey) {
        self.sampling(instance);
        if let Some(warm) = self.warm.get_mut(&instance.id) {
            warm.random
                .entry(key)
                .or_insert_with(|| instance.random_streams.get(&key).cloned());
        }
    }
    pub(super) fn unavailable(&mut self, instance: &DynamicInstance, key: SampleKey) {
        self.sampling(instance);
        if let Some(warm) = self.warm.get_mut(&instance.id) {
            warm.unavailable
                .entry(key)
                .or_insert_with(|| instance.unavailable_samples.get(&key).cloned());
        }
    }
    /// Only call before an actual hold-map mutation (initial capture, membership change or
    /// completed Resume). Steady playing/paused frames do not copy retained histories.
    pub(super) fn held(&mut self, instance: &DynamicInstance) {
        self.sampling(instance);
        if let Some(warm) = self.warm.get_mut(&instance.id) {
            warm.held_before.get_or_insert_with(|| {
                (
                    instance.synchronized_hold_values.clone(),
                    instance.synchronized_hold_angle_sources.clone(),
                )
            });
        }
    }
    fn restore(&mut self, runtime: &mut DynamicRuntime) {
        for (id, previous) in self.cold.drain() {
            match previous {
                Some(previous) => {
                    runtime.instances.insert(id, previous);
                }
                None => {
                    runtime.instances.remove(&id);
                }
            }
        }
        for (id, undo) in &mut self.warm {
            if let Some(instance) = runtime.instances.get_mut(id) {
                undo.restore(instance);
            }
        }
        for (id, previous) in self.bound.drain() {
            match previous {
                Some(previous) => {
                    runtime.bound_instances.insert(id, previous);
                }
                None => {
                    runtime.bound_instances.remove(&id);
                }
            }
        }
        for id in self.fallback_definitions.drain() {
            runtime.definitions.remove(&id);
            runtime.compiled_lanes.remove(&id);
        }
        if let Some(previous) = self.global_paused.take() {
            runtime.global_paused = previous;
        }
        if let Some(previous) = self.sample_boundary.take() {
            runtime.sample_boundary = previous;
        }
    }
    fn recycle_warm(&mut self, id: Uuid, mut warm: SamplingUndo) {
        if let Some(mut previous) = warm.previous_last.take() {
            previous.clear();
            self.spare_last.insert(id, previous);
        }
        warm.random.clear();
        warm.unavailable.clear();
        warm.held_before = None;
        self.spare_sampling.insert(id, warm);
    }
    fn recycle(&mut self, runtime: &DynamicRuntime) {
        self.recorded_controls.clear();
        self.cold.clear();
        self.bound.clear();
        self.fallback_definitions.clear();
        self.global_paused = None;
        self.sample_boundary = None;
        let mut warm = std::mem::take(&mut self.warm);
        for (id, state) in warm.drain() {
            self.recycle_warm(id, state);
        }
        self.warm = warm;
        self.spare_last
            .retain(|id, _| runtime.instances.contains_key(id));
        self.spare_sampling
            .retain(|id, _| runtime.instances.contains_key(id));
    }
}

impl DynamicRuntime {
    /// Commit sampling and output reconciliation only when the caller's complete calculation
    /// succeeds. The closure must include family composition and final frame encoding. Errors
    /// and unwinding restore semantic runtime state; immutable compilation caches may warm.
    /// Definition installation/restore are cold external boundaries, not output operations.
    pub fn with_output_frame_transaction<T, E>(
        &mut self,
        scratch: &mut DynamicOutputFrameScratch,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        assert!(
            self.output_frame_undo.is_none(),
            "Dynamic output transactions cannot nest"
        );
        self.output_frame_undo = Some(std::mem::take(&mut scratch.undo));
        struct Guard<'a> {
            runtime: &'a mut DynamicRuntime,
            scratch: &'a mut DynamicOutputFrameScratch,
            commit: bool,
        }
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                let mut undo = self
                    .runtime
                    .output_frame_undo
                    .take()
                    .expect("active output transaction");
                if !self.commit {
                    undo.restore(self.runtime);
                } else if let Some(journal) = &mut self.runtime.control_recording {
                    journal.append_recorded(std::mem::take(&mut undo.recorded_controls));
                }
                undo.recycle(self.runtime);
                self.scratch.undo = undo;
            }
        }
        let mut guard = Guard {
            runtime: self,
            scratch,
            commit: false,
        };
        let result = operation(guard.runtime);
        guard.commit = result.is_ok();
        result
    }

    pub(super) fn journal_instance(&mut self, id: Uuid) {
        if let Some(undo) = &mut self.output_frame_undo {
            undo.record_instance(id, self.instances.get(&id));
        }
    }
}
