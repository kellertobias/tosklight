use super::{
    ObjectUpdate, candidate, invalid_object, patch_heads::ProfileHeadResolver,
    records::decode_unique_records, stage_updates,
};
use crate::{ActionError, lossless_json};
use light_fixture::{
    PatchedFixture, PatchedFixtureCompiler, PatchedFixtureProfileReference,
    PortablePatchedFixtureRecord, ResolvedFixtureProfileRevision, migrate_patched_fixture_to_v2,
};
use light_show::{
    FixtureProfileRevision, FixtureProfileRevisionId, PortableShowCandidate,
    PortableShowCandidateObject, PortableShowDocument, PortableShowTransaction,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn stage_inline_migrations(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
) -> Result<(), ActionError> {
    let (profiles, updates) = {
        let candidate = candidate(document, transaction)?;
        collect_inline(candidate)?
    };
    let patch_changed = !profiles.is_empty() || !updates.is_empty();
    for profile in profiles.into_values() {
        transaction
            .put_fixture_profile_revision(profile)
            .map_err(|error| super::invalid_candidate(error.to_string()))?;
    }
    stage_updates(transaction, updates);
    if patch_changed {
        transaction.mark_patch_changed();
    }
    Ok(())
}

fn collect_inline(
    candidate: PortableShowCandidate<'_>,
) -> Result<
    (
        BTreeMap<FixtureProfileRevisionId, FixtureProfileRevision>,
        Vec<ObjectUpdate>,
    ),
    ActionError,
> {
    let mut profiles = BTreeMap::new();
    let mut updates = Vec::new();
    for decoded in decode_unique_records(candidate)? {
        let object = decoded.object;
        let record = decoded.record;
        if !record.is_legacy_inline() {
            continue;
        }
        let (record, profile) = migrate_inline_record(object, record)?;
        retain_materialized_profile(candidate, &mut profiles, profile)?;
        if record.body() != object.body() {
            updates.push(ObjectUpdate::from_object(object, record.into_body()));
        }
    }
    Ok((profiles, updates))
}

fn migrate_inline_record(
    object: PortableShowCandidateObject<'_>,
    record: PortablePatchedFixtureRecord,
) -> Result<(PortablePatchedFixtureRecord, FixtureProfileRevision), ActionError> {
    let mut fixture = serde_json::from_value::<PatchedFixture>(record.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let before = serde_json::to_value(&fixture).map_err(|error| invalid_object(object, error))?;
    migrate_patched_fixture_to_v2(&mut fixture).map_err(|error| invalid_object(object, error))?;
    let after = serde_json::to_value(&fixture).map_err(|error| invalid_object(object, error))?;
    let mut migrated = record.into_body();
    lossless_json::apply_delta(&mut migrated, &before, &after);

    let mut record = PortablePatchedFixtureRecord::decode(migrated)
        .map_err(|error| invalid_object(object, error))?;
    let reference = record
        .selected_profile_reference()
        .map_err(|error| invalid_object(object, error))?
        .ok_or_else(|| invalid_object(object, "legacy fixture has no portable profile identity"))?;
    let raw_profile = record
        .body()
        .pointer("/definition/profile_snapshot")
        .filter(|profile| !profile.is_null())
        .cloned()
        .ok_or_else(|| invalid_object(object, "legacy fixture has no inline profile snapshot"))?;
    let profile = FixtureProfileRevision::from_profile(raw_profile)
        .map_err(|error| invalid_object(object, error))?;
    ensure_profile_identity(object, reference, &profile)?;
    record
        .migrate_legacy_to_profile_reference(reference)
        .map_err(|error| invalid_object(object, error))?;
    Ok((record, profile))
}

fn retain_materialized_profile(
    candidate: PortableShowCandidate<'_>,
    profiles: &mut BTreeMap<FixtureProfileRevisionId, FixtureProfileRevision>,
    profile: FixtureProfileRevision,
) -> Result<(), ActionError> {
    let id = profile.id();
    if let Some(existing) = candidate.fixture_profile_revision(id.profile_id(), id.revision()) {
        return ensure_matching_profile(existing, &profile);
    }
    if let Some(existing) = profiles.get(id) {
        return ensure_matching_profile(existing, &profile);
    }
    profiles.insert(id.clone(), profile);
    Ok(())
}

fn ensure_matching_profile(
    existing: &FixtureProfileRevision,
    candidate: &FixtureProfileRevision,
) -> Result<(), ActionError> {
    if existing.digest() == candidate.digest() {
        return Ok(());
    }
    Err(super::invalid_candidate(format!(
        "fixture profile {} revision {} has conflicting candidate content digests {} and {}",
        candidate.id().profile_id().0,
        candidate.id().revision(),
        existing.digest().as_str(),
        candidate.digest().as_str()
    )))
}

pub(super) fn stage_lean_migrations(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
) -> Result<(), ActionError> {
    let updates = {
        let candidate = candidate(document, transaction)?;
        collect_lean(candidate)?
    };
    let patch_changed = !updates.is_empty();
    stage_updates(transaction, updates);
    if patch_changed {
        transaction.mark_patch_changed();
    }
    Ok(())
}

fn collect_lean(candidate: PortableShowCandidate<'_>) -> Result<Vec<ObjectUpdate>, ActionError> {
    let resolver = |reference: PatchedFixtureProfileReference| {
        candidate
            .fixture_profile_revision(reference.profile_id, reference.profile_revision)
            .map(|profile| {
                ResolvedFixtureProfileRevision::new(
                    profile.id().profile_id(),
                    profile.id().revision(),
                    profile.digest().as_str(),
                    profile.profile().clone(),
                )
            })
    };
    let mut compiler = PatchedFixtureCompiler::new(resolver);
    let mut fixtures = Vec::new();
    for decoded in decode_unique_records(candidate)? {
        let object = decoded.object;
        let record = decoded.record;
        if record.is_legacy_inline() {
            return Err(invalid_object(
                object,
                "inline fixture remained after profile materialization",
            ));
        }
        let reference = record
            .profile_reference()
            .map_err(|error| invalid_object(object, error))?
            .ok_or_else(|| invalid_object(object, "fixture profile reference is missing"))?;
        let fixture = compiler
            .compile(&record)
            .map_err(|error| invalid_object(object, error))?;
        fixtures.push(LeanFixtureMigration {
            object,
            record,
            reference,
            fixture,
        });
    }
    reconcile_fixture_numbers(&mut fixtures)?;
    let mut heads = ProfileHeadResolver::new(candidate);
    fixtures
        .into_iter()
        .filter_map(|fixture| finish_lean_migration(fixture, &mut heads).transpose())
        .collect()
}

struct LeanFixtureMigration<'a> {
    object: PortableShowCandidateObject<'a>,
    record: PortablePatchedFixtureRecord,
    reference: PatchedFixtureProfileReference,
    fixture: PatchedFixture,
}

fn finish_lean_migration(
    mut migration: LeanFixtureMigration<'_>,
    heads: &mut ProfileHeadResolver<'_>,
) -> Result<Option<ObjectUpdate>, ActionError> {
    migrate_patched_fixture_to_v2(&mut migration.fixture)
        .map_err(|error| invalid_object(migration.object, error))?;
    let mut patch = migration
        .record
        .patch()
        .map_err(|error| invalid_object(migration.object, error))?;
    patch.fixture_number = migration.fixture.fixture_number;
    patch.virtual_fixture_number = migration.fixture.virtual_fixture_number;
    patch.split_patches = migration.fixture.split_patches;
    patch.multipatch = migration.fixture.multipatch;
    patch.logical_heads =
        heads.reconcile(migration.object, migration.reference, patch.logical_heads)?;
    migration
        .record
        .update_patch_allowing_identity_changes(&patch)
        .map_err(|error| invalid_object(migration.object, error))?;
    Ok((migration.record.body() != migration.object.body())
        .then(|| ObjectUpdate::from_object(migration.object, migration.record.into_body())))
}

fn reconcile_fixture_numbers(fixtures: &mut [LeanFixtureMigration<'_>]) -> Result<(), ActionError> {
    let all_missing = !fixtures.is_empty()
        && fixtures.iter().all(|fixture| {
            numeric_field(fixture.object.body(), "fixture_number").is_none()
                && numeric_field(fixture.object.body(), "virtual_fixture_number").is_none()
        });
    let inferred = all_missing.then(|| infer_fixture_numbers(fixtures));
    let mut used_virtual = fixtures
        .iter()
        .filter_map(|fixture| {
            numeric_field(fixture.object.body(), "virtual_fixture_number")
                .and_then(|number| u32::try_from(number).ok())
        })
        .collect::<BTreeSet<_>>();
    let mut next_virtual = 1_u32;
    for fixture in fixtures {
        if let Some(inferred) = &inferred {
            fixture.fixture.fixture_number = inferred.get(fixture.object.key().id()).copied();
        }
        if fixture.fixture.definition.is_dmx_patchable() {
            continue;
        }
        fixture.fixture.fixture_number = None;
        if fixture.fixture.virtual_fixture_number.is_none() {
            next_virtual = next_available(next_virtual, &used_virtual).ok_or_else(|| {
                invalid_object(fixture.object, "virtual fixture number range is exhausted")
            })?;
            fixture.fixture.virtual_fixture_number = Some(next_virtual);
            used_virtual.insert(next_virtual);
            next_virtual = next_virtual.saturating_add(1);
        }
    }
    Ok(())
}

fn infer_fixture_numbers(fixtures: &[LeanFixtureMigration<'_>]) -> BTreeMap<String, u32> {
    let mut candidates = fixtures
        .iter()
        .map(|fixture| {
            (
                fixture.object.key().id().to_owned(),
                numeric_field(fixture.object.body(), "universe").unwrap_or(u64::MAX),
                numeric_field(fixture.object.body(), "address").unwrap_or(u64::MAX),
                fixture
                    .object
                    .body()
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| (left.1, left.2, &left.0).cmp(&(right.1, right.2, &right.0)));
    let mut inferred = BTreeMap::new();
    let mut used = BTreeSet::new();
    for (id, _, _, name) in &candidates {
        if let Some(number) = legacy_default_fixture_number(name)
            && used.insert(number)
        {
            inferred.insert(id.clone(), number);
        }
    }
    let mut next = 1_u32;
    for (id, _, _, _) in candidates {
        if inferred.contains_key(&id) {
            continue;
        }
        let Some(available) = next_available(next, &used) else {
            break;
        };
        inferred.insert(id, available);
        used.insert(available);
        next = available.saturating_add(1);
    }
    inferred
}

fn next_available(mut number: u32, used: &BTreeSet<u32>) -> Option<u32> {
    loop {
        if !used.contains(&number) {
            return Some(number);
        }
        number = number.checked_add(1)?;
    }
}

fn numeric_field(body: &serde_json::Value, field: &str) -> Option<u64> {
    body.get(field).and_then(serde_json::Value::as_u64)
}

fn legacy_default_fixture_number(name: &str) -> Option<u32> {
    let trailing = || name.rsplit_once(' ')?.1.parse::<u32>().ok();
    match name {
        "Middle ACL Set" => Some(28),
        "Outside ACL Set" => Some(29),
        "Stage Hazer" => Some(99),
        "Overhead RGB Multi-patch" => Some(999),
        _ if name.starts_with("Front Fresnel ") => trailing(),
        _ if name.starts_with("Back Profile ") => {
            trailing().and_then(|value| value.checked_add(100))
        }
        _ if name.starts_with("Back LED Wash ") => {
            trailing().and_then(|value| value.checked_add(200))
        }
        _ if name.starts_with("Back Trackspot ") => {
            trailing().and_then(|value| value.checked_add(300))
        }
        _ if name.starts_with("Floor RGBW PAR ") => {
            trailing().and_then(|value| value.checked_add(400))
        }
        _ if name.starts_with("Back RGB Sunstrip ") => {
            trailing().and_then(|value| value.checked_add(500))
        }
        _ if name.starts_with("Front RGB Strobe ") => {
            trailing().and_then(|value| value.checked_add(600))
        }
        _ if name.starts_with("Dimmer ") => trailing(),
        _ if name.starts_with("RGB LED ") => trailing().and_then(|value| value.checked_add(20)),
        _ => None,
    }
}

fn ensure_profile_identity(
    object: PortableShowCandidateObject<'_>,
    reference: PatchedFixtureProfileReference,
    profile: &FixtureProfileRevision,
) -> Result<(), ActionError> {
    if profile.id().profile_id() == reference.profile_id
        && profile.id().revision() == reference.profile_revision
    {
        Ok(())
    } else {
        Err(invalid_object(
            object,
            "inline profile identity differs from the selected fixture definition",
        ))
    }
}
