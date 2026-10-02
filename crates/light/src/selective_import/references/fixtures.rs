use super::locations::{
    add_optional_direct_reference, array_at, identity_at, primary_identity, scalar_at,
    value_location,
};
use super::programming::ProgrammingReferences;
use crate::selective_import::model::ImportProfileReference;
use crate::selective_import::{
    ImportIdentityFormat, ImportObjectDescriptor, ImportObjectReference, ImportOwnedIdentity,
    ImportProfileKey, ImportReferenceLocation,
};
use light_core::FixtureId;
use light_show::{PortableShowDocument, PortableShowObject, PortableShowObjectKey};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct IdentityOwner {
    pub object: PortableShowObjectKey,
    pub slot: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FixtureIdentityCatalog {
    owners: BTreeMap<String, IdentityOwner>,
    ambiguous: BTreeSet<String>,
    patch_layers: BTreeSet<String>,
}

impl FixtureIdentityCatalog {
    pub fn from_document(document: &PortableShowDocument) -> Self {
        let mut catalog = Self::default();
        for object in document
            .objects()
            .filter(|object| matches!(object.key().kind(), "fixture" | "patched_fixture"))
        {
            if let Ok(identities) = fixture_identities(object) {
                for identity in identities {
                    catalog.insert(
                        identity.value,
                        IdentityOwner {
                            object: object.key().clone(),
                            slot: identity.slot,
                        },
                    );
                }
            }
        }
        catalog.patch_layers.extend(
            document
                .objects_of_kind("patch_layer")
                .map(|object| object.key().id().to_owned()),
        );
        catalog
    }

    pub fn values(&self) -> impl Iterator<Item = String> + '_ {
        self.owners.keys().cloned()
    }

    pub(super) fn resolve(&self, value: &str) -> Result<Option<&IdentityOwner>, String> {
        if self.ambiguous.contains(value) {
            return Err(format!("fixture identity {value} is owned more than once"));
        }
        Ok(self.owners.get(value))
    }

    pub(super) fn has_patch_layer(&self, layer_id: &str) -> bool {
        self.patch_layers.contains(layer_id)
    }

    fn insert(&mut self, value: String, owner: IdentityOwner) {
        if self
            .owners
            .get(&value)
            .is_some_and(|existing| existing != &owner)
        {
            self.ambiguous.insert(value);
            return;
        }
        self.owners.insert(value, owner);
    }
}

pub(super) fn fixture_descriptor(
    object: &PortableShowObject,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = ImportObjectDescriptor {
        identities: fixture_identities(object)?,
        ..ImportObjectDescriptor::default()
    };
    if let Some(mut reference) = fixture_profile_reference(object)? {
        add_position_calibration_profile_locations(object.body(), &mut reference);
        super::installed_color::add_installed_color_references(
            object.body(),
            &reference,
            &mut descriptor,
        );
        descriptor.profile_references.push(reference);
    }
    if object
        .body()
        .get("layer_id")
        .and_then(Value::as_str)
        .is_some_and(|layer_id| {
            source.has_patch_layer(layer_id) || target.has_patch_layer(layer_id)
        })
    {
        add_optional_direct_reference(object.body(), "/layer_id", "patch_layer", &mut descriptor)?;
    }
    add_freeze_references(object, source, target, &mut descriptor)?;
    Ok(descriptor)
}

fn add_position_calibration_profile_locations(
    body: &Value,
    reference: &mut ImportProfileReference,
) {
    let mut prefixes = vec!["/position_calibration".to_owned()];
    for index in 0..array_at(body, "/multipatch").map_or(0, Vec::len) {
        prefixes.push(format!("/multipatch/{index}/position_calibration"));
    }
    let expected = reference.key.profile_id.0.to_string();
    for prefix in prefixes {
        let pointer = format!("{prefix}/axis_overrides/source_identity/profile_id");
        if body.pointer(&pointer).and_then(Value::as_str) == Some(expected.as_str()) {
            // Duplicate preserves mode/node UUIDs and geometry. Rebase only the matching
            // profile proof; stale geometry/mode/axis evidence must never be regenerated.
            reference
                .id_locations
                .push(value_location(pointer, ImportIdentityFormat::Full));
        }
    }
}

