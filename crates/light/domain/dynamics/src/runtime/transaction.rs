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

pub(super) struct SamplingUndo {
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
        let state = fresh_sampling(self.spare_sampling.remove(&instance.id), instance);
        self.warm.insert(instance.id, state);
    }
    /// Lend the journal of one instance's sampling to a worker (TL-639 round 7): its warm
    /// record and spare buffers move into the handle, which records exactly what this journal
    /// would; [`Self::take_back`] returns them.
    pub(super) fn lend(&mut self, id: Uuid) -> InstanceUndo {
        InstanceUndo {
            id,
            cold: self.cold.contains_key(&id),
            warm: self.warm.remove(&id),
            spare_sampling: self.spare_sampling.remove(&id),
            spare_last: self.spare_last.remove(&id),
        }
    }
    pub(super) fn take_back(&mut self, lent: InstanceUndo) {
        let InstanceUndo {
            id,
            cold: _,
            warm,
            spare_sampling,
            spare_last,
        } = lent;
        if let Some(warm) = warm {
            self.warm.insert(id, warm);
        }
        if let Some(spare) = spare_sampling {
            self.spare_sampling.insert(id, spare);
        }
        if let Some(spare) = spare_last {
            self.spare_last.insert(id, spare);
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

fn fresh_sampling(spare: Option<SamplingUndo>, instance: &DynamicInstance) -> SamplingUndo {
    let mut state = spare.unwrap_or_else(|| SamplingUndo {
        clock: SamplingClock::capture(instance),
        previous_last: None,
        random: HashMap::new(),
        unavailable: HashMap::new(),
        held_before: None,
    });
    state.clock = SamplingClock::capture(instance);
    state
}

impl SamplingUndo {
    /// Keep the instance's previous samples, once per frame, swapping in a spare buffer.
    fn begin_samples(
        &mut self,
        instance: &mut DynamicInstance,
        spare: impl FnOnce() -> Option<SampleValues>,
    ) {
        if self.previous_last.is_none() {
            let mut spare = spare().unwrap_or_default();
            spare.clear();
            self.previous_last = Some(std::mem::replace(&mut instance.last_sample_values, spare));
        }
    }
}

/// What sampling journals about one instance before it changes it: the frame's whole journal
/// ([`OutputFrameUndo`]) or one instance's lent part of it ([`InstanceUndo`], TL-639 round 7).
pub(super) trait SamplingJournal {
    /// The instance's warm record, created on first use; `None` while the instance is
    /// journaled whole (cold), which restores everything.
    fn warm(&mut self, instance: &DynamicInstance) -> Option<&mut SamplingUndo>;
    fn begin_samples(&mut self, instance: &mut DynamicInstance);

    fn random(&mut self, instance: &DynamicInstance, key: RandomKey) {
        if let Some(warm) = self.warm(instance) {
            warm.random
                .entry(key)
                .or_insert_with(|| instance.random_streams.get(&key).cloned());
        }
    }
    fn unavailable(&mut self, instance: &DynamicInstance, key: SampleKey) {
        if let Some(warm) = self.warm(instance) {
            warm.unavailable
                .entry(key)
                .or_insert_with(|| instance.unavailable_samples.get(&key).cloned());
        }
    }
    /// Only call before an actual hold-map mutation (initial capture, membership change or
    /// completed Resume). Steady playing/paused frames do not copy retained histories.
    fn held(&mut self, instance: &DynamicInstance) {
        if let Some(warm) = self.warm(instance) {
            warm.held_before.get_or_insert_with(|| {
                (
                    instance.synchronized_hold_values.clone(),
                    instance.synchronized_hold_angle_sources.clone(),
                )
            });
        }
    }
}

impl SamplingJournal for OutputFrameUndo {
    fn warm(&mut self, instance: &DynamicInstance) -> Option<&mut SamplingUndo> {
        self.sampling(instance);
        self.warm.get_mut(&instance.id)
    }
    fn begin_samples(&mut self, instance: &mut DynamicInstance) {
        self.sampling(instance);
        let id = instance.id;
        if let Some(warm) = self.warm.get_mut(&id) {
            warm.begin_samples(instance, || self.spare_last.remove(&id));
        }
    }
}

/// One instance's lent journal (TL-639 round 7), see [`OutputFrameUndo::lend`].
pub(super) struct InstanceUndo {
    id: Uuid,
    cold: bool,
    warm: Option<SamplingUndo>,
    spare_sampling: Option<SamplingUndo>,
    spare_last: Option<SampleValues>,
}

impl SamplingJournal for InstanceUndo {
    fn warm(&mut self, instance: &DynamicInstance) -> Option<&mut SamplingUndo> {
        debug_assert_eq!(
            instance.id, self.id,
            "a lent journal serves its own instance"
        );
        if self.cold {
            return None;
        }
        if self.warm.is_none() {
            self.warm = Some(fresh_sampling(self.spare_sampling.take(), instance));
        }
        self.warm.as_mut()
    }
    fn begin_samples(&mut self, instance: &mut DynamicInstance) {
        if self.warm(instance).is_none() {
            return;
        }
        let spare_last = &mut self.spare_last;
        if let Some(warm) = self.warm.as_mut() {
            warm.begin_samples(instance, || spare_last.take());
        }
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

/// A sampling journal as functions take it.
pub(super) type Journal = dyn SamplingJournal + 'static;

/// The frame's journal as a [`Journal`].
pub(super) fn journal(undo: &mut OutputFrameUndo) -> &mut Journal {
    undo
}
