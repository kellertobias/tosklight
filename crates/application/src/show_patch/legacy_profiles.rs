use super::PatchFixturesCommand;
use super::profiles::ProfileKey;
use super::record_index::StoredFixtureRecords;
use crate::{ActionError, ActionErrorKind};
use light_core::FixtureId;
use light_show::{FixtureProfileRevision, PortableShowDocument};
use std::collections::BTreeMap;

/// Exact inline revisions owned by legacy fixtures touched by one patch command.
///
/// These values must enter the candidate before the fixture records are converted to lean
/// references. Resolving the same identity from the desk library would make a portable show's
/// behavior depend on installation-local state and could discard unknown profile fields.
pub(super) fn materialize_touched_legacy_profiles(
    document: &PortableShowDocument,
    stored: &StoredFixtureRecords,
    command: &PatchFixturesCommand,
) -> Result<BTreeMap<ProfileKey, FixtureProfileRevision>, ActionError> {
    let mut discovered = BTreeMap::new();
    for fixture in &command.fixtures {
        discover_fixture_profile(stored, fixture.patch.fixture_id, &mut discovered)?;
    }
    retain_profiles_missing_from_document(document, discovered)
}

fn discover_fixture_profile(
    stored: &StoredFixtureRecords,
    fixture_id: FixtureId,
    discovered: &mut BTreeMap<ProfileKey, FixtureProfileRevision>,
) -> Result<(), ActionError> {
    let Some(existing) = stored.get(fixture_id) else {
        return Ok(());
    };
    let record = &existing.record;
    if !record.is_legacy_inline() {
        return Ok(());
    }
    let reference = record
        .selected_profile_reference()
        .map_err(patch_error)?
        .ok_or_else(|| invalid("legacy fixture has no portable profile reference"))?;
    let raw_profile = record
        .body()
        .pointer("/definition/profile_snapshot")
        .filter(|profile| !profile.is_null())
        .ok_or_else(|| invalid("legacy fixture has no inline profile snapshot"))?;
    let profile = FixtureProfileRevision::from_profile(raw_profile.clone()).map_err(store_error)?;
    ensure_reference_identity(&profile, reference)?;
    insert_exact_revision(discovered, profile)
}

fn retain_profiles_missing_from_document(
    document: &PortableShowDocument,
    discovered: BTreeMap<ProfileKey, FixtureProfileRevision>,
) -> Result<BTreeMap<ProfileKey, FixtureProfileRevision>, ActionError> {
    let mut missing = BTreeMap::new();
    for (key, inline) in discovered {
        let Some(stored) = document.fixture_profile_revision(FixtureId(key.0), key.1) else {
            missing.insert(key, inline);
            continue;
        };
        ensure_same_digest(stored, &inline)?;
    }
    Ok(missing)
}

fn ensure_reference_identity(
    profile: &FixtureProfileRevision,
    reference: light_fixture::PatchedFixtureProfileReference,
) -> Result<(), ActionError> {
    let actual = profile.id();
    if actual.profile_id() == reference.profile_id
        && actual.revision() == reference.profile_revision
    {
        Ok(())
    } else {
        Err(invalid(
            "legacy fixture definition and inline profile identities differ",
        ))
    }
}

fn insert_exact_revision(
    profiles: &mut BTreeMap<ProfileKey, FixtureProfileRevision>,
    candidate: FixtureProfileRevision,
) -> Result<(), ActionError> {
    let key = (candidate.id().profile_id().0, candidate.id().revision());
    if let Some(existing) = profiles.get(&key) {
        ensure_same_digest(existing, &candidate)
    } else {
        profiles.insert(key, candidate);
        Ok(())
    }
}

fn ensure_same_digest(
    existing: &FixtureProfileRevision,
    candidate: &FixtureProfileRevision,
) -> Result<(), ActionError> {
    if existing.digest() == candidate.digest() {
        Ok(())
    } else {
        Err(invalid(format!(
            "fixture profile {} revision {} has conflicting inline content digests {} and {}",
            candidate.id().profile_id().0,
            candidate.id().revision(),
            existing.digest().as_str(),
            candidate.digest().as_str(),
        )))
    }
}

fn patch_error(error: light_fixture::PortablePatchError) -> ActionError {
    invalid(error.to_string())
}

fn store_error(error: light_show::StoreError) -> ActionError {
    invalid(error.to_string())
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
