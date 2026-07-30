use super::model::PlannedWrite;
use super::{
    ImportBlocker, ImportConflict, ImportObjectAction, ImportObjectDescriptor,
    SelectiveShowImportPorts, SelectiveShowImportPreview, SelectiveShowImportRequest,
    identity::IdentityAllocator,
    references::{
        FixtureIdentityCatalog, IdentityMap, ProfileMap, is_registered_object_kind,
        registered_descriptor,
    },
};
use crate::AssetReference;
use light_show::{
    FixtureProfileRevision, PortableShowDocument, PortableShowObject, PortableShowObjectKey,
};
use std::collections::{BTreeMap, BTreeSet};

mod conflicts;
mod dependencies;
mod output;
mod profiles;
use dependencies::DependencyKey;
use profiles::RequiredProfile;

pub(super) struct ImportPlan {
    pub preview: SelectiveShowImportPreview,
    pub writes: Vec<PlannedWrite>,
    pub profiles: Vec<FixtureProfileRevision>,
    pub profile_map: ProfileMap,
    pub asset_copies: Vec<AssetReference>,
}

pub(super) fn build_plan<P: SelectiveShowImportPorts>(
    request: &SelectiveShowImportRequest,
    source_snapshot: &P::ImportSourceSnapshot,
    source: &PortableShowDocument,
    target: &PortableShowDocument,
    ports: &P,
) -> ImportPlan {
    Planner::new(request, source_snapshot, source, target, ports).build()
}

struct PlannedItem {
    body: serde_json::Value,
    descriptor: ImportObjectDescriptor,
    destination: PortableShowObjectKey,
    destination_identities: BTreeMap<String, String>,
    action: ImportObjectAction,
}

pub(super) struct Planner<'a, P: SelectiveShowImportPorts> {
    request: &'a SelectiveShowImportRequest,
    source_snapshot: &'a P::ImportSourceSnapshot,
    source: &'a PortableShowDocument,
    target: &'a PortableShowDocument,
    ports: &'a P,
    source_fixtures: FixtureIdentityCatalog,
    target_fixtures: FixtureIdentityCatalog,
    source_custom_descriptors: CustomDescriptorCatalog,
    target_custom_descriptors: CustomDescriptorCatalog,
    allocator: IdentityAllocator,
    next_fixture_number: Option<u64>,
    pending: BTreeSet<PortableShowObjectKey>,
    scoped_stage_layouts: BTreeSet<PortableShowObjectKey>,
    scoped_stage_identities: BTreeSet<String>,
    items: BTreeMap<PortableShowObjectKey, PlannedItem>,
    dependencies: BTreeSet<DependencyKey>,
    bound_identities: IdentityMap,
    conflicts: Vec<ImportConflict>,
    blockers: Vec<ImportBlocker>,
    required_profiles: BTreeMap<super::ImportProfileKey, RequiredProfile>,
    required_assets: BTreeSet<AssetReference>,
}

