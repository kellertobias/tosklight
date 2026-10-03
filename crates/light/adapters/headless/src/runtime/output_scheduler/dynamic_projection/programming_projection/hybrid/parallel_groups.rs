//! The ordinary owner groups of one cohort on several threads (TL-639 round 5).
//!
//! After the Position batch, every remaining group composes one `(target, owner)` row: it reads
//! the frame's immutable tokens and the cohort's sources for its own target, and observes through
//! its own family lane. The frame gives each target to one chunk, and each chunk composes its
//! targets' groups in the cohort's order on a worker, against a fork of the sources and a private
//! staging of the Color and Focus/Zoom lanes. The frame then takes back every chunk's keyed
//! changes (caches, bindings, stagings, kept rows) and walks the cohort's groups in order once
//! more: a group a worker composed contributes its rows, requirements and source logs there; a
//! group the worker left (a Position group the batch did not handle, which composes through the
//! Position observer) or could not finish (a descriptor its lane has not compiled), and every
//! later group of that chunk, is composed by the frame itself at its place. The result is the
//! single-threaded loop's, in its order, for any worker count.
use super::super::super::family_inputs::CapturedFamilyInput;
use super::super::static_rows::{ROW_SHARDS, StaticFamilyRows, row_shard};
use super::staged::{CohortView, StaticOnlyTargets, compose_owner_group};
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::family_lanes::{
    FamilyLanesShared, FamilyLanesWorker, FamilySidecar, FamilyStaging,
};
use std::cell::Cell;

/// Below this many groups a cohort composes them in turn. Tests take the parallel path from
/// two groups, so every hybrid test also proves it equals the single-threaded loop.
const MIN_PARALLEL_GROUPS: usize = if cfg!(test) { 2 } else { 64 };
/// Groups per chunk at least.
const MIN_GROUPS_PER_CHUNK: usize = if cfg!(test) { 1 } else { 16 };

/// The lanes a `FamilyLanes` observer lends to a parallel section, and its sidecar wrapper.
pub(in crate::runtime) type ParallelLanes<'s, 'l, T> =
    (&'s FamilyLanesShared<'s, 'l>, fn(FamilySidecar) -> T);

/// How one group of a chunk was composed.
enum GroupRecord {
    /// By the worker: where its rows, requirements and source logs end in the chunk's output.
    Worker {
        projections: usize,
        requirements: usize,
        logs: (usize, usize),
    },
    /// Left to the frame's thread (a Position group the batch did not handle).
    Frame,
}

/// One chunk's composed rows, in group order, and where it stopped.
struct ChunkOutput {
    projections: Vec<OwnedHybridProjection<FamilySidecar>>,
    requirements: Vec<HybridFamilyRequirement>,
    sources: super::super::fork::SourcesChanges,
    staging: FamilyStaging,
    rows: StaticFamilyRows,
    /// One record per member group before `stopped`.
    groups: Vec<GroupRecord>,
    /// The first member the frame composes itself with every later one, and the worker's error
    /// there, if any.
    stopped: Option<(usize, Option<DynamicRuntimeError>)>,
}

/// The cohort's targets cut into chunks along the kept rows' shards: each chunk's shard range,
/// each group's chunk, and each chunk's member groups in cohort order. A target's groups always
/// share one chunk.
struct Partition {
    shards: Vec<std::ops::Range<usize>>,
    chunk_of_group: Vec<usize>,
    members: Vec<Vec<usize>>,
}

fn partition(groups: &[CapturedFamilyInput], chunks: usize) -> Partition {
    let chunks = chunks.clamp(1, ROW_SHARDS);
    let shards = (0..chunks)
        .map(|chunk| light_engine::parallel::chunk_range(ROW_SHARDS, chunks, chunk))
        .collect::<Vec<_>>();
    let mut chunk_of_shard = [0; ROW_SHARDS];
    for (chunk, range) in shards.iter().enumerate() {
        chunk_of_shard[range.clone()].fill(chunk);
    }
    let mut members = vec![Vec::new(); chunks];
    let chunk_of_group = groups
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let chunk = chunk_of_shard[row_shard(entry.group.target)];
            members[chunk].push(index);
            chunk
        })
        .collect();
    Partition {
        shards,
        chunk_of_group,
        members,
    }
}

/// Where the frame's walk stands in one chunk's output.
struct ChunkCursor {
    rows: std::vec::IntoIter<OwnedHybridProjection<FamilySidecar>>,
    required: Vec<HybridFamilyRequirement>,
    logs: (
        Vec<super::super::CurrentResolutionRequirement>,
        Vec<TransitionError>,
    ),
    records: std::vec::IntoIter<GroupRecord>,
    stopped: Option<(usize, Option<DynamicRuntimeError>)>,
    /// The next member group.
    next: usize,
    /// Rows, requirements and source logs taken so far.
    taken: (usize, usize, (usize, usize)),
}

