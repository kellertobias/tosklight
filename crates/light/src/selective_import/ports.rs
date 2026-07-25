use super::{ImportManagedAssetAction, ImportObjectDescriptor, SelectiveShowImportChange};
use crate::{ActionContext, ActionError, ActiveShowPorts, AssetReference};
use light_core::ShowId;
use light_show::{FixtureProfileRevision, PortableShowDocument, PortableShowObject};

/// Active-show adapters plus typed source-show and managed-asset capabilities needed by import.
pub trait SelectiveShowImportPorts: ActiveShowPorts {
    /// Immutable source revision plus access to the exact managed assets named by that revision.
    /// Adapters backed by mutable stores must retain a snapshot/archive handle rather than merely
    /// remembering the current show id.
    type ImportSourceSnapshot;
    type PreparedImportAssets;

    fn open_import_source_snapshot(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::ImportSourceSnapshot, ActionError>;

    fn import_source_document<'a>(
        &self,
        source: &'a Self::ImportSourceSnapshot,
    ) -> &'a PortableShowDocument;

    /// Describes a capability-owned object kind that is not in the core descriptor registry.
    /// Planning also asks about unselected custom objects so their semantic identities can be
    /// reserved; implementations must therefore be deterministic, side-effect free, and cheap.
    fn describe_import_object(
        &self,
        object: &PortableShowObject,
    ) -> Result<Option<ImportObjectDescriptor>, ActionError> {
        let _ = object;
        Ok(None)
    }

    /// Returns only explicitly versioned assets owned by this profile capability. Inline data
    /// URLs need no managed-asset entry and therefore use the default empty result.
    fn describe_import_profile_assets(
        &self,
        profile: &FixtureProfileRevision,
    ) -> Result<Vec<AssetReference>, ActionError> {
        let _ = profile;
        Ok(Vec::new())
    }

    fn inspect_import_asset(
        &self,
        source: &Self::ImportSourceSnapshot,
        target_show_id: ShowId,
        asset: AssetReference,
    ) -> Result<ImportManagedAssetAction, ActionError>;

    /// Prepares an operation-scoped, reversible batch without making any target-show namespace
    /// links visible. The adapter must clean up any partial work before returning `Err`.
    fn prepare_import_assets(
        &self,
        context: &ActionContext,
        source: &Self::ImportSourceSnapshot,
        target_show_id: ShowId,
        assets: &[AssetReference],
    ) -> Result<Self::PreparedImportAssets, ActionError>;

    /// Reports the exact immutable asset revisions held by the invisible prepared batch. The
    /// service validates this receipt before it starts the active-show transaction.
    fn prepared_import_assets<'a>(
        &self,
        prepared: &'a Self::PreparedImportAssets,
    ) -> &'a [AssetReference];

    /// Removes an invisible prepared batch after the show transaction fails.
    fn compensate_import_assets(
        &self,
        prepared: Self::PreparedImportAssets,
    ) -> Result<(), ActionError>;

    /// Publishes the exact prepared namespace links after the show commit. This operation must be
    /// infallible: adapters should use an atomic visibility switch or persist a durable publication
    /// receipt and record any later cleanup debt internally.
    fn publish_import_assets(&self, prepared: Self::PreparedImportAssets);

    fn reconcile_selective_import(&self, change: &SelectiveShowImportChange);
}
