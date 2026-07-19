//! Server-owned storage and runtime ports for selective show import.

use super::{
    AppState, ServerActiveShowPorts, ServerActiveShowUnitOfWork, reconcile_group_projections,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowPorts,
    ActiveShowUnitOfWork, AssetReference, ImportManagedAssetAction, SelectiveShowImportChange,
    SelectiveShowImportPorts,
};
use light_core::ShowId;
use light_engine::{EngineSnapshot, PreparedEngineSnapshot};
use light_show::{PortableShowDocument, PortableShowObjectUndo, ShowStore, StoreError};
use parking_lot::Mutex;
use std::sync::Arc;

pub(super) struct ImportSourceSnapshot {
    pub(super) document: PortableShowDocument,
}

#[derive(Clone)]
pub(super) struct ServerSelectiveImportPorts {
    state: AppState,
    active: ServerActiveShowPorts,
    previous_routes: Arc<Mutex<Vec<light_output::OutputRoute>>>,
}

impl ServerSelectiveImportPorts {
    pub(super) fn new(state: AppState) -> Self {
        Self {
            active: ServerActiveShowPorts::show_objects(state.clone()),
            state,
            previous_routes: Arc::default(),
        }
    }

    pub(super) fn source_catalog(
        &self,
        context: &ActionContext,
        source_show_id: ShowId,
    ) -> Result<ImportSourceSnapshot, ActionError> {
        self.open_import_source_snapshot(context, source_show_id)
    }
}

impl ActiveShowPorts for ServerSelectiveImportPorts {
    type UnitOfWork = ServerActiveShowUnitOfWork;
    type PreparedRuntime = PreparedEngineSnapshot;

    fn run_active_show_lifecycle<T>(
        &self,
        _context: &ActionContext,
        _show_id: ShowId,
        operation: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        let _activation = self.state.activation_lock.clone().blocking_lock_owned();
        operation()
    }

    fn begin_active_show(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        let unit = self.active.begin_active_show(context, show_id)?;
        *self.previous_routes.lock() = unit
            .document()
            .objects_of_kind("route")
            .filter_map(|object| serde_json::from_value(object.body().clone()).ok())
            .collect();
        Ok(unit)
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: light_core::Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        self.active
            .prepare_object_undo(unit, kind, object_id, expected_object_revision)
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.active.prepare_runtime(snapshot)
    }

    fn install_runtime(&self, prepared: Self::PreparedRuntime) {
        self.active.install_runtime(prepared);
    }

    fn reconcile_object_changes(&self, changes: &[ActiveShowObjectChange]) {
        self.active.reconcile_object_changes(changes);
    }
}

impl SelectiveShowImportPorts for ServerSelectiveImportPorts {
    type ImportSourceSnapshot = ImportSourceSnapshot;
    type PreparedImportAssets = Vec<AssetReference>;

    fn open_import_source_snapshot(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::ImportSourceSnapshot, ActionError> {
        let entry = self
            .state
            .desk
            .lock()
            .show(show_id)
            .map_err(store_error)?
            .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "source show not found"))?;
        let document = ShowStore::open(&entry.path)
            .and_then(|store| store.portable_document())
            .map_err(store_error)?;
        Ok(ImportSourceSnapshot { document })
    }

    fn import_source_document<'a>(
        &self,
        source: &'a Self::ImportSourceSnapshot,
    ) -> &'a PortableShowDocument {
        &source.document
    }

    fn inspect_import_asset(
        &self,
        _source: &Self::ImportSourceSnapshot,
        _target_show_id: ShowId,
        _asset: AssetReference,
    ) -> Result<ImportManagedAssetAction, ActionError> {
        // The current show archive has no external managed-asset store. A future capability may
        // supply one; reporting Missing keeps preview honest and prevents partial imports.
        Ok(ImportManagedAssetAction::Missing)
    }

    fn prepare_import_assets(
        &self,
        _context: &ActionContext,
        _source: &Self::ImportSourceSnapshot,
        _target_show_id: ShowId,
        assets: &[AssetReference],
    ) -> Result<Self::PreparedImportAssets, ActionError> {
        if assets.is_empty() {
            Ok(Vec::new())
        } else {
            Err(ActionError::new(
                ActionErrorKind::Unavailable,
                "managed-asset storage is not configured",
            ))
        }
    }

    fn prepared_import_assets<'a>(
        &self,
        prepared: &'a Self::PreparedImportAssets,
    ) -> &'a [AssetReference] {
        prepared
    }

    fn compensate_import_assets(
        &self,
        _prepared: Self::PreparedImportAssets,
    ) -> Result<(), ActionError> {
        Ok(())
    }

    fn publish_import_assets(&self, prepared: Self::PreparedImportAssets) {
        debug_assert!(prepared.is_empty());
    }

    fn reconcile_selective_import(&self, change: &SelectiveShowImportChange) {
        if change
            .objects
            .iter()
            .any(|object| object.key.kind() == "group")
        {
            reconcile_group_projections(&self.state);
        }
        self.reconcile_fixture_media(change);
        self.terminate_replaced_routes(change);
    }
}

impl ServerSelectiveImportPorts {
    fn reconcile_fixture_media(&self, change: &SelectiveShowImportChange) {
        for object in &change.objects {
            if object.key.kind() != "patched_fixture" {
                continue;
            }
            if let Ok(fixture) =
                serde_json::from_value::<light_fixture::PatchedFixture>(object.body.clone())
            {
                self.state
                    .media_cache
                    .lock()
                    .clear_fixture(&fixture.fixture_id.0.to_string());
                self.state.media_status.write().remove(&fixture.fixture_id);
            }
        }
    }

    fn terminate_replaced_routes(&self, change: &SelectiveShowImportChange) {
        if !change
            .objects
            .iter()
            .any(|object| object.key.kind() == "route")
        {
            return;
        }
        let (Some(output), Ok(runtime)) = (
            self.state.network_output.clone(),
            tokio::runtime::Handle::try_current(),
        ) else {
            return;
        };
        let routes = self.previous_routes.lock().clone();
        let sequences = Arc::clone(&self.state.output_sequences);
        runtime.spawn(async move {
            let _ = output
                .terminate_routes(&routes, &mut *sequences.lock().await)
                .await;
        });
    }
}

fn store_error(error: StoreError) -> ActionError {
    let kind = match error {
        StoreError::Sql(_) => ActionErrorKind::Unavailable,
        StoreError::RevisionConflict { .. }
        | StoreError::DocumentRevisionConflict { .. }
        | StoreError::FixtureProfileRevisionConflict { .. } => ActionErrorKind::Conflict,
        StoreError::Uuid(_) | StoreError::Json(_) | StoreError::Invalid(_) => {
            ActionErrorKind::Invalid
        }
    };
    ActionError::new(kind, error.to_string())
}
