//! Kept compositions of static-only family rows (TL-639 round 3).
//!
//! A static-only owner (a family no Dynamic samples, kept so the physical adapter fits its
//! static value) recomposed its base every frame for the same answer. Composition without
//! samples reads only the owner, the base value and the fixed authoring model: it adopts
//! nothing, samples no Current and resolves no frame transition. So a kept composition is
//! exact while the base is equal. The kept trace answers the observer's field queries exactly
//! as the fresh one did.
//!
//! The observer's source projection is kept too. It reads the trace query, the bound static
//! evidence occurrence and that occurrence's record, nothing else, so it is exact while the
//! requested fields, the occurrence and the record catalogue are unchanged. The occurrence is
//! bound every frame, which keeps the evidence alive and detects a changed source.

use super::*;
use light_core::programming::ProgrammingFieldScope;
use light_dynamics::{DynamicSourceOccurrenceId, FamilyTraceArena};

/// Kept rows of one evaluating lane, by target and owner. Rows no cohort used are dropped at
/// the end of the next cohort.
///
/// Rows live in [`ROW_SHARDS`] maps by target (TL-639 round 5), so a parallel section lends
/// each worker the shards of its own targets by moving maps, never rows.
pub(super) struct StaticFamilyRows {
    shards: Vec<RowShard>,
    /// The index of `shards[0]` among all shards (non-zero only for a lent part).
    first: usize,
    cohort: u64,
}

type RowShard = FxHashMap<(FixtureId, ProgrammingOwner), StaticFamilyRow>;

/// Shards of a lane's kept rows; parallel sections cut targets along them.
pub(super) const ROW_SHARDS: usize = 64;

/// The shard of a target's rows (and its parallel chunk's unit).
pub(super) fn row_shard(target: FixtureId) -> usize {
    let (high, low) = target.0.as_u64_pair();
    ((high ^ low) % ROW_SHARDS as u64) as usize
}

impl Default for StaticFamilyRows {
    fn default() -> Self {
        Self {
            shards: (0..ROW_SHARDS).map(|_| RowShard::default()).collect(),
            first: 0,
            cohort: 0,
        }
    }
}

pub(super) struct StaticFamilyRow {
    base: AttributeValue,
    pub(super) value: AttributeValue,
    pub(super) trace: FamilyTraceArena,
    pub(super) projection: RefCell<Option<KeptProjection>>,
    /// The observer's pure derivations of `value` and `trace` (TL-639 round 4).
    pub(super) observed: RefCell<KeptObservation>,
    cohort: u64,
}

/// Answers an observer derives from a kept row alone, kept with the row: the fields its value
/// consumes are a function of (owner, value), and the control sources of a field scope a function
/// of (owner, trace, scope). A recomposed row starts with none.
#[derive(Default)]
pub(in crate::runtime::output_scheduler::dynamic_projection) struct KeptObservation {
    pub consumed: Option<ProgrammingFieldScope>,
    pub controls: Option<(
        ProgrammingFieldScope,
        Option<Vec<light_dynamics::FamilyControlContribution>>,
    )>,
}

/// One kept answer of `CapturedFamilyObservation::project_fields`.
pub(super) struct KeptProjection {
    pub fields: ProgrammingFieldScope,
    /// The family address whose static occurrence the query bound, if it bound one.
    pub address: Option<DynamicValueAddress>,
    pub baseline: Option<DynamicSourceOccurrenceId>,
    /// The record catalogue the answer read (None: the frame had no catalogue). Records are
    /// immutable, so an unchanged catalogue proves an unchanged baseline record.
    pub records: Option<crate::runtime::dynamic_source_origins::RecordsIdentity>,
    pub result: DynamicFamilySourceProjection,
}

impl StaticFamilyRows {
    /// The kept row for `base`, composing it with `compose` (which fills `scratch`) when there
    /// is none or its base changed. A failed composition keeps nothing.
    pub fn row(
        &mut self,
        key: (FixtureId, ProgrammingOwner),
        base: &AttributeValue,
        scratch: &mut light_dynamics::RetainedFamilyCompositionScratch,
        compose: impl FnOnce(
            &mut light_dynamics::RetainedFamilyCompositionScratch,
        ) -> Result<AttributeValue, TransitionError>,
    ) -> Result<&StaticFamilyRow, TransitionError> {
        let cohort = self.cohort;
        let rows = &mut self.shards[row_shard(key.0) - self.first];
        let kept = rows.get(&key).is_some_and(|row| row.base == *base);
        if !kept {
            let value = compose(scratch)?;
            rows.insert(
                key,
                StaticFamilyRow {
                    base: base.clone(),
                    value,
                    trace: scratch.family_trace().clone(),
                    projection: RefCell::new(None),
                    observed: RefCell::default(),
                    cohort,
                },
            );
        }
        let row = rows.get_mut(&key).expect("kept or just inserted");
        row.cohort = cohort;
        Ok(row)
    }

    /// Lend the shards `ranges[chunk]` to parallel worker `chunk` (TL-639 round 5), stamped
    /// with this cohort; [`Self::take_back`] returns them.
    pub fn lend(&mut self, ranges: &[std::ops::Range<usize>]) -> Vec<StaticFamilyRows> {
        ranges
            .iter()
            .map(|range| StaticFamilyRows {
                shards: self.shards[range.clone()]
                    .iter_mut()
                    .map(std::mem::take)
                    .collect(),
                first: range.start,
                cohort: self.cohort,
            })
            .collect()
    }

    /// Take back shards lent to a worker, with whatever it composed or used.
    pub fn take_back(&mut self, lent: StaticFamilyRows) {
        for (offset, shard) in lent.shards.into_iter().enumerate() {
            self.shards[lent.first + offset - self.first] = shard;
        }
    }

    /// Drops the rows the finished cohort did not use and starts the next one.
    pub fn finish_cohort(&mut self) {
        let cohort = self.cohort;
        for rows in &mut self.shards {
            rows.retain(|_, row| row.cohort == cohort);
        }
        self.cohort += 1;
    }
}

#[cfg(test)]
impl StaticFamilyRows {
    fn is_empty(&self) -> bool {
        self.shards.iter().all(FxHashMap::is_empty)
    }

    fn contains(&self, key: &(FixtureId, ProgrammingOwner)) -> bool {
        self.shards[row_shard(key.0) - self.first].contains_key(key)
    }
}

#[cfg(test)]
mod tests;
