use super::record_index::StoredFixtureRecords;
use super::{
    PatchFixtureAxis, PatchFixtureCandidate, PatchFixtureUpdateAction, PatchFixtureUpdateIntent,
};
use crate::{ActionError, ActionErrorKind};
use light_fixture::{MultiPatchInstance, PatchedFixturePatch};

pub(super) fn resolve_fixture_updates(
    stored: &StoredFixtureRecords,
    updates: &[PatchFixtureUpdateIntent],
) -> Result<Vec<PatchFixtureCandidate>, ActionError> {
    updates
        .iter()
        .map(|update| resolve_fixture_update(stored, update))
        .collect()
}

fn resolve_fixture_update(
    stored: &StoredFixtureRecords,
    update: &PatchFixtureUpdateIntent,
) -> Result<PatchFixtureCandidate, ActionError> {
    let existing = stored
        .get(update.fixture_id)
        .ok_or_else(|| not_found("patched fixture does not exist"))?;
    let profile = existing
        .record
        .selected_profile_reference()
        .map_err(patch_error)?
        .ok_or_else(|| invalid("patched fixture has no portable profile reference"))?;
    let mut patch = existing.record.patch().map_err(patch_error)?;
    apply_update(&mut patch, update)?;
    Ok(PatchFixtureCandidate { profile, patch })
}

fn apply_update(
    patch: &mut PatchedFixturePatch,
    update: &PatchFixtureUpdateIntent,
) -> Result<(), ActionError> {
    match &update.action {
        PatchFixtureUpdateAction::SetMasters {
            group_masters_enabled,
            grand_master_enabled,
        } => {
            require_root(update)?;
            patch.group_masters_enabled = *group_masters_enabled;
            patch.grand_master_enabled = *grand_master_enabled;
        }
        PatchFixtureUpdateAction::SetPanTilt {
            invert_pan,
            invert_tilt,
        } => match exact_copy(patch, update.multipatch_instance_id)? {
            Some(instance) => {
                instance.invert_pan = *invert_pan;
                instance.invert_tilt = *invert_tilt;
            }
            None => {
                patch.invert_pan = *invert_pan;
                patch.invert_tilt = *invert_tilt;
            }
        },
        PatchFixtureUpdateAction::SetMoveInBlack {
            enabled,
            delay_millis,
        } => {
            require_root(update)?;
            patch.move_in_black_enabled = *enabled;
            patch.move_in_black_delay_millis = *delay_millis;
        }
        PatchFixtureUpdateAction::SetLocationAxis { axis, millimetres } => {
            match exact_copy(patch, update.multipatch_instance_id)? {
                Some(instance) => set_location_axis(&mut instance.location, *axis, *millimetres),
                None => set_location_axis(&mut patch.location, *axis, *millimetres),
            }
        }
        PatchFixtureUpdateAction::SetRotationAxis { axis, degrees } => {
            match exact_copy(patch, update.multipatch_instance_id)? {
                Some(instance) => set_rotation_axis(&mut instance.rotation, *axis, *degrees),
                None => set_rotation_axis(&mut patch.rotation, *axis, *degrees),
            }
        }
        PatchFixtureUpdateAction::SetBracketAngle { degrees } => {
            match exact_copy(patch, update.multipatch_instance_id)? {
                Some(instance) => instance.bracket_angle = *degrees,
                None => patch.bracket_angle = *degrees,
            }
        }
        PatchFixtureUpdateAction::SetShaperModuleAngle { degrees } => {
            match exact_copy(patch, update.multipatch_instance_id)? {
                Some(instance) => instance.shaper_angle = *degrees,
                None => patch.shaper_angle = *degrees,
            }
        }
        PatchFixtureUpdateAction::SetStaticShaperAngle { element, degrees } => {
            let index = usize::from(*element - 1);
            match exact_copy(patch, update.multipatch_instance_id)? {
                Some(instance) => {
                    instance.installed_appearance.shaper_angles_degrees[index] = *degrees
                }
                None => patch.installed_appearance.shaper_angles_degrees[index] = *degrees,
            }
        }
        PatchFixtureUpdateAction::SetInstalledAppearance { appearance } => {
            match exact_copy(patch, update.multipatch_instance_id)? {
                Some(instance) => instance.installed_appearance = appearance.clone(),
                None => patch.installed_appearance = appearance.clone(),
            }
        }
        PatchFixtureUpdateAction::SetFreeze { freeze } => {
            require_root(update)?;
            let valid_targets = std::iter::once(patch.fixture_id)
                .chain(patch.logical_heads.iter().map(|head| head.fixture_id))
                .collect::<std::collections::HashSet<_>>();
            if freeze
                .targets
                .keys()
                .any(|fixture_id| !valid_targets.contains(fixture_id))
            {
                return Err(invalid(
                    "fixture Freeze state contains an identity outside the patched fixture",
                ));
            }
            patch.freeze = freeze.clone();
        }
    }
    Ok(())
}

fn require_root(update: &PatchFixtureUpdateIntent) -> Result<(), ActionError> {
    if update.multipatch_instance_id.is_some() {
        Err(invalid(
            "the requested fixture update is owned by the root fixture",
        ))
    } else {
        Ok(())
    }
}

fn exact_copy(
    patch: &mut PatchedFixturePatch,
    instance_id: Option<uuid::Uuid>,
) -> Result<Option<&mut MultiPatchInstance>, ActionError> {
    let Some(instance_id) = instance_id else {
        return Ok(None);
    };
    patch
        .multipatch
        .iter_mut()
        .find(|instance| instance.id == instance_id)
        .map(Some)
        .ok_or_else(|| not_found("multi-patch instance does not exist"))
}

fn set_location_axis(
    location: &mut light_fixture::FixtureLocation,
    axis: PatchFixtureAxis,
    value: i32,
) {
    match axis {
        PatchFixtureAxis::X => location.x = value,
        PatchFixtureAxis::Y => location.y = value,
        PatchFixtureAxis::Z => location.z = value,
    }
}

fn set_rotation_axis(
    rotation: &mut light_fixture::FixtureVector,
    axis: PatchFixtureAxis,
    value: f32,
) {
    match axis {
        PatchFixtureAxis::X => rotation.x = value,
        PatchFixtureAxis::Y => rotation.y = value,
        PatchFixtureAxis::Z => rotation.z = value,
    }
}

fn patch_error(error: light_fixture::PortablePatchError) -> ActionError {
    invalid(error.to_string())
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}
