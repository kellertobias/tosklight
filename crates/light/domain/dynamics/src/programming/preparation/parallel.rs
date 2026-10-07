//! Family preparation on several threads (TL-639 round 5).
//!
//! A controller's preparation (one instance, controller and target) reads its own samples, last
//! frame's compiled samples of its own lanes, and Current of its own target; it writes family
//! samples, legacy fragments and requirements in order. The frame gives each target to one chunk
//! (by [`target_shard`]), and each chunk prepares its targets' controllers in the frame's order
//! on a worker, against the caller's fork of the sources, into an ordered record. The frame then
//! walks every controller in order once more and replays each record into its groups, or
//! prepares the controller itself where the worker could not (its fork reported the controller
//! needs the frame), with everything after it in that chunk. The groups, legacy fragments,
//! requirements and compiled-sample cache are those of the single-threaded loop for any worker
//! count.
use super::*;

/// Shards targets are cut into for parallel sections; a section's chunks are shard ranges.
pub const TARGET_SHARDS: usize = 64;

/// The shard of a target.
pub fn target_shard(target: FixtureId) -> usize {
    let (high, low) = target.0.as_u64_pair();
    ((high ^ low) % TARGET_SHARDS as u64) as usize
}

/// The chunk of `shard` when the shards are cut into `chunks` contiguous ranges.
pub fn shard_chunk(shard: usize, chunks: usize) -> usize {
    let chunks = chunks.clamp(1, TARGET_SHARDS);
    // The inverse of `start = TARGET_SHARDS * chunk / chunks`.
    ((shard + 1) * chunks - 1) / TARGET_SHARDS
}

/// The sources one worker prepares against: the frame's sources forked for its targets.
pub trait PreparationSources: DynamicValueSourceResolver {
    /// Close one controller. `Some` with the fork's ordered log lengths (see
    /// [`PreparationWorkers::append_logs`]) when the fork could answer everything the controller
    /// asked; `None` when the frame must prepare it (its fork forgets the controller).
    fn finish_controller(&self) -> Option<(usize, usize)>;
}

/// The caller's workers and source forks for one parallel preparation.
pub trait PreparationWorkers {
    /// How many chunks to cut the targets into.
    fn chunks(&self) -> usize;

    /// Run `prepare(chunk, fork)` for every chunk on the frame's workers, each with its own fork
    /// of the frame's sources, and return the results in chunk order. Before returning, the
    /// forks' keyed changes (caches, bindings) are back in the frame's sources; their ordered
    /// logs wait for [`Self::append_logs`].
    fn run(
        &mut self,
        prepare: &(dyn Fn(usize, &dyn PreparationSources) -> PreparedChunk + Sync),
    ) -> Result<Vec<PreparedChunk>, TransitionError>;

    /// [`Self::run`] for a parallel deferred completion (TL-639 round 6): the same forks,
    /// one [`PreparationSources::finish_controller`] per typed lane.
    fn run_completion(
        &mut self,
        complete: &(dyn Fn(usize, &dyn PreparationSources) -> crate::CompletedChunk + Sync),
    ) -> Result<Vec<crate::CompletedChunk>, TransitionError>;

    /// Append chunk `chunk`'s fork logs between two [`PreparationSources::finish_controller`]
    /// lengths, as one controller recorded them in turn.
    fn append_logs(&mut self, chunk: usize, from: (usize, usize), to: (usize, usize));

    /// Sort the controllers' keys (distinct: each ends with its sample index); on the workers'
    /// threads if the caller has them (TL-639 round 6).
    fn sort_controller_keys(&self, keys: &mut [ControllerSortKey]) {
        keys.sort_unstable();
    }

    /// Sort the prepared groups by target and owner (distinct: one group per pair).
    fn sort_families(&self, families: &mut [DynamicFamilySampleGroup]) {
        families.sort_unstable_by_key(family_order);
    }
}

/// One controller sort key: the controller key, the lane and the sample's index.
pub type ControllerSortKey = ((Uuid, Uuid, Uuid), Uuid, usize);