impl<'a, P: SelectiveShowImportPorts> Planner<'a, P> {
    fn new(
        request: &'a SelectiveShowImportRequest,
        source_snapshot: &'a P::ImportSourceSnapshot,
        source: &'a PortableShowDocument,
        target: &'a PortableShowDocument,
        ports: &'a P,
    ) -> Self {
        let source_fixtures = FixtureIdentityCatalog::from_document(source);
        let target_fixtures = FixtureIdentityCatalog::from_document(target);
        let reserves_generated_identities = request.mode == super::ImportLoadMode::AddToEnd
            || request
                .conflict_resolutions
                .values()
                .any(|resolution| matches!(resolution, super::ImportConflictResolution::Duplicate))
            || request
                .profile_conflict_resolutions
                .values()
                .any(|resolution| {
                    matches!(
                        resolution,
                        super::ImportProfileConflictResolution::Duplicate
                    )
                });
        let (source_custom_descriptors, target_custom_descriptors) =
            if reserves_generated_identities {
                (
                    CustomDescriptorCatalog::from_document(source, ports),
                    CustomDescriptorCatalog::from_document(target, ports),
                )
            } else {
                (
                    CustomDescriptorCatalog::default(),
                    CustomDescriptorCatalog::default(),
                )
            };
        let mut keys = target
            .objects()
            .map(|object| object.key().clone())
            .collect::<Vec<_>>();
        let mut identity_values = target_fixtures
            .values()
            .chain(target_custom_descriptors.identity_values())
            .chain(
                target
                    .fixture_profile_revisions()
                    .iter()
                    .map(|profile| profile.id().profile_id().0.to_string()),
            )
            .collect::<Vec<_>>();
        if request.mode == super::ImportLoadMode::ReplaceByPosition {
            keys.extend(source.objects().map(|object| object.key().clone()));
            identity_values.extend(source_fixtures.values());
            identity_values.extend(source_custom_descriptors.identity_values());
            identity_values.extend(
                source
                    .fixture_profile_revisions()
                    .iter()
                    .map(|profile| profile.id().profile_id().0.to_string()),
            );
        }
        let mut pending = request.selected_objects.clone();
        let selected_layers = request
            .selected_objects
            .iter()
            .filter(|key| key.kind() == "patch_layer")
            .map(|key| key.id().to_owned())
            .collect::<BTreeSet<_>>();
        if !selected_layers.is_empty() {
            pending.extend(
                source
                    .objects()
                    .filter(|object| matches!(object.key().kind(), "fixture" | "patched_fixture"))
                    .filter(|object| {
                        object
                            .body()
                            .get("layer_id")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|layer| selected_layers.contains(layer))
                    })
                    .map(|object| object.key().clone()),
            );
        }
        let scoped_stage_identities = pending
            .iter()
            .filter_map(|key| source.object(key.kind(), key.id()))
            .filter(|object| matches!(object.key().kind(), "fixture" | "patched_fixture"))
            .filter_map(|object| {
                registered_descriptor(object, &source_fixtures, &target_fixtures)
                    .ok()
                    .flatten()
            })
            .flat_map(|descriptor| {
                descriptor
                    .identities
                    .into_iter()
                    .map(|identity| identity.value)
            })
            .collect::<BTreeSet<_>>();
        let scoped_stage_layouts = if selected_layers.is_empty() {
            BTreeSet::new()
        } else {
            source
                .objects()
                .filter(|object| object.key().kind() == "stage_layout")
                .map(|object| object.key().clone())
                .filter(|key| !request.selected_objects.contains(key))
                .collect::<BTreeSet<_>>()
        };
        pending.extend(scoped_stage_layouts.iter().cloned());
        let next_fixture_number = target
            .objects()
            .filter(|object| matches!(object.key().kind(), "fixture" | "patched_fixture"))
            .filter_map(|object| object.body().get("fixture_number"))
            .filter_map(serde_json::Value::as_u64)
            .max()
            .map_or(Some(1), |number| number.checked_add(1));
        Self {
            request,
            source_snapshot,
            source,
            target,
            ports,
            source_fixtures,
            target_fixtures,
            source_custom_descriptors,
            target_custom_descriptors,
            allocator: IdentityAllocator::new(
                request.source_show_id,
                request.target_show_id,
                keys,
                identity_values,
            ),
            next_fixture_number,
            pending,
            scoped_stage_layouts,
            scoped_stage_identities,
            items: BTreeMap::new(),
            dependencies: BTreeSet::new(),
            bound_identities: BTreeMap::new(),
            conflicts: Vec::new(),
            blockers: Vec::new(),
            required_profiles: BTreeMap::new(),
            required_assets: BTreeSet::new(),
        }
    }

    fn build(mut self) -> ImportPlan {
        self.validate_request();
        while let Some(key) = self.pending.pop_first() {
            self.visit(key);
        }
        self.validate_unused_resolutions();
        let (profiles, profile_previews, profile_map) = self.plan_profiles();
        let (managed_assets, asset_copies) = self.plan_assets();
        let identities = self.identities();
        let writes = self.rewrite_writes(&identities, &profile_map);
        let writes = self.merge_scoped_stage_writes(writes);
        let preview = self.preview(profile_previews, managed_assets);
        ImportPlan {
            preview,
            writes,
            profiles,
            profile_map,
            asset_copies,
        }
    }

    fn validate_request(&mut self) {
        if self.request.selected_objects.is_empty() {
            self.blockers.push(ImportBlocker::EmptySelection);
        }
        if self.request.source_show_id == self.request.target_show_id {
            self.blockers.push(ImportBlocker::SameShow);
        }
        for key in &self.request.selected_objects {
            if self.source.object(key.kind(), key.id()).is_none() {
                self.blockers.push(ImportBlocker::MissingObject {
                    key: key.clone(),
                    required_by: None,
                });
            }
        }
    }

    fn visit(&mut self, key: PortableShowObjectKey) {
        if self.items.contains_key(&key) {
            return;
        }
        let Some(source) = self.source.object(key.kind(), key.id()).cloned() else {
            return;
        };
        let scoped_stage = self.scoped_stage_layouts.contains(&key);
        let mut descriptor = self.describe_source(&source);
        let mut body = if scoped_stage {
            descriptor.references.retain(|reference| {
                self.scoped_stage_identities
                    .contains(&reference.source_identity)
            });
            filtered_stage_layout(source.body(), &self.scoped_stage_identities)
        } else {
            source.body().clone()
        };
        let positional_destination = (!scoped_stage
            && self.request.mode == super::ImportLoadMode::ReplaceByPosition)
            .then(|| self.positional_destination(&key, &body, &descriptor))
            .flatten();
        let (action, destination) = if scoped_stage {
            (ImportObjectAction::MergeScoped, key.clone())
        } else {
            self.action_for(&key, &body, positional_destination)
        };
        if matches!(action, ImportObjectAction::Duplicate { .. })
            && matches!(key.kind(), "fixture" | "patched_fixture")
        {
            self.assign_appended_fixture_number(&key, &mut body);
        }
        let destination_identities =
            self.destination_identities(&key, &destination, &action, &descriptor);
        let traverses = !matches!(action, ImportObjectAction::KeepDestination);
        self.items.insert(
            key.clone(),
            PlannedItem {
                body,
                descriptor: descriptor.clone(),
                destination,
                destination_identities,
                action,
            },
        );
        if traverses {
            self.visit_references(&key, &descriptor);
            self.visit_profile_references(&key, &descriptor);
            self.required_assets
                .extend(descriptor.managed_assets.iter().copied());
        }
    }

    fn merge_scoped_stage_writes(&self, mut writes: Vec<PlannedWrite>) -> Vec<PlannedWrite> {
        for write in &mut writes {
            if !self.scoped_stage_layouts.contains(&write.destination) {
                continue;
            }
            let Some(target) = self
                .target
                .object(write.destination.kind(), write.destination.id())
            else {
                continue;
            };
            write.body = merge_stage_layout(target.body(), &write.body);
        }
        writes
    }

    fn describe_source(&mut self, object: &PortableShowObject) -> ImportObjectDescriptor {
        let custom = if is_registered_object_kind(object.key().kind()) {
            Ok(None)
        } else {
            self.custom_descriptor(object, DocumentSide::Source)
        };
        self.describe(object, custom)
    }

    fn describe_target(&mut self, object: &PortableShowObject) -> ImportObjectDescriptor {
        let custom = if is_registered_object_kind(object.key().kind()) {
            Ok(None)
        } else {
            self.custom_descriptor(object, DocumentSide::Target)
        };
        self.describe(object, custom)
    }

    fn custom_descriptor(
        &mut self,
        object: &PortableShowObject,
        side: DocumentSide,
    ) -> Result<Option<ImportObjectDescriptor>, String> {
        let cached = match side {
            DocumentSide::Source => self.source_custom_descriptors.descriptor(object.key()),
            DocumentSide::Target => self.target_custom_descriptors.descriptor(object.key()),
        };
        if let Some(cached) = cached {
            return cached;
        }
        let descriptor = self
            .ports
            .describe_import_object(object)
            .map_err(|error| error.message);
        match side {
            DocumentSide::Source => self
                .source_custom_descriptors
                .insert(object.key().clone(), descriptor.clone()),
            DocumentSide::Target => self
                .target_custom_descriptors
                .insert(object.key().clone(), descriptor.clone()),
        }
        descriptor
    }

    fn describe(
        &mut self,
        object: &PortableShowObject,
        custom: Result<Option<ImportObjectDescriptor>, String>,
    ) -> ImportObjectDescriptor {
        match registered_descriptor(object, &self.source_fixtures, &self.target_fixtures) {
            Ok(Some(descriptor)) => descriptor,
            Ok(None) => match custom {
                Ok(Some(descriptor)) => descriptor,
                Ok(None) => {
                    self.blockers.push(ImportBlocker::UnsupportedObject {
                        key: object.key().clone(),
                    });
                    key_only_descriptor(object)
                }
                Err(message) => {
                    self.blockers.push(ImportBlocker::InvalidDescriptor {
                        key: object.key().clone(),
                        message,
                    });
                    key_only_descriptor(object)
                }
            },
            Err(message) => {
                self.blockers.push(ImportBlocker::InvalidDescriptor {
                    key: object.key().clone(),
                    message,
                });
                key_only_descriptor(object)
            }
        }
    }

    fn assign_appended_fixture_number(
        &mut self,
        key: &PortableShowObjectKey,
        body: &mut serde_json::Value,
    ) {
        let Some(number) = self.next_fixture_number else {
            self.blockers.push(ImportBlocker::InvalidResolution {
                key: key.clone(),
                message: "fixture number space is exhausted".into(),
            });
            return;
        };
        let Some(object) = body.as_object_mut() else {
            self.blockers.push(ImportBlocker::InvalidDescriptor {
                key: key.clone(),
                message: "fixture body is not an object".into(),
            });
            return;
        };
        object.insert("fixture_number".into(), serde_json::Value::from(number));
        self.next_fixture_number = number.checked_add(1);
    }

    fn positional_destination(
        &mut self,
        source: &PortableShowObjectKey,
        body: &serde_json::Value,
        descriptor: &ImportObjectDescriptor,
    ) -> Option<PortableShowObjectKey> {
        let position = positional_identity(source.kind(), body, descriptor)?;
        let candidates = self
            .target
            .objects_of_kind(source.kind())
            .cloned()
            .collect::<Vec<_>>();
        candidates.into_iter().find_map(|candidate| {
            let candidate_descriptor = self.describe_target(&candidate);
            (positional_identity(
                candidate.key().kind(),
                candidate.body(),
                &candidate_descriptor,
            ) == Some(position.clone()))
            .then(|| candidate.key().clone())
        })
    }

    pub(super) fn bind_destination(&mut self, dependency: &PortableShowObjectKey) {
        let Some(target) = self
            .target
            .object(dependency.kind(), dependency.id())
            .cloned()
        else {
            return;
        };
        let descriptor = self.describe_target(&target);
        self.bound_identities.extend(
            descriptor
                .identities
                .into_iter()
                .map(|identity| ((dependency.clone(), identity.slot), identity.value)),
        );
    }
}