/// Compose every ordinary group of the cohort, on `workers` threads when the observer offers
/// its lanes and the cohort is large enough.
#[allow(clippy::too_many_arguments)]
pub(super) fn compose_owner_groups<T, R: HybridFrameResolver>(
    view: &CohortView<'_, '_, PreparedFamilySources<'_>, R>,
    groups: &[CapturedFamilyInput],
    static_only: &StaticOnlyTargets,
    handled_position: &FxHashSet<FixtureId>,
    observer: &mut impl HybridFrameObserver<T>,
    (composition, static_rows, scratch): (
        &mut RetainedFamilyCompositionScratch,
        &mut StaticFamilyRows,
        &mut Vec<RetainedFamilyCompositionScratch>,
    ),
    projections: &mut Vec<OwnedHybridProjection<T>>,
    requirements: &mut Vec<HybridFamilyRequirement>,
    pool: Option<&light_engine::parallel::OutputPool>,
) -> Result<(), DynamicRuntimeError> {
    macro_rules! compose_here {
        ($entry:expr) => {
            compose_owner_group(
                view,
                $entry,
                static_only,
                handled_position,
                observer,
                (composition, static_rows),
                projections,
                requirements,
            )
        };
    }
    let mut parallel = None;
    let mut partitioned = None;
    if let Some(pool) = pool.filter(|_| groups.len() >= MIN_PARALLEL_GROUPS && view.typed.forks()) {
        let chunks =
            light_engine::parallel::chunk_count(groups.len(), pool.workers(), MIN_GROUPS_PER_CHUNK);
        let partition = partition(groups, chunks);
        observer.with_parallel_lanes(&mut |lanes| {
            if let Some(lanes) = lanes {
                parallel = Some(compose_in_parallel(
                    view,
                    groups,
                    &partition,
                    (static_only, handled_position),
                    lanes,
                    (static_rows, scratch),
                    pool,
                ));
            }
        });
        partitioned = Some(partition);
    }
    let (Some((outputs, wrap)), Some(partition)) = (parallel, partitioned) else {
        for entry in groups {
            compose_here!(entry)?;
        }
        return Ok(());
    };
    // Keyed state first (caches, bindings, stagings, kept rows), so every group the frame
    // composes itself below reads what it would have read in turn.
    let mut chunks = Vec::with_capacity(outputs.len());
    for output in outputs {
        let logs = view.typed.apply_fork(output.sources).map_err(invalid)?;
        observer
            .merge_parallel(view.frame.token, output.staging)
            .map_err(invalid)?;
        static_rows.take_back(output.rows);
        chunks.push(ChunkCursor {
            rows: output.projections.into_iter(),
            required: output.requirements,
            logs,
            records: output.groups.into_iter(),
            stopped: output.stopped,
            next: 0,
            taken: (0, 0, (0, 0)),
        });
    }
    for (entry, &chunk) in groups.iter().zip(&partition.chunk_of_group) {
        let cursor = &mut chunks[chunk];
        let member = cursor.next;
        cursor.next += 1;
        if let Some((stopped, error)) = &mut cursor.stopped
            && member >= *stopped
        {
            if let Some(error) = error.take() {
                return Err(error);
            }
            compose_here!(entry)?;
            continue;
        }
        let record = cursor
            .records
            .next()
            .expect("one record per member before the stop");
        let GroupRecord::Worker {
            projections: rows_to,
            requirements: required_to,
            logs: logs_to,
        } = record
        else {
            compose_here!(entry)?;
            continue;
        };
        let (rows_from, required_from, logs_from) = cursor.taken;
        projections.extend(cursor.rows.by_ref().take(rows_to - rows_from).map(|row| {
            OwnedHybridProjection {
                target: row.target,
                owner: row.owner,
                value: row.value,
                metadata: row.metadata,
                sidecar: wrap(row.sidecar),
            }
        }));
        requirements.extend_from_slice(&cursor.required[required_from..required_to]);
        view.typed.append_fork_logs(
            &cursor.logs.0[logs_from.0..logs_to.0],
            &cursor.logs.1[logs_from.1..logs_to.1],
        );
        cursor.taken = (rows_to, required_to, logs_to);
    }
    Ok(())
}

