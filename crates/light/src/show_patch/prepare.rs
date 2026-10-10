use super::legacy_profiles::materialize_touched_legacy_profiles;
use super::placement::assign_placement_addresses;
use super::profiles::{
    ResolvedMode, ResolvedModes, ResolvedProfiles, profile_mode_is_position_point,
};
use super::projection::build_change;
use super::record_index::StoredFixtureRecords;
use super::records::{
    PositionReferences, StagedFixture, build_records, stage_group_pruning, stage_records,
    stage_removals,
};
use super::update::resolve_fixture_updates;
use super::vector_spread::apply_vector_spreads;
use super::{PatchChange, PatchFixturesCommand, PatchPerformancePhase, ShowPatchPorts};
use crate::{
    ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowObjectKind,
    PreparedShowCandidate, prepare_show_candidate,
};
use light_show::{
    PortableShowCandidate, PortableShowDocument, PortableShowObjectKey, PortableShowTransaction,
};
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
    super::replacement::reconcile_replacements(
        &stored,
        &profiles,
        &mut expanded,
        &command.fixture_updates,
    )?;
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
    let mut staged = stage_patch(document, command, plan)?;
    if staged.is_empty() {
        return staged.noop_change(document).map(PreparedPatch::Noop);
    }
    ports.prepare_programming_replacements(&staged.programming_replacements)?;
    let transaction = staged.take_transaction();
    let compile_started = Instant::now();
    let candidate = prepare_show_candidate(document, transaction);
    ports.record_patch_performance_phase(PatchPerformancePhase::Compile, compile_started.elapsed());
    let candidate = candidate?;
    let projection = document
        .candidate(candidate.transaction())
        .map_err(candidate_error)?;
    let (change, group_changes) = staged.finish(document, projection, &BTreeSet::new())?;
    Ok(PreparedPatch::Mutation(Box::new(PreparedMutation {
        candidate,
        change,
        group_changes,
    })))
}

/// A patch command staged into one portable transaction, before compilation.
///
/// The patch capability compiles it directly; a sync transaction first adds its own non-patch
/// object writes, so that one CAD gesture commits — and compiles — exactly once.
pub(crate) struct StagedPatch {
    transaction: PortableShowTransaction,
    fixtures: Vec<StagedFixture>,
    removed: Vec<light_core::FixtureId>,
    modes: ResolvedModes,
    pruned_groups: Vec<String>,
    command: PatchFixturesCommand,
    programming_objects: BTreeSet<PortableShowObjectKey>,
    programming_replacements: Vec<super::PatchProgrammingReplacement>,
}

fn stage_patch(
    document: &PortableShowDocument,
    command: &PatchFixturesCommand,
    plan: PatchPlan,
) -> Result<StagedPatch, ActionError> {
    let stored = StoredFixtureRecords::load(document)?;
    let profiles = plan.profiles;
    let assigned_command = PatchFixturesCommand {
        show_id: command.show_id,
        fixtures: plan.fixtures,
        remove_fixture_ids: command.remove_fixture_ids.clone(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
        fixture_updates: command.fixture_updates.clone(),
    };
    let references = position_references(document, &stored, &profiles, &assigned_command);
    let fixtures = build_records(&stored, &profiles, &assigned_command, &references)?;
    let mut transaction = document.transaction();
    let modes = profiles.stage(&mut transaction)?;
    stage_records(&mut transaction, &fixtures);
    let removed = stage_removals(&stored, &mut transaction, &command.remove_fixture_ids);
    let pruned_groups = stage_group_pruning(document, &mut transaction, &removed);
    let projection = document.candidate(&transaction).map_err(candidate_error)?;
    let programming_replacements =
        super::programming_replacement::build(document, projection, command, &fixtures)?;
    let programming_objects = super::programming_replacement::stage(
        document,
        &mut transaction,
        &programming_replacements,
    )?;
    if !transaction.is_empty() {
        transaction.mark_patch_changed();
    }
    Ok(StagedPatch {
        transaction,
        fixtures,
        removed,
        modes,
        pruned_groups,
        programming_objects,
        programming_replacements,
        command: assigned_command,
    })
}

/// Plans and stages a patch command inside an already-open active-show unit, for a caller that
/// commits it together with other changes.
pub(crate) fn stage_patch_command<P: ShowPatchPorts>(
    document: &PortableShowDocument,
    command: &PatchFixturesCommand,
    ports: &P,
) -> Result<StagedPatch, ActionError> {
    let plan = plan_patch(document, command, ports)?;
    let staged = stage_patch(document, command, plan)?;
    ports.prepare_programming_replacements(&staged.programming_replacements)?;
    Ok(staged)
}

impl StagedPatch {
    /// `true` when the command already matches the document and changes nothing.
    pub(crate) fn is_empty(&self) -> bool {
        self.transaction.is_empty()
    }

    pub(crate) fn take_transaction(&mut self) -> PortableShowTransaction {
        let expected = self.transaction.expected_revision();
        std::mem::replace(
            &mut self.transaction,
            PortableShowTransaction::new(expected),
        )
    }

    fn noop_change(&self, document: &PortableShowDocument) -> Result<PatchChange, ActionError> {
        let candidate = document
            .candidate(&self.transaction)
            .map_err(candidate_error)?;
        build_change(candidate, &self.fixtures, &self.removed, &self.modes)
    }

    /// Checks the compiled candidate changes nothing outside this patch (and `extra`, the
    /// caller's own staged objects), then projects the committed patch change and the Groups its
    /// removals pruned.
    pub(crate) fn finish(
        &self,
        document: &PortableShowDocument,
        projection: PortableShowCandidate<'_>,
        extra: &BTreeSet<PortableShowObjectKey>,
    ) -> Result<(PatchChange, Vec<ActiveShowObjectChange>), ActionError> {
        let mut allowed = extra.clone();
        allowed.extend(self.programming_objects.iter().cloned());
        ensure_patch_scoped_candidate(
            document,
            projection,
            &self.command,
            &self.pruned_groups,
            &allowed,
        )?;
        let change = build_change(projection, &self.fixtures, &self.removed, &self.modes)?;
        let mut group_changes = pruned_group_changes(projection, &self.pruned_groups)?;
        for key in &self.programming_objects {
            let kind = match key.kind() {
                "group" => ActiveShowObjectKind::Group,
                "preset" => ActiveShowObjectKind::Preset,
                "cue_list" => ActiveShowObjectKind::CueList,
                _ => unreachable!(),
            };
            if group_changes
                .iter()
                .any(|change| change.object_id == key.id() && change.kind == kind)
            {
                continue;
            }
            let object = projection.object(key.kind(), key.id()).ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::Internal,
                    "replacement source object is missing",
                )
            })?;
            group_changes.push(
                ActiveShowObjectChange::present(
                    kind,
                    key.id().to_owned(),
                    object.revision(),
                    object.body().clone(),
                )
                .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?,
            );
        }
        Ok((change, group_changes))
    }
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
    extra: &BTreeSet<PortableShowObjectKey>,
) -> Result<(), ActionError> {
    let fixture_ids = command_fixture_ids(command);
    let has_unrelated_write = candidate.objects().any(|object| {
        if extra.contains(object.key()) {
            return false;
        }
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
        !extra.contains(object.key())
            && candidate
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