/// Holds refer to this fixture's existing owners and physical copies. Stale IDs are retained,
/// rather than accidentally rebound to a different fixture through the global identity catalog.
fn add_freeze_references(
    object: &PortableShowObject,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let Some(targets) = object
        .body()
        .pointer("/freeze/targets")
        .and_then(Value::as_object)
    else {
        return Ok(());
    };
    let identities = descriptor.identities.clone();
    for (owner, frozen) in targets {
        if let Some(identity) = identities.iter().find(|identity| {
            identity.value == *owner
                && (identity.slot == "object" || identity.slot.starts_with("head:"))
        }) {
            add_local_freeze_reference(
                object,
                identity,
                ImportReferenceLocation::ObjectKey {
                    object_pointer: "/freeze/targets".into(),
                    key: owner.clone(),
                },
                descriptor,
            );
        }
        let escaped = owner.replace('~', "~0").replace('/', "~1");
        let prefix = format!("/freeze/targets/{escaped}");
        let first_reference = descriptor.references.len();
        ProgrammingReferences {
            body: object.body(),
            source,
            target,
            descriptor: &mut *descriptor,
        }
        .attribute_map(&format!("{prefix}/values"))?;
        // A held Target may outlive its Point. Keep real dependencies remappable, while absent
        // fixture/Point references stay loadable. Native Color profile checks remain strict.
        for reference in &mut descriptor.references[first_reference..] {
            if matches!(reference.target.kind(), "fixture" | "patched_fixture") {
                reference.allow_missing = true;
            }
        }
        for (index, instance) in frozen
            .pointer("/position_native/instances")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let instance_id = scalar_at(instance, "/instance_id")?;
            if let Some(identity) = identities.iter().find(|identity| {
                identity.value == instance_id
                    && (identity.slot == "object" || identity.slot.starts_with("multipatch:"))
            }) {
                add_local_freeze_reference(
                    object,
                    identity,
                    value_location(
                        format!("{prefix}/position_native/instances/{index}/instance_id"),
                        ImportIdentityFormat::Full,
                    ),
                    descriptor,
                );
            }
            // Profile duplication currently preserves child channel identities. Signatures
            // and raw words must also remain exact, including stale or incompatible holds.
        }
    }
    Ok(())
}

fn add_local_freeze_reference(
    object: &PortableShowObject,
    identity: &ImportOwnedIdentity,
    location: ImportReferenceLocation,
    descriptor: &mut ImportObjectDescriptor,
) {
    descriptor.references.push(ImportObjectReference {
        target: object.key().clone(),
        target_slot: identity.slot.clone(),
        source_identity: identity.value.clone(),
        location,
        allow_missing: false,
    });
}

fn fixture_identities(object: &PortableShowObject) -> Result<Vec<ImportOwnedIdentity>, String> {
    let mut identities = vec![primary_identity(
        object,
        "/fixture_id",
        ImportIdentityFormat::Full,
    )?];
    let mut head_slots = BTreeSet::new();
    for (index, head) in array_at(object.body(), "/logical_heads")
        .into_iter()
        .flatten()
        .enumerate()
    {
        let head_index = head
            .get("head_index")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("logical head {index} has no integer head_index"))?;
        let slot = format!("head:{head_index}");
        if !head_slots.insert(slot.clone()) {
            return Err(format!(
                "logical head index {head_index} occurs more than once"
            ));
        }
        identities.push(identity_at(
            head,
            &format!("/logical_heads/{index}/fixture_id"),
            "/fixture_id",
            slot,
        )?);
    }
    for (index, instance) in array_at(object.body(), "/multipatch")
        .into_iter()
        .flatten()
        .enumerate()
    {
        let value = scalar_at(instance, "/id")?;
        let slot = format!("multipatch:{index}");
        identities.push(ImportOwnedIdentity {
            slot,
            value,
            location: Some(value_location(
                format!("/multipatch/{index}/id"),
                ImportIdentityFormat::Full,
            )),
        });
    }
    Ok(identities)
}

fn fixture_profile_reference(
    object: &PortableShowObject,
) -> Result<Option<ImportProfileReference>, String> {
    if let Some(reference) = top_level_profile_reference(object.body())? {
        return Ok(Some(reference));
    }
    let Some(snapshot) = object.body().pointer("/definition/profile_snapshot") else {
        return Ok(None);
    };
    if snapshot.is_null() {
        return Ok(None);
    }
    let key = profile_key(snapshot, "legacy inline profile")?;
    let mut id_locations = vec![value_location(
        "/definition/profile_snapshot/id",
        ImportIdentityFormat::Full,
    )];
    if object
        .body()
        .pointer("/definition/profile_id")
        .and_then(Value::as_str)
        .is_some_and(|id| id == key.profile_id.0.to_string())
    {
        id_locations.push(value_location(
            "/definition/profile_id",
            ImportIdentityFormat::Full,
        ));
    }
    Ok(Some(ImportProfileReference {
        key,
        id_locations,
        inline_profile: Some(snapshot.clone()),
    }))
}

pub(super) fn top_level_profile_reference(
    body: &Value,
) -> Result<Option<ImportProfileReference>, String> {
    let (Some(id), Some(revision)) = (
        body.get("profile_id").and_then(Value::as_str),
        body.get("profile_revision").and_then(Value::as_u64),
    ) else {
        return Ok(None);
    };
    let profile_id = Uuid::parse_str(id)
        .map(FixtureId)
        .map_err(|error| format!("profile_id is invalid: {error}"))?;
    Ok(Some(ImportProfileReference {
        key: ImportProfileKey {
            profile_id,
            revision,
        },
        id_locations: vec![value_location("/profile_id", ImportIdentityFormat::Full)],
        inline_profile: None,
    }))
}

fn profile_key(profile: &Value, label: &str) -> Result<ImportProfileKey, String> {
    let id = profile
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} has no profile id"))?;
    let revision = profile
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{label} has no profile revision"))?;
    Ok(ImportProfileKey {
        profile_id: Uuid::parse_str(id)
            .map(FixtureId)
            .map_err(|error| format!("{label} profile id is invalid: {error}"))?,
        revision,
    })
}