/// Compose the cohort's ordinary groups on `workers` threads, one chunk of targets each, and
/// return each chunk's output in chunk order. The sources are frozen for the section and thawed
/// before this returns.
#[allow(clippy::type_complexity)]
fn compose_in_parallel<T, R>(
    view: &CohortView<'_, '_, PreparedFamilySources<'_>, R>,
    groups: &[CapturedFamilyInput],
    partition: &Partition,
    (static_only, handled_position): (&StaticOnlyTargets, &FxHashSet<FixtureId>),
    (shared, wrap): ParallelLanes<'_, '_, T>,
    (static_rows, scratch): (
        &mut StaticFamilyRows,
        &mut Vec<RetainedFamilyCompositionScratch>,
    ),
    pool: &light_engine::parallel::OutputPool,
) -> (Vec<ChunkOutput>, fn(FamilySidecar) -> T) {
    let lent = static_rows
        .lend(&partition.shards)
        .into_iter()
        .map(|rows| parking_lot::Mutex::new(Some(rows)))
        .collect::<Vec<_>>();
    scratch.resize_with(pool.workers().max(scratch.len()), Default::default);
    let shared_view = SharedView {
        frame: view.frame,
        static_sources: view.static_sources,
        static_token: view.static_token,
        scalar_token: view.scalar_token,
        legacy_owners: view.legacy_owners,
        control: view.control,
    };
    let frozen = view.typed.freeze();
    let outputs = view.typed.with_fork_base(&frozen, |base| {
        light_engine::parallel::run_ordered(
            Some(pool),
            scratch,
            partition.members.len(),
            |scratch, chunk| {
                let rows = lent[chunk].lock().take().unwrap_or_default();
                compose_chunk(
                    &shared_view,
                    (shared, base),
                    groups,
                    &partition.members[chunk],
                    (static_only, handled_position),
                    scratch,
                    rows,
                )
            },
        )
    });
    view.typed.thaw(frozen);
    (outputs, wrap)
}

/// What every worker of a section reads of the cohort's view.
struct SharedView<'a> {
    frame: HybridFrameContext<'a>,
    static_sources: &'a PreparedFamilySources<'a>,
    static_token: &'a PreparedStaticFamilyFrame,
    scalar_token: &'a PreparedStaticFamilyFrame,
    legacy_owners: &'a FxHashSet<(FixtureId, ProgrammingOwner)>,
    control: &'a super::staged::EndpointControl<'a>,
}

/// One worker's chunk: compose its member groups in order against a fork of the cohort's
/// sources and private lane stagings, until the first group it cannot finish alone.
#[allow(clippy::type_complexity)]
fn compose_chunk(
    view: &SharedView<'_>,
    (shared, base): (
        &FamilyLanesShared<'_, '_>,
        super::super::fork::ForkBase<'_, PreparedFamilySources<'_>>,
    ),
    groups: &[CapturedFamilyInput],
    members: &[usize],
    (static_only, handled_position): (&StaticOnlyTargets, &FxHashSet<FixtureId>),
    composition: &mut RetainedFamilyCompositionScratch,
    mut rows: StaticFamilyRows,
) -> ChunkOutput {
    let missed = Cell::new(false);
    let lanes = FamilyLanesWorker::new(shared, &missed);
    let frame = view.frame;
    let adopt = |target, original: &AttributeValue, address: &DynamicValueAddress| {
        lanes.adopt(frame, target, original, address)
    };
    let typed = base.fork(&adopt);
    let worker_view = CohortView {
        frame,
        resolver: &lanes,
        typed: &typed,
        static_sources: view.static_sources,
        static_token: view.static_token,
        scalar_token: view.scalar_token,
        legacy_owners: view.legacy_owners,
        control: view.control,
    };
    let mut observer = WorkerObserver { lanes: &lanes };
    let (mut projections, mut requirements) = (Vec::new(), Vec::new());
    let (mut records, mut stopped) = (Vec::with_capacity(members.len()), None);
    for (member, &index) in members.iter().enumerate() {
        let entry = &groups[index];
        let group = &entry.group;
        // A Position group the batch did not handle composes through the Position observer,
        // which stays on the frame's thread; one held by a scalar guard only records that.
        if group.owner == ProgrammingOwner::Position
            && !handled_position.contains(&group.target)
            && super::staged::scalar_owner_guard(
                view.legacy_owners,
                view.static_token,
                view.scalar_token,
                group.target,
                group.owner,
            )
            .is_none()
        {
            records.push(GroupRecord::Frame);
            continue;
        }
        let marks = (lanes.mark(), projections.len(), requirements.len());
        let composed = compose_owner_group(
            &worker_view,
            entry,
            static_only,
            handled_position,
            &mut observer,
            (composition, &mut rows),
            &mut projections,
            &mut requirements,
        );
        if missed.get() {
            typed.abort_group();
            lanes.truncate(marks.0);
            projections.truncate(marks.1);
            requirements.truncate(marks.2);
            stopped = Some((member, None));
            break;
        }
        typed.commit_group();
        if let Err(error) = composed {
            stopped = Some((member, Some(error)));
            break;
        }
        records.push(GroupRecord::Worker {
            projections: projections.len(),
            requirements: requirements.len(),
            logs: typed.log_lengths(),
        });
    }
    ChunkOutput {
        projections,
        requirements,
        sources: typed.into_changes(),
        staging: lanes.into_staging(),
        rows,
        groups: records,
        stopped,
    }
}

/// The worker's observer: ordinary groups observe through its lane view.
struct WorkerObserver<'w, 's, 'l> {
    lanes: &'w FamilyLanesWorker<'s, 'l>,
}

impl HybridFrameObserver<FamilySidecar> for WorkerObserver<'_, '_, '_> {
    fn observe(
        &mut self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, FamilySidecar), TransitionError> {
        self.lanes.observe(observation)
    }
}
