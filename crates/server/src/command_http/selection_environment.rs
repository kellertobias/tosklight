use std::collections::{HashMap, HashSet};

use light_application::{ProgrammingSelectionEnvironment, ProgrammingSelectionQuery};
use light_core::FixtureId;

use super::super::AppState;

pub(super) fn selection_environment(
    state: &AppState,
    query: &ProgrammingSelectionQuery,
) -> ProgrammingSelectionEnvironment {
    let snapshot = state.engine.snapshot();
    match query {
        ProgrammingSelectionQuery::Fixtures(requested) => ProgrammingSelectionEnvironment {
            show_revision: snapshot.revision,
            selectable_fixtures: selectable_fixtures(&snapshot.fixtures, requested),
            groups: HashMap::new(),
        },
        ProgrammingSelectionQuery::Groups(requested) => ProgrammingSelectionEnvironment {
            show_revision: snapshot.revision,
            selectable_fixtures: HashMap::new(),
            groups: group_dependency_closure(&snapshot.groups, requested),
        },
    }
}

fn selectable_fixtures(
    fixtures: &[light_fixture::PatchedFixture],
    requested: &[FixtureId],
) -> HashMap<FixtureId, Vec<FixtureId>> {
    let mut requested = requested.iter().copied().collect::<HashSet<_>>();
    let mut selectable = HashMap::new();
    if requested.is_empty() {
        return selectable;
    }
    for fixture in fixtures {
        if requested.remove(&fixture.fixture_id) {
            selectable.insert(
                fixture.fixture_id,
                super::super::selectable_fixture_ids(fixture),
            );
        }
        for head in &fixture.logical_heads {
            if requested.remove(&head.fixture_id) {
                selectable.insert(head.fixture_id, vec![head.fixture_id]);
            }
        }
        if requested.is_empty() {
            break;
        }
    }
    selectable
}

fn group_dependency_closure(
    groups: &[light_programmer::GroupDefinition],
    requested: &[String],
) -> HashMap<String, light_programmer::GroupDefinition> {
    let index = groups
        .iter()
        .map(|group| (group.id.as_str(), group))
        .collect::<HashMap<_, _>>();
    let mut pending = requested.to_vec();
    let mut selected = HashMap::new();
    while let Some(id) = pending.pop() {
        if selected.contains_key(&id) {
            continue;
        }
        let Some(group) = index.get(id.as_str()) else {
            continue;
        };
        if let Some(derived) = &group.derived_from {
            pending.push(derived.source_group_id.clone());
        }
        selected.insert(id, (*group).clone());
    }
    selected
}