/// The order prepared family groups are returned in.
pub fn family_order(group: &DynamicFamilySampleGroup) -> (Uuid, &'static str) {
    (group.target.0, group.owner.id())
}

/// One write of a controller's preparation, in order.
enum Prepared {
    Append(FixtureId, ProgrammingOwner, Vec<FamilyCompositionSample>),
    Legacy(DynamicRuntimeSample),
    Require(DynamicFamilyPreparationRequirement),
}

/// One chunk's ordered record.
#[derive(Default)]
pub struct PreparedChunk {
    writes: Vec<Prepared>,
    /// Per prepared controller: where its writes end, and the fork's log lengths after it.
    controllers: Vec<(usize, (usize, usize))>,
    /// Compiled samples this chunk keeps.
    keep: Vec<(CacheKey, CompiledSample)>,
    /// The first controller the frame prepares itself with every later one of the chunk, and
    /// the worker's error there, if any.
    stopped: Option<(usize, Option<TransitionError>)>,
}

impl PreparationOutput for Vec<Prepared> {
    fn append(
        &mut self,
        target: FixtureId,
        owner: ProgrammingOwner,
        samples: Vec<FamilyCompositionSample>,
    ) {
        if !samples.is_empty() {
            self.push(Prepared::Append(target, owner, samples));
        }
    }

    fn legacy(&mut self, sample: DynamicRuntimeSample) {
        self.push(Prepared::Legacy(sample));
    }

    fn require(&mut self, requirement: DynamicFamilyPreparationRequirement) {
        self.push(Prepared::Require(requirement));
    }
}

/// [`prepare_dynamic_family_samples_with_requirements`] with `workers`: the same groups,
/// legacy fragments, requirements and cache, prepared on several threads.
pub fn prepare_dynamic_family_samples_in_parallel<'a>(
    samples: &[DynamicRuntimeSample],
    sampling_requirements: &[DynamicFamilyPreparationRequirement],
    sources: &dyn DynamicValueSourceResolver,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    scratch: &'a mut DynamicFamilyPreparationScratch,
    workers: &mut dyn PreparationWorkers,
) -> Result<PreparedDynamicFamilySamples<'a>, TransitionError> {
    scratch.clear_output();
    scratch
        .sampling_requirements
        .extend_from_slice(sampling_requirements);
    scratch
        .requirements
        .extend_from_slice(sampling_requirements);
    let mut previous = std::mem::take(&mut scratch.cache);
    let result = prepare_in_parallel(
        samples,
        sources,
        native_models,
        &mut previous,
        scratch,
        workers,
    );
    scratch.retired = previous;
    if let Err(error) = result {
        scratch.clear();
        return Err(error);
    }
    workers.sort_families(&mut scratch.families);
    Ok(PreparedDynamicFamilySamples {
        families: &scratch.families,
        legacy: &scratch.legacy,
        requirements: &scratch.requirements,
    })
}

