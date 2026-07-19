use super::legacy_profiles::materialize_touched_legacy_profiles;
use super::profiles::ResolvedProfiles;
use super::projection::build_change;
use super::record_index::StoredFixtureRecords;
use super::records::{build_records, stage_records, stage_removals};
use super::{PatchChange, PatchFixturesCommand, ShowPatchPorts};
use crate::{ActionError, ActionErrorKind, PreparedShowCandidate, prepare_show_candidate};
use light_show::{PortableShowCandidate, PortableShowDocument};
use std::collections::BTreeSet;

pub(super) struct PatchPlan {
    profiles: ResolvedProfiles,
}

pub(super) enum PreparedPatch {
    Noop(PatchChange),
    Mutation(Box<PreparedMutation>),
}

pub(super) struct PreparedMutation {
    pub(super) candidate: PreparedShowCandidate,
    pub(super) change: PatchChange,
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
    let materialized = materialize_touched_legacy_profiles(document, &stored, command)?;
    let profiles = ResolvedProfiles::resolve(document, command, materialized, ports)?;
    Ok(PatchPlan { profiles })
}

pub(super) fn prepare_patch(
    document: &PortableShowDocument,
    command: &PatchFixturesCommand,
    plan: PatchPlan,
) -> Result<PreparedPatch, ActionError> {
    let stored = StoredFixtureRecords::load(document)?;
    let profiles = plan.profiles;
    let fixtures = build_records(&stored, &profiles, command)?;
    let mut transaction = document.transaction();
    let modes = profiles.stage(&mut transaction)?;
    stage_records(&mut transaction, &fixtures);
    let removed = stage_removals(&stored, &mut transaction, &command.remove_fixture_ids);
    if transaction.is_empty() {
        let candidate = document.candidate(&transaction).map_err(candidate_error)?;
        return build_change(candidate, &fixtures, &removed, &modes).map(PreparedPatch::Noop);
    }
    transaction.mark_patch_changed();
    let candidate = prepare_show_candidate(document, transaction)?;
    let projection = document
        .candidate(candidate.transaction())
        .map_err(candidate_error)?;
    ensure_patch_scoped_candidate(document, projection, command)?;
    let change = build_change(projection, &fixtures, &removed, &modes)?;
    Ok(PreparedPatch::Mutation(Box::new(PreparedMutation {
        candidate,
        change,
    })))
}

fn ensure_patch_scoped_candidate(
    document: &PortableShowDocument,
    candidate: PortableShowCandidate<'_>,
    command: &PatchFixturesCommand,
) -> Result<(), ActionError> {
    let fixture_ids = command_fixture_ids(command);
    let has_unrelated_write = candidate.objects().any(|object| {
        let changed = document
            .object(object.key().kind(), object.key().id())
            .is_none_or(|stored| stored.body() != object.body());
        changed && !allowed_patch_body(object.key().kind(), object.body(), &fixture_ids)
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
