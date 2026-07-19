use super::{
    FixtureProfileRevision, FixtureProfileRevisionId, PortablePatchRevision, PortableShowDocument,
    PortableShowObject, PortableShowObjectKey, PortableShowRevision, bump_revision,
    profile_revision::{
        FixtureProfileRevisionInsertStatus, insert_fixture_profile_revision_in, profile_conflict,
    },
    repository::{delete_current, immediate_transaction, write_current},
    store::{bump_patch_revision, current_patch_revision, current_revision},
};
use crate::{ShowStore, StoreError};
use chrono::Utc;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Atomic candidate mutation guarded by one whole-show revision.
#[derive(Clone, Debug)]
pub struct PortableShowTransaction {
    pub(super) expected: PortableShowRevision,
    pub(super) writes: BTreeMap<PortableShowObjectKey, Value>,
    pub(super) deletes: BTreeSet<PortableShowObjectKey>,
    pub(super) profile_revisions: BTreeMap<FixtureProfileRevisionId, FixtureProfileRevision>,
    pub(super) patch_changed: bool,
}

/// Targeted result of one committed portable-show transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableShowCommit {
    revision: PortableShowRevision,
    patch_revision: PortablePatchRevision,
    written: Vec<PortableShowObject>,
    deleted: Vec<PortableShowObjectKey>,
    profile_revisions: Vec<FixtureProfileRevision>,
}

impl PortableShowCommit {
    pub const fn revision(&self) -> PortableShowRevision {
        self.revision
    }

    pub const fn patch_revision(&self) -> PortablePatchRevision {
        self.patch_revision
    }

    pub fn written_objects(&self) -> &[PortableShowObject] {
        &self.written
    }

    pub fn deleted_objects(&self) -> &[PortableShowObjectKey] {
        &self.deleted
    }

    pub fn fixture_profile_revisions(&self) -> &[FixtureProfileRevision] {
        &self.profile_revisions
    }

    pub fn written_object(&self, kind: &str, id: &str) -> Option<&PortableShowObject> {
        self.written
            .iter()
            .find(|object| object.key().kind() == kind && object.key().id() == id)
    }
}

impl PortableShowTransaction {
    pub fn new(expected: PortableShowRevision) -> Self {
        Self {
            expected,
            writes: BTreeMap::new(),
            deletes: BTreeSet::new(),
            profile_revisions: BTreeMap::new(),
            patch_changed: false,
        }
    }

    pub const fn expected_revision(&self) -> PortableShowRevision {
        self.expected
    }

    /// Marks this transaction as one patch change; repeated calls remain one change.
    pub fn mark_patch_changed(&mut self) -> &mut Self {
        self.patch_changed = true;
        self
    }

    /// Adds or replaces a raw object while retaining every supplied JSON field.
    pub fn put(
        &mut self,
        kind: impl Into<String>,
        id: impl Into<String>,
        body: Value,
    ) -> &mut Self {
        let key = PortableShowObjectKey::new(kind, id);
        self.deletes.remove(&key);
        self.writes.insert(key, body);
        self
    }

    /// Writes an object previously loaded from a portable document.
    pub fn put_object(&mut self, object: &PortableShowObject) -> &mut Self {
        let key = object.key().clone();
        self.deletes.remove(&key);
        self.writes.insert(key, object.body().clone());
        self
    }

    pub fn delete(&mut self, kind: impl Into<String>, id: impl Into<String>) -> &mut Self {
        let key = PortableShowObjectKey::new(kind, id);
        self.writes.remove(&key);
        self.deletes.insert(key);
        self
    }

    pub fn put_fixture_profile_revision(
        &mut self,
        candidate: FixtureProfileRevision,
    ) -> Result<&mut Self, StoreError> {
        if let Some(existing) = self.profile_revisions.get(candidate.id()) {
            if existing.digest() != candidate.digest() {
                return Err(profile_conflict(existing, &candidate));
            }
            return Ok(self);
        }
        self.profile_revisions
            .insert(candidate.id().clone(), candidate);
        Ok(self)
    }

    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
            && self.deletes.is_empty()
            && self.profile_revisions.is_empty()
            && !self.patch_changed
    }

    pub fn change_count(&self) -> usize {
        self.writes.len()
            + self.deletes.len()
            + self.profile_revisions.len()
            + usize::from(self.patch_changed)
    }
}

