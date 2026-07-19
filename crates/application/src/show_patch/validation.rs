use super::{PatchFixtureCandidate, PatchFixturesCommand};
use crate::{ActionContext, ActionError, ActionErrorKind};
use std::collections::HashSet;

const MAX_PATCH_FIXTURES: usize = 10_000;
const MAX_PATCH_ELEMENTS: usize = 100_000;
const MAX_SPLITS_PER_BINDING: usize = 512;
const MAX_MULTIPATCH_PER_FIXTURE: usize = 10_000;
const MAX_HIGHLIGHT_OVERRIDES: usize = 4_096;
const MAX_NAME_BYTES: usize = 512;
const MAX_LAYER_BYTES: usize = 128;

pub(super) fn validate_action(
    context: &ActionContext,
    command: &PatchFixturesCommand,
) -> Result<(), ActionError> {
    validate_request_identity(context)?;
    let change_count = command
        .fixtures
        .len()
        .checked_add(command.remove_fixture_ids.len())
        .ok_or_else(|| invalid("patch batch is too large"))?;
    if change_count == 0 || change_count > MAX_PATCH_FIXTURES {
        return Err(invalid(format!(
            "patch batch must contain 1-{MAX_PATCH_FIXTURES} fixture changes"
        )));
    }
    let mut fixture_ids = HashSet::with_capacity(command.fixtures.len());
    let mut elements = command.remove_fixture_ids.len();
    for fixture in &command.fixtures {
        elements = elements
            .checked_add(validate_fixture(fixture)?)
            .ok_or_else(|| invalid("patch batch is too large"))?;
        if elements > MAX_PATCH_ELEMENTS {
            return Err(invalid(format!(
                "patch batch must contain at most {MAX_PATCH_ELEMENTS} nested patch elements"
            )));
        }
        if !fixture_ids.insert(fixture.patch.fixture_id) {
            return Err(invalid("patch batch contains a duplicate fixture identity"));
        }
    }
    let mut removals = HashSet::with_capacity(command.remove_fixture_ids.len());
    for fixture_id in &command.remove_fixture_ids {
        if !removals.insert(*fixture_id) {
            return Err(invalid("patch batch contains a duplicate removal identity"));
        }
        if fixture_ids.contains(fixture_id) {
            return Err(invalid(
                "patch batch cannot update and remove the same fixture identity",
            ));
        }
    }
    Ok(())
}

fn validate_request_identity(context: &ActionContext) -> Result<(), ActionError> {
    let Some(request_id) = context.request_id.as_deref() else {
        return Err(invalid("patch operation requires request_id"));
    };
    if request_id.is_empty() || request_id.len() > 128 {
        return Err(invalid("request_id must contain 1-128 bytes"));
    }
    if context.expected_revision.is_none() {
        return Err(invalid(
            "patch operation requires a whole-show expected revision",
        ));
    }
    Ok(())
}

fn validate_fixture(fixture: &PatchFixtureCandidate) -> Result<usize, ActionError> {
    let patch = &fixture.patch;
    if patch.name.len() > MAX_NAME_BYTES {
        return Err(invalid("fixture name is too long"));
    }
    if patch.layer_id.is_empty() || patch.layer_id.len() > MAX_LAYER_BYTES {
        return Err(invalid("patch layer identity must contain 1-128 bytes"));
    }
    validate_splits(&patch.split_patches, "fixture")?;
    if patch.multipatch.len() > MAX_MULTIPATCH_PER_FIXTURE {
        return Err(invalid("fixture has too many multipatch instances"));
    }
    if patch.highlight_overrides.len() > MAX_HIGHLIGHT_OVERRIDES {
        return Err(invalid("fixture has too many Highlight overrides"));
    }
    if !finite_rotation(patch.rotation) {
        return Err(invalid("fixture rotations must be finite"));
    }
    let mut elements = 1_usize
        .saturating_add(patch.split_patches.len())
        .saturating_add(patch.highlight_overrides.len());
    for instance in &patch.multipatch {
        if instance.name.len() > MAX_NAME_BYTES {
            return Err(invalid("multipatch name is too long"));
        }
        if !finite_rotation(instance.rotation) {
            return Err(invalid("fixture rotations must be finite"));
        }
        validate_splits(&instance.split_patches, "multipatch instance")?;
        elements = elements
            .saturating_add(1)
            .saturating_add(instance.split_patches.len());
    }
    Ok(elements)
}

fn validate_splits(splits: &[light_fixture::SplitPatch], owner: &str) -> Result<(), ActionError> {
    if splits.is_empty() || splits.len() > MAX_SPLITS_PER_BINDING {
        return Err(invalid(format!(
            "{owner} must contain 1-{MAX_SPLITS_PER_BINDING} split assignments"
        )));
    }
    let mut numbers = HashSet::with_capacity(splits.len());
    for split in splits {
        if split.split == 0 || !numbers.insert(split.split) {
            return Err(invalid(format!(
                "{owner} split numbers must be unique and greater than zero"
            )));
        }
        if split.universe.is_some() != split.address.is_some() {
            return Err(invalid(format!(
                "{owner} split universe and address must both be set or both be absent"
            )));
        }
    }
    Ok(())
}

fn finite_rotation(rotation: light_fixture::FixtureVector) -> bool {
    rotation.x.is_finite() && rotation.y.is_finite() && rotation.z.is_finite()
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
