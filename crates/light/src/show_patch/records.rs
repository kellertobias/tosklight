use super::profiles::{ResolvedLogicalHead, ResolvedMode, ResolvedProfiles};
use super::record_index::{StoredFixtureRecord, StoredFixtureRecords};
use super::{PatchFixtureCandidate, PatchFixturesCommand};
use crate::{ActionError, ActionErrorKind};
use light_core::FixtureId;
use light_fixture::{
    PatchedFixturePatch, PatchedFixtureProfileReference, PatchedHead, PortablePatchedFixtureRecord,
};
use light_show::{PortableShowDocument, PortableShowTransaction};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

type StableHeadIds = HashMap<uuid::Uuid, FixtureId>;
type LegacyHeadIndices = HashMap<u16, FixtureId>;

pub(super) struct StagedFixture {
    pub(super) record: PortablePatchedFixtureRecord,
    pub(super) profile: PatchedFixtureProfileReference,
    pub(super) patch: PatchedFixturePatch,
    pub(super) changed: bool,
    previous_object_id: Option<String>,
}

/// Which fixtures a position reference may name once this command has been applied.
///
/// A fixture's `position_master` names a 3D Point it is slaved to. The point has to be a fixture
/// the show will hold — one already stored, or one this same command patches — and it has to
/// actually be a 3D Point: slaving a lantern to another lantern would move it by attributes the
/// other lantern does not have. A 3D Point takes no reference of its own; the visualizer and the
/// desk apply one level, and a chain of points would be a picture that quietly disagreed with the
/// aim. A reference to a fixture the show no longer holds is not a fault: the point was deleted,
/// so the slave is stored as placed against the stage, exactly as it is drawn.
pub(super) struct PositionReferences {
    /// Every fixture the show will hold after the command.
    known: HashSet<FixtureId>,
    /// Those of them that are 3D Points.
    points: HashSet<FixtureId>,
}

impl PositionReferences {
    pub(super) fn new(known: HashSet<FixtureId>, points: HashSet<FixtureId>) -> Self {
        Self { known, points }
    }

    /// The reference a stored patch keeps, or why the candidate is refused.
    fn resolve(
        &self,
        fixture_id: FixtureId,
        master: Option<uuid::Uuid>,
        is_point: bool,
    ) -> Result<Option<uuid::Uuid>, ActionError> {
        let Some(master) = master else {
            return Ok(None);
        };
        let master_id = FixtureId(master);
        if master_id == fixture_id {
            return Err(invalid(format!(
                "fixture {} cannot be its own position reference",
                fixture_id.0
            )));
        }
        if is_point {
            return Err(invalid(format!(
                "3D Point {} cannot take a position reference; a point is what other fixtures follow",
                fixture_id.0
            )));
        }
        if !self.known.contains(&master_id) {
            // The point is gone: the fixture stands against the stage again.
            return Ok(None);
        }
        if !self.points.contains(&master_id) {
            return Err(invalid(format!(
                "position reference {master} of fixture {} is not a 3D Point",
                fixture_id.0
            )));
        }
        Ok(Some(master))
    }
}

pub(super) fn build_records(
    stored: &StoredFixtureRecords,
    profiles: &ResolvedProfiles,
    command: &PatchFixturesCommand,
    references: &PositionReferences,
) -> Result<Vec<StagedFixture>, ActionError> {
    command
        .fixtures
        .iter()
        .map(|input| build_record(stored, profiles, input, references))
        .collect()
}

pub(super) fn stage_records(transaction: &mut PortableShowTransaction, fixtures: &[StagedFixture]) {
    for fixture in fixtures.iter().filter(|fixture| fixture.changed) {
        let object_id = fixture.patch.fixture_id.0.to_string();
        if let Some(previous) = fixture.previous_object_id.as_deref()
            && previous != object_id
        {
            transaction.delete("patched_fixture", previous);
        }
        transaction.put("patched_fixture", object_id, fixture.record.body().clone());
    }
}

pub(super) fn stage_removals(
    stored: &StoredFixtureRecords,
    transaction: &mut PortableShowTransaction,
    fixture_ids: &[FixtureId],
) -> Vec<FixtureId> {
    let mut removed = Vec::with_capacity(fixture_ids.len());
    for fixture_id in fixture_ids {
        let Some(existing) = stored.get(*fixture_id) else {
            continue;
        };
        transaction.delete("patched_fixture", existing.object_id.clone());
        removed.push(*fixture_id);
    }
    removed
}

/// A fixture removed from the show leaves every stored Group in the same transaction. The
/// remaining members keep their order, and a Group left without members stays stored as an
/// intentionally empty Group. Returns the ids of the Groups that changed.
pub(super) fn stage_group_pruning(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
    removed: &[FixtureId],
) -> Vec<String> {
    if removed.is_empty() {
        return Vec::new();
    }
    let removed: HashSet<String> = removed.iter().map(|id| id.0.to_string()).collect();
    let mut pruned = Vec::new();
    for object in document.objects_of_kind("group") {
        let mut body = object.body().clone();
        let mut changed = prune_fixture_ids(body.get_mut("fixtures"), &removed);
        if let Some(source) = body.get_mut("source")
            && source.get("type").and_then(Value::as_str) == Some("explicit")
        {
            changed |= prune_fixture_ids(source.get_mut("fixture_ids"), &removed);
        }
        if changed {
            let object_id = object.key().id().to_owned();
            transaction.put("group", object_id.clone(), body);
            pruned.push(object_id);
        }
    }
    pruned
}

