//! Binding a captured static baseline, against the catalogue itself or against a parallel
//! worker's overlay of it (TL-639 round 5).
//!
//! A frame's family groups bind the static baseline of their own `(target, owner)` only, and
//! every other catalogue read they make is a record lookup. A worker therefore binds into an
//! [`OriginsOverlay`] over the frame's catalogue, and the frame applies the overlays in group
//! order ([`DynamicSourceOrigins::apply_overlay`]). The binding logic is one body
//! ([`bind_static_evidence_in`]) for both, so a worker allocates, reuses and forgets exactly what
//! the catalogue would have; occurrence ids are random either way.

use super::*;
use rustc_hash::FxHashMap;

/// Record lookup, the only catalogue read a family source projection makes.
pub(in crate::runtime) trait SourceRecordLookup {
    fn record(&self, id: DynamicSourceOccurrenceId) -> Option<&Arc<DynamicSourceRecord>>;
}

/// Read and write access to an origins catalogue: the catalogue, or an overlay of it.
pub(in crate::runtime) trait OriginsStore: SourceRecordLookup {
    fn binding(&self, binding: &DynamicSourceBinding) -> Option<DynamicSourceOccurrenceId>;
    fn cached_static(&self, binding: &DynamicSourceBinding) -> Option<&CachedStaticEvidence>;
    fn insert_record(&mut self, id: DynamicSourceOccurrenceId, record: Arc<DynamicSourceRecord>);
    fn set_binding(&mut self, binding: DynamicSourceBinding, id: DynamicSourceOccurrenceId);
    fn remove_binding(&mut self, binding: &DynamicSourceBinding);
    fn set_cached_static(&mut self, binding: DynamicSourceBinding, cached: CachedStaticEvidence);
    fn remove_cached_static(&mut self, binding: &DynamicSourceBinding);
}

impl SourceRecordLookup for DynamicSourceOrigins {
    fn record(&self, id: DynamicSourceOccurrenceId) -> Option<&Arc<DynamicSourceRecord>> {
        self.records.get(&id)
    }
}

impl OriginsStore for DynamicSourceOrigins {
    fn binding(&self, binding: &DynamicSourceBinding) -> Option<DynamicSourceOccurrenceId> {
        self.bindings.get(binding).copied()
    }

    fn cached_static(&self, binding: &DynamicSourceBinding) -> Option<&CachedStaticEvidence> {
        self.static_evidence.get(binding)
    }

    fn insert_record(&mut self, id: DynamicSourceOccurrenceId, record: Arc<DynamicSourceRecord>) {
        Arc::make_mut(&mut self.records).insert(id, record);
    }

    fn set_binding(&mut self, binding: DynamicSourceBinding, id: DynamicSourceOccurrenceId) {
        Arc::make_mut(&mut self.bindings).insert(binding, id);
    }

    fn remove_binding(&mut self, binding: &DynamicSourceBinding) {
        Arc::make_mut(&mut self.bindings).remove(binding);
    }

    fn set_cached_static(&mut self, binding: DynamicSourceBinding, cached: CachedStaticEvidence) {
        Arc::make_mut(&mut self.static_evidence).insert(binding, cached);
    }

    fn remove_cached_static(&mut self, binding: &DynamicSourceBinding) {
        Arc::make_mut(&mut self.static_evidence).remove(binding);
    }
}

/// A worker's bindings, records and evidence cache over a catalogue it may not write, by group
/// (see `programming_projection::memo`): the group being composed, then the worker's earlier
/// groups, then the catalogue.
pub(in crate::runtime) struct OriginsOverlay<'b> {
    base: &'b DynamicSourceOrigins,
    group: OriginsChanges,
    chunk: OriginsChanges,
}

impl<'b> OriginsOverlay<'b> {
    pub fn new(base: &'b DynamicSourceOrigins) -> Self {
        Self {
            base,
            group: OriginsChanges::default(),
            chunk: OriginsChanges::default(),
        }
    }

    pub fn base(&self) -> &'b DynamicSourceOrigins {
        self.base
    }

    pub fn commit_group(&mut self) {
        let OriginsChanges {
            records,
            bindings,
            static_evidence,
        } = &mut self.group;
        self.chunk.records.extend(records.drain());
        self.chunk.bindings.extend(bindings.drain());
        self.chunk.static_evidence.extend(static_evidence.drain());
    }

    pub fn abort_group(&mut self) {
        self.group.records.clear();
        self.group.bindings.clear();
        self.group.static_evidence.clear();
    }

    /// The overlay's changes, owned, to apply to the catalogue after the section.
    pub fn into_changes(mut self) -> OriginsChanges {
        self.commit_group();
        self.chunk
    }

    fn layered<T>(&self, read: impl Fn(&OriginsChanges) -> Option<T>) -> Option<T> {
        read(&self.group).or_else(|| read(&self.chunk))
    }
}

