use std::collections::{HashMap, HashSet};

use light_application::ProgrammingValuesEnvironment;
use light_core::{AttributeKey, AttributeValue, FixtureId};

use super::super::AppState;

pub(super) fn values_environment(state: &AppState) -> ProgrammingValuesEnvironment {
    let snapshot = state.output.snapshot();
    let (group_members, group_rank_counts) = resolved_group_members(&snapshot);
    ProgrammingValuesEnvironment {
        fixture_ids: fixture_ids(&snapshot.fixtures),
        group_memberships: group_members
            .iter()
            .map(|(id, members)| (id.clone(), members.len()))
            .collect(),
        group_rank_counts,
        group_members,
        // Linked captures must come from the authoritative resolved context. Profile defaults are
        // a separate fallback so a relative turn can start correctly without taking ownership of
        // unrelated linked attributes.
        current_values: state.output.resolved_values(),
        default_values: profile_defaults(&snapshot.fixtures),
        supported_attributes: supported_attributes(&snapshot.fixtures),
        activation_links: state.attributes.activation_links(),
    }
}

fn profile_defaults(
    fixtures: &[light_fixture::PatchedFixture],
) -> HashMap<(FixtureId, AttributeKey), AttributeValue> {
    let mut values = HashMap::new();
    for fixture in fixtures {
        for parameter in fixture
            .definition
            .heads
            .iter()
            .filter(|head| head.shared)
            .flat_map(|head| &head.parameters)
        {
            values.insert(
                (fixture.fixture_id, parameter.attribute.clone()),
                AttributeValue::Normalized(parameter.default),
            );
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
                values.insert(
                    (logical.fixture_id, parameter.attribute.clone()),
                    AttributeValue::Normalized(parameter.default),
                );
            }
        }
    }
    values
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
    snapshot: &light_engine::EngineSnapshot,
) -> (HashMap<String, Vec<FixtureId>>, HashMap<String, usize>) {
    let by_id = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let positions = snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(fixture_id, position)| {
            (
                *fixture_id,
                light_dynamics::Position3d {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                    z: f64::from(position.z),
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let resolved = snapshot
        .groups
        .iter()
        .map(|group| {
            // Unresolvable derived groups fall back to the stored membership so the
            // group stays addressable; the mutation itself still validates elsewhere.
            let spatial = light_programmer::resolve_group_spatial(&group.id, &by_id, &positions);
            spatial.map_or_else(
                |_| {
                    let members = light_programmer::resolve_group(&group.id, &by_id)
                        .unwrap_or_else(|_| group.fixtures.clone());
                    (group.id.clone(), members.clone(), members.len())
                },
                |resolved| {
                    (
                        group.id.clone(),
                        resolved.ranked_selection.ordered_fixture_ids,
                        resolved.ranked_selection.rank_count,
                    )
                },
            )
        })
        .collect::<Vec<_>>();
    (
        resolved
            .iter()
            .map(|(id, members, _)| (id.clone(), members.clone()))
            .collect(),
        resolved
            .into_iter()
            .map(|(id, _, rank_count)| (id, rank_count))
            .collect(),
    )
}
