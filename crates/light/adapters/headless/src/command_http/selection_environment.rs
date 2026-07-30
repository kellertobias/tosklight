use std::collections::{HashMap, HashSet};

use light_application::{ProgrammingSelectionEnvironment, ProgrammingSelectionQuery};
use light_core::FixtureId;
use light_programmer::{StageGridPosition, StageGridPosition2d};

use super::super::AppState;
use crate::runtime::ActiveShowRepository;

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
            stage_positions_2d: HashMap::new(),
            stage_positions_3d: HashMap::new(),
        },
        ProgrammingSelectionQuery::Groups(requested) => ProgrammingSelectionEnvironment {
            show_revision: snapshot.revision,
            selectable_fixtures: HashMap::new(),
            groups: group_dependency_closure(&snapshot.groups, requested),
            stage_positions_2d: HashMap::new(),
            stage_positions_3d: HashMap::new(),
        },
        ProgrammingSelectionQuery::Grid(requested) => {
            let (stage_positions_2d, stage_positions_3d) =
                stage_positions(state, &snapshot.fixtures, requested)?;
            ProgrammingSelectionEnvironment {
                show_revision: snapshot.revision,
                selectable_fixtures: HashMap::new(),
                groups: HashMap::new(),
                stage_positions_2d,
                stage_positions_3d,
            }
        }
    })
}

fn stage_positions(
    state: &AppState,
    fixtures: &[light_fixture::PatchedFixture],
    requested: &[FixtureId],
) -> Result<
    (
        HashMap<FixtureId, StageGridPosition2d>,
        HashMap<FixtureId, StageGridPosition>,
    ),
    light_application::ActionError,
> {
    let requested = requested.iter().copied().collect::<HashSet<_>>();
    if requested.is_empty() {
        return Ok((HashMap::new(), HashMap::new()));
    }
    let active = state.active_show.current().ok_or_else(|| {
        light_application::ActionError::new(
            light_application::ActionErrorKind::NotFound,
            "selection grid requires an active show",
        )
    })?;
    let store = ActiveShowRepository::open(&active.path).map_err(|error| {
        light_application::ActionError::new(
            light_application::ActionErrorKind::Unavailable,
            format!("selection grid could not open the active show: {error}"),
        )
    })?;
    let (_, object) = store
        .object_with_portable_revision("stage_layout", "main")
        .map_err(|error| {
            light_application::ActionError::new(
                light_application::ActionErrorKind::Unavailable,
                format!("selection grid could not read the Stage layout: {error}"),
            )
        })?;
    let layout = object
        .map(|object| {
            serde_json::from_value::<light_application::StageLayout>(object.body).map_err(|error| {
                light_application::ActionError::new(
                    light_application::ActionErrorKind::Conflict,
                    format!("stored Stage layout is invalid: {error}"),
                )
            })
        })
        .transpose()?
        .unwrap_or_default();

    let patch_order = store.objects("patched_fixture").map_err(|error| {
        light_application::ActionError::new(
            light_application::ActionErrorKind::Unavailable,
            format!("selection grid could not read the Patch order: {error}"),
        )
    })?;
    let patch_indices = patch_order
        .into_iter()
        .enumerate()
        .map(|(index, object)| (object.id, index))
        .collect::<HashMap<_, _>>();
    let mut positions_2d = HashMap::new();
    let mut positions_3d = HashMap::new();
    for fixture in fixtures {
        let id = fixture.fixture_id.0.to_string();
        let index = patch_indices.get(&id).copied().unwrap_or(0);
        let (position_2d, position_3d) = resolved_positions(&layout, &id, index);
        for identity in std::iter::once(fixture.fixture_id)
            .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
            .filter(|identity| requested.contains(identity))
        {
            if let Some(position) = position_2d {
                positions_2d.insert(identity, position);
            }
            positions_3d.insert(identity, position_3d);
        }
    }
    Ok((positions_2d, positions_3d))
}

fn resolved_positions(
    layout: &light_application::StageLayout,
    id: &str,
    index: usize,
) -> (Option<StageGridPosition2d>, StageGridPosition) {
    let position_2d = layout
        .positions
        .get(id)
        .map(|position| StageGridPosition2d {
            x: position.x,
            y: position.y,
        });
    let position_3d = layout
        .positions_3d
        .get(id)
        .map(|position| StageGridPosition {
            x: position.x,
            y: position.y,
            z: position.z,
        })
        .or_else(|| {
            layout.positions.get(id).map(|position| StageGridPosition {
                x: (position.x / 100.0 - 0.5) * 12.0,
                y: (position.y / 100.0) * 8.0,
                z: 5.0,
            })
        })
        .unwrap_or_else(|| default_stage_position(index));
    (position_2d, position_3d)
}

fn default_stage_position(index: usize) -> StageGridPosition {
    StageGridPosition {
        x: -5.25 + ((index % 8) as f64) * 1.5,
        y: 1.0 + ((index / 8) as f64) * 1.6,
        z: 5.0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use light_application::{StageLayout, StagePosition2d, StagePosition3d};

    #[test]
    fn stored_2d_and_3d_positions_remain_independent_and_legacy_xyz_is_exact() {
        let id = FixtureId::new().0.to_string();
        let layout = StageLayout {
            positions: HashMap::from([(
                id.clone(),
                StagePosition2d {
                    x: 90.0,
                    y: 25.0,
                    rotation: 0.0,
                },
            )]),
            positions_3d: HashMap::from([(
                id.clone(),
                StagePosition3d {
                    x: -2.0,
                    y: 7.0,
                    z: 3.0,
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        let (position_2d, position_3d) = resolved_positions(&layout, &id, 99);
        assert_eq!(position_2d, Some(StageGridPosition2d { x: 90.0, y: 25.0 }));
        assert_eq!(
            position_3d,
            StageGridPosition {
                x: -2.0,
                y: 7.0,
                z: 3.0
            }
        );

        let legacy = StageLayout {
            positions: layout.positions,
            ..Default::default()
        };
        let migrated = resolved_positions(&legacy, &id, 99).1;
        assert!((migrated.x - 4.8).abs() < f64::EPSILON * 8.0);
        assert_eq!(migrated.y, 2.0);
        assert_eq!(migrated.z, 5.0);
    }
}
