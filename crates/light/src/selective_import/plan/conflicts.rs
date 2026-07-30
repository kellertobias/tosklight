use super::Planner;
use crate::selective_import::{
    ImportBlocker, ImportConflict, ImportConflictResolution, ImportLoadMode, ImportObjectAction,
    ImportObjectDescriptor, SelectiveShowImportPorts,
};
use light_show::PortableShowObjectKey;
use std::collections::BTreeMap;

impl<P: SelectiveShowImportPorts> Planner<'_, P> {
    pub(super) fn action_for(
        &mut self,
        key: &PortableShowObjectKey,
        source_body: &serde_json::Value,
        positional_destination: Option<PortableShowObjectKey>,
    ) -> (ImportObjectAction, PortableShowObjectKey) {
        let resolution = self.request.conflict_resolutions.get(key).copied();
        if matches!(self.request.mode, ImportLoadMode::AddToEnd) && resolution.is_none() {
            return self.duplicate_action(key);
        }
        if key.kind() == "schedule"
            && !matches!(resolution, Some(ImportConflictResolution::KeepDestination))
        {
            return self.duplicate_action(key);
        }
        let destination = positional_destination.unwrap_or_else(|| key.clone());
        let Some(target) = self.target.object(destination.kind(), destination.id()) else {
            return self.action_without_conflict(key, resolution);
        };
        if target.body() == source_body {
            return self.action_for_identical(key, &destination, resolution);
        }
        self.conflicts.push(ImportConflict {
            key: key.clone(),
            resolution,
        });
        match resolution {
            Some(ImportConflictResolution::KeepDestination) => {
                (ImportObjectAction::KeepDestination, destination)
            }
            Some(ImportConflictResolution::ReplaceDestination) => {
                (ImportObjectAction::ReplaceDestination, destination)
            }
            Some(ImportConflictResolution::Duplicate) => self.duplicate_action(key),
            None => (ImportObjectAction::ReplaceDestination, destination),
        }
    }

    fn action_without_conflict(
        &mut self,
        key: &PortableShowObjectKey,
        resolution: Option<ImportConflictResolution>,
    ) -> (ImportObjectAction, PortableShowObjectKey) {
        match resolution {
            None => (ImportObjectAction::ImportPreservingId, key.clone()),
            Some(ImportConflictResolution::Duplicate) => self.duplicate_action(key),
            Some(resolution) => {
                self.blockers.push(ImportBlocker::InvalidResolution {
                    key: key.clone(),
                    message: format!("{resolution:?} requires an existing destination object"),
                });
                (ImportObjectAction::ImportPreservingId, key.clone())
            }
        }
    }

    fn action_for_identical(
        &mut self,
        key: &PortableShowObjectKey,
        destination: &PortableShowObjectKey,
        resolution: Option<ImportConflictResolution>,
    ) -> (ImportObjectAction, PortableShowObjectKey) {
        match resolution {
            Some(ImportConflictResolution::Duplicate) => self.duplicate_action(key),
            Some(ImportConflictResolution::ReplaceDestination) | None => {
                (ImportObjectAction::SkipIdentical, destination.clone())
            }
            Some(ImportConflictResolution::KeepDestination) => {
                (ImportObjectAction::KeepDestination, destination.clone())
            }
        }
    }

    fn duplicate_action(
        &mut self,
        key: &PortableShowObjectKey,
    ) -> (ImportObjectAction, PortableShowObjectKey) {
        match self.allocator.duplicate_key(key) {
            Ok(destination) => (
                ImportObjectAction::Duplicate {
                    destination: destination.clone(),
                },
                destination,
            ),
            Err(message) => {
                self.blockers.push(ImportBlocker::InvalidResolution {
                    key: key.clone(),
                    message,
                });
                (ImportObjectAction::BlockedConflict, key.clone())
            }
        }
    }

    pub(super) fn destination_identities(
        &mut self,
        source: &PortableShowObjectKey,
        destination: &PortableShowObjectKey,
        action: &ImportObjectAction,
        descriptor: &ImportObjectDescriptor,
    ) -> BTreeMap<String, String> {
        if matches!(action, ImportObjectAction::KeepDestination) {
            return self.destination_descriptor_identities(destination, descriptor);
        }
        if matches!(action, ImportObjectAction::ReplaceDestination) && source != destination {
            return self.replacement_destination_identities(source, destination, descriptor);
        }
        let mut identities = BTreeMap::new();
        for identity in &descriptor.identities {
            let value = if matches!(action, ImportObjectAction::Duplicate { .. }) {
                if identity.slot == "object" {
                    destination.id().to_owned()
                } else {
                    match self.allocator.nested_uuid(source, &identity.slot) {
                        Ok(value) => value,
                        Err(message) => {
                            self.blockers.push(ImportBlocker::InvalidResolution {
                                key: source.clone(),
                                message,
                            });
                            identity.value.clone()
                        }
                    }
                }
            } else {
                identity.value.clone()
            };
            identities.insert(identity.slot.clone(), value);
        }
        identities
    }

    fn destination_descriptor_identities(
        &mut self,
        destination: &PortableShowObjectKey,
        source_descriptor: &ImportObjectDescriptor,
    ) -> BTreeMap<String, String> {
        let Some(target) = self
            .target
            .object(destination.kind(), destination.id())
            .cloned()
        else {
            return BTreeMap::new();
        };
        let target_descriptor = self.describe_target(&target);
        let target_by_slot = target_descriptor
            .identities
            .into_iter()
            .map(|identity| (identity.slot, identity.value))
            .collect::<BTreeMap<_, _>>();
        let mut identities = BTreeMap::new();
        for source_identity in &source_descriptor.identities {
            match target_by_slot.get(&source_identity.slot) {
                Some(value) => {
                    identities.insert(source_identity.slot.clone(), value.clone());
                }
                None => self.blockers.push(ImportBlocker::InvalidResolution {
                    key: destination.clone(),
                    message: format!(
                        "destination has no semantic identity slot {}",
                        source_identity.slot
                    ),
                }),
            }
        }
        identities
    }

    fn replacement_destination_identities(
        &mut self,
        source: &PortableShowObjectKey,
        destination: &PortableShowObjectKey,
        source_descriptor: &ImportObjectDescriptor,
    ) -> BTreeMap<String, String> {
        let target_by_slot = self
            .target
            .object(destination.kind(), destination.id())
            .cloned()
            .map(|target| self.describe_target(&target))
            .into_iter()
            .flat_map(|descriptor| descriptor.identities)
            .map(|identity| (identity.slot, identity.value))
            .collect::<BTreeMap<_, _>>();
        source_descriptor
            .identities
            .iter()
            .map(|identity| {
                let value = target_by_slot
                    .get(&identity.slot)
                    .cloned()
                    .unwrap_or_else(|| {
                        self.allocator
                            .nested_uuid(source, &identity.slot)
                            .unwrap_or_else(|message| {
                                self.blockers.push(ImportBlocker::InvalidResolution {
                                    key: source.clone(),
                                    message,
                                });
                                identity.value.clone()
                            })
                    });
                (identity.slot.clone(), value)
            })
            .collect()
    }

    pub(super) fn validate_unused_resolutions(&mut self) {
        for key in self.request.conflict_resolutions.keys() {
            if !self.items.contains_key(key) {
                self.blockers.push(ImportBlocker::InvalidResolution {
                    key: key.clone(),
                    message: "object is outside the selected dependency closure".into(),
                });
            }
        }
        for key in self.request.profile_conflict_resolutions.keys() {
            if !self.required_profiles.contains_key(key) {
                self.blockers.push(ImportBlocker::InvalidProfileResolution {
                    key: *key,
                    message: "profile is outside the selected dependency closure".into(),
                });
            }
        }
    }
}
