use super::sync_requests::{SyncRequestRecord, insert_sync_request};
use super::{
    FixtureProfileRevision, FixtureProfileRevisionId, PortablePatchRevision, PortableShowDocument,
    PortableShowObject, PortableShowObjectKey, PortableShowObjectRedo, PortableShowObjectUndo,
    PortableShowRevision, bump_revision,
    profile_revision::{
        FixtureProfileRevisionInsertStatus, insert_fixture_profile_revision_in, profile_conflict,
    },
    repository::{
        delete_current, immediate_transaction, restore_staged_redo, restore_staged_undo,
        write_current,
    },
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
    pub(super) undoes: BTreeMap<PortableShowObjectKey, PortableShowUndoCondition>,
    pub(super) redoes: BTreeMap<PortableShowObjectKey, PortableShowUndoCondition>,
    pub(super) deletes: BTreeSet<PortableShowObjectKey>,
    pub(super) profile_revisions: BTreeMap<FixtureProfileRevisionId, FixtureProfileRevision>,
    pub(super) patch_changed: bool,
    /// Metadata values to write (`Some`) or remove (`None`) in the same commit.
    pub(super) metadata: BTreeMap<String, Option<String>>,
    /// The sync request identity recorded atomically with these changes.
    pub(super) sync_request: Option<SyncRequestRecord>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PortableShowUndoCondition {
    expected_object_revision: light_core::Revision,
    history_row_id: i64,
}

/// Targeted result of one committed portable-show transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableShowCommit {
    revision: PortableShowRevision,
    patch_revision: PortablePatchRevision,
    written: Vec<PortableShowObject>,
    deleted: Vec<PortableShowObjectKey>,
    profile_revisions: Vec<FixtureProfileRevision>,
    metadata: Vec<(String, Option<String>)>,
    sync_request: Option<(uuid::Uuid, String)>,
}

impl PortableShowCommit {
    /// Metadata keys this commit wrote (`Some`) or removed (`None`).
    pub fn metadata_changes(&self) -> &[(String, Option<String>)] {
        &self.metadata
    }

    /// `(association_id, request_id)` of the sync request recorded with this commit, if any.
    pub fn sync_request(&self) -> Option<(uuid::Uuid, &str)> {
        self.sync_request
            .as_ref()
            .map(|(association, request)| (*association, request.as_str()))
    }

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
            undoes: BTreeMap::new(),
            redoes: BTreeMap::new(),
            deletes: BTreeSet::new(),
            profile_revisions: BTreeMap::new(),
            patch_changed: false,
            metadata: BTreeMap::new(),
            sync_request: None,
        }
    }

    /// Writes one portable-show metadata value atomically with this transaction's objects.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.metadata.insert(key.into(), Some(value.into()));
        self
    }

    /// Removes one portable-show metadata value atomically with this transaction's objects.
    pub fn remove_metadata(&mut self, key: impl Into<String>) -> &mut Self {
        self.metadata.insert(key.into(), None);
        self
    }

    /// Records a sync request identity in the same commit as these changes. The row is written
    /// only when the transaction changes the show, so a retry finds it exactly when its edit
    /// landed.
    pub fn record_sync_request(&mut self, record: SyncRequestRecord) -> &mut Self {
        self.sync_request = Some(record);
        self
    }

    pub fn sync_request(&self) -> Option<&SyncRequestRecord> {
        self.sync_request.as_ref()
    }

    /// Metadata values staged by this transaction: written (`Some`) or removed (`None`).
    pub fn metadata_changes(&self) -> &BTreeMap<String, Option<String>> {
        &self.metadata
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
        self.undoes.remove(&key);
        self.redoes.remove(&key);
        self.writes.insert(key, body);
        self
    }

    /// Writes an object previously loaded from a portable document.
    pub fn put_object(&mut self, object: &PortableShowObject) -> &mut Self {
        let key = object.key().clone();
        self.deletes.remove(&key);
        self.undoes.remove(&key);
        self.redoes.remove(&key);
        self.writes.insert(key, object.body().clone());
        self
    }

    /// Stages the exact previous raw body while retaining its atomic history-pop condition.
    pub fn undo_object(&mut self, undo: PortableShowObjectUndo) -> &mut Self {
        let (key, body, expected_object_revision, history_row_id) = undo.into_parts();
        self.deletes.remove(&key);
        self.writes.insert(key.clone(), body);
        self.redoes.remove(&key);
        self.undoes.insert(
            key,
            PortableShowUndoCondition {
                expected_object_revision,
                history_row_id,
            },
        );
        self
    }

    /// Stages the exact next raw body while retaining its atomic forward-history condition.
    pub fn redo_object(&mut self, redo: PortableShowObjectRedo) -> &mut Self {
        let (key, body, expected_object_revision, history_row_id) = redo.into_parts();
        self.deletes.remove(&key);
        self.writes.insert(key.clone(), body);
        self.undoes.remove(&key);
        self.redoes.insert(
            key,
            PortableShowUndoCondition {
                expected_object_revision,
                history_row_id,
            },
        );
        self
    }

    pub fn delete(&mut self, kind: impl Into<String>, id: impl Into<String>) -> &mut Self {
        let key = PortableShowObjectKey::new(kind, id);
        self.writes.remove(&key);
        self.undoes.remove(&key);
        self.redoes.remove(&key);
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
            && self.metadata.is_empty()
            && !self.patch_changed
    }

    pub fn change_count(&self) -> usize {
        self.writes.len()
            + self.deletes.len()
            + self.profile_revisions.len()
            + self.metadata.len()
            + usize::from(self.patch_changed)
    }

    /// Storage kinds whose current objects are written or deleted by this transaction.
    pub fn changed_object_kinds(&self) -> impl Iterator<Item = &str> {
        self.writes
            .keys()
            .map(PortableShowObjectKey::kind)
            .chain(self.deletes.iter().map(PortableShowObjectKey::kind))
    }

    /// Raw bodies this transaction writes (object puts and staged Undo/Redo bodies).
    pub fn object_writes(&self) -> impl Iterator<Item = (&PortableShowObjectKey, &Value)> {
        self.writes.iter()
    }

    pub fn changed_object_keys(&self) -> impl Iterator<Item = &PortableShowObjectKey> {
        self.writes.keys().chain(self.deletes.iter())
    }

    pub const fn patch_changed(&self) -> bool {
        self.patch_changed
    }

    pub fn fixture_profile_revisions_changed(&self) -> bool {
        !self.profile_revisions.is_empty()
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
        let sync_request = changes.sync_request.clone();
        let applied = apply_changes(&tx, changes)?;
        let (revision, patch_revision) =
            committed_revisions(&tx, applied.changed(), applied.patch_changed)?;
        let sync_request = match sync_request {
            Some(record) if applied.changed() => {
                insert_sync_request(&tx, &record, revision.value())?;
                Some((record.association_id, record.request_id))
            }
            _ => None,
        };
        tx.commit()?;
        let mut applied = applied;
        applied.sync_request = sync_request;
        Ok(applied.into_commit(revision, patch_revision))
    }
}

