use super::legacy_profiles::materialize_touched_legacy_profiles;
use super::placement::assign_placement_addresses;
use super::profiles::{ResolvedMode, ResolvedProfiles, profile_mode_is_position_point};
use super::projection::build_change;
use super::record_index::StoredFixtureRecords;
use super::records::{
    PositionReferences, build_records, stage_group_pruning, stage_records, stage_removals,
};
use super::update::resolve_fixture_updates;
use super::vector_spread::apply_vector_spreads;
use super::{PatchChange, PatchFixturesCommand, PatchPerformancePhase, ShowPatchPorts};
use crate::{
    ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowObjectKind,
    PreparedShowCandidate, prepare_show_candidate,
};
use light_show::{PortableShowCandidate, PortableShowDocument};
use std::{collections::BTreeSet, time::Instant};

pub(super) struct PatchPlan {
    profiles: ResolvedProfiles,
    fixtures: Vec<super::PatchFixtureCandidate>,
}

pub(super) enum PreparedPatch {
    Noop(PatchChange),
    Mutation(Box<PreparedMutation>),
}

pub(super) struct PreparedMutation {
    pub(super) candidate: PreparedShowCandidate,
    pub(super) change: PatchChange,
    /// Groups whose membership lost a removed fixture in the same transaction.
    pub(super) group_changes: Vec<ActiveShowObjectChange>,
}

/// Resolves immutable external profile revisions against one coherent patch snapshot.
///
/// The caller deliberately performs this potentially slow work after releasing the shared
/// active-show mutation gate. `prepare_patch` must therefore rebase the resulting intent against
/// the current document after the gate is reacquired.
pub(super) fn plan_patch<P: ShowPatchPorts>(
    document: &PortableShowDocument,
    command: &PatchFixturesCommand,
    ports: &P,
) -> Result<PatchPlan, ActionError> {
    let stored = StoredFixtureRecords::load(document)?;
    let mut expanded = command.clone();
    // Complete Patch upserts predate Freeze and do not carry captured semantic values. Preserve
    // those values before sparse updates are expanded; a sparse SetFreeze action must still be
    // able to deliberately replace the state with an empty map when unfreezing.
    for fixture in &mut expanded.fixtures {
        if fixture.patch.freeze.is_empty()
            && let Some(existing) = stored.get(fixture.patch.fixture_id)
        {
            fixture.patch.freeze = existing
                .record
                .patch()
                .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?
                .freeze;
        }
    }
    expanded
        .fixtures
        .extend(resolve_fixture_updates(&stored, &command.fixture_updates)?);
    expanded.fixture_updates.clear();
    let materialized = materialize_touched_legacy_profiles(document, &stored, &expanded)?;
    let profiles = ResolvedProfiles::resolve(document, &expanded, materialized, ports)?;
    let fixtures = assign_placement_addresses(&expanded, &profiles)?;
    let fixtures = apply_vector_spreads(fixtures, &command.vector_spreads)?;
    Ok(PatchPlan { profiles, fixtures })
}

