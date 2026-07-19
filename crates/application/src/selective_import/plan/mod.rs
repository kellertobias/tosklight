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
    pending: BTreeSet<PortableShowObjectKey>,
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
        let reserves_generated_identities = request
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
        let keys = source
            .objects()
            .chain(target.objects())
            .map(|object| object.key().clone())
            .collect::<Vec<_>>();
        let identity_values = source_fixtures
            .values()
            .chain(target_fixtures.values())
            .chain(source_custom_descriptors.identity_values())
            .chain(target_custom_descriptors.identity_values())
            .chain(
                source
                    .fixture_profile_revisions()
                    .iter()
                    .map(|profile| profile.id().profile_id().0.to_string()),
            )
            .chain(
                target
                    .fixture_profile_revisions()
                    .iter()
                    .map(|profile| profile.id().profile_id().0.to_string()),
            )
            .collect::<Vec<_>>();
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
            pending: request.selected_objects.clone(),
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
        let descriptor = self.describe_source(&source);
        let (action, destination) = self.action_for(&key, source.body());
        let destination_identities =
            self.destination_identities(&key, &destination, &action, &descriptor);
        let traverses = !matches!(action, ImportObjectAction::KeepDestination);
        self.items.insert(
            key.clone(),
            PlannedItem {
                body: source.body().clone(),
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
