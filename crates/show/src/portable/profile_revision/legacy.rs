use super::{FixtureProfileRevision, FixtureProfileRevisionId, profile_conflict};
use crate::{
    StoreError,
    portable::{PortableShowDocument, PortableShowObjectKey},
};
use serde_json::Value;
use std::collections::BTreeMap;

/// One legacy inline `profile_snapshot` and its exact owning object location.
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyInlineProfileSnapshot {
    owner: PortableShowObjectKey,
    json_pointer: String,
    profile: FixtureProfileRevision,
}

impl LegacyInlineProfileSnapshot {
    pub fn owner(&self) -> &PortableShowObjectKey {
        &self.owner
    }

    pub fn json_pointer(&self) -> &str {
        &self.json_pointer
    }

    pub fn profile(&self) -> &FixtureProfileRevision {
        &self.profile
    }
}

impl PortableShowDocument {
    pub fn discover_legacy_inline_profile_snapshots(
        &self,
    ) -> Result<Vec<LegacyInlineProfileSnapshot>, StoreError> {
        discover_legacy_inline_profile_snapshots(self)
    }

    pub fn canonical_legacy_fixture_profile_revisions(
        &self,
    ) -> Result<Vec<FixtureProfileRevision>, StoreError> {
        let snapshots = self.discover_legacy_inline_profile_snapshots()?;
        canonicalize_legacy_inline_profile_snapshots(&snapshots)
    }
}

pub fn discover_legacy_inline_profile_snapshots(
    document: &PortableShowDocument,
) -> Result<Vec<LegacyInlineProfileSnapshot>, StoreError> {
    let mut snapshots = Vec::new();
    for object in document.objects() {
        visit_legacy_inline_profile_snapshots(object.key(), object.body(), &mut |snapshot| {
            snapshots.push(snapshot);
            Ok(())
        })?;
    }
    Ok(snapshots)
}

pub fn canonicalize_legacy_inline_profile_snapshots(
    snapshots: &[LegacyInlineProfileSnapshot],
) -> Result<Vec<FixtureProfileRevision>, StoreError> {
    let mut revisions = BTreeMap::<FixtureProfileRevisionId, FixtureProfileRevision>::new();
    for snapshot in snapshots {
        insert_canonical(&mut revisions, snapshot.profile.clone())?;
    }
    Ok(revisions.into_values().collect())
}

pub(crate) fn visit_legacy_inline_profile_snapshots(
    owner: &PortableShowObjectKey,
    value: &Value,
    visitor: &mut impl FnMut(LegacyInlineProfileSnapshot) -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    match owner.kind() {
        "fixture" => visit_legacy_fixture_object(owner, value, visitor),
        "patched_fixture" => visit_fixture(owner, value, "", visitor),
        "fixture_bundle" => visit_fixture_bundle(owner, value, visitor),
        _ => Ok(()),
    }
}

fn visit_legacy_fixture_object(
    owner: &PortableShowObjectKey,
    value: &Value,
    visitor: &mut impl FnMut(LegacyInlineProfileSnapshot) -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    visit_fixture(owner, value, "", visitor)?;
    visit_fixture_bundle(owner, value, visitor)
}

fn visit_fixture(
    owner: &PortableShowObjectKey,
    value: &Value,
    pointer: &str,
    visitor: &mut impl FnMut(LegacyInlineProfileSnapshot) -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    let Some(snapshot) = value.pointer("/definition/profile_snapshot") else {
        return Ok(());
    };
    if snapshot.is_null() {
        return Ok(());
    }
    let snapshot_pointer = format!("{pointer}/definition/profile_snapshot");
    visitor(decode_snapshot(owner, &snapshot_pointer, snapshot)?)
}

fn visit_fixture_bundle(
    owner: &PortableShowObjectKey,
    value: &Value,
    visitor: &mut impl FnMut(LegacyInlineProfileSnapshot) -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    let Some(fixtures) = value.get("fixtures").and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, fixture) in fixtures.iter().enumerate() {
        visit_fixture(owner, fixture, &format!("/fixtures/{index}"), visitor)?;
    }
    Ok(())
}

fn decode_snapshot(
    owner: &PortableShowObjectKey,
    pointer: &str,
    value: &Value,
) -> Result<LegacyInlineProfileSnapshot, StoreError> {
    let profile = FixtureProfileRevision::from_profile(value.clone()).map_err(|error| {
        StoreError::Invalid(format!(
            "invalid inline profile snapshot in {}/{} at {pointer}: {error}",
            owner.kind(),
            owner.id()
        ))
    })?;
    Ok(LegacyInlineProfileSnapshot {
        owner: owner.clone(),
        json_pointer: pointer.to_owned(),
        profile,
    })
}

fn insert_canonical(
    revisions: &mut BTreeMap<FixtureProfileRevisionId, FixtureProfileRevision>,
    candidate: FixtureProfileRevision,
) -> Result<(), StoreError> {
    if let Some(existing) = revisions.get(candidate.id()) {
        if existing.digest() != candidate.digest() {
            return Err(profile_conflict(existing, &candidate));
        }
        return Ok(());
    }
    revisions.insert(candidate.id().clone(), candidate);
    Ok(())
}
