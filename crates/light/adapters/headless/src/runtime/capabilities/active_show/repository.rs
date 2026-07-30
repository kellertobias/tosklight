//! Capability-owned boundary around portable Show persistence.
//!
//! Runtime adapters depend on this repository rather than opening or passing SQLite-backed
//! `ShowStore` values. The wrapper deliberately exposes only the persistence commands and immutable
//! projections used by the active-Show and Show-library capabilities.

use light_core::{Revision, ShowId};
use light_show::{
    AtomicObjectDelete, AtomicObjectWrite, PortableShowCommit, PortableShowDocument,
    PortableShowObjectUndo, PortableShowRevision, PortableShowTransaction, RevisionCopySource,
    ScheduleOccurrenceClaim, ScheduleOccurrenceClaimResult, ScheduleOccurrenceRecord,
    ScheduleOccurrenceResolution, ShowStore, StoreError, VersionedObject,
};
use std::path::Path;

pub(crate) struct ActiveShowRepository {
    store: ShowStore,
}

impl ActiveShowRepository {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        ShowStore::open(path).map(|store| Self { store })
    }

    pub(crate) fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), StoreError> {
        self.store.backup_to(destination)
    }

    pub(crate) fn set_identity(
        &self,
        id: ShowId,
        name: &str,
        revision_copy: Option<&RevisionCopySource>,
    ) -> Result<(), StoreError> {
        self.store.set_identity(id, name, revision_copy)
    }

    pub(crate) fn id(&self) -> Result<ShowId, StoreError> {
        self.store.id()
    }

    pub(crate) fn revision_copy_source(&self) -> Result<Option<RevisionCopySource>, StoreError> {
        self.store.revision_copy_source()
    }

    pub(crate) fn portable_document(&self) -> Result<PortableShowDocument, StoreError> {
        self.store.portable_document()
    }

    pub(crate) fn portable_revision(&self) -> Result<PortableShowRevision, StoreError> {
        self.store.portable_revision()
    }

    pub(crate) fn apply_portable_transaction(
        &self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, StoreError> {
        self.store.apply_portable_transaction(transaction)
    }

    pub(crate) fn prepare_object_undo(
        &self,
        kind: &str,
        object_id: &str,
        expected: Revision,
    ) -> Result<PortableShowObjectUndo, StoreError> {
        self.store.prepare_object_undo(kind, object_id, expected)
    }

    #[cfg(test)]
    pub(crate) fn undo_object(
        &self,
        kind: &str,
        object_id: &str,
        expected: Revision,
    ) -> Result<Revision, StoreError> {
        self.store.undo_object(kind, object_id, expected)
    }

    pub(crate) fn objects(&self, kind: &str) -> Result<Vec<VersionedObject>, StoreError> {
        self.store.objects(kind)
    }

    pub(crate) fn objects_with_portable_revision(
        &self,
        kind: &str,
    ) -> Result<(PortableShowRevision, Vec<VersionedObject>), StoreError> {
        self.store.objects_with_portable_revision(kind)
    }

    pub(crate) fn object_with_portable_revision(
        &self,
        kind: &str,
        object_id: &str,
    ) -> Result<(PortableShowRevision, Option<VersionedObject>), StoreError> {
        self.store.object_with_portable_revision(kind, object_id)
    }

    pub(crate) fn claim_schedule_occurrence(
        &self,
        claim: &ScheduleOccurrenceClaim,
    ) -> Result<ScheduleOccurrenceClaimResult, StoreError> {
        self.store.claim_schedule_occurrence(claim)
    }

    pub(crate) fn resolve_schedule_occurrence(
        &self,
        schedule_id: &str,
        occurrence_id: &str,
        resolution: ScheduleOccurrenceResolution,
        resolved_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<ScheduleOccurrenceRecord, StoreError> {
        self.store
            .resolve_schedule_occurrence(schedule_id, occurrence_id, resolution, resolved_at)
    }

    pub(crate) fn schedule_occurrence_history(
        &self,
        schedule_id: &str,
    ) -> Result<Vec<ScheduleOccurrenceRecord>, StoreError> {
        self.store.schedule_occurrence_history(schedule_id)
    }

    pub(crate) fn interrupt_claimed_schedule_occurrences(
        &self,
        resolved_at: chrono::DateTime<chrono::Utc>,
        reason: &str,
    ) -> Result<usize, StoreError> {
        self.store
            .interrupt_claimed_schedule_occurrences(resolved_at, reason)
    }

    pub(crate) fn put_object(
        &self,
        kind: &str,
        object_id: &str,
        body: &serde_json::Value,
        expected: Revision,
    ) -> Result<Revision, StoreError> {
        self.store.put_object(kind, object_id, body, expected)
    }

    pub(crate) fn delete_object(&self, kind: &str, object_id: &str) -> Result<bool, StoreError> {
        self.store.delete_object(kind, object_id)
    }

    pub(crate) fn mutate_objects_atomically(
        &self,
        writes: &[AtomicObjectWrite<'_>],
        deletes: &[AtomicObjectDelete<'_>],
    ) -> Result<Vec<Revision>, StoreError> {
        self.store.mutate_objects_atomically(writes, deletes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_preserves_portable_revision_and_backup_identity() {
        let directory = std::env::temp_dir().join(format!(
            "light-active-show-repository-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source = directory.join("source.show");
        let backup = directory.join("backup.show");
        let show_id = light_show::initialise_show(&source, "Repository boundary").unwrap();
        let repository = ActiveShowRepository::open(&source).unwrap();

        repository
            .put_object(
                "group",
                "1",
                &serde_json::json!({"name":"Front","fixtures":[]}),
                0,
            )
            .unwrap();
        let source_document = repository.portable_document().unwrap();
        assert_eq!(source_document.id(), show_id);
        assert!(source_document.revision().value() > 0);

        repository.backup_to(&backup).unwrap();
        let backup_document = ActiveShowRepository::open(&backup)
            .unwrap()
            .portable_document()
            .unwrap();
        assert_eq!(backup_document.id(), source_document.id());
        assert_eq!(backup_document.revision(), source_document.revision());
        assert_eq!(
            backup_document.objects().collect::<Vec<_>>(),
            source_document.objects().collect::<Vec<_>>()
        );

        std::fs::remove_dir_all(directory).unwrap();
    }
}
