use super::{PatchedFixturePatch, PortablePatchError};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum IdentityPolicy {
    Preserve,
    AllowChanges,
}

pub(super) fn validate_patch_identities(
    current: &PatchedFixturePatch,
    updated: &PatchedFixturePatch,
    policy: IdentityPolicy,
) -> Result<(), PortablePatchError> {
    ensure_fixture_identity(current.fixture_id, updated.fixture_id)?;
    let updated_multipatch = multipatch_ids(updated)?;
    let updated_heads = logical_head_ids(updated)?;
    if policy == IdentityPolicy::AllowChanges {
        return Ok(());
    }
    ensure_identity_set("multipatch", multipatch_ids(current)?, updated_multipatch)?;
    ensure_identity_set("logical head", logical_head_ids(current)?, updated_heads)
}

pub(super) fn validate_new_patch_identities(
    patch: &PatchedFixturePatch,
) -> Result<(), PortablePatchError> {
    multipatch_ids(patch)?;
    logical_head_ids(patch)?;
    Ok(())
}

fn ensure_fixture_identity(
    expected: light_core::FixtureId,
    actual: light_core::FixtureId,
) -> Result<(), PortablePatchError> {
    if expected == actual {
        Ok(())
    } else {
        Err(PortablePatchError::FixtureIdentityChanged { expected, actual })
    }
}

fn multipatch_ids(patch: &PatchedFixturePatch) -> Result<BTreeSet<Uuid>, PortablePatchError> {
    unique_ids(
        patch.multipatch.iter().map(|instance| instance.id),
        "multipatch",
    )
}

fn logical_head_ids(patch: &PatchedFixturePatch) -> Result<BTreeSet<Uuid>, PortablePatchError> {
    unique_ids(
        patch.logical_heads.iter().map(|head| head.fixture_id.0),
        "logical head",
    )
}

fn unique_ids(
    identities: impl Iterator<Item = Uuid>,
    collection: &'static str,
) -> Result<BTreeSet<Uuid>, PortablePatchError> {
    let identities = identities.collect::<Vec<_>>();
    let unique = identities.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() == identities.len() {
        Ok(unique)
    } else {
        Err(PortablePatchError::DuplicateNestedIdentity { collection })
    }
}

fn ensure_identity_set(
    collection: &'static str,
    current: BTreeSet<Uuid>,
    updated: BTreeSet<Uuid>,
) -> Result<(), PortablePatchError> {
    if current == updated {
        Ok(())
    } else {
        Err(PortablePatchError::NestedIdentityChanged { collection })
    }
}
