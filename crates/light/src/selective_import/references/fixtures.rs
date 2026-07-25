use super::locations::{array_at, identity_at, primary_identity, scalar_at, value_location};
use crate::selective_import::model::ImportProfileReference;
use crate::selective_import::{
    ImportIdentityFormat, ImportObjectDescriptor, ImportOwnedIdentity, ImportProfileKey,
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
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = ImportObjectDescriptor {
        identities: fixture_identities(object)?,
        ..ImportObjectDescriptor::default()
    };
    if let Some(reference) = fixture_profile_reference(object)? {
        descriptor.profile_references.push(reference);
    }
    Ok(descriptor)
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
    let mut multipatch_slots = BTreeSet::new();
    for (index, instance) in array_at(object.body(), "/multipatch")
        .into_iter()
        .flatten()
        .enumerate()
    {
        let value = scalar_at(instance, "/id")?;
        let slot = format!("multipatch:{value}");
        if !multipatch_slots.insert(slot.clone()) {
            return Err(format!("multipatch identity {value} occurs more than once"));
        }
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