/// What one worker's overlay changed. `None` entries are removals.
#[derive(Default)]
pub(in crate::runtime) struct OriginsChanges {
    records: FxHashMap<DynamicSourceOccurrenceId, Arc<DynamicSourceRecord>>,
    bindings: FxHashMap<DynamicSourceBinding, Option<DynamicSourceOccurrenceId>>,
    static_evidence: FxHashMap<DynamicSourceBinding, Option<CachedStaticEvidence>>,
}

impl SourceRecordLookup for OriginsOverlay<'_> {
    fn record(&self, id: DynamicSourceOccurrenceId) -> Option<&Arc<DynamicSourceRecord>> {
        self.group
            .records
            .get(&id)
            .or_else(|| self.chunk.records.get(&id))
            .or_else(|| self.base.records.get(&id))
    }
}

impl OriginsStore for OriginsOverlay<'_> {
    fn binding(&self, binding: &DynamicSourceBinding) -> Option<DynamicSourceOccurrenceId> {
        match self.layered(|changes| changes.bindings.get(binding).copied()) {
            Some(changed) => changed,
            None => self.base.bindings.get(binding).copied(),
        }
    }

    fn cached_static(&self, binding: &DynamicSourceBinding) -> Option<&CachedStaticEvidence> {
        match self
            .group
            .static_evidence
            .get(binding)
            .or_else(|| self.chunk.static_evidence.get(binding))
        {
            Some(changed) => changed.as_ref(),
            None => self.base.static_evidence.get(binding),
        }
    }

    fn insert_record(&mut self, id: DynamicSourceOccurrenceId, record: Arc<DynamicSourceRecord>) {
        self.group.records.insert(id, record);
    }

    fn set_binding(&mut self, binding: DynamicSourceBinding, id: DynamicSourceOccurrenceId) {
        self.group.bindings.insert(binding, Some(id));
    }

    fn remove_binding(&mut self, binding: &DynamicSourceBinding) {
        self.group.bindings.insert(*binding, None);
    }

    fn set_cached_static(&mut self, binding: DynamicSourceBinding, cached: CachedStaticEvidence) {
        self.group.static_evidence.insert(binding, Some(cached));
    }

    fn remove_cached_static(&mut self, binding: &DynamicSourceBinding) {
        self.group.static_evidence.insert(*binding, None);
    }
}

impl DynamicSourceOrigins {
    /// Apply one worker's changes. Workers bind disjoint `(target, owner)` baselines, so the
    /// result does not depend on which worker applies first; a record id another worker drew as
    /// well is rejected rather than overwritten.
    pub fn apply_overlay(&mut self, changes: OriginsChanges) -> Result<(), IntentError> {
        for (id, record) in changes.records {
            if self.records.contains_key(&id) {
                return Err(IntentError(
                    "two parallel workers drew the same Dynamic source occurrence".into(),
                ));
            }
            self.insert_record(id, record);
        }
        for (binding, id) in changes.bindings {
            match id {
                Some(id) => self.set_binding(binding, id),
                None if self.bindings.contains_key(&binding) => self.remove_binding(&binding),
                None => {}
            }
        }
        for (binding, cached) in changes.static_evidence {
            match cached {
                Some(cached) => self.set_cached_static(binding, cached),
                None if self.static_evidence.contains_key(&binding) => {
                    self.remove_cached_static(&binding)
                }
                None => {}
            }
        }
        Ok(())
    }
}

