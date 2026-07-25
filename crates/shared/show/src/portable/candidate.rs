use super::{
    FixtureProfileRevision, FixtureProfileRevisionId, PortablePatchRevision, PortableShowDocument,
    PortableShowObject, PortableShowObjectKey, PortableShowRevision, PortableShowTransaction,
    profile_revision::profile_conflict,
};
use crate::StoreError;
use light_core::{FixtureId, Revision, ShowId};
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, btree_map},
    iter::Peekable,
    slice,
};

/// Borrowed projection of a document with one uncommitted transaction overlaid.
#[derive(Clone, Copy, Debug)]
pub struct PortableShowCandidate<'a> {
    document: &'a PortableShowDocument,
    transaction: &'a PortableShowTransaction,
    revision: PortableShowRevision,
    patch_revision: PortablePatchRevision,
}

/// Borrowed object projected from either the document or a staged write.
#[derive(Clone, Copy, Debug)]
pub struct PortableShowCandidateObject<'a> {
    key: &'a PortableShowObjectKey,
    body: &'a Value,
    revision: Revision,
}

impl<'a> PortableShowCandidateObject<'a> {
    pub const fn key(&self) -> &'a PortableShowObjectKey {
        self.key
    }

    pub const fn body(&self) -> &'a Value {
        self.body
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }
}

impl PortableShowDocument {
    /// Builds a validated, allocation-free view of the transaction's candidate state.
    pub fn candidate<'a>(
        &'a self,
        transaction: &'a PortableShowTransaction,
    ) -> Result<PortableShowCandidate<'a>, StoreError> {
        PortableShowCandidate::new(self, transaction)
    }
}

impl<'a> PortableShowCandidate<'a> {
    fn new(
        document: &'a PortableShowDocument,
        transaction: &'a PortableShowTransaction,
    ) -> Result<Self, StoreError> {
        ensure_expected_document(document, transaction)?;
        ensure_profile_revisions_compatible(document, transaction)?;
        ensure_object_revisions_available(document, transaction)?;
        let changed = candidate_changes_document(document, transaction);
        Ok(Self {
            document,
            transaction,
            revision: predicted_show_revision(document, changed)?,
            patch_revision: predicted_patch_revision(document, transaction.patch_changed)?,
        })
    }

    pub const fn id(&self) -> ShowId {
        self.document.id()
    }

    pub fn name(&self) -> &str {
        self.document.name()
    }

    pub const fn base_revision(&self) -> PortableShowRevision {
        self.document.revision()
    }

    /// Returns the patch revision before the transaction is applied.
    pub const fn base_patch_revision(&self) -> PortablePatchRevision {
        self.document.patch_revision()
    }

    /// Predicts the whole-show revision produced by a successful commit.
    pub const fn revision(&self) -> PortableShowRevision {
        self.revision
    }

    /// Predicts the targeted patch revision produced by a successful commit.
    pub const fn patch_revision(&self) -> PortablePatchRevision {
        self.patch_revision
    }

    pub fn metadata(&self) -> &BTreeMap<String, String> {
        self.document.metadata()
    }

    pub fn object(&self, kind: &str, id: &str) -> Option<PortableShowCandidateObject<'a>> {
        let lookup = PortableShowObjectKey::new(kind, id);
        if self.transaction.deletes.contains(&lookup) {
            return None;
        }
        if let Some((key, body)) = self.transaction.writes.get_key_value(&lookup) {
            return Some(self.staged_object(key, body));
        }
        self.document.object(kind, id).map(candidate_object)
    }

    /// Predicts an object's revision, or returns `None` when it will be absent.
    pub fn object_revision(&self, kind: &str, id: &str) -> Option<Revision> {
        self.object(kind, id).map(|object| object.revision())
    }

    pub fn objects(&self) -> PortableShowCandidateObjects<'a> {
        PortableShowCandidateObjects::new(*self)
    }

    pub fn objects_of_kind<'b>(
        &'b self,
        kind: &'b str,
    ) -> impl Iterator<Item = PortableShowCandidateObject<'a>> + 'b {
        self.objects()
            .filter(move |object| object.key().kind() == kind)
    }

    pub fn fixture_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Option<&'a FixtureProfileRevision> {
        let id = FixtureProfileRevisionId::new(profile_id, revision).ok()?;
        self.transaction
            .profile_revisions
            .get(&id)
            .or_else(|| self.document.fixture_profile_revision(profile_id, revision))
    }

    pub fn fixture_profile_revisions(&self) -> PortableShowCandidateProfiles<'a> {
        PortableShowCandidateProfiles::new(*self)
    }

    fn staged_object(
        &self,
        key: &'a PortableShowObjectKey,
        body: &'a Value,
    ) -> PortableShowCandidateObject<'a> {
        let revision = self
            .document
            .object(key.kind(), key.id())
            .map_or(1, |object| object.revision() + 1);
        PortableShowCandidateObject {
            key,
            body,
            revision,
        }
    }
}

/// Ordered, allocation-free iterator over candidate objects.
pub struct PortableShowCandidateObjects<'a> {
    candidate: PortableShowCandidate<'a>,
    base: Peekable<slice::Iter<'a, PortableShowObject>>,
    writes: Peekable<btree_map::Iter<'a, PortableShowObjectKey, Value>>,
}

impl<'a> PortableShowCandidateObjects<'a> {
    fn new(candidate: PortableShowCandidate<'a>) -> Self {
        Self {
            base: candidate.document.object_slice().iter().peekable(),
            writes: candidate.transaction.writes.iter().peekable(),
            candidate,
        }
    }

    fn discard_deleted_base(&mut self) {
        while self
            .base
            .peek()
            .is_some_and(|object| self.candidate.transaction.deletes.contains(object.key()))
        {
            self.base.next();
        }
    }

