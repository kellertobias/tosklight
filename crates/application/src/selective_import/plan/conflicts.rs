use super::Planner;
use crate::selective_import::{
    ImportBlocker, ImportConflict, ImportConflictResolution, ImportObjectAction,
    ImportObjectDescriptor, SelectiveShowImportPorts,
};
use light_show::PortableShowObjectKey;
use std::collections::BTreeMap;

impl<P: SelectiveShowImportPorts> Planner<'_, P> {
    pub(super) fn action_for(
        &mut self,
        key: &PortableShowObjectKey,
        source_body: &serde_json::Value,
    ) -> (ImportObjectAction, PortableShowObjectKey) {
        let resolution = self.request.conflict_resolutions.get(key).copied();
        let Some(target) = self.target.object(key.kind(), key.id()) else {
            return self.action_without_conflict(key, resolution);
        };
        if target.body() == source_body {
            return self.action_for_identical(key, resolution);
        }
        self.conflicts.push(ImportConflict {
            key: key.clone(),
            resolution,
        });
        match resolution {
            Some(ImportConflictResolution::KeepDestination) => {
                (ImportObjectAction::KeepDestination, key.clone())
            }
            Some(ImportConflictResolution::ReplaceDestination) => {
                (ImportObjectAction::ReplaceDestination, key.clone())
            }
            Some(ImportConflictResolution::Duplicate) => self.duplicate_action(key),
            None => {
                self.blockers
                    .push(ImportBlocker::ObjectConflict { key: key.clone() });
                (ImportObjectAction::BlockedConflict, key.clone())
            }
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
        resolution: Option<ImportConflictResolution>,
    ) -> (ImportObjectAction, PortableShowObjectKey) {
        match resolution {
            Some(ImportConflictResolution::Duplicate) => self.duplicate_action(key),
            Some(ImportConflictResolution::ReplaceDestination) | None => {
                (ImportObjectAction::SkipIdentical, key.clone())
            }
            Some(ImportConflictResolution::KeepDestination) => {
                (ImportObjectAction::KeepDestination, key.clone())
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
