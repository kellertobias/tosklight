use super::{
    PortableShowDocument, PortableShowObject, PortableShowObjectKey, PortableShowRevision,
    bump_revision,
    repository::{delete_current, immediate_transaction, write_current},
    store::current_revision,
};
use crate::{ShowStore, StoreError};
use chrono::Utc;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Atomic candidate mutation guarded by one whole-show revision.
#[derive(Clone, Debug)]
pub struct PortableShowTransaction {
    expected: PortableShowRevision,
    writes: BTreeMap<PortableShowObjectKey, Value>,
    deletes: BTreeSet<PortableShowObjectKey>,
}

/// Targeted result of one committed portable-show transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableShowCommit {
    revision: PortableShowRevision,
    written: Vec<PortableShowObject>,
    deleted: Vec<PortableShowObjectKey>,
}

impl PortableShowCommit {
    pub const fn revision(&self) -> PortableShowRevision {
        self.revision
    }

    pub fn written_objects(&self) -> &[PortableShowObject] {
        &self.written
    }

    pub fn deleted_objects(&self) -> &[PortableShowObjectKey] {
        &self.deleted
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
        }
    }

    pub const fn expected_revision(&self) -> PortableShowRevision {
        self.expected
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

    pub fn is_empty(&self) -> bool {
        self.writes.is_empty() && self.deletes.is_empty()
    }

    pub fn change_count(&self) -> usize {
        self.writes.len() + self.deletes.len()
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
        let revision = committed_revision(&tx, applied.changed())?;
        tx.commit()?;
        Ok(applied.into_commit(revision))
    }
}

struct AppliedChanges {
    written: Vec<PortableShowObject>,
    deleted: Vec<PortableShowObjectKey>,
}

impl AppliedChanges {
    fn changed(&self) -> bool {
        !self.written.is_empty() || !self.deleted.is_empty()
    }

    fn into_commit(self, revision: PortableShowRevision) -> PortableShowCommit {
        PortableShowCommit {
            revision,
            written: self.written,
            deleted: self.deleted,
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
    } = changes;
    let written = apply_writes(tx, writes)?;
    let deleted = apply_deletes(tx, deletes)?;
    Ok(AppliedChanges { written, deleted })
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

fn committed_revision(
    tx: &rusqlite::Transaction<'_>,
    changed: bool,
) -> Result<PortableShowRevision, StoreError> {
    if changed {
        bump_revision(tx)
    } else {
        current_revision(tx)
    }
}
