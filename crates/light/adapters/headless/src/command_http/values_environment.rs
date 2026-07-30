use std::collections::{HashMap, HashSet};

use light_application::ProgrammingValuesEnvironment;
use light_core::{AttributeKey, FixtureId};

use super::super::AppState;

pub(super) fn values_environment(state: &AppState) -> ProgrammingValuesEnvironment {
    let snapshot = state.output.snapshot();
    let group_members = resolved_group_members(&snapshot.groups);
    ProgrammingValuesEnvironment {
        fixture_ids: fixture_ids(&snapshot.fixtures),
        group_memberships: group_members
            .iter()
            .map(|(id, members)| (id.clone(), members.len()))
            .collect(),
        group_members,
        // Linked captures must come from the authoritative context projection. Do not invent
        // profile defaults for addresses absent from that frozen resolved view.
        current_values: state.output.resolved_values(),
        supported_attributes: supported_attributes(&snapshot.fixtures),
        activation_links: state.attributes.activation_links(),
    }
}

fn supported_attributes(
    fixtures: &[light_fixture::PatchedFixture],
) -> HashMap<FixtureId, HashSet<AttributeKey>> {
    let mut result = HashMap::new();
    for fixture in fixtures {
        result.insert(
            fixture.fixture_id,
            fixture
                .definition
                .heads
                .iter()
                .filter(|head| head.shared)
                .flat_map(|head| {
                    head.parameters
                        .iter()
                        .map(|parameter| parameter.attribute.clone())
                })
                .collect(),
        );
        for logical in &fixture.logical_heads {
            result.insert(
                logical.fixture_id,
                fixture
                    .definition
                    .heads
                    .iter()
                    .filter(|head| head.index == logical.head_index)
                    .flat_map(|head| {
                        head.parameters
                            .iter()
                            .map(|parameter| parameter.attribute.clone())
                    })
                    .collect(),
            );
        }
    }
    result
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

fn resolved_group_members(
    groups: &[light_programmer::GroupDefinition],
) -> HashMap<String, Vec<FixtureId>> {
    let by_id = groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    groups
        .iter()
        .map(|group| {
            // Unresolvable derived groups fall back to the stored membership so the
            // group stays addressable; the mutation itself still validates elsewhere.
            let members = light_programmer::resolve_group(&group.id, &by_id)
                .unwrap_or_else(|_| group.fixtures.clone());
            (group.id.clone(), members)
        })
        .collect()
}