fn prepare_in_parallel(
    samples: &[DynamicRuntimeSample],
    sources: &dyn DynamicValueSourceResolver,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    previous: &mut HashMap<CacheKey, CompiledSample>,
    scratch: &mut DynamicFamilyPreparationScratch,
    workers: &mut dyn PreparationWorkers,
) -> Result<(), TransitionError> {
    sort_controllers(samples, scratch, &|keys| workers.sort_controller_keys(keys));
    let chunks = workers.chunks().clamp(1, TARGET_SHARDS);
    let mut members = vec![Vec::new(); chunks];
    scratch.chunk_of_controller.clear();
    for (index, range) in scratch.controllers.iter().enumerate() {
        let target = samples[scratch.order[range.start]].target;
        let chunk = shard_chunk(target_shard(target), chunks);
        scratch.chunk_of_controller.push(chunk);
        members[chunk].push(index);
    }
    let prepared = {
        let (order, controllers) = (&scratch.order, &scratch.controllers);
        let sampling_requirements = &scratch.sampling_requirements;
        let shared: &HashMap<CacheKey, CompiledSample> = previous;
        workers.run(&|chunk, fork| {
            prepare_chunk(
                samples,
                order,
                controllers,
                &members[chunk],
                sampling_requirements,
                shared,
                fork,
                native_models,
            )
        })?
    };
    let mut cursors = Vec::with_capacity(prepared.len());
    for mut chunk in prepared {
        scratch.cache.extend(chunk.keep.drain(..));
        cursors.push(ChunkCursor {
            writes: chunk.writes.into_iter(),
            controllers: chunk.controllers.into_iter(),
            stopped: chunk.stopped,
            next: 0,
            taken: (0, (0, 0)),
        });
    }
    let DynamicFamilyPreparationScratch {
        order,
        controllers,
        chunk_of_controller,
        controller,
        position,
        families,
        family_indices,
        legacy,
        requirements,
        sampling_requirements,
        cache,
        ..
    } = scratch;
    let mut out = FamilyOutput {
        families,
        family_indices,
        legacy,
        requirements,
    };
    for (range, &chunk) in controllers.iter().zip(chunk_of_controller.iter()) {
        let cursor = &mut cursors[chunk];
        let member = cursor.next;
        cursor.next += 1;
        if let Some((stop, error)) = &mut cursor.stopped
            && member >= *stop
        {
            if let Some(error) = error.take() {
                return Err(error);
            }
            let mut preparer = Preparer {
                controller: &mut *controller,
                position: &mut *position,
                sampling_requirements,
                previous: Previous::Owned(&mut *previous),
                keep: Keep::Cache(&mut *cache),
                out: &mut out,
            };
            preparer.prepare_controller(samples, &order[range.clone()], sources, native_models)?;
            continue;
        }
        let (writes_to, logs_to) = cursor
            .controllers
            .next()
            .expect("one record per prepared controller");
        let (writes_from, logs_from) = cursor.taken;
        for write in cursor.writes.by_ref().take(writes_to - writes_from) {
            match write {
                Prepared::Append(target, owner, samples) => out.append(target, owner, samples),
                Prepared::Legacy(sample) => out.legacy(sample),
                Prepared::Require(requirement) => out.require(requirement),
            }
        }
        workers.append_logs(chunk, logs_from, logs_to);
        cursor.taken = (writes_to, logs_to);
    }
    controller.clear();
    Ok(())
}

/// Where the frame's walk stands in one chunk's record.
struct ChunkCursor {
    writes: std::vec::IntoIter<Prepared>,
    controllers: std::vec::IntoIter<(usize, (usize, usize))>,
    stopped: Option<(usize, Option<TransitionError>)>,
    /// The next member controller.
    next: usize,
    /// Writes and fork log lengths taken so far.
    taken: (usize, (usize, usize)),
}

/// One worker's chunk: prepare its member controllers in order, until the first one its fork
/// cannot answer or that fails.
#[allow(clippy::too_many_arguments)]
fn prepare_chunk(
    samples: &[DynamicRuntimeSample],
    order: &[usize],
    controllers: &[std::ops::Range<usize>],
    members: &[usize],
    sampling_requirements: &[DynamicFamilyPreparationRequirement],
    previous: &HashMap<CacheKey, CompiledSample>,
    fork: &dyn PreparationSources,
    native_models: Option<&dyn DynamicNativeModelResolver>,
) -> PreparedChunk {
    let mut chunk = PreparedChunk::default();
    let (mut controller, mut position, mut keep) = (Vec::new(), Vec::new(), Vec::new());
    for (member, &index) in members.iter().enumerate() {
        let writes = chunk.writes.len();
        let prepared = Preparer {
            controller: &mut controller,
            position: &mut position,
            sampling_requirements,
            previous: Previous::Shared(previous),
            keep: Keep::Log(&mut keep),
            out: &mut chunk.writes,
        }
        .prepare_controller(
            samples,
            &order[controllers[index].clone()],
            fork,
            native_models,
        );
        let Some(logs) = fork.finish_controller() else {
            chunk.writes.truncate(writes);
            keep.clear();
            chunk.stopped = Some((member, None));
            break;
        };
        if let Err(error) = prepared {
            chunk.stopped = Some((member, Some(error)));
            break;
        }
        chunk.keep.append(&mut keep);
        chunk.controllers.push((chunk.writes.len(), logs));
    }
    chunk
}
