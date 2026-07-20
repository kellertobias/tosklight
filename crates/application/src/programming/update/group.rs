use std::collections::HashSet;

use light_core::FixtureId;
use light_programmer::{GroupDefinition, ProgrammerUpdateContent, merge_ordered_group_membership};

use super::error::UpdateError;
use super::model::{
    ExistingContentMode, UpdateAddress, UpdateIgnoreReason, UpdateItemOutcome, UpdateMode,
    UpdatePreview, UpdatePreviewItem, UpdateTargetFamily, UpdateTargetIdentity,
};
use super::plan::{AtomicUpdatePlan, PlannedUpdateObject, ensure_revision};

pub fn preview_group_update(
    group: &GroupDefinition,
    resolved_membership: &[FixtureId],
    mode: ExistingContentMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<UpdatePreview, UpdateError> {
    if !programmer.has_selection() {
        return Err(UpdateError::EmptyProgrammer {
            target_family: UpdateTargetFamily::Group,
        });
    }
    let existing = resolved_membership.iter().copied().collect::<HashSet<_>>();
    let mut selected = HashSet::new();
    let items = programmer
        .selected_fixtures
        .iter()
        .filter(|fixture_id| selected.insert(**fixture_id))
        .map(|fixture_id| UpdatePreviewItem {
            address: UpdateAddress::GroupMembership {
                fixture_id: *fixture_id,
            },
            outcome: membership_outcome(&existing, *fixture_id, mode),
        })
        .collect();
    Ok(UpdatePreview {
        target: UpdateTargetIdentity::group(group),
        mode: UpdateMode::ExistingContent(mode),
        items,
    })
}

fn membership_outcome(
    existing: &HashSet<FixtureId>,
    fixture_id: FixtureId,
    mode: ExistingContentMode,
) -> UpdateItemOutcome {
    if existing.contains(&fixture_id) {
        UpdateItemOutcome::Unchanged { source: None }
    } else if mode == ExistingContentMode::AddNew {
        UpdateItemOutcome::AddNew
    } else {
        UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NewGroupMember,
        }
    }
}

pub fn plan_group_update(
    group: &GroupDefinition,
    resolved_membership: &[FixtureId],
    current_revision: u64,
    expected_revision: u64,
    mode: ExistingContentMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<AtomicUpdatePlan, UpdateError> {
    ensure_revision(expected_revision, current_revision)?;
    let preview = preview_group_update(group, resolved_membership, mode, programmer)?;
    if !preview.has_real_change() {
        return Err(UpdateError::NoOp {
            target: preview.target,
        });
    }
    let mut updated = group.clone();
    updated.fixtures =
        merge_ordered_group_membership(resolved_membership, &programmer.selected_fixtures);
    // Normal Group Merge dereferences only when it actually adds membership.
    updated.derived_from = None;
    updated.frozen_from = None;
    Ok(AtomicUpdatePlan {
        target: preview.target.clone(),
        expected_revision,
        preview,
        object: PlannedUpdateObject::Group(updated),
    })
}
