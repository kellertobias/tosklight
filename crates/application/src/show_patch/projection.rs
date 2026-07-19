use super::profiles::ResolvedModes;
use super::records::StagedFixture;
use super::{PatchChange, PatchFixtureProjection, PatchProfileRevisionProjection};
use crate::{ActionError, ActionErrorKind};
use light_fixture::PatchPolicy;
use light_show::{FixtureProfileRevision, PortableShowCandidate};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) fn build_change(
    candidate: PortableShowCandidate<'_>,
    fixtures: &[StagedFixture],
    removed_fixture_ids: &[light_core::FixtureId],
    modes: &ResolvedModes,
) -> Result<PatchChange, ActionError> {
    Ok(PatchChange {
        show_id: candidate.id(),
        show_revision: candidate.revision(),
        patch_revision: candidate.patch_revision(),
        fixtures: fixture_projections(candidate, fixtures)?,
        removed_fixture_ids: removed_fixture_ids.to_vec(),
        profile_revisions: profile_projections(candidate, fixtures, modes)?,
    })
}

fn fixture_projections(
    candidate: PortableShowCandidate<'_>,
    fixtures: &[StagedFixture],
) -> Result<Vec<PatchFixtureProjection>, ActionError> {
    fixtures
        .iter()
        .filter(|fixture| fixture.changed)
        .map(|fixture| {
            let id = fixture.patch.fixture_id.0.to_string();
            let fixture_revision = candidate
                .object_revision("patched_fixture", &id)
                .ok_or_else(|| invalid("staged fixture is absent from candidate"))?;
            Ok(PatchFixtureProjection {
                fixture_revision,
                profile: fixture.profile,
                patch: fixture.patch.clone(),
            })
        })
        .collect()
}

fn profile_projections(
    candidate: PortableShowCandidate<'_>,
    fixtures: &[StagedFixture],
    resolved_modes: &ResolvedModes,
) -> Result<Vec<PatchProfileRevisionProjection>, ActionError> {
    let mut modes = BTreeMap::<(Uuid, u64), BTreeSet<Uuid>>::new();
    for fixture in fixtures.iter().filter(|fixture| fixture.changed) {
        modes
            .entry((
                fixture.profile.profile_id.0,
                fixture.profile.profile_revision,
            ))
            .or_default()
            .insert(fixture.profile.mode_id);
    }
    modes
        .into_iter()
        .map(|((profile_id, revision), modes)| {
            let profile = candidate
                .fixture_profile_revision(light_core::FixtureId(profile_id), revision)
                .ok_or_else(|| invalid("candidate is missing a referenced fixture profile"))?;
            profile_projection(profile, modes, resolved_modes)
        })
        .collect()
}

pub(super) fn profile_projection(
    stored: &FixtureProfileRevision,
    modes: BTreeSet<Uuid>,
    resolved_modes: &ResolvedModes,
) -> Result<PatchProfileRevisionProjection, ActionError> {
    let profile = stored.profile();
    Ok(PatchProfileRevisionProjection {
        profile_id: stored.id().profile_id(),
        profile_revision: stored.id().revision(),
        content_digest: stored.digest().as_str().to_owned(),
        manufacturer: required_string(profile, "manufacturer")?,
        name: required_string(profile, "name")?,
        fixture_type: required_string(profile, "fixture_type")?,
        patch_policy: patch_policy(profile)?,
        referenced_modes: modes
            .into_iter()
            .map(|mode_id| {
                resolved_modes
                    .get(light_fixture::PatchedFixtureProfileReference {
                        profile_id: stored.id().profile_id(),
                        profile_revision: stored.id().revision(),
                        mode_id,
                    })
                    .map(|mode| mode.projection().clone())
            })
            .collect::<Result<_, _>>()?,
    })
}

fn patch_policy(profile: &serde_json::Value) -> Result<PatchPolicy, ActionError> {
    match profile
        .get("patch_policy")
        .and_then(serde_json::Value::as_str)
    {
        None | Some("dmx") => Ok(PatchPolicy::Dmx),
        Some("visual_only") => Ok(PatchPolicy::VisualOnly),
        Some(_) => Err(invalid("fixture profile has an invalid patch policy")),
    }
}

fn required_string(value: &serde_json::Value, field: &str) -> Result<String, ActionError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("fixture profile {field} must be a string")))
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
