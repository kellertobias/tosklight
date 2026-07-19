use super::{PlannedWrite, Planner};
use crate::{
    AssetReference,
    selective_import::{
        AppliedImportObject, ImportBlocker, ImportDependency, ImportManagedAssetAction,
        ImportManagedAssetPreview, ImportObjectAction, ImportProfilePreview,
        SelectiveShowImportPorts, SelectiveShowImportPreview,
        references::{IdentityMap, ProfileMap, rewrite_body},
    },
};
use light_show::PortableShowObjectKey;

impl<P: SelectiveShowImportPorts> Planner<'_, P> {
    pub(super) fn identities(&self) -> IdentityMap {
        self.items
            .iter()
            .flat_map(|(source, item)| {
                item.destination_identities
                    .iter()
                    .map(move |(slot, value)| ((source.clone(), slot.clone()), value.clone()))
            })
            .chain(
                self.bound_identities
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            )
            .collect()
    }

    pub(super) fn rewrite_writes(
        &mut self,
        identities: &IdentityMap,
        profiles: &ProfileMap,
    ) -> Vec<PlannedWrite> {
        let mut writes = Vec::new();
        for (source, item) in &self.items {
            if !matches!(
                item.action,
                ImportObjectAction::ImportPreservingId
                    | ImportObjectAction::ReplaceDestination
                    | ImportObjectAction::Duplicate { .. }
            ) {
                continue;
            }
            match rewrite_body(&item.body, source, &item.descriptor, identities, profiles) {
                Ok(body) => writes.push(PlannedWrite {
                    destination: item.destination.clone(),
                    body,
                }),
                Err(message) => self.blockers.push(ImportBlocker::ReferenceRewrite {
                    owner: source.clone(),
                    message,
                }),
            }
        }
        writes
    }

    pub(super) fn preview(
        self,
        profiles: Vec<ImportProfilePreview>,
        managed_assets: Vec<ImportManagedAssetPreview>,
    ) -> SelectiveShowImportPreview {
        SelectiveShowImportPreview {
            request: self.request.clone(),
            source_revision: self.source.revision(),
            target_revision: self.target.revision(),
            objects: self
                .items
                .into_iter()
                .map(|(source, item)| AppliedImportObject {
                    source,
                    destination: item.destination,
                    action: item.action,
                })
                .collect(),
            dependencies: self
                .dependencies
                .into_iter()
                .map(ImportDependency::from)
                .collect(),
            conflicts: self.conflicts,
            profiles,
            managed_assets,
            blockers: self.blockers,
        }
    }

    pub(super) fn plan_assets(&mut self) -> (Vec<ImportManagedAssetPreview>, Vec<AssetReference>) {
        let mut previews = Vec::new();
        let mut copies = Vec::new();
        for asset in self.required_assets.iter().copied() {
            let action = match self.ports.inspect_import_asset(
                self.source_snapshot,
                self.request.target_show_id,
                asset,
            ) {
                Ok(action) => action,
                Err(error) => {
                    self.blockers.push(ImportBlocker::InvalidDescriptor {
                        key: PortableShowObjectKey::new("managed_asset", asset.id.0.to_string()),
                        message: error.message,
                    });
                    ImportManagedAssetAction::Missing
                }
            };
            match action {
                ImportManagedAssetAction::Copy => copies.push(asset),
                ImportManagedAssetAction::Missing => {
                    self.blockers
                        .push(ImportBlocker::MissingManagedAsset { asset });
                }
                ImportManagedAssetAction::BlockedConflict => {
                    self.blockers
                        .push(ImportBlocker::ManagedAssetConflict { asset });
                }
                ImportManagedAssetAction::SkipIdentical => {}
            }
            previews.push(ImportManagedAssetPreview { asset, action });
        }
        (previews, copies)
    }
}