fn positional_identity(
    kind: &str,
    body: &serde_json::Value,
    descriptor: &ImportObjectDescriptor,
) -> Option<String> {
    if matches!(kind, "fixture" | "patched_fixture") {
        return body
            .get("fixture_number")
            .and_then(serde_json::Value::as_u64)
            .map(|number| number.to_string());
    }
    descriptor
        .identities
        .iter()
        .find(|identity| identity.slot == "object")
        .map(|identity| identity.value.clone())
}

/// Caches capability-owned descriptors once per planning pass. Besides avoiding repeated adapter
/// work, this gives the allocator a complete set of custom semantic identities before it chooses
/// any duplicate key.
#[derive(Default)]
struct CustomDescriptorCatalog {
    entries: BTreeMap<PortableShowObjectKey, Result<Option<ImportObjectDescriptor>, String>>,
}

#[derive(Clone, Copy)]
enum DocumentSide {
    Source,
    Target,
}

impl CustomDescriptorCatalog {
    fn from_document<P: SelectiveShowImportPorts>(
        document: &PortableShowDocument,
        ports: &P,
    ) -> Self {
        let entries = document
            .objects()
            .filter(|object| !is_registered_object_kind(object.key().kind()))
            .map(|object| {
                (
                    object.key().clone(),
                    ports
                        .describe_import_object(object)
                        .map_err(|error| error.message),
                )
            })
            .collect();
        Self { entries }
    }

