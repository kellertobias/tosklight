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
#[derive(Default)]
pub(super) struct StaticFamilyRows {
    rows: FxHashMap<(FixtureId, ProgrammingOwner), StaticFamilyRow>,
    cohort: u64,
}

pub(super) struct StaticFamilyRow {
    base: AttributeValue,
    pub(super) value: AttributeValue,
    pub(super) trace: FamilyTraceArena,
    pub(super) projection: RefCell<Option<KeptProjection>>,
    cohort: u64,
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
        let kept = self.rows.get(&key).is_some_and(|row| row.base == *base);
        if !kept {
            let value = compose(scratch)?;
            self.rows.insert(
                key,
                StaticFamilyRow {
                    base: base.clone(),
                    value,
                    trace: scratch.family_trace().clone(),
                    projection: RefCell::new(None),
                    cohort,
                },
            );
        }
        let row = self.rows.get_mut(&key).expect("kept or just inserted");
        row.cohort = cohort;
        Ok(row)
    }

    /// Drops the rows the finished cohort did not use and starts the next one.
    pub fn finish_cohort(&mut self) {
        let cohort = self.cohort;
        self.rows.retain(|_, row| row.cohort == cohort);
        self.cohort += 1;
    }
}

#[cfg(test)]
mod tests;
