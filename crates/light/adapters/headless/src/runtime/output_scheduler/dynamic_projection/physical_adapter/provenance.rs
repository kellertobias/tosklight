//! Completed per-observation evidence. Sharing never reuses a query across frames.
use super::*;
use std::{fmt, ops::Deref};

/// Owned immutable source/control evidence for exactly the consumed fields.
#[derive(Clone)]
pub(in crate::runtime) struct PhysicalProvenance(Arc<PhysicalProvenanceData>);

/// Read-only after the observation has completed its mutable source projection.
pub(in crate::runtime) struct PhysicalProvenanceData {
    pub fields: ProgrammingFieldScope,
    pub sources: DynamicFamilySourceProjection,
    /// None when the composer has no controller trace for these fields.
    pub controls: Option<Vec<FamilyControlContribution>>,
}

impl PhysicalProvenance {
    pub fn new(
        fields: ProgrammingFieldScope,
        sources: DynamicFamilySourceProjection,
        controls: Option<Vec<FamilyControlContribution>>,
    ) -> Self {
        Self(Arc::new(PhysicalProvenanceData {
            fields,
            sources,
            controls,
        }))
    }
}

impl Deref for PhysicalProvenance {
    type Target = PhysicalProvenanceData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Preserve the original derived struct's name, fields and order in retained fingerprints.
impl fmt::Debug for PhysicalProvenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhysicalProvenance")
            .field("fields", &self.fields)
            .field("sources", &self.sources)
            .field("controls", &self.controls)
            .finish()
    }
}
