//! A lane's static targets of one owner, scanned on the output pool (TL-639 round 6).
//!
//! Finding the fixtures and logical heads whose static baseline holds an owner's family value
//! and which the lane has a destination for reads only the baseline and the lane's compiled
//! descriptors. Chunks of fixtures are scanned on the pool against the lane's read-only state;
//! the frame then takes the found targets in fixture/head order with the sequential scan's
//! duplicate and error rules. A descriptor the lane has not compiled for this generation (the
//! first frame after a patch change) sends the whole scan back to the frame's thread.
use super::worker::LaneAccess;
use super::*;
use std::cell::Cell;

/// From this many fixtures a scan runs on the pool.
const MIN_PARALLEL_FIXTURES: usize = if cfg!(test) { 2 } else { 512 };

/// A target whose baseline holds the owner's family, as the lane answered for it.
enum Found {
    Owned(FixtureId),
    Failed(FixtureId, TransitionError),
}

impl<A: PhysicalFamilyAdapter> PhysicalAdapterLane<A>
where
    for<'l> LaneShared<'l, A>: Sync,
{
    /// Append to `targets`, in fixture/head order, every target whose `baseline` value of
    /// `owner` `has_family` accepts and which this lane has a destination for, skipping pairs
    /// already in `seen` (and adding the new ones), exactly as a scan on the frame's thread.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::runtime) fn static_targets(
        &self,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        owner: ProgrammingOwner,
        has_family: fn(&AttributeValue) -> bool,
        targets: &mut Vec<(FixtureId, ProgrammingOwner)>,
        seen: &mut rustc_hash::FxHashSet<(FixtureId, ProgrammingOwner)>,
        pool: Option<&light_engine::parallel::OutputPool>,
    ) -> Result<(), TransitionError> {
        let fixtures = &frame.capture.snapshot().fixtures;
        let scanned = pool
            .filter(|_| fixtures.len() >= MIN_PARALLEL_FIXTURES)
            .and_then(|pool| self.scan_on(pool, frame, baseline, owner, has_family));
        let Some(found) = scanned else {
            for fixture in fixtures.iter() {
                for target in std::iter::once(fixture.fixture_id)
                    .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
                {
                    if !baseline
                        .value(target, owner.key_ref())
                        .is_some_and(has_family)
                        || seen.contains(&(target, owner))
                    {
                        continue;
                    }
                    match self.descriptor(frame, target, owner) {
                        Ok(_) => {
                            seen.insert((target, owner));
                            targets.push((target, owner));
                        }
                        Err(TransitionError::Requires(_)) => {}
                        Err(error) => return Err(error),
                    }
                }
            }
            return Ok(());
        };
        for found in found {
            match found {
                Found::Owned(target) => {
                    if seen.insert((target, owner)) {
                        targets.push((target, owner));
                    }
                }
                Found::Failed(target, error) => {
                    if !seen.contains(&(target, owner)) {
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    /// The scan on `pool`, or `None` when a worker met a descriptor the lane has not compiled.
    fn scan_on(
        &self,
        pool: &light_engine::parallel::OutputPool,
        frame: HybridFrameContext<'_>,
        baseline: &light_engine::PreparedStaticFamilyFrame,
        owner: ProgrammingOwner,
        has_family: fn(&AttributeValue) -> bool,
    ) -> Option<Vec<Found>> {
        let fixtures = &frame.capture.snapshot().fixtures;
        let chunks = light_engine::parallel::chunk_count(fixtures.len(), pool.workers(), 64);
        let mut slots = vec![(); pool.workers()];
        let scanned = self.with_shared(|shared| {
            light_engine::parallel::run_ordered(Some(pool), &mut slots, chunks, |_, chunk| {
                let missed = Cell::new(false);
                let lane = LaneWorker::new(shared, &missed, Default::default());
                let mut found = Vec::new();
                for fixture in
                    &fixtures[light_engine::parallel::chunk_range(fixtures.len(), chunks, chunk)]
                {
                    for target in std::iter::once(fixture.fixture_id)
                        .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
                    {
                        if !baseline
                            .value(target, owner.key_ref())
                            .is_some_and(has_family)
                        {
                            continue;
                        }
                        match lane.descriptor(frame, target, owner) {
                            Ok(_) => found.push(Found::Owned(target)),
                            Err(TransitionError::Requires(_)) => {}
                            Err(_) if missed.get() => return None,
                            Err(error) => found.push(Found::Failed(target, error)),
                        }
                    }
                }
                Some(found)
            })
        });
        let mut found = Vec::new();
        for chunk in scanned {
            found.extend(chunk?);
        }
        Some(found)
    }
}
