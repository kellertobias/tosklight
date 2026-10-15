use super::{PlannedWrite, Planner};
use crate::{
    AssetReference,
    selective_import::{
        AppliedImportObject, ImportBlocker, ImportDependency, ImportManagedAssetAction,
        ImportManagedAssetPreview, ImportObjectAction, ImportProfilePreview,
        SelectiveShowImportPorts, SelectiveShowImportPreview,
        references::{IdentityMap, PinnedProfileMap, PresetOwnerRewrite, ProfileMap, rewrite_body},
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
        copies: &[light_show::FixtureProfileRevision],
    ) -> Vec<PlannedWrite> {
        let pinned_profiles: PinnedProfileMap = profiles
            .iter()
            .filter_map(|(source, destination)| {
                let original = self.required_profiles.get(source)?.profile.clone()?;
                let copied = copies
                    .iter()
                    .find(|copy| {
                        copy.id().profile_id() == destination.profile_id
                            && copy.id().revision() == destination.revision
                    })
                    .or_else(|| {
                        self.target
                            .fixture_profile_revision(destination.profile_id, destination.revision)
                    })?
                    .clone();
                Some((*source, (original, copied)))
            })
            .collect();
        let written_presets = self
            .items
            .iter()
            .filter(|(source, item)| {
                source.kind() == "preset"
                    && matches!(
                        item.action,
                        ImportObjectAction::ImportPreservingId
                            | ImportObjectAction::ReplaceDestination
                            | ImportObjectAction::Duplicate { .. }
                    )
            })
            .map(|(source, _)| source.clone())
            .collect();
        let owners = PresetOwnerRewrite {
            identities,
            written_presets: &written_presets,
            source_fixtures: &self.source_fixtures,
            target_fixtures: &self.target_fixtures,
        };
        let mut writes = Vec::new();
        for (source, item) in &self.items {
            if !matches!(
                item.action,
                ImportObjectAction::ImportPreservingId
                    | ImportObjectAction::ReplaceDestination
                    | ImportObjectAction::MergeScoped
                    | ImportObjectAction::Duplicate { .. }
            ) {
                continue;
            }
            match rewrite_body(
                &item.body,
                source,
                &item.descriptor,
                identities,
                profiles,
                &pinned_profiles,
            )
            .and_then(|mut body| {
                owners.rewrite(&item.body, &mut body, source, &item.descriptor)?;
                Ok(body)
            }) {
                Ok(mut body) => {
                    if source.kind() == "preset"
                        && matches!(item.action, ImportObjectAction::ReplaceDestination)
                    {
                        preserve_replaced_preset_instance(
                            &mut body,
                            self.target
                                .object(item.destination.kind(), item.destination.id()),
                        );
                    }
                    if source.kind() == "schedule" {
                        reset_schedule_anchor(&mut body);
                    }
                    writes.push(PlannedWrite {
                        destination: item.destination.clone(),
                        body,
                    })
                }
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

fn reset_schedule_anchor(body: &mut serde_json::Value) {
    let Some(trigger) = body.get_mut("trigger") else {
        return;
    };
    match trigger.get("type").and_then(serde_json::Value::as_str) {
        Some("interval") => {
            trigger["enabled_at"] = serde_json::Value::String(chrono::Utc::now().to_rfc3339());
        }
        Some("calendar")
            if trigger
                .pointer("/rule/type")
                .and_then(serde_json::Value::as_str)
                == Some("every_n_days") =>
        {
            trigger["rule"]["anchor"] = serde_json::Value::String(
                chrono::Local::now()
                    .date_naive()
                    .format("%Y-%m-%d")
                    .to_string(),
            );
        }
        _ => {}
    }
}

/// Replacement edits the destination instance; a legacy source has no identity slot
/// to rewrite, so retain the existing destination identity explicitly before migration.
/// New/copied legacy Presets still use the ordinary one-time identity migration.
fn preserve_replaced_preset_instance(
    body: &mut serde_json::Value,
    destination: Option<&light_show::PortableShowObject>,
) {
    if let Some(instance) = destination
        .and_then(|object| object.body().get("instance_id"))
        .filter(|value| !value.is_null())
        && let Some(body) = body.as_object_mut()
    {
        body.insert("instance_id".into(), instance.clone());
    }
}
