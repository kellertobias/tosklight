//! A native installation's validation and placement on the output pool (TL-639 round 7).
//!
//! Every check of one write depends on the write, its run's owner (validated where a run of
//! equal `(target, owner, instance)` keys starts in the write order) and earlier writes of the
//! same physical instance (duplicates and shared controls); placement writes only that
//! instance's row. So the writes are cut by instance: each chunk checks and places its
//! instances' writes in their original order, stopping at its first rejection. The earliest
//! rejection over all chunks is the one the single pass meets first, since nothing before it
//! depends on another chunk. Without one, the instance rows, the targets and the counts are
//! merged in the order the single pass first created them, so the installation, the order of
//! its maps and the completeness check are the single pass's.
use super::*;
use crate::parallel::OutputPool;

/// Below this many writes an installation runs on the caller (two in tests).
const MIN_PARALLEL_WRITES: usize = if cfg!(test) { 2 } else { 2048 };

pub(super) fn pool_for(pool: Option<&OutputPool>, writes: usize) -> Option<&OutputPool> {
    pool.filter(|pool| pool.workers() > 1 && writes >= MIN_PARALLEL_WRITES)
}

type Key = (FixtureId, ProgrammingOwner, Uuid);

/// Where a run started, with its target and destination (root, fixture index).
type Run = (usize, (FixtureId, ProgrammingOwner), (FixtureId, usize));

fn key(write: &FamilyNativeWrite) -> Key {
    (write.target, write.owner, write.instance_id)
}

/// One chunk's checked and placed writes.
#[derive(Default)]
struct Chunk {
    /// The chunk's first rejection: the write's position and the error.
    rejected: Option<(usize, EngineError)>,
    instances: FxHashMap<Uuid, NativePositionInstance>,
    /// Where each instance was first written.
    created: Vec<(usize, Uuid)>,
    /// Where each run started, with its target and destination.
    runs: Vec<Run>,
    counts: FxHashMap<Key, usize>,
}

impl PreparedStaticFamilyFrame {
    pub(super) fn install_in_parallel(
        &self,
        capture: &PreparedOutputFrame,
        writes: &[FamilyNativeWrite],
        pool: &OutputPool,
    ) -> Result<NativePositionProjection, EngineError> {
        let chunks = crate::parallel::chunk_count(writes.len(), pool.workers(), 256);
        let mut members = vec![Vec::new(); chunks];
        for (position, write) in writes.iter().enumerate() {
            let (high, low) = write.instance_id.as_u64_pair();
            members[((high ^ low) % chunks as u64) as usize].push(position);
        }
        let mut scratch = vec![(); pool.workers()];
        let checked = crate::parallel::run_ordered(Some(pool), &mut scratch, chunks, |_, chunk| {
            self.check_chunk(capture, writes, &members[chunk])
        });
        merge(capture, checked)
    }

    /// Check and place `members` (positions in `writes`, ascending) in order.
    fn check_chunk(
        &self,
        capture: &PreparedOutputFrame,
        writes: &[FamilyNativeWrite],
        members: &[usize],
    ) -> Chunk {
        let mut chunk = Chunk::default();
        let mut seen = FxHashSet::default();
        let mut run = None;
        for &position in members {
            let write = &writes[position];
            if position == 0 || key(&writes[position - 1]) != key(write) {
                match self.validate_family_owner(capture, write) {
                    Ok((destination, mode, footprint)) => {
                        chunk.runs.push((
                            position,
                            (write.target, write.owner),
                            (destination.root, destination.fixture_index),
                        ));
                        run = Some((destination, mode, footprint));
                    }
                    Err(error) => {
                        chunk.rejected = Some((position, error));
                        break;
                    }
                }
            }
            // A run's writes are consecutive, and all of one instance, so its start is here.
            let (destination, mode, footprint) = run.as_ref().expect("the run of this write");
            let checked = validate_family_control(mode, footprint, write).and_then(|()| {
                let index = write.channel_index as usize;
                if !seen.insert((write.owner, write.target, write.instance_id, index)) {
                    return Err(invalid(format!(
                        "native {} owner writes a control twice",
                        family(write.owner)
                    )));
                }
                *chunk.counts.entry(key(write)).or_default() += 1;
                install_in(&mut chunk.instances, write, destination)
            });
            match checked {
                Ok(true) => chunk.created.push((position, write.instance_id)),
                Ok(false) => {}
                Err(error) => {
                    chunk.rejected = Some((position, error));
                    break;
                }
            }
        }
        chunk
    }
}

/// The earliest rejection, or the installation merged in the single pass's creation order.
fn merge(
    capture: &PreparedOutputFrame,
    chunks: Vec<Chunk>,
) -> Result<NativePositionProjection, EngineError> {
    if let Some(error) = chunks
        .iter()
        .filter_map(|chunk| chunk.rejected.as_ref().map(|(position, _)| *position))
        .min()
    {
        let rejected = chunks
            .into_iter()
            .find_map(|chunk| chunk.rejected.filter(|(position, _)| *position == error));
        return Err(rejected.expect("the earliest rejection").1);
    }
    let mut runs = chunks
        .iter()
        .flat_map(|chunk| chunk.runs.iter().copied())
        .collect::<Vec<_>>();
    runs.sort_unstable_by_key(|run| run.0);
    let mut targets = FxHashMap::default();
    for (_, target, destination) in runs {
        targets.insert(target, destination);
    }
    let mut counts = FxHashMap::default();
    let mut created = Vec::new();
    let mut rows = Vec::with_capacity(chunks.len());
    for (chunk, checked) in chunks.into_iter().enumerate() {
        counts.extend(checked.counts);
        created.extend(
            checked
                .created
                .into_iter()
                .map(|(position, instance)| (position, chunk, instance)),
        );
        rows.push(checked.instances);
    }
    verify_complete(capture, &targets, &counts)?;
    created.sort_unstable_by_key(|created| created.0);
    // Inserted in creation order into a map grown like the single pass's, so its layout and
    // iteration order are that map's.
    let mut instances = FxHashMap::default();
    for (_, chunk, instance) in created {
        let row = rows[chunk].remove(&instance).expect("a created row");
        instances.insert(instance, row);
    }
    Ok(NativePositionProjection {
        instances: Arc::new(instances),
    })
}
