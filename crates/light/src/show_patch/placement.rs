use super::{
    PatchFixtureCandidate, PatchFixturesCommand, PatchPlacementIntent, PatchSplitPlacementMode,
    profiles::ResolvedProfiles,
};
use crate::{ActionError, ActionErrorKind};
use light_core::FixtureId;
use std::collections::HashMap;

pub(super) fn assign_placement_addresses(
    command: &PatchFixturesCommand,
    profiles: &ResolvedProfiles,
) -> Result<Vec<PatchFixtureCandidate>, ActionError> {
    let mut fixtures = command.fixtures.clone();
    let indices = fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| (fixture.patch.fixture_id, index))
        .collect::<HashMap<_, _>>();
    for placement in &command.placements {
        assign_placement(placement, &indices, &mut fixtures, profiles)?;
    }
    Ok(fixtures)
}

fn assign_placement(
    placement: &PatchPlacementIntent,
    indices: &HashMap<FixtureId, usize>,
    fixtures: &mut [PatchFixtureCandidate],
    profiles: &ResolvedProfiles,
) -> Result<(), ActionError> {
    for split_intent in &placement.splits {
        let overrides = match &split_intent.mode {
            PatchSplitPlacementMode::Consecutive => HashMap::new(),
            PatchSplitPlacementMode::OperatorOverrides(overrides) => overrides
                .iter()
                .map(|override_| {
                    (
                        override_.fixture_id,
                        (override_.universe, override_.address),
                    )
                })
                .collect(),
        };
        let mut offset = 0_u32;
        for fixture_id in &placement.fixture_ids {
            let index = *indices
                .get(fixture_id)
                .ok_or_else(|| invalid("patch placement fixture is unavailable"))?;
            let fixture = &mut fixtures[index];
            let footprint = profiles
                .mode(fixture.profile)?
                .split_footprint(split_intent.split)?;
            let computed = computed_assignment(
                split_intent.universe,
                split_intent.address,
                offset,
                *fixture_id,
                &overrides,
            )?;
            let assignment = fixture
                .patch
                .split_patches
                .iter_mut()
                .find(|assignment| assignment.split == split_intent.split)
                .ok_or_else(|| {
                    invalid(format!(
                        "fixture {} is missing split {} required by its placement intent",
                        fixture.patch.fixture_id.0, split_intent.split
                    ))
                })?;
            assignment.universe = computed.map(|value| value.0);
            assignment.address = computed.map(|value| value.1);
            offset = offset
                .checked_add(u32::from(footprint))
                .ok_or_else(|| invalid("patch placement footprint progression overflowed"))?;
        }
    }
    Ok(())
}

fn computed_assignment(
    universe: Option<u16>,
    address: Option<u16>,
    offset: u32,
    fixture_id: FixtureId,
    overrides: &HashMap<FixtureId, (u16, u16)>,
) -> Result<Option<(u16, u16)>, ActionError> {
    if let Some(override_) = overrides.get(&fixture_id) {
        return Ok(Some(*override_));
    }
    let (Some(universe), Some(address)) = (universe, address) else {
        return Ok(None);
    };
    let address = u32::from(address)
        .checked_add(offset)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| invalid("computed patch address exceeds the supported range"))?;
    Ok(Some((universe, address)))
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