/// Validate a captured static family once per immutable evidence allocation. Ordinary singleton
/// observations compare borrowed metadata because Playback may recreate them on every frame.
/// Multi-source transitions reuse their captured Arc throughout the fade.
pub(in crate::runtime) fn bind_static_evidence_in(
    store: &mut (impl OriginsStore + ?Sized),
    binding: DynamicSourceBinding,
    evidence: &Arc<light_engine::ContributionFamilyEvidence>,
) -> Result<DynamicSourceOccurrenceId, IntentError> {
    if let Some(id) = store.binding(&binding) {
        if let [entry] = evidence.entries() {
            let record = store
                .record(id)
                .expect("an active Dynamic binding has its source record");
            if let DynamicSourceOrigin::StaticBaseline { sources } = &record.origin
                && let [existing] = sources.as_slice()
                && existing.matches_evidence(entry)
            {
                forget_static_evidence_in(store, &binding);
                return Ok(id);
            }
        } else if let Some(cached) = store.cached_static(&binding)
            && cached.occurrence_id == id
            && cached.evidence.as_ptr() == Arc::as_ptr(evidence)
        {
            // The Weak keeps its allocation identity reserved even after the value dies; an
            // incoming strong Arc cannot match a different, recycled allocation.
            return Ok(id);
        }
    }
    let sources = evidence
        .entries()
        .iter()
        .map(|entry| {
            DynamicStaticSourceEntry::from_evidence(
                DynamicStaticSource::from_contribution(entry.source()),
                entry,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    // bind validates before changing records, bindings or the runtime cache. A distinct
    // allocation with equivalent evidence keeps its occurrence and only warms this cache.
    let id = bind_in(
        store,
        binding,
        DynamicSourceOrigin::StaticBaseline { sources },
    )?;
    if evidence.entries().len() > 1 {
        store.set_cached_static(
            binding,
            CachedStaticEvidence {
                occurrence_id: id,
                evidence: Arc::downgrade(evidence),
            },
        );
    } else {
        forget_static_evidence_in(store, &binding);
    }
    Ok(id)
}

pub(super) fn forget_static_evidence_in(
    store: &mut (impl OriginsStore + ?Sized),
    binding: &DynamicSourceBinding,
) {
    if store.cached_static(binding).is_some() {
        store.remove_cached_static(binding);
    }
}

/// TL-641: a random version-4 UUID for a new occurrence without a system entropy call per id.
///
/// Starting a Dynamic on thousands of targets binds tens of thousands of sources in its first
/// frame, and `Uuid::new_v4` asks the operating system for entropy for every one of them. Each
/// thread seeds a SplitMix64 stream from one `Uuid::new_v4` instead; the ids stay random,
/// unpredictable between processes and v4-shaped, and `bind_in` still rejects a collision.
fn random_occurrence_uuid() -> Uuid {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<Option<u64>> = const { Cell::new(None) };
    }
    fn split_mix(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = *state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }
    STATE.with(|cell| {
        let mut state = cell
            .get()
            .unwrap_or_else(|| Uuid::new_v4().as_u64_pair().0 ^ Uuid::new_v4().as_u64_pair().1);
        let high = split_mix(&mut state);
        let low = split_mix(&mut state);
        cell.set(Some(state));
        uuid::Builder::from_random_bytes(((u128::from(high) << 64) | u128::from(low)).to_be_bytes())
            .into_uuid()
    })
}

pub(super) fn bind_in(
    store: &mut (impl OriginsStore + ?Sized),
    binding: DynamicSourceBinding,
    mut origin: DynamicSourceOrigin,
) -> Result<DynamicSourceOccurrenceId, IntentError> {
    validate_binding(binding)?;
    origin.validate(binding)?;
    origin.validate_controller(binding)?;
    origin.canonicalize();
    if let Some(id) = store.binding(&binding) {
        let existing = store
            .record(id)
            .expect("an active Dynamic binding has its source record");
        if existing.origin == origin {
            return Ok(id);
        }
    }
    let id = loop {
        let candidate = DynamicSourceOccurrenceId::new(random_occurrence_uuid())?;
        if store.record(candidate).is_none() {
            break candidate;
        }
    };
    store.insert_record(
        id,
        Arc::new(DynamicSourceRecord {
            occurrence_id: id,
            binding,
            origin,
        }),
    );
    store.set_binding(binding, id);
    forget_static_evidence_in(store, &binding);
    Ok(id)
}

/// Remove an active binding; its record stays for paused and interrupted history.
pub(in crate::runtime) fn unbind_in(
    store: &mut (impl OriginsStore + ?Sized),
    binding: &DynamicSourceBinding,
) -> bool {
    if store.binding(binding).is_none() {
        return false;
    }
    store.remove_binding(binding);
    forget_static_evidence_in(store, binding);
    true
}

#[cfg(test)]
mod random_id_tests {
    use super::random_occurrence_uuid;
    use std::collections::HashSet;

    #[test]
    fn occurrence_ids_are_distinct_random_v4_uuids_on_every_thread() {
        let ids = (0..4)
            .map(|_| {
                std::thread::spawn(|| {
                    (0..10_000)
                        .map(|_| random_occurrence_uuid())
                        .collect::<Vec<_>>()
                })
            })
            .flat_map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert!(
            ids.iter()
                .all(|id| id.get_version_num() == 4 && id.get_variant() == uuid::Variant::RFC4122)
        );
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    }
}