pub(super) fn prepare_patch<P: ShowPatchPorts>(
    document: &PortableShowDocument,
    command: &PatchFixturesCommand,
    plan: PatchPlan,
    ports: &P,
) -> Result<PreparedPatch, ActionError> {
    let stored = StoredFixtureRecords::load(document)?;
    let profiles = plan.profiles;
    let assigned_command = PatchFixturesCommand {
        show_id: command.show_id,
        fixtures: plan.fixtures,
        remove_fixture_ids: command.remove_fixture_ids.clone(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
        fixture_updates: Vec::new(),
    };
    let references = position_references(document, &stored, &profiles, &assigned_command);
    let fixtures = build_records(&stored, &profiles, &assigned_command, &references)?;
    let mut transaction = document.transaction();
    let modes = profiles.stage(&mut transaction)?;
    stage_records(&mut transaction, &fixtures);
    let removed = stage_removals(&stored, &mut transaction, &command.remove_fixture_ids);
    let pruned_groups = stage_group_pruning(document, &mut transaction, &removed);
    if transaction.is_empty() {
        let candidate = document.candidate(&transaction).map_err(candidate_error)?;
        return build_change(candidate, &fixtures, &removed, &modes).map(PreparedPatch::Noop);
    }
    transaction.mark_patch_changed();
    let compile_started = Instant::now();
    let candidate = prepare_show_candidate(document, transaction);
    ports.record_patch_performance_phase(PatchPerformancePhase::Compile, compile_started.elapsed());
    let candidate = candidate?;
    let projection = document
        .candidate(candidate.transaction())
        .map_err(candidate_error)?;
    ensure_patch_scoped_candidate(document, projection, &assigned_command, &pruned_groups)?;
    let change = build_change(projection, &fixtures, &removed, &modes)?;
    let group_changes = pruned_group_changes(projection, &pruned_groups)?;
    Ok(PreparedPatch::Mutation(Box::new(PreparedMutation {
        candidate,
        change,
        group_changes,
    })))
}

/// Every fixture the show holds once the command is applied, and which of them are 3D Points.
///
/// A candidate's mode is already resolved. A stored fixture the command does not touch is judged
/// by the profile revision the show carries for it; a legacy inline record, which predates points
/// entirely, is known but is not a point.
fn position_references(
    document: &PortableShowDocument,
    stored: &StoredFixtureRecords,
    profiles: &ResolvedProfiles,
    command: &PatchFixturesCommand,
) -> PositionReferences {
    let removed: std::collections::HashSet<_> = command.remove_fixture_ids.iter().collect();
    let mut known = std::collections::HashSet::new();
    let mut points = std::collections::HashSet::new();
    for fixture in &command.fixtures {
        known.insert(fixture.patch.fixture_id);
        if profiles
            .mode(fixture.profile)
            .is_ok_and(ResolvedMode::is_position_point)
        {
            points.insert(fixture.patch.fixture_id);
        }
    }
    for record in stored.iter() {
        let Ok(fixture_id) = record.record.fixture_id() else {
            continue;
        };
        if removed.contains(&fixture_id) || known.contains(&fixture_id) {
            continue;
        }
        known.insert(fixture_id);
        let Ok(Some(reference)) = record.record.profile_reference() else {
            continue;
        };
        let is_point = document
            .fixture_profile_revision(reference.profile_id, reference.profile_revision)
            .is_some_and(|profile| {
                profile_mode_is_position_point(profile.profile(), reference.mode_id)
            });
        if is_point {
            points.insert(fixture_id);
        }
    }
    PositionReferences::new(known, points)
}

fn pruned_group_changes(
    candidate: PortableShowCandidate<'_>,
    pruned_groups: &[String],
) -> Result<Vec<ActiveShowObjectChange>, ActionError> {
    pruned_groups
        .iter()
        .map(|group_id| {
            let object = candidate.object("group", group_id).ok_or_else(|| {
                ActionError::new(ActionErrorKind::Internal, "pruned Group is missing")
            })?;
            ActiveShowObjectChange::present(
                ActiveShowObjectKind::Group,
                group_id.clone(),
                object.revision(),
                object.body().clone(),
            )
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))
        })
        .collect()
}

fn ensure_patch_scoped_candidate(
    document: &PortableShowDocument,
    candidate: PortableShowCandidate<'_>,
    command: &PatchFixturesCommand,
    pruned_groups: &[String],
) -> Result<(), ActionError> {
    let fixture_ids = command_fixture_ids(command);
    let has_unrelated_write = candidate.objects().any(|object| {
        let changed = document
            .object(object.key().kind(), object.key().id())
            .is_none_or(|stored| stored.body() != object.body());
        let pruned_group = object.key().kind() == "group"
            && pruned_groups.iter().any(|id| id == object.key().id());
        changed
            && !pruned_group
            && !allowed_patch_body(object.key().kind(), object.body(), &fixture_ids)
    });
    let has_unrelated_delete = document.objects().any(|object| {
        candidate
            .object(object.key().kind(), object.key().id())
            .is_none()
            && !allowed_patch_body(object.key().kind(), object.body(), &fixture_ids)
    });
    if has_unrelated_write || has_unrelated_delete {
        return Err(ActionError::new(
            ActionErrorKind::Unavailable,
            "active show requires canonical migration before patching",
        )
        .at_revision(document.patch_revision().value()));
    }
    Ok(())
}

fn command_fixture_ids(command: &PatchFixturesCommand) -> BTreeSet<String> {
    command
        .fixtures
        .iter()
        .map(|fixture| fixture.patch.fixture_id.0.to_string())
        .chain(
            command
                .remove_fixture_ids
                .iter()
                .map(|fixture_id| fixture_id.0.to_string()),
        )
        .collect()
}

fn allowed_patch_body(
    kind: &str,
    body: &serde_json::Value,
    fixture_ids: &BTreeSet<String>,
) -> bool {
    kind == "patched_fixture"
        && body
            .get("fixture_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|fixture_id| fixture_ids.contains(fixture_id))
}

fn candidate_error(error: light_show::StoreError) -> ActionError {
    let kind = match error {
        light_show::StoreError::DocumentRevisionConflict { .. } => ActionErrorKind::Conflict,
        _ => ActionErrorKind::Invalid,
    };
    ActionError::new(kind, error.to_string())
}
