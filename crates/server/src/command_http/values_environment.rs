use std::collections::{HashMap, HashSet};

use light_application::ProgrammingValuesEnvironment;
use light_core::{AttributeKey, AttributeValue, FixtureId};

use super::super::AppState;

pub(super) fn values_environment(state: &AppState) -> ProgrammingValuesEnvironment {
    let snapshot = state.engine.snapshot();
    let mut current_values = default_values(&snapshot.fixtures);
    current_values.extend(state.engine.resolved_values());
    ProgrammingValuesEnvironment {
        fixture_ids: fixture_ids(&snapshot.fixtures),
        group_memberships: group_memberships(&snapshot.groups),
        current_values,
        supported_attributes: supported_attributes(&snapshot.fixtures),
        activation_links: HashMap::new(),
    }
}

fn default_values(
    fixtures: &[light_fixture::PatchedFixture],
) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
    let mut result = HashMap::new();
    for fixture in fixtures {
        for head in fixture.definition.heads.iter().filter(|head| head.shared) {
            for parameter in &head.parameters {
                result.insert(
                    (fixture.fixture_id, parameter.attribute.clone()),
                    AttributeValue::Normalized(parameter.default),
                );
            }
        }
        for logical in &fixture.logical_heads {
            let Some(head) = fixture
                .definition
                .heads
                .iter()
                .find(|head| head.index == logical.head_index)
            else {
                continue;
            };
            for parameter in &head.parameters {
                result.insert(
                    (logical.fixture_id, parameter.attribute.clone()),
                    AttributeValue::Normalized(parameter.default),
                );
            }
        }
    }
    result
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