    fn descriptor(
        &self,
        key: &PortableShowObjectKey,
    ) -> Option<Result<Option<ImportObjectDescriptor>, String>> {
        self.entries.get(key).cloned()
    }

    fn insert(
        &mut self,
        key: PortableShowObjectKey,
        descriptor: Result<Option<ImportObjectDescriptor>, String>,
    ) {
        self.entries.insert(key, descriptor);
    }

    fn identity_values(&self) -> impl Iterator<Item = String> + '_ {
        self.entries
            .values()
            .filter_map(|descriptor| descriptor.as_ref().ok())
            .filter_map(Option::as_ref)
            .flat_map(|descriptor| {
                descriptor
                    .identities
                    .iter()
                    .map(|identity| identity.value.clone())
            })
    }
}

fn key_only_descriptor(object: &PortableShowObject) -> ImportObjectDescriptor {
    ImportObjectDescriptor {
        identities: vec![super::ImportOwnedIdentity {
            slot: "object".into(),
            value: object.key().id().into(),
            location: None,
        }],
        ..ImportObjectDescriptor::default()
    }
}

fn filtered_stage_layout(
    source: &serde_json::Value,
    identities: &BTreeSet<String>,
) -> serde_json::Value {
    let mut filtered = source.clone();
    for field in ["positions", "positions3d"] {
        if let Some(entries) = filtered
            .get_mut(field)
            .and_then(serde_json::Value::as_object_mut)
        {
            entries.retain(|identity, _| identities.contains(identity));
        }
    }
    filtered
}

fn merge_stage_layout(
    target: &serde_json::Value,
    imported: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = target.clone();
    for field in ["positions", "positions3d"] {
        let Some(imported_entries) = imported.get(field).and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        let Some(merged_object) = merged.as_object_mut() else {
            return imported.clone();
        };
        let target_entries = merged_object
            .entry(field)
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        let Some(target_entries) = target_entries.as_object_mut() else {
            *target_entries = serde_json::Value::Object(imported_entries.clone());
            continue;
        };
        target_entries.extend(imported_entries.clone());
    }
    merged
}