    fn next_base(&mut self) -> Option<PortableShowCandidateObject<'a>> {
        self.base.next().map(candidate_object)
    }

    fn next_write(&mut self) -> Option<PortableShowCandidateObject<'a>> {
        self.writes
            .next()
            .map(|(key, body)| self.candidate.staged_object(key, body))
    }
}

impl<'a> Iterator for PortableShowCandidateObjects<'a> {
    type Item = PortableShowCandidateObject<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.discard_deleted_base();
        let order = object_order(self.base.peek().copied(), self.writes.peek().copied());
        match order {
            None => None,
            Some(Ordering::Less) => self.next_base(),
            Some(Ordering::Greater) => self.next_write(),
            Some(Ordering::Equal) => {
                self.base.next();
                self.next_write()
            }
        }
    }
}

/// Ordered, allocation-free iterator over candidate fixture-profile revisions.
pub struct PortableShowCandidateProfiles<'a> {
    base: Peekable<slice::Iter<'a, FixtureProfileRevision>>,
    staged: Peekable<btree_map::Values<'a, FixtureProfileRevisionId, FixtureProfileRevision>>,
}

impl<'a> PortableShowCandidateProfiles<'a> {
    fn new(candidate: PortableShowCandidate<'a>) -> Self {
        Self {
            base: candidate
                .document
                .fixture_profile_revisions()
                .iter()
                .peekable(),
            staged: candidate.transaction.profile_revisions.values().peekable(),
        }
    }
}

impl<'a> Iterator for PortableShowCandidateProfiles<'a> {
    type Item = &'a FixtureProfileRevision;

    fn next(&mut self) -> Option<Self::Item> {
        match profile_order(self.base.peek().copied(), self.staged.peek().copied()) {
            None => None,
            Some(Ordering::Less) => self.base.next(),
            Some(Ordering::Greater) => self.staged.next(),
            Some(Ordering::Equal) => {
                self.base.next();
                self.staged.next()
            }
        }
    }
}

fn ensure_expected_document(
    document: &PortableShowDocument,
    transaction: &PortableShowTransaction,
) -> Result<(), StoreError> {
    if document.revision() == transaction.expected {
        Ok(())
    } else {
        Err(StoreError::DocumentRevisionConflict {
            expected: transaction.expected,
            current: document.revision(),
        })
    }
}

fn ensure_profile_revisions_compatible(
    document: &PortableShowDocument,
    transaction: &PortableShowTransaction,
) -> Result<(), StoreError> {
    for candidate in transaction.profile_revisions.values() {
        let id = candidate.id();
        if let Some(existing) = document.fixture_profile_revision(id.profile_id(), id.revision())
            && existing.digest() != candidate.digest()
        {
            return Err(profile_conflict(existing, candidate));
        }
    }
    Ok(())
}

fn ensure_object_revisions_available(
    document: &PortableShowDocument,
    transaction: &PortableShowTransaction,
) -> Result<(), StoreError> {
    let exhausted = transaction.writes.keys().any(|key| {
        document
            .object(key.kind(), key.id())
            .is_some_and(|object| object.revision() >= i64::MAX as u64)
    });
    if exhausted {
        Err(StoreError::Invalid("object revision overflow".into()))
    } else {
        Ok(())
    }
}

fn candidate_changes_document(
    document: &PortableShowDocument,
    transaction: &PortableShowTransaction,
) -> bool {
    transaction.patch_changed
        || !transaction.writes.is_empty()
        || transaction
            .deletes
            .iter()
            .any(|key| document.object(key.kind(), key.id()).is_some())
        || transaction.profile_revisions.values().any(|candidate| {
            let id = candidate.id();
            document
                .fixture_profile_revision(id.profile_id(), id.revision())
                .is_none()
        })
}

fn predicted_show_revision(
    document: &PortableShowDocument,
    changed: bool,
) -> Result<PortableShowRevision, StoreError> {
    if !changed {
        return Ok(document.revision());
    }
    document
        .revision()
        .value()
        .checked_add(1)
        .map(PortableShowRevision::new)
        .ok_or_else(|| StoreError::Invalid("portable show revision overflow".into()))
}

fn predicted_patch_revision(
    document: &PortableShowDocument,
    changed: bool,
) -> Result<PortablePatchRevision, StoreError> {
    if !changed {
        return Ok(document.patch_revision());
    }
    document
        .patch_revision()
        .value()
        .checked_add(1)
        .map(PortablePatchRevision::new)
        .ok_or_else(|| StoreError::Invalid("portable patch revision overflow".into()))
}

fn candidate_object(object: &PortableShowObject) -> PortableShowCandidateObject<'_> {
    PortableShowCandidateObject {
        key: object.key(),
        body: object.body(),
        revision: object.revision(),
    }
}

fn object_order(
    base: Option<&PortableShowObject>,
    staged: Option<(&PortableShowObjectKey, &Value)>,
) -> Option<Ordering> {
    match (base, staged) {
        (Some(base), Some((key, _))) => Some(base.key().cmp(key)),
        (Some(_), None) => Some(Ordering::Less),
        (None, Some(_)) => Some(Ordering::Greater),
        (None, None) => None,
    }
}

fn profile_order(
    base: Option<&FixtureProfileRevision>,
    staged: Option<&FixtureProfileRevision>,
) -> Option<Ordering> {
    match (base, staged) {
        (Some(base), Some(staged)) => Some(base.id().cmp(staged.id())),
        (Some(_), None) => Some(Ordering::Less),
        (None, Some(_)) => Some(Ordering::Greater),
        (None, None) => None,
    }
}
