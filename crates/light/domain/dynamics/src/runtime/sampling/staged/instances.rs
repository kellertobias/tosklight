//! Pinning and completing a frame's instances on several threads (TL-639 round 7).
//!
//! Pinning an instance reads the frame's immutable sources and its own definition, and writes
//! only that instance: its clocks, Random streams and held values, its recycled controller work
//! and its part of the output-frame journal. Completing a pinned instance (once every typed
//! lane's evaluation is known) writes the instance's lane caches, held and last samples, its
//! journal part, and its own run of samples and requirements. Nothing one instance does is read
//! by another, so the frame lends each instance its journal part ([`OutputFrameUndo::lend`]),
//! runs the instances on the workers, and takes back the journals, samples and requirements in
//! the frame's instance order. The result is the single-threaded loop's for any worker count.
//!
//! [`OutputFrameUndo::lend`]: transaction::OutputFrameUndo::lend
use super::resolve::Evaluated;
use super::*;
use std::sync::Mutex;

/// Below this many pinned lanes (targets times lanes) a frame pins its instances in turn.
const MIN_PARALLEL_PIN_LANES: usize = if cfg!(test) { 2 } else { 256 };

/// The caller's worker threads for a frame's per-instance work.
pub trait InstanceWorkers: Send + Sync {
    /// Run `run(index)` once for every index below `count` on the workers and return when every
    /// call has returned. Which thread ran an index is never observable.
    fn run_indexed(&self, count: usize, run: &(dyn Fn(usize) + Sync));
}

/// One instance's prepared frame, waiting to be pinned.
pub(super) struct PinJob {
    pub(super) instance_id: Uuid,
    pub(super) cycle_duration_millis: u64,
    pub(super) frame: SamplingFrame,
    pub(super) addresses: Arc<[Option<FrameAddress>]>,
}

/// The scalar and authored sources pinning reads; only shared ones pin on workers.
#[derive(Clone, Copy)]
pub(super) enum PinSources<'a> {
    Local(
        &'a dyn ScalarSourceResolver,
        &'a dyn DynamicValueSourceResolver,
    ),
    Shared(
        &'a (dyn ScalarSourceResolver + Sync),
        &'a (dyn DynamicValueSourceResolver + Sync),
    ),
}

impl<'a> PinSources<'a> {
    fn local(
        self,
    ) -> (
        &'a dyn ScalarSourceResolver,
        &'a dyn DynamicValueSourceResolver,
    ) {
        match self {
            Self::Local(sources, authored) => (sources, authored),
            Self::Shared(sources, authored) => (sources, authored),
        }
    }
}

/// The frame-wide inputs every pinned instance reads.
pub(super) struct PinInputs<'a> {
    pub(super) now_millis: u64,
    pub(super) output_interval_millis: u64,
    pub(super) sources: PinSources<'a>,
}

impl DynamicRuntime {
    /// Pin `jobs` in order, on `workers` when there are enough of them.
    pub(super) fn pin_jobs(
        &mut self,
        jobs: Vec<PinJob>,
        inputs: &PinInputs<'_>,
        buffers: &mut SamplingWorkBuffers,
        workers: Option<&dyn InstanceWorkers>,
    ) -> Result<Vec<PinnedInstance>, DynamicRuntimeError> {
        let lanes = jobs
            .iter()
            .map(|job| job.frame.targets.len() * job.frame.definition.lanes.len())
            .sum::<usize>();
        let shared = match inputs.sources {
            PinSources::Shared(sources, authored) => Some((sources, authored)),
            PinSources::Local(..) => None,
        };
        let parallel = workers
            .zip(shared)
            .filter(|_| jobs.len() > 1 && lanes >= MIN_PARALLEL_PIN_LANES);
        let (sources, authored) = inputs.sources.local();
        match parallel {
            Some((workers, shared)) => self.pin_in_parallel(jobs, inputs, shared, buffers, workers),
            None => jobs
                .into_iter()
                .map(|job| {
                    let instance = self
                        .instances
                        .get_mut(&job.instance_id)
                        .expect("pinned instance");
                    pin_samples(
                        instance,
                        job.instance_id,
                        inputs.now_millis,
                        job.cycle_duration_millis,
                        inputs.output_interval_millis,
                        sources,
                        authored,
                        job.frame,
                        job.addresses,
                        buffers,
                        self.output_frame_undo.as_mut().map(transaction::journal),
                    )
                })
                .collect(),
        }
    }

    fn pin_in_parallel(
        &mut self,
        jobs: Vec<PinJob>,
        inputs: &PinInputs<'_>,
        (sources, authored): (
            &(dyn ScalarSourceResolver + Sync),
            &(dyn DynamicValueSourceResolver + Sync),
        ),
        buffers: &mut SamplingWorkBuffers,
        workers: &dyn InstanceWorkers,
    ) -> Result<Vec<PinnedInstance>, DynamicRuntimeError> {
        let (now_millis, output_interval_millis) =
            (inputs.now_millis, inputs.output_interval_millis);
        let mut instances =
            lend_instances(&mut self.instances, jobs.iter().map(|job| job.instance_id));
        let tasks = jobs
            .into_iter()
            .map(|job| {
                let id = job.instance_id;
                Mutex::new(PinTask {
                    instance: instances.remove(&id).expect("pinned instance"),
                    controllers: buffers.controllers.remove(&id).unwrap_or_default(),
                    envelopes: buffers.spare_envelopes.pop().unwrap_or_default(),
                    undo: self.output_frame_undo.as_mut().map(|undo| undo.lend(id)),
                    job: Some(job),
                    pinned: None,
                })
            })
            .collect::<Vec<_>>();
        workers.run_indexed(tasks.len(), &|index| {
            let mut task = tasks[index].lock().expect("one pinning per instance");
            let task = &mut *task;
            let job = task.job.take().expect("pinned once");
            task.pinned = Some(pin_instance(
                task.instance,
                (job.instance_id, now_millis),
                (job.cycle_duration_millis, output_interval_millis),
                (sources, authored),
                (job.frame, job.addresses),
                std::mem::take(&mut task.controllers),
                &mut task.envelopes,
                task.undo
                    .as_mut()
                    .map(|undo| undo as &mut transaction::Journal),
            ));
        });
        let mut plans = Vec::with_capacity(tasks.len());
        let mut failed = None;
        for task in tasks {
            let task = task.into_inner().expect("one pinning per instance");
            if let (Some(journal), Some(undo)) = (self.output_frame_undo.as_mut(), task.undo) {
                journal.take_back(undo);
            }
            let mut envelopes = task.envelopes;
            envelopes.clear();
            buffers.spare_envelopes.push(envelopes);
            match task.pinned.expect("every instance pinned") {
                Ok(plan) => plans.push(plan),
                Err(error) => {
                    failed.get_or_insert(error);
                }
            }
        }
        match failed {
            Some(error) => Err(error),
            None => Ok(plans),
        }
    }
}

