use std::collections::HashSet;

use light_application::ProgrammingValuesEnvironment;
use light_core::FixtureId;

use super::super::AppState;

pub(super) fn values_environment(state: &AppState) -> ProgrammingValuesEnvironment {
    let snapshot = state.engine.snapshot();
    ProgrammingValuesEnvironment {
        fixture_ids: fixture_ids(&snapshot.fixtures),
        group_ids: snapshot
            .groups
            .iter()
            .map(|group| group.id.clone())
            .collect(),
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
