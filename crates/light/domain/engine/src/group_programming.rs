//! Cold membership/rank expansion shared by static Group and Programmer contribution plans.
use light_core::{AttributeKey, AttributeValue, FixtureId, programming::*};
use light_dynamics::RankedSelection;

pub(crate) fn compile_group_values(
    value: &AttributeValue,
    ranking: &RankedSelection,
) -> Result<Vec<(FixtureId, AttributeValue)>, IntentError> {
    let members = ranking
        .ordered_fixture_ids
        .iter()
        .map(|id| (*id, ranking.rank_by_fixture[id]))
        .collect::<Vec<_>>();
    compile_group_member_values(value, &members, ranking.rank_count, |_| {
        Ok(FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        })
    })
}

/// A semantic Group Color addresses the member's optical colour, not a new root master.
/// Keep native/legacy values at their authored address and let explicit correspondence win
/// at the caller. Only independent, physically modelled Color heads of this exact root qualify.
pub(crate) fn semantic_color_destinations(
    target: FixtureId,
    attribute: &AttributeKey,
    value: Option<&AttributeValue>,
    fixtures: &[light_fixture::PatchedFixture],
    profiles: &crate::profile_projection_plan::ProfileProjectionIndex,
) -> Vec<FixtureId> {
    if attribute != AttributeKey::color_ref()
        || value.is_some_and(|value| !matches!(value,
            AttributeValue::ColorProgram(program) if matches!(program.as_ref(), ColorProgram::Semantic { .. })))
    {
        return vec![target];
    }
    let Some((root, index)) = profiles.owner(target) else {
        return vec![target];
    };
    let Some(fixture) = fixtures
        .get(index)
        .filter(|fixture| fixture.fixture_id == root && root == target)
    else {
        return vec![target];
    };
    let Some(mode) = crate::fixture::profile_mode(fixture) else {
        return vec![target];
    };
    let footprint = |owner| {
        profiles.family_footprint(
            fixture,
            mode,
            (owner, ProgrammingOwner::Color),
            fixture.fixture_id.0,
        )
    };
    if footprint(target).is_some_and(|channels| !channels.is_empty()) {
        return vec![target];
    }
    let mut destinations = Vec::new();
    let mut channels = std::collections::HashSet::new();
    for (index, head) in mode.heads.iter().enumerate() {
        let owner = crate::fixture::profile_head_owner(fixture, index, head);
        let Some(owned) = footprint(owner).filter(|owned| !owned.is_empty()) else {
            continue;
        };
        if destinations.contains(&owner) {
            continue;
        }
        if owned.iter().any(|channel| !channels.insert(*channel)) {
            // Shared native controls cannot be claimed by separate logical family writers.
            return vec![target];
        }
        destinations.push(owner);
    }
    if destinations.is_empty() {
        return vec![target];
    }
    if value.is_none() {
        destinations.push(target);
    }
    destinations
}
