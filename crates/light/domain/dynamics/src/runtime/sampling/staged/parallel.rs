//! Deferred typed completion on several threads (TL-639 round 6).
//!
//! Completing a typed lane reads the lane's compiled configuration and keyframe cache, its
//! controller's pinned values and Current of its own target; it writes the lane's value,
//! requirements and a keyframe transition for the lane's cache. The frame gives each target to
//! one chunk (by [`crate::target_shard`]); each chunk evaluates its targets' lanes in frame
//! order on a worker, against the caller's fork of the sources, into an ordered record, and
//! changes nothing. The frame then resolves every plan exactly as the single-threaded
//! completion does, taking each lane's evaluation from its chunk's record (appending the
//! fork's logs at that place) or, where the worker could not (its fork reported the lane needs
//! the frame), evaluating that lane and every later one of its chunk itself. Values,
//! requirements, keyframe caches and source logs are those of the single-threaded loop for any
//! worker count.
use super::resolve::{Evaluated, evaluate};
use super::*;
use crate::{PreparationSources, PreparationWorkers, shard_chunk, target_shard};

/// One chunk's ordered record of evaluated typed lanes.
#[derive(Default)]
pub struct CompletedChunk {
    /// Per evaluated lane: its evaluation and the fork's log lengths after it.
    evaluated: Vec<(Evaluated, (usize, usize))>,
    /// The first member lane the frame evaluates itself, with every later one of the chunk.
    stopped: Option<usize>,
}

/// Where the frame's walk stands in one chunk's record.
struct ChunkCursor {
    evaluated: std::vec::IntoIter<(Evaluated, (usize, usize))>,
    stopped: Option<usize>,
    next: usize,
    logs: (usize, usize),
}

/// One typed lane to evaluate, in frame order.
struct Item<'a> {
    plan: usize,
    work: usize,
    lane: usize,
    compiled: &'a crate::CompiledProgrammingLane,
}

impl<'frame> DeferredTypedSampling<'frame> {
    /// The typed lanes still to evaluate, in frame order.
    pub fn typed_lane_count(&self) -> usize {
        self.plans
            .iter()
            .flat_map(|plan| &plan.controllers)
            .map(|work| work.typed_indices.len())
            .sum()
    }