/// One instance's pinning, as a worker takes it.
struct PinTask<'a> {
    instance: &'a mut DynamicInstance,
    controllers: Vec<PinnedController>,
    envelopes: rustc_hash::FxHashMap<RandomKey, f32>,
    undo: Option<transaction::InstanceUndo>,
    job: Option<PinJob>,
    pinned: Option<Result<PinnedInstance, DynamicRuntimeError>>,
}

/// One instance's completion, as a worker takes it.
struct CompleteTask<'a> {
    instance: &'a mut DynamicInstance,
    plan: &'a mut PinnedInstance,
    evaluated: std::vec::IntoIter<Evaluated>,
    undo: Option<transaction::InstanceUndo>,
    samples: Vec<DynamicRuntimeSample>,
    requirements: Vec<DynamicFamilyPreparationRequirement>,
    result: Option<Result<(), DynamicRuntimeError>>,
}

/// The instances `ids` names, each borrowed on its own.
fn lend_instances(
    instances: &mut HashMap<Uuid, DynamicInstance>,
    ids: impl Iterator<Item = Uuid>,
) -> rustc_hash::FxHashMap<Uuid, &mut DynamicInstance> {
    let wanted = ids.collect::<rustc_hash::FxHashSet<_>>();
    instances
        .iter_mut()
        .filter(|(id, _)| wanted.contains(*id))
        .map(|(id, instance)| (*id, instance))
        .collect()
}

impl DeferredTypedSampling<'_> {
    /// Whether the plans have no typed lane to evaluate and enough lanes to complete on the
    /// workers.
    pub(super) fn instance_parallel_without_typed(&self) -> bool {
        self.plans.len() > 1
            && self.typed_lane_count() == 0
            && self
                .plans
                .iter()
                .flat_map(|plan| &plan.controllers)
                .map(|work| work.lanes.len())
                .sum::<usize>()
                >= MIN_PARALLEL_PIN_LANES
    }

    /// Apply every plan's evaluations (`evaluated[plan]`, in the order `resolve_deferred` asks
    /// for them) and emit its samples, the plans on `workers`; then append the samples and
    /// requirements in plan order. Every typed lane must have its evaluation.
    pub(super) fn complete_instances_in_parallel(
        &mut self,
        evaluated: Vec<Vec<Evaluated>>,
        workers: &dyn InstanceWorkers,
    ) -> Result<(), DynamicRuntimeError> {
        let runtime = &mut *self.runtime;
        let mut instances = lend_instances(
            &mut runtime.instances,
            self.plans.iter().map(|plan| plan.instance_id),
        );
        let mut spare = std::mem::take(&mut *self.spare_outputs);
        let tasks = self
            .plans
            .iter_mut()
            .zip(evaluated)
            .map(|(plan, evaluated)| {
                let id = plan.instance_id;
                let (samples, requirements) = spare.pop().unwrap_or_default();
                Ok(Mutex::new(CompleteTask {
                    instance: instances
                        .remove(&id)
                        .ok_or(DynamicRuntimeError::MissingInstance)?,
                    plan,
                    evaluated: evaluated.into_iter(),
                    undo: runtime.output_frame_undo.as_mut().map(|undo| undo.lend(id)),
                    samples,
                    requirements,
                    result: None,
                }))
            })
            .collect::<Result<Vec<_>, DynamicRuntimeError>>()?;
        workers.run_indexed(tasks.len(), &|index| {
            let mut task = tasks[index].lock().expect("one completion per instance");
            let task = &mut *task;
            let evaluated = &mut task.evaluated;
            task.result = Some(complete_samples(
                task.instance,
                task.plan,
                &crate::programming::UnavailableProgrammingSources,
                &mut task.samples,
                Some(&mut task.requirements),
                task.undo
                    .as_mut()
                    .map(|undo| undo as &mut transaction::Journal),
                &mut || {
                    Some(
                        evaluated
                            .next()
                            .expect("a worker evaluation for every typed lane"),
                    )
                },
            ));
        });
        let mut failed = None;
        for task in tasks {
            let mut task = task.into_inner().expect("one completion per instance");
            if let (Some(journal), Some(undo)) = (runtime.output_frame_undo.as_mut(), task.undo) {
                journal.take_back(undo);
            }
            if let Err(error) = task.result.expect("every instance completed") {
                failed.get_or_insert(error);
            }
            self.samples.append(&mut task.samples);
            self.requirements.append(&mut task.requirements);
            spare.push((task.samples, task.requirements));
        }
        *self.spare_outputs = spare;
        failed.map_or(Ok(()), Err)
    }
}