struct AppliedChanges {
    written: Vec<PortableShowObject>,
    deleted: Vec<PortableShowObjectKey>,
    profile_revisions: Vec<FixtureProfileRevision>,
    metadata: Vec<(String, Option<String>)>,
    patch_changed: bool,
    sync_request: Option<(uuid::Uuid, String)>,
}

impl AppliedChanges {
    fn changed(&self) -> bool {
        self.patch_changed
            || !self.written.is_empty()
            || !self.deleted.is_empty()
            || !self.profile_revisions.is_empty()
            || !self.metadata.is_empty()
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
            metadata: self.metadata,
            sync_request: self.sync_request,
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
        undoes,
        redoes,
        deletes,
        profile_revisions,
        patch_changed,
        metadata,
        sync_request: _,
    } = changes;
    Ok(AppliedChanges {
        profile_revisions: apply_profile_revisions(tx, profile_revisions)?,
        written: apply_writes(tx, writes, undoes, redoes)?,
        deleted: apply_deletes(tx, deletes)?,
        metadata: apply_metadata(tx, metadata)?,
        patch_changed,
        sync_request: None,
    })
}

fn apply_metadata(
    tx: &rusqlite::Transaction<'_>,
    metadata: BTreeMap<String, Option<String>>,
) -> Result<Vec<(String, Option<String>)>, StoreError> {
    for (key, value) in &metadata {
        match value {
            Some(value) => tx.execute(
                "INSERT INTO metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                (key, value),
            )?,
            None => tx.execute("DELETE FROM metadata WHERE key=?1", [key])?,
        };
    }
    Ok(metadata.into_iter().collect())
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
    mut undoes: BTreeMap<PortableShowObjectKey, PortableShowUndoCondition>,
    mut redoes: BTreeMap<PortableShowObjectKey, PortableShowUndoCondition>,
) -> Result<Vec<PortableShowObject>, StoreError> {
    let updated_at = Utc::now().to_rfc3339();
    let mut written = Vec::with_capacity(writes.len());
    for (key, body) in writes {
        let revision = match (undoes.remove(&key), redoes.remove(&key)) {
            (Some(undo), None) => restore_staged_undo(
                tx,
                &key,
                undo.expected_object_revision,
                undo.history_row_id,
                &updated_at,
            )?,
            (None, Some(redo)) => restore_staged_redo(
                tx,
                &key,
                redo.expected_object_revision,
                redo.history_row_id,
                &updated_at,
            )?,
            (None, None) => write_current(tx, &key, &body, &updated_at)?,
            (Some(_), Some(_)) => unreachable!("one object cannot be undo and redo"),
        };
        written.push(PortableShowObject::new(
            key,
            body,
            revision,
            updated_at.clone(),
        ));
    }
    debug_assert!(undoes.is_empty(), "every staged undo also owns a write");
    debug_assert!(redoes.is_empty(), "every staged redo also owns a write");
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
