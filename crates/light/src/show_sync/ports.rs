use crate::{
    ActionContext, ActionError, ActiveShowObjectChange, ActiveShowPorts, PatchChange,
    PatchFixtureCandidate, PatchFixtureProjection, PatchPerformancePhase, ShowPatchPorts,
};
use light_core::{FixtureId, Revision, ShowId};
use light_engine::EngineSnapshot;
use light_show::{
    FixtureProfileRevision, PortableShowObjectRedo, PortableShowObjectUndo, SyncAppliedRequest,
};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use uuid::Uuid;

/// Adapters a sync transaction needs beyond the patch capability's own.
pub trait ShowSyncPorts: ShowPatchPorts {
    /// The stored outcome of a request this show already applied, read inside the open unit so
    /// the lookup and the commit that would follow it are ordered by one gate.
    fn applied_sync_request(
        &self,
        unit: &Self::UnitOfWork,
        association_id: Uuid,
        request_id: &str,
    ) -> Result<Option<SyncAppliedRequest>, ActionError>;

    /// The client-facing patch document of one stored fixture, which field edits address.
    fn patch_fixture_document(
        &self,
        fixture: &PatchFixtureProjection,
    ) -> Result<Value, ActionError>;

    /// The patch candidate a complete patch document describes.
    fn patch_fixture_candidate(
        &self,
        document: Value,
    ) -> Result<PatchFixtureCandidate, ActionError>;

    /// The installed runtime compiled from the normalized active document, when the adapter can
    /// prove that identity. A transaction without patch changes then shares every projection.
    fn installed_snapshot(&self) -> Option<Arc<EngineSnapshot>> {
        None
    }
}

/// Profile resolution that consults the profile revisions a sync request carried before asking
/// the desk's own library. Used only while planning the request's patch operations.
pub(super) struct RetainedProfilePorts<'a, P> {
    pub(super) inner: &'a P,
    pub(super) retained: &'a BTreeMap<(Uuid, Revision), FixtureProfileRevision>,
}

impl<P: ShowPatchPorts> ActiveShowPorts for RetainedProfilePorts<'_, P> {
    type UnitOfWork = P::UnitOfWork;
    type PreparedRuntime = P::PreparedRuntime;

    fn authorize_mutation(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.inner.authorize_mutation(context)
    }

    fn run_active_show_lifecycle<T>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        operation: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        self.inner
            .run_active_show_lifecycle(context, show_id, operation)
    }

    fn begin_active_show(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.inner.begin_active_show(context, show_id)
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        self.inner
            .prepare_object_undo(unit, kind, object_id, expected_object_revision)
    }

    fn prepare_object_redo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: Revision,
    ) -> Result<PortableShowObjectRedo, ActionError> {
        self.inner
            .prepare_object_redo(unit, kind, object_id, expected_object_revision)
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.inner.prepare_runtime(snapshot)
    }

    fn normalized_active_snapshot(&self) -> Option<Arc<EngineSnapshot>> {
        self.inner.normalized_active_snapshot()
    }

    fn install_runtime(&self, context: &ActionContext, prepared: Self::PreparedRuntime) {
        self.inner.install_runtime(context, prepared);
    }

    fn reconcile_object_changes(&self, changes: &[ActiveShowObjectChange]) {
        self.inner.reconcile_object_changes(changes);
    }
}

impl<P: ShowPatchPorts> ShowPatchPorts for RetainedProfilePorts<'_, P> {
    fn authorize_patch_read(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.inner.authorize_patch_read(context)
    }

    fn authorize_patch(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.inner.authorize_patch(context)
    }

    fn resolve_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError> {
        match self.retained.get(&(profile_id.0, revision)) {
            Some(profile) => Ok(profile.clone()),
            None => self.inner.resolve_profile_revision(profile_id, revision),
        }
    }

    fn reconcile_patch_change(&self, change: &PatchChange) {
        self.inner.reconcile_patch_change(change);
    }

    fn record_patch_performance_phase(&self, phase: PatchPerformancePhase, elapsed: Duration) {
        self.inner.record_patch_performance_phase(phase, elapsed);
    }
}
