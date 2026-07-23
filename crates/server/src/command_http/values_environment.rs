use std::collections::{HashMap, HashSet};

use light_application::ProgrammingValuesEnvironment;
use light_core::FixtureId;

use super::super::AppState;

pub(super) fn values_environment(state: &AppState) -> ProgrammingValuesEnvironment {
    let snapshot = state.engine.snapshot();
    ProgrammingValuesEnvironment {
        fixture_ids: fixture_ids(&snapshot.fixtures),
        group_memberships: group_memberships(&snapshot.groups),
    }
}

fn fixture_ids(fixtures: &[light_fixture::PatchedFixture]) -> HashSet<FixtureId> {
    fixtures
        .iter()
        .flat_map(|fixture| {
            std::iter::once(fixture.fixture_id)
                .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
        })
        .collect()
}

fn group_memberships(groups: &[light_programmer::GroupDefinition]) -> HashMap<String, usize> {
    let by_id = groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    groups
        .iter()
        .map(|group| {
            // Unresolvable derived groups fall back to the stored membership so the
            // group stays addressable; the mutation itself still validates elsewhere.
            let size = light_programmer::resolve_group(&group.id, &by_id)
                .map(|fixtures| fixtures.len())
                .unwrap_or(group.fixtures.len());
            (group.id.clone(), size)
        })
        .collect()
}
