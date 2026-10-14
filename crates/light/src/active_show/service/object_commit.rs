//! Portable object commit with unchanged output retained for desk-only window layouts.
use super::*;

impl ActiveShowService {
    pub(super) fn commit_object_changes<P: ActiveShowPorts>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        mut unit: P::UnitOfWork,
        ports: &P,
        prepared: PreparedObjectChanges,
        operation: &str,
    ) -> Result<CommittedObjectChanges, ActionError> {
        // A desk window layout has no output projection. Reinstalling an identical lighting
        // generation would invalidate the last accepted native frame until the next output tick.
        // Inspect the complete prepared transaction, including compatibility migrations; any
        // non-layout rider still takes the ordinary validated runtime installation boundary.
        let retain_output = ports.normalized_active_snapshot().is_some()
            && !prepared.transaction.patch_changed()
            && !prepared.transaction.fixture_profile_revisions_changed()
            && prepared.transaction.metadata_changes().is_empty()
            && prepared
                .transaction
                .changed_object_kinds()
                .all(|kind| kind == "user_layout");
        let runtime = if retain_output {
            None
        } else {
            Some(ports.prepare_runtime(prepared.snapshot)?)
        };
        let mut persist = || {
            unit.backup(&backup_identity(context, show_id, operation))?;
            unit.commit(prepared.transaction)
        };
        let commit = match runtime {
            Some(runtime) => ports.finalize_runtime(context, runtime, persist)?,
            None => persist()?,
        };
        let show_revision = commit.revision();
        let migration_changes = migration_changes(&commit, &prepared.changes);
        let migrated_routes = migrated_route_changes(show_id, &commit, None);
        ports.reconcile_object_changes(&prepared.changes);
        let committed = self.publish_object_changes(
            context,
            show_id,
            show_revision,
            prepared.changes,
            migration_changes,
        );
        for migrated in &migrated_routes {
            self.events
                .publish(EventDraft::output_route_changed(context, migrated.clone()));
        }
        Ok(CommittedObjectChanges {
            migrated_routes,
            ..committed
        })
    }
}
