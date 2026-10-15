//! Exact stable Preset-instance edges; missing sources keep their materialized fallback.
use super::{
    FixtureIdentityCatalog, IdentityMap,
    locations::{fixture_reference, value_location},
};
use crate::selective_import::{
    ImportIdentityFormat, ImportObjectDescriptor, ImportObjectReference, ImportOwnedIdentity,
};
use light_show::{PortableShowDocument, PortableShowObject, PortableShowObjectKey};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct PresetInstanceCatalog {
    owners: BTreeMap<String, Vec<PortableShowObjectKey>>,
}

impl PresetInstanceCatalog {
    pub fn from_document(document: &PortableShowDocument) -> Self {
        let mut catalog = Self::default();
        for object in document.objects_of_kind("preset") {
            if let Some(instance) = object
                .body()
                .get("instance_id")
                .and_then(Value::as_str)
                .and_then(|raw| Uuid::parse_str(raw).ok())
            {
                catalog
                    .owners
                    .entry(instance.to_string())
                    .or_default()
                    .push(object.key().clone());
            }
        }
        catalog
    }

    pub fn identity_values(&self) -> impl Iterator<Item = String> + '_ {
        self.owners.keys().cloned()
    }

    fn resolve(&self, instance: &str) -> Result<Option<&PortableShowObjectKey>, String> {
        let normalized = Uuid::parse_str(instance)
            .map_err(|_| format!("invalid Preset instance reference {instance}"))?
            .to_string();
        match self.owners.get(&normalized).map(Vec::as_slice) {
            None | Some([]) => Ok(None),
            Some([owner]) => Ok(Some(owner)),
            Some(_) => Err(format!("ambiguous Preset instance identity {instance}")),
        }
    }
}

pub(super) fn add_preset_instance_identity(
    body: &Value,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let Some(instance) = body.get("instance_id").filter(|value| !value.is_null()) else {
        return Ok(());
    };
    let raw = instance
        .as_str()
        .ok_or("Preset instance identity must be a UUID string")?;
    let id = Uuid::parse_str(raw).map_err(|_| "Preset instance identity must be a UUID string")?;
    if id.is_nil() {
        return Err("Preset instance identity must not be nil".into());
    }
    descriptor.identities.push(ImportOwnedIdentity {
        slot: "preset_instance".into(),
        value: raw.into(),
        location: Some(value_location("/instance_id", ImportIdentityFormat::Full)),
    });
    Ok(())
}

pub(crate) fn add_cue_preset_links(
    object: &PortableShowObject,
    source: &PresetInstanceCatalog,
    target: &PresetInstanceCatalog,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    for pointer in cue_reference_pointers(object.body(), object.key().kind()) {
        let identity_pointer = format!("{pointer}/preset_instance_id");
        let Some(instance) = object
            .body()
            .pointer(&identity_pointer)
            .and_then(Value::as_str)
        else {
            return Err(format!("invalid live Preset reference at {pointer}"));
        };
        // A destination-only instance is already resolved by the candidate compiler.
        // Do not turn its pool key into a source dependency: the source can have an
        // unrelated instance at that same number. Keep the exact identity and owners.
        let owner = source.resolve(instance)?;
        if owner.is_none() {
            target.resolve(instance)?; // Still reject an ambiguous destination catalog.
        }
        // Deleted sources likewise retain their recorded fallback without guessing a key.
        if let Some(owner) = owner {
            descriptor.references.push(ImportObjectReference {
                target: owner.clone(),
                target_slot: "preset_instance".into(),
                source_identity: instance.into(),
                location: value_location(&identity_pointer, ImportIdentityFormat::Full),
                allow_missing: false,
            });
        }
    }
    Ok(())
}

fn cue_reference_pointers(body: &Value, kind: &str) -> Vec<String> {
    if kind != "cue_list" {
        return Vec::new();
    }
    body.get("cues")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .flat_map(|(cue_index, cue)| {
            ["changes", "group_changes"]
                .into_iter()
                .flat_map(move |scope| {
                    cue.get(scope)
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .enumerate()
                        .filter(|(_, change)| {
                            change
                                .get("preset_reference")
                                .is_some_and(|value| !value.is_null())
                        })
                        .map(move |(index, _)| {
                            format!("/cues/{cue_index}/{scope}/{index}/preset_reference")
                        })
                })
        })
        .collect()
}

pub(crate) struct PresetOwnerRewrite<'a> {
    pub identities: &'a IdentityMap,
    pub written_presets: &'a BTreeSet<PortableShowObjectKey>,
    pub source_fixtures: &'a FixtureIdentityCatalog,
    pub target_fixtures: &'a FixtureIdentityCatalog,
}

impl PresetOwnerRewrite<'_> {
    pub fn rewrite(
        &self,
        source: &Value,
        output: &mut Value,
        owner: &PortableShowObjectKey,
        descriptor: &ImportObjectDescriptor,
    ) -> Result<(), String> {
        for pointer in cue_reference_pointers(source, owner.kind()) {
            let identity = value_location(
                &format!("{pointer}/preset_instance_id"),
                ImportIdentityFormat::Full,
            );
            let Some(link) = descriptor
                .references
                .iter()
                .find(|link| link.location == identity)
            else {
                continue;
            };
            // Kept/identical or destination-only Presets retain THEIR existing value-owner
            // identities, even when the imported Cue's target fixture has been duplicated.
            if !self.written_presets.contains(&link.target) {
                continue;
            }
            let owner_pointer = format!("{pointer}/source_owner");
            match source
                .pointer(&format!("{owner_pointer}/type"))
                .and_then(Value::as_str)
            {
                Some("fixture") => {
                    self.fixture(source, output, &format!("{owner_pointer}/fixture_id"))?
                }
                Some("group") => {
                    let path = format!("{owner_pointer}/group_id");
                    if let Some(id) = source.pointer(&path).and_then(Value::as_str)
                        && let Some(destination) = self
                            .identities
                            .get(&(PortableShowObjectKey::new("group", id), "object".into()))
                    {
                        *output
                            .pointer_mut(&path)
                            .ok_or("missing Preset Group source owner")? =
                            Value::String(destination.clone());
                    }
                }
                _ => {}
            }
            self.fixture(source, output, &format!("{pointer}/member_fixture"))?;
        }
        Ok(())
    }

    fn fixture(&self, source: &Value, output: &mut Value, pointer: &str) -> Result<(), String> {
        let Some(id) = source.pointer(pointer).and_then(Value::as_str) else {
            return Ok(());
        };
        let reference = fixture_reference(
            id.into(),
            value_location(pointer, ImportIdentityFormat::Full),
            self.source_fixtures,
            self.target_fixtures,
        )?;
        if let Some(destination) = self
            .identities
            .get(&(reference.target, reference.target_slot))
        {
            *output
                .pointer_mut(pointer)
                .ok_or("missing Preset fixture source owner")? = Value::String(destination.clone());
        }
        Ok(())
    }
}