impl PortableShowDocument {
    /// Starts a candidate transaction against this exact document revision.
    pub fn transaction(&self) -> PortableShowTransaction {
        PortableShowTransaction::new(self.revision())
    }
}

impl ShowStore {
    /// Atomically applies all raw object changes or none if the document is stale.
    pub fn apply_portable_transaction(
        &self,
        changes: PortableShowTransaction,
    ) -> Result<PortableShowCommit, StoreError> {
        let tx = immediate_transaction(&self.conn)?;
        ensure_document_revision(&tx, changes.expected)?;
        let applied = apply_changes(&tx, changes)?;
        let (revision, patch_revision) =
            committed_revisions(&tx, applied.changed(), applied.patch_changed)?;
        tx.commit()?;
        Ok(applied.into_commit(revision, patch_revision))
    }
}

struct AppliedChanges {
    written: Vec<PortableShowObject>,
    deleted: Vec<PortableShowObjectKey>,
    profile_revisions: Vec<FixtureProfileRevision>,
    patch_changed: bool,
}

impl AppliedChanges {
    fn changed(&self) -> bool {
        self.patch_changed
            || !self.written.is_empty()
            || !self.deleted.is_empty()
            || !self.profile_revisions.is_empty()
    }

    fn into_commit(
        self,
        revision: PortableShowRevision,
        patch_revision: PortablePatchRevision,
    ) -> PortableShowCommit {
        PortableShowCommit {
            revision,
            patch_revision,
            written: self.written,
            deleted: self.deleted,
            profile_revisions: self.profile_revisions,
        }
    }
}

fn ensure_document_revision(
    tx: &rusqlite::Transaction<'_>,
    expected: PortableShowRevision,
) -> Result<(), StoreError> {
    let current = current_revision(tx)?;
    if current == expected {
        Ok(())
    } else {
        Err(StoreError::DocumentRevisionConflict { expected, current })
    }
}

fn apply_changes(
    tx: &rusqlite::Transaction<'_>,
    changes: PortableShowTransaction,
) -> Result<AppliedChanges, StoreError> {
    let PortableShowTransaction {
        expected: _,
        writes,
        deletes,
        profile_revisions,
        patch_changed,
    } = changes;
    Ok(AppliedChanges {
        profile_revisions: apply_profile_revisions(tx, profile_revisions)?,
        written: apply_writes(tx, writes)?,
        deleted: apply_deletes(tx, deletes)?,
        patch_changed,
    })
}

fn apply_profile_revisions(
    tx: &rusqlite::Transaction<'_>,
    profiles: BTreeMap<FixtureProfileRevisionId, FixtureProfileRevision>,
) -> Result<Vec<FixtureProfileRevision>, StoreError> {
    let mut inserted = Vec::with_capacity(profiles.len());
    for profile in profiles.into_values() {
        if insert_fixture_profile_revision_in(tx, &profile)?
            == FixtureProfileRevisionInsertStatus::Inserted
        {
            inserted.push(profile);
        }
    }
    Ok(inserted)
}

fn apply_writes(
    tx: &rusqlite::Transaction<'_>,
    writes: BTreeMap<PortableShowObjectKey, Value>,
) -> Result<Vec<PortableShowObject>, StoreError> {
    let updated_at = Utc::now().to_rfc3339();
    let mut written = Vec::with_capacity(writes.len());
    for (key, body) in writes {
        let revision = write_current(tx, &key, &body, &updated_at)?;
        written.push(PortableShowObject::new(
            key,
            body,
            revision,
            updated_at.clone(),
        ));
    }
    Ok(written)
}

fn apply_deletes(
    tx: &rusqlite::Transaction<'_>,
    deletes: BTreeSet<PortableShowObjectKey>,
) -> Result<Vec<PortableShowObjectKey>, StoreError> {
    let mut deleted = Vec::with_capacity(deletes.len());
    for key in deletes {
        if delete_current(tx, &key)? {
            deleted.push(key);
        }
    }
    Ok(deleted)
}

fn committed_revisions(
    tx: &rusqlite::Transaction<'_>,
    changed: bool,
    patch_changed: bool,
) -> Result<(PortableShowRevision, PortablePatchRevision), StoreError> {
    let patch_revision = if patch_changed {
        bump_patch_revision(tx)?
    } else {
        current_patch_revision(tx)?
    };
    let show_revision = if changed {
        bump_revision(tx)?
    } else {
        current_revision(tx)?
    };
    Ok((show_revision, patch_revision))
}