    /// [`Self::complete`] with `workers`: the same samples, requirements, keyframe caches and
    /// source logs, with the typed lanes evaluated on several threads.
    pub fn complete_in_parallel(
        mut self,
        sources: &dyn DynamicValueSourceResolver,
        workers: &mut dyn PreparationWorkers,
    ) -> Result<CompletedDynamicSamples<'frame>, DynamicRuntimeError> {
        let chunks = workers.chunks().clamp(1, crate::TARGET_SHARDS);
        let mut chunk_of_item = Vec::new();
        let mut plan_of_item = Vec::new();
        let recorded = {
            let runtime = &*self.runtime;
            let plans = &*self.plans;
            let mut items = Vec::new();
            let mut presets = Vec::with_capacity(plans.len());
            for (plan_index, plan) in plans.iter().enumerate() {
                let instance = runtime
                    .instances
                    .get(&plan.instance_id)
                    .ok_or(DynamicRuntimeError::MissingInstance)?;
                presets.push(Arc::clone(&instance.preset_values.by_binding));
                for (work_index, work) in plan.controllers.iter().enumerate() {
                    for &lane in &work.typed_indices {
                        let pinned = &work.lanes[lane];
                        if !matches!(pinned.value, PinnedValue::Typed { .. }) {
                            continue;
                        }
                        let id = plan.frame.definition.lanes[pinned.lane_index].id;
                        let compiled = instance
                            .programming_lanes
                            .get(&id)
                            .expect("pinned compiled lane");
                        chunk_of_item.push(shard_chunk(target_shard(pinned.target), chunks));
                        plan_of_item.push(plan_index);
                        items.push(Item {
                            plan: plan_index,
                            work: work_index,
                            lane,
                            compiled,
                        });
                    }
                }
            }
            let mut members = vec![Vec::new(); chunks];
            for (index, chunk) in chunk_of_item.iter().enumerate() {
                members[*chunk].push(index);
            }
            let (items, presets, members) = (&items, &presets, &members);
            workers
                .run_completion(&|chunk, fork| {
                    evaluate_chunk(plans, items, presets, &members[chunk], fork)
                })
                .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?
        };
        let mut cursors = recorded
            .into_iter()
            .map(|chunk| ChunkCursor {
                evaluated: chunk.evaluated.into_iter(),
                stopped: chunk.stopped,
                next: 0,
                logs: (0, 0),
            })
            .collect::<Vec<_>>();
        // TL-639 round 7: with every lane evaluated by a worker, the plans apply and emit on the
        // workers too; the forks' logs are appended in lane order first, as the walk below would.
        if let Some(instances) = self.workers.filter(|_| {
            self.plans.len() > 1 && cursors.iter().all(|cursor| cursor.stopped.is_none())
        }) {
            let mut evaluated = (0..self.plans.len())
                .map(|_| Vec::new())
                .collect::<Vec<_>>();
            for (chunk, plan) in chunk_of_item.into_iter().zip(plan_of_item) {
                let cursor = &mut cursors[chunk];
                let (evaluation, logs) = cursor
                    .evaluated
                    .next()
                    .expect("one evaluation per recorded member");
                workers.append_logs(chunk, cursor.logs, logs);
                cursor.logs = logs;
                evaluated[plan].push(evaluation);
            }
            self.complete_instances_in_parallel(evaluated, instances)?;
            return Ok(self.finish());
        }
        let mut items = chunk_of_item.into_iter();
        let mut recorded = || {
            let chunk = items.next().expect("one record per typed lane");
            let cursor = &mut cursors[chunk];
            let member = cursor.next;
            cursor.next += 1;
            if cursor.stopped.is_some_and(|stop| member >= stop) {
                return None;
            }
            let (evaluated, logs) = cursor
                .evaluated
                .next()
                .expect("one evaluation per recorded member");
            workers.append_logs(chunk, cursor.logs, logs);
            cursor.logs = logs;
            Some(evaluated)
        };
        for plan in self.plans.iter_mut() {
            let instance = self
                .runtime
                .instances
                .get_mut(&plan.instance_id)
                .ok_or(DynamicRuntimeError::MissingInstance)?;
            complete_samples(
                instance,
                plan,
                sources,
                self.samples,
                Some(self.requirements),
                self.runtime
                    .output_frame_undo
                    .as_mut()
                    .map(transaction::journal),
                &mut recorded,
            )?;
        }
        Ok(self.finish())
    }
}

/// One worker's chunk: evaluate its member lanes in order, until the first one its fork cannot
/// answer.
fn evaluate_chunk(
    plans: &[PinnedInstance],
    items: &[Item<'_>],
    presets: &[Arc<HashMap<(Uuid, FixtureId), crate::DynamicValue>>],
    members: &[usize],
    fork: &dyn PreparationSources,
) -> CompletedChunk {
    let mut chunk = CompletedChunk {
        evaluated: Vec::with_capacity(members.len()),
        stopped: None,
    };
    for (member, &index) in members.iter().enumerate() {
        let item = &items[index];
        let plan = &plans[item.plan];
        let work = &plan.controllers[item.work];
        let sources = super::super::super::preset_values::RetainedPresetSources {
            current: fork,
            instance_id: plan.instance_id,
            values: Arc::clone(&presets[item.plan]),
        };
        let evaluated = evaluate(
            item.compiled,
            plan,
            work,
            &work.lanes[item.lane],
            &sources,
            true,
        );
        let Some(logs) = fork.finish_controller() else {
            chunk.stopped = Some(member);
            break;
        };
        chunk.evaluated.push((evaluated, logs));
    }
    chunk
}
