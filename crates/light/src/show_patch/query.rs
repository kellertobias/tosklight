use super::profiles::{ProfileKey, ResolvedMode, profile_key, resolve_selected_modes};
use super::projection::profile_projection;
use super::record_index::{StoredFixtureRecord, StoredFixtureRecords};
use super::{PatchFixtureProjection, PatchProfileRevisionProjection, PatchSnapshot};
use crate::{ActionError, ActionErrorKind};
use light_fixture::PatchedFixtureProfileReference;
use light_show::{FixtureProfileRevision, PortableShowDocument};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn build_snapshot(
    document: &PortableShowDocument,
) -> Result<PatchSnapshot, ActionError> {
    let mut profiles = BTreeMap::new();
    let stored = StoredFixtureRecords::load(document)?;
    let fixtures = stored
        .iter()
        .map(|record| decode_fixture(document, record, &mut profiles))
        .collect::<Result<Vec<_>, _>>()?;
    let references = fixtures.iter().map(|fixture| fixture.profile);
    let modes = resolve_selected_modes(references.clone(), |key| {
        let profile = profiles
            .get(&(key.0, key.1))
            .ok_or_else(|| invalid("patch snapshot is missing a profile revision"))?;
        ResolvedMode::from_profile(profile.profile(), key.2)
    })?;
    let profile_revisions = snapshot_profiles(&profiles, references, &modes)?;
    Ok(PatchSnapshot {
        show_id: document.id(),
        show_revision: document.revision(),
        patch_revision: document.patch_revision(),
        event_sequence: 0,
        fixtures,
        profile_revisions,
    })
}

fn decode_fixture(
    document: &PortableShowDocument,
    stored: &StoredFixtureRecord,
    profiles: &mut BTreeMap<ProfileKey, FixtureProfileRevision>,
) -> Result<PatchFixtureProjection, ActionError> {
    let record = &stored.record;
    let profile = record
        .selected_profile_reference()
        .map_err(patch_error)?
        .ok_or_else(|| invalid("patched fixture has no portable profile reference"))?;
    insert_profile(
        document,
        record.body(),
        record.is_legacy_inline(),
        profile,
        profiles,
    )?;
    Ok(PatchFixtureProjection {
        fixture_revision: stored.revision,
        profile,
        patch: record.patch().map_err(patch_error)?,
    })
}

fn insert_profile(
    document: &PortableShowDocument,
    body: &serde_json::Value,
    legacy: bool,
    reference: PatchedFixtureProfileReference,
    profiles: &mut BTreeMap<ProfileKey, FixtureProfileRevision>,
) -> Result<(), ActionError> {
    let stored = document
        .fixture_profile_revision(reference.profile_id, reference.profile_revision)
        .cloned();
    let inline = legacy
        .then(|| inline_profile(body, reference))
        .transpose()?;
    if let (Some(stored), Some(inline)) = (&stored, &inline) {
        ensure_same_profile(stored, inline)?;
    }
    let profile = stored
        .or(inline)
        .ok_or_else(|| invalid("patched fixture references a missing profile revision"))?;
    insert_exact_profile(profiles, profile)
}

fn inline_profile(
    body: &serde_json::Value,
    reference: PatchedFixtureProfileReference,
) -> Result<FixtureProfileRevision, ActionError> {
    let raw = body
        .pointer("/definition/profile_snapshot")
        .filter(|profile| !profile.is_null())
        .ok_or_else(|| invalid("legacy fixture has no inline profile snapshot"))?;
    let profile = FixtureProfileRevision::from_profile(raw.clone()).map_err(store_error)?;
    if profile.id().profile_id() == reference.profile_id
        && profile.id().revision() == reference.profile_revision
    {
        Ok(profile)
    } else {
        Err(invalid(
            "legacy fixture definition and inline profile identities differ",
        ))
    }
}

fn insert_exact_profile(
    profiles: &mut BTreeMap<ProfileKey, FixtureProfileRevision>,
    profile: FixtureProfileRevision,
) -> Result<(), ActionError> {
    let key = (profile.id().profile_id().0, profile.id().revision());
    if let Some(existing) = profiles.get(&key) {
        ensure_same_profile(existing, &profile)
    } else {
        profiles.insert(key, profile);
        Ok(())
    }
}

fn ensure_same_profile(
    existing: &FixtureProfileRevision,
    candidate: &FixtureProfileRevision,
) -> Result<(), ActionError> {
    if existing.digest() == candidate.digest() {
        Ok(())
    } else {
        Err(invalid("fixture profile revision has conflicting content"))
    }
}

fn snapshot_profiles(
    profiles: &BTreeMap<ProfileKey, FixtureProfileRevision>,
    references: impl IntoIterator<Item = PatchedFixtureProfileReference>,
    modes: &super::profiles::ResolvedModes,
) -> Result<Vec<PatchProfileRevisionProjection>, ActionError> {
    let mut selected = BTreeMap::<ProfileKey, BTreeSet<uuid::Uuid>>::new();
    for reference in references {
        selected
            .entry(profile_key(reference))
            .or_default()
            .insert(reference.mode_id);
    }
    selected
        .into_iter()
        .map(|(key, mode_ids)| {
            let profile = profiles
                .get(&key)
                .ok_or_else(|| invalid("patch snapshot is missing a profile revision"))?;
            profile_projection(profile, mode_ids, modes)
        })
        .collect()
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
