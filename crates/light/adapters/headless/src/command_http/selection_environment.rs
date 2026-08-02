use std::collections::{HashMap, HashSet};

use light_application::{ProgrammingSelectionEnvironment, ProgrammingSelectionQuery};
use light_core::FixtureId;

use super::super::AppState;

pub(super) fn selection_environment(
    state: &AppState,
    query: &ProgrammingSelectionQuery,
) -> Result<ProgrammingSelectionEnvironment, light_application::ActionError> {
    let snapshot = state.output.snapshot();
    Ok(match query {
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
    })
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
        match &group.source {
            Some(light_programmer::GroupFixtureSource::References { references }) => {
                pending.extend(
                    references
                        .iter()
                        .map(|reference| reference.group_id.clone()),
                );
            }
            Some(light_programmer::GroupFixtureSource::Explicit { .. }) => {}
            None => {
                if let Some(derived) = &group.derived_from {
                    pending.push(derived.source_group_id.clone());
                }
            }
        }
        selected.insert(id, (*group).clone());
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_programmer::{
        DerivedGroup, GroupDefinition, GroupFixtureSource, GroupReference, SelectionRule,
    };

    fn explicit_group(id: &str, fixtures: Vec<FixtureId>) -> GroupDefinition {
        GroupDefinition {
            id: id.into(),
            source: Some(GroupFixtureSource::Explicit {
                fixture_ids: fixtures,
            }),
            ..Default::default()
        }
    }

    fn reference_group(id: &str, source_ids: &[&str]) -> GroupDefinition {
        GroupDefinition {
            id: id.into(),
            source: Some(GroupFixtureSource::References {
                references: source_ids
                    .iter()
                    .map(|source_id| GroupReference {
                        group_id: (*source_id).into(),
                        rule: SelectionRule::All,
                    })
                    .collect(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn group_dependency_closure_follows_nested_canonical_references_once() {
        let fixture = FixtureId::new();
        let groups = vec![
            explicit_group("left", vec![fixture]),
            explicit_group("right", vec![]),
            reference_group("pair", &["left", "right"]),
            reference_group("nested", &["pair", "left", "missing"]),
        ];

        let closure = group_dependency_closure(&groups, &["nested".into()]);

        assert_eq!(closure.len(), 4);
        for id in ["nested", "pair", "left", "right"] {
            assert!(
                closure.contains_key(id),
                "canonical dependency {id} is present"
            );
        }
        assert!(!closure.contains_key("missing"));
        let error = light_programmer::resolve_group("nested", &closure).unwrap_err();
        assert!(error.contains("group missing does not exist"), "{error}");
    }

    #[test]
    fn group_dependency_closure_preserves_canonical_precedence_and_legacy_fallback() {
        let fixture = FixtureId::new();
        let mut canonical = explicit_group("canonical", vec![fixture]);
        canonical.derived_from = Some(DerivedGroup {
            source_group_id: "ignored-legacy".into(),
            rule: SelectionRule::All,
        });
        let legacy = GroupDefinition {
            id: "legacy".into(),
            fixtures: vec![],
            derived_from: Some(DerivedGroup {
                source_group_id: "legacy-source".into(),
                rule: SelectionRule::All,
            }),
            ..Default::default()
        };
        let groups = vec![
            canonical,
            legacy,
            explicit_group("ignored-legacy", vec![]),
            explicit_group("legacy-source", vec![]),
        ];

        let closure = group_dependency_closure(&groups, &["canonical".into(), "legacy".into()]);

        assert!(closure.contains_key("canonical"));
        assert!(closure.contains_key("legacy"));
        assert!(closure.contains_key("legacy-source"));
        assert!(!closure.contains_key("ignored-legacy"));
    }

    #[test]
    fn group_dependency_closure_terminates_for_canonical_cycles() {
        let groups = vec![
            reference_group("a", &["b"]),
            reference_group("b", &["c"]),
            reference_group("c", &["a"]),
        ];

        let closure = group_dependency_closure(&groups, &["a".into()]);

        assert_eq!(closure.len(), 3);
        let error = light_programmer::resolve_group("a", &closure).unwrap_err();
        assert!(error.contains("a -> b -> c -> a"), "{error}");
    }
}
