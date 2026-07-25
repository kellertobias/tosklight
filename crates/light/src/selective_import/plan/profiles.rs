use super::Planner;
use crate::selective_import::{
    ImportBlocker, ImportObjectDescriptor, ImportProfileAction, ImportProfileConflictResolution,
    ImportProfileKey, ImportProfilePreview, references::ProfileMap,
};
use light_show::{FixtureProfileRevision, PortableShowObjectKey};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct RequiredProfile {
    pub(super) profile: Option<FixtureProfileRevision>,
    owners: BTreeSet<PortableShowObjectKey>,
}

impl<P: crate::selective_import::SelectiveShowImportPorts> Planner<'_, P> {
    pub(super) fn visit_profile_references(
        &mut self,
        owner: &PortableShowObjectKey,
        descriptor: &ImportObjectDescriptor,
    ) {
        for reference in &descriptor.profile_references {
            let profile = match &reference.inline_profile {
                Some(body) => FixtureProfileRevision::from_profile(body.clone())
                    .map_err(|error| format!("invalid inline profile snapshot: {error}")),
                None => self
                    .source
                    .fixture_profile_revision(reference.key.profile_id, reference.key.revision)
                    .cloned()
                    .ok_or_else(|| "profile revision is absent from the source show".into()),
            };
            let profile = match profile {
                Ok(profile) => Some(profile),
                Err(message) => {
                    self.blockers.push(ImportBlocker::MissingProfile {
                        key: reference.key,
                        required_by: owner.clone(),
                    });
                    if reference.inline_profile.is_some() {
                        self.blockers.push(ImportBlocker::InvalidDescriptor {
                            key: owner.clone(),
                            message,
                        });
                    }
                    None
                }
            };
            let required = self
                .required_profiles
                .entry(reference.key)
                .or_insert_with(|| RequiredProfile {
                    profile: profile.clone(),
                    owners: BTreeSet::new(),
                });
            if let (Some(existing), Some(candidate)) = (&required.profile, profile)
                && existing.digest() != candidate.digest()
            {
                self.blockers.push(ImportBlocker::InvalidDescriptor {
                    key: owner.clone(),
                    message: format!(
                        "profile {} revision {} has conflicting source snapshots",
                        reference.key.profile_id.0, reference.key.revision
                    ),
                });
            }
            required.owners.insert(owner.clone());
            if let Some(profile) = &required.profile {
                match self.ports.describe_import_profile_assets(profile) {
                    Ok(assets) => self.required_assets.extend(assets),
                    Err(error) => self.blockers.push(ImportBlocker::InvalidDescriptor {
                        key: owner.clone(),
                        message: error.message,
                    }),
                }
            }
        }
    }

    pub(super) fn plan_profiles(
        &mut self,
    ) -> (
        Vec<FixtureProfileRevision>,
        Vec<ImportProfilePreview>,
        ProfileMap,
    ) {
        let mut copies = Vec::new();
        let mut previews = Vec::new();
        let mut mappings = ProfileMap::new();
        let mut duplicated_families = BTreeMap::new();
        let required = self
            .required_profiles
            .iter()
            .map(|(key, value)| (*key, value.profile.clone()))
            .collect::<Vec<_>>();
        for (source_key, source) in required {
            let Some(source) = source else {
                previews.push(ImportProfilePreview {
                    source: source_key,
                    destination: source_key,
                    action: ImportProfileAction::Missing,
                });
                continue;
            };
            let (destination, action) = match self
                .target
                .fixture_profile_revision(source_key.profile_id, source_key.revision)
            {
                None => {
                    copies.push(source.clone());
                    (source_key, ImportProfileAction::Copy)
                }
                Some(target) if target.digest() == source.digest() => {
                    (source_key, ImportProfileAction::SkipIdentical)
                }
                Some(_) => match self
                    .request
                    .profile_conflict_resolutions
                    .get(&source_key)
                    .copied()
                {
                    Some(ImportProfileConflictResolution::KeepDestination) => {
                        (source_key, ImportProfileAction::KeepDestination)
                    }
                    Some(ImportProfileConflictResolution::Duplicate) => {
                        let destination_id = match duplicated_families.get(&source_key.profile_id.0)
                        {
                            Some(id) => *id,
                            None => match self.allocator.profile_id(source_key.profile_id) {
                                Ok(id) => {
                                    duplicated_families.insert(source_key.profile_id.0, id);
                                    id
                                }
                                Err(message) => {
                                    self.blockers.push(ImportBlocker::InvalidProfileResolution {
                                        key: source_key,
                                        message,
                                    });
                                    source_key.profile_id
                                }
                            },
                        };
                        let destination = ImportProfileKey {
                            profile_id: destination_id,
                            revision: source_key.revision,
                        };
                        match duplicate_profile(&source, destination) {
                            Ok(profile) => copies.push(profile),
                            Err(message) => {
                                self.blockers.push(ImportBlocker::InvalidProfileResolution {
                                    key: source_key,
                                    message,
                                })
                            }
                        }
                        (destination, ImportProfileAction::Duplicate { destination })
                    }
                    None => {
                        self.blockers
                            .push(ImportBlocker::ProfileConflict { key: source_key });
                        (source_key, ImportProfileAction::BlockedConflict)
                    }
                },
            };
            mappings.insert(source_key, destination);
            previews.push(ImportProfilePreview {
                source: source_key,
                destination,
                action,
            });
        }
        (copies, previews, mappings)
    }
}

fn duplicate_profile(
    source: &FixtureProfileRevision,
    destination: ImportProfileKey,
) -> Result<FixtureProfileRevision, String> {
    let mut body = source.profile().clone();
    let object = body
        .as_object_mut()
        .ok_or_else(|| "fixture profile is not an object".to_owned())?;
    object.insert(
        "id".into(),
        serde_json::Value::String(destination.profile_id.0.to_string()),
    );
    FixtureProfileRevision::new(destination.profile_id, destination.revision, body)
        .map_err(|error| error.to_string())
}
