//! TL-842 C2: materialized leaf provenance shared between consecutive frames.
use super::*;

/// The leaves of one materialized authored leaf: its source, and its Current dependency.
fn leaf_provenance(
    authored: FamilyTraceSource,
    dependency: Option<&crate::DynamicSourceDependency>,
) -> Arc<[trace::FamilyTraceLeaf]> {
    match dependency {
        Some(dependency) => Arc::new([
            trace::FamilyTraceLeaf::Source(authored),
            trace::FamilyTraceLeaf::Current(
                FamilyTraceSource {
                    role: FamilyTraceRole::CalculationDependency,
                    occurrence: dependency.occurrence,
                    ..authored
                },
                dependency.clone(),
            ),
        ]),
        None => Arc::new([trace::FamilyTraceLeaf::Source(authored)]),
    }
}

/// One interned leaf provenance: its dependency input and the shared leaves.
pub(crate) struct LeafProvenanceEntry(
    Option<crate::DynamicSourceDependency>,
    Arc<[trace::FamilyTraceLeaf]>,
);
pub(crate) type LeafProvenanceMap = rustc_hash::FxHashMap<FamilyTraceSource, LeafProvenanceEntry>;
pub(crate) type LeafProvenanceLog = (FamilyTraceSource, LeafProvenanceEntry);

/// TL-842 C2: the leaf provenance one preparation materialized, so the next can share it.
/// Entries are keyed by their complete inputs (the authored source with its rank and
/// footprint, plus the dependency), so a shared entry is exactly what would be rebuilt. Each
/// frame keeps only what it used.
#[derive(Default)]
pub(crate) struct LeafProvenanceCache {
    previous: LeafProvenanceMap,
    next: LeafProvenanceMap,
}

impl LeafProvenanceCache {
    /// Start a frame: last frame's entries become reusable; unused ones end with this frame.
    pub(crate) fn begin_frame(&mut self) {
        std::mem::swap(&mut self.previous, &mut self.next);
        self.next.clear();
    }

    pub(crate) fn clear(&mut self) {
        self.previous.clear();
        self.next.clear();
    }

    pub(crate) fn owned(&mut self) -> LeafProvenance<'_> {
        LeafProvenance::Owned(self)
    }

    /// Last frame's entries, read by parallel workers that log what they use.
    pub(crate) fn previous(&self) -> &LeafProvenanceMap {
        &self.previous
    }

    pub(crate) fn keep(
        &mut self,
        logged: impl IntoIterator<Item = (FamilyTraceSource, LeafProvenanceEntry)>,
    ) {
        self.next.extend(logged);
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> (usize, usize) {
        (self.previous.len(), self.next.len())
    }
}

/// Where one preparer finds and records leaf provenance.
pub(crate) enum LeafProvenance<'a> {
    /// Always build (callers without a retained preparation).
    Fresh,
    Owned(&'a mut LeafProvenanceCache),
    Shared {
        previous: &'a LeafProvenanceMap,
        log: &'a mut Vec<LeafProvenanceLog>,
    },
}

impl LeafProvenance<'_> {
    pub(super) fn intern(
        &mut self,
        authored: FamilyTraceSource,
        dependency: Option<&crate::DynamicSourceDependency>,
    ) -> Arc<[trace::FamilyTraceLeaf]> {
        let matches = |entry: &&LeafProvenanceEntry| entry.0.as_ref() == dependency;
        match self {
            Self::Fresh => leaf_provenance(authored, dependency),
            Self::Owned(cache) => {
                let leaves = match cache.next.get(&authored).filter(matches) {
                    Some(entry) => return Arc::clone(&entry.1),
                    None => cache
                        .previous
                        .get(&authored)
                        .filter(matches)
                        .map(|entry| Arc::clone(&entry.1))
                        .unwrap_or_else(|| leaf_provenance(authored, dependency)),
                };
                cache.next.insert(
                    authored,
                    LeafProvenanceEntry(dependency.cloned(), Arc::clone(&leaves)),
                );
                leaves
            }
            Self::Shared { previous, log } => {
                let leaves = previous
                    .get(&authored)
                    .filter(matches)
                    .map(|entry| Arc::clone(&entry.1))
                    .unwrap_or_else(|| leaf_provenance(authored, dependency));
                log.push((
                    authored,
                    LeafProvenanceEntry(dependency.cloned(), Arc::clone(&leaves)),
                ));
                leaves
            }
        }
    }
}