fn prune_fixture_ids(ids: Option<&mut Value>, removed: &HashSet<String>) -> bool {
    let Some(Value::Array(ids)) = ids else {
        return false;
    };
    let before = ids.len();
    ids.retain(|id| id.as_str().is_none_or(|id| !removed.contains(id)));
    ids.len() != before
}

fn build_record(
    stored: &StoredFixtureRecords,
    profiles: &ResolvedProfiles,
    input: &PatchFixtureCandidate,
    references: &PositionReferences,
) -> Result<StagedFixture, ActionError> {
    let mode = profiles.mode(input.profile)?;
    let existing = stored.get(input.patch.fixture_id);
    let existing_patch = existing
        .map(|existing| existing.record.patch())
        .transpose()
        .map_err(patch_error)?;
    let mut patch = normalized_patch(input, existing_patch.as_ref(), mode, references)?;
    let changed = record_changed(existing, existing_patch.as_ref(), input.profile, &patch)?;
    let record = updated_record(
        existing.map(|existing| existing.record.clone()),
        input.profile,
        &patch,
    )?;
    patch.logical_heads = record.patch().map_err(patch_error)?.logical_heads;
    Ok(StagedFixture {
        record,
        profile: input.profile,
        patch,
        changed,
        previous_object_id: existing.map(|existing| existing.object_id.clone()),
    })
}

fn normalized_patch(
    input: &PatchFixtureCandidate,
    existing: Option<&PatchedFixturePatch>,
    mode: &ResolvedMode,
    references: &PositionReferences,
) -> Result<PatchedFixturePatch, ActionError> {
    let mut patch = input.patch.clone();
    let existing_heads = existing
        .map(|patch| patch.logical_heads.clone())
        .unwrap_or_default();
    patch.logical_heads = reconcile_heads(mode.logical_heads(), existing_heads)?;
    patch.position_master = references.resolve(
        patch.fixture_id,
        patch.position_master,
        mode.is_position_point(),
    )?;
    normalize_compatibility_addresses(&mut patch);
    Ok(patch)
}

fn record_changed(
    existing: Option<&StoredFixtureRecord>,
    existing_patch: Option<&PatchedFixturePatch>,
    profile: PatchedFixtureProfileReference,
    patch: &PatchedFixturePatch,
) -> Result<bool, ActionError> {
    let Some(record) = existing else {
        return Ok(true);
    };
    if record.record.is_inline()
        || record.object_id != patch.fixture_id.0.to_string()
        || existing_patch != Some(patch)
    {
        return Ok(true);
    }
    Ok(record.record.profile_reference().map_err(patch_error)? != Some(profile))
}

fn updated_record(
    existing: Option<PortablePatchedFixtureRecord>,
    profile: PatchedFixtureProfileReference,
    patch: &PatchedFixturePatch,
) -> Result<PortablePatchedFixtureRecord, ActionError> {
    let Some(mut record) = existing else {
        return PortablePatchedFixtureRecord::from_profile_reference(profile, patch.clone())
            .map_err(patch_error);
    };
    migrate_legacy_record(&mut record)?;
    record
        .update_patch_allowing_identity_changes(patch)
        .map_err(patch_error)?;
    record
        .update_profile_reference(profile)
        .map_err(patch_error)?;
    Ok(record)
}

fn migrate_legacy_record(record: &mut PortablePatchedFixtureRecord) -> Result<(), ActionError> {
    if !record.is_inline() {
        return Ok(());
    }
    let reference = record
        .selected_profile_reference()
        .map_err(patch_error)?
        .ok_or_else(|| invalid("legacy fixture must be migrated before patch editing"))?;
    record
        .into_profile_reference(reference)
        .map_err(patch_error)
}

fn reconcile_heads(
    mode_heads: &[ResolvedLogicalHead],
    existing: Vec<PatchedHead>,
) -> Result<Vec<PatchedHead>, ActionError> {
    let (mut stable, mut legacy) = index_existing_heads(existing)?;
    Ok(mode_heads
        .iter()
        .copied()
        .map(|head| PatchedHead {
            profile_head_id: Some(head.profile_head_id),
            head_index: head.head_index,
            fixture_id: stable
                .remove(&head.profile_head_id)
                .or_else(|| legacy.remove(&head.head_index))
                .unwrap_or_else(FixtureId::new),
        })
        .collect())
}

fn index_existing_heads(
    existing: Vec<PatchedHead>,
) -> Result<(StableHeadIds, LegacyHeadIndices), ActionError> {
    let mut stable = HashMap::new();
    let mut legacy = HashMap::new();
    for head in existing {
        let duplicate = match head.profile_head_id {
            Some(profile_head_id) => stable.insert(profile_head_id, head.fixture_id).is_some(),
            None => legacy.insert(head.head_index, head.fixture_id).is_some(),
        };
        if duplicate {
            return Err(invalid(
                "patched fixture contains duplicate logical head identity",
            ));
        }
    }
    Ok((stable, legacy))
}

fn normalize_compatibility_addresses(patch: &mut PatchedFixturePatch) {
    let primary = patch.split_patches.iter().find(|split| split.split == 1);
    patch.universe = primary.and_then(|split| split.universe);
    patch.address = primary.and_then(|split| split.address);
    for instance in &mut patch.multipatch {
        let primary = instance.split_patches.iter().find(|split| split.split == 1);
        instance.universe = primary.and_then(|split| split.universe);
        instance.address = primary.and_then(|split| split.address);
    }
}

fn patch_error(error: light_fixture::PortablePatchError) -> ActionError {
    invalid(error.to_string())
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

#[cfg(test)]
mod tests;
