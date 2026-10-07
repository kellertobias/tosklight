use std::collections::{HashMap, HashSet};

use light_application::ProgrammingValuesEnvironment;
use light_core::{AttributeKey, AttributeValue, FixtureId};

use super::super::AppState;

pub(super) fn values_environment(state: &AppState) -> ProgrammingValuesEnvironment {
    let snapshot = state.output.snapshot();
    let (group_members, group_rank_counts, group_ranks) = resolved_group_members(&snapshot);
    let mut attributes = supported_attributes(&snapshot.fixtures);
    // Native channel names are not the semantic Position owner, especially for custom aliases
    // and shared ancestral motors. Reuse cold geometry ownership; do not compile another solver.
    for (owner, keys) in &mut attributes {
        if state
            .output
            .engine()
            .position_has_native_controls(&snapshot, *owner)
        {
            keys.insert(light_core::programming::ProgrammingOwner::Position.key());
        }
    }
    let supported_programming_contract = state.output.supported_programming_contract();
    let mut default_values = profile_defaults(&snapshot.fixtures);
    if supported_programming_contract >= light_core::programming::PROGRAMMING_CONTRACT_VERSION {
        semantic_color_defaults(&attributes, &mut default_values);
    }
    ProgrammingValuesEnvironment {
        supported_programming_contract,
        fixture_ids: fixture_ids(&snapshot.fixtures),
        group_memberships: group_members
            .iter()
            .map(|(id, members)| (id.clone(), members.len()))
            .collect(),
        group_rank_counts,
        group_ranks,
        group_members,
        // Linked captures must come from the authoritative resolved context. Profile defaults are
        // a separate fallback so a relative turn can start correctly without taking ownership of
        // unrelated linked attributes.
        current_values: state.output.resolved_values(),
        default_values,
        supported_attributes: attributes,
        activation_links: state.attributes.activation_links(),
        family_contexts: Default::default(),
        group_family_contexts: Default::default(),
        group_family_templates: Default::default(),
        displayed_source_hold: None,
        color_adoption: None,
    }
}

pub(super) fn attribute_wraps(
    state: &AppState,
    fixture_id: FixtureId,
    attribute: &AttributeKey,
) -> bool {
    state.output.snapshot().fixtures.iter().any(|fixture| {
        let owns_parent = fixture.fixture_id == fixture_id;
        fixture.definition.heads.iter().any(|head| {
            let owns_head = (head.shared && owns_parent)
                || fixture.logical_heads.iter().any(|patched| {
                    patched.fixture_id == fixture_id && patched.head_index == head.index
                });
            owns_head
                && head.parameters.iter().any(|parameter| {
                    parameter.attribute == *attribute
                        && parameter.capabilities.is_empty()
                        && parameter.metadata.wrap
                })
        })
    })
}

/// TL-552 follow-up: the complete Color a first semantic Color edit starts from on a colour-capable
/// head that nothing has given a colour yet: open white (the semantic default intent), the colour
/// such a head shows at its channel defaults. Without it the first Color encoder turn or dialog
/// pick on a fresh fixture had no complete family to edit and was refused. Authored, current and
/// adopted colours still take precedence (`current_values` and the active Programmer value).
fn semantic_color_defaults(
    attributes: &HashMap<FixtureId, HashSet<AttributeKey>>,
    defaults: &mut light_engine::ResolvedValues,
) {
    use light_core::programming::{ColorIntent, ColorProgram, ProgrammingOwner};
    let white = AttributeValue::ColorProgram(std::sync::Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    for (fixture, keys) in attributes {
        if keys
            .iter()
            .any(|key| &*key.0 == "color" || key.0.starts_with("color."))
        {
            defaults
                .entry((*fixture, ProgrammingOwner::Color.key()))
                .or_insert_with(|| white.clone());
        }
    }
}

fn profile_defaults(fixtures: &[light_fixture::PatchedFixture]) -> light_engine::ResolvedValues {
    let mut values = light_engine::ResolvedValues::default();
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
) -> (
    HashMap<String, Vec<FixtureId>>,
    HashMap<String, usize>,
    HashMap<String, HashMap<FixtureId, usize>>,
) {
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
                    let ranks = members
                        .iter()
                        .enumerate()
                        .map(|(rank, id)| (*id, rank))
                        .collect();
                    (group.id.clone(), members.clone(), members.len(), ranks)
                },
                |resolved| {
                    (
                        group.id.clone(),
                        resolved.ranked_selection.ordered_fixture_ids,
                        resolved.ranked_selection.rank_count,
                        resolved.ranked_selection.rank_by_fixture,
                    )
                },
            )
        })
        .collect::<Vec<_>>();
    (
        resolved
            .iter()
            .map(|(id, members, _, _)| (id.clone(), members.clone()))
            .collect(),
        resolved
            .iter()
            .map(|(id, _, rank_count, _)| (id.clone(), *rank_count))
            .collect(),
        resolved
            .into_iter()
            .map(|(id, _, _, ranks)| (id, ranks))
            .collect(),
    )
}

/// Capture only when an actual component gesture begins, under the existing Programmer/desk
/// mutation boundary. One accepted frame supplies all selected owner contexts and seed values.
/// Pending adoption reads only the injected Pending source (the session Programmer's retained
/// episode and its accepted pair); with no source it stays quiet. Live is never a fallback.
pub(super) fn prepare_family_edit_context(
    state: &AppState,
    session: light_core::SessionId,
    preload: bool,
    intent: &light_application::ProgrammingValueIntent,
    environment: &mut ProgrammingValuesEnvironment,
) {
    use light_application::ProgrammingValueOperation;
    use light_core::programming::{
        ComponentEdit, PositionIntent, ProgrammingComponent, ProgrammingOwner,
    };
    let ProgrammingValueOperation::ComponentEdits(edits) = &intent.operation else {
        return;
    };
    let owner = edits.first().map(ComponentEdit::owner);
    if owner.is_none_or(|owner| {
        !matches!(
            owner,
            ProgrammingOwner::Position | ProgrammingOwner::Color | ProgrammingOwner::Zoom
        )
    }) {
        return;
    }
    let members = intent
        .group_id
        .as_ref()
        .map_or(intent.fixture_ids.as_slice(), |id| {
            environment
                .group_members
                .get(id)
                .map(Vec::as_slice)
                .unwrap_or(&[])
        });
    if owner == Some(ProgrammingOwner::Color) {
        let members = members.to_vec();
        color_capture::prepare_native_color_context(
            state,
            session,
            preload,
            intent,
            edits,
            &members,
            environment,
        );
        return;
    }
    if owner == Some(ProgrammingOwner::Zoom) {
        let members = members.to_vec();
        prepare_zoom_context(state, session, preload, intent, &members, environment);
        return;
    }
    let angle_edit = edits.iter().any(|edit| {
        matches!(
            edit,
            ComponentEdit::ActivateAngles
                | ComponentEdit::Scalar {
                    component: ProgrammingComponent::Pan | ProgrammingComponent::Tilt,
                    ..
                }
        )
    });
    // Clear stale injected adoption only for this family; ordinary authored Angle values are
    // still usable when the destination cannot supply a physical pose.
    for fixture in members {
        let context = environment.family_contexts.entry(*fixture).or_default();
        context.solved_angles = None;
        context.position_adoption_attempted = angle_edit;
    }
    if let Some(reference) = edits.iter().find_map(|edit| match edit {
        ComponentEdit::Target { reference } => Some(*reference),
        _ => None,
    }) {
        for fixture in members {
            environment
                .default_values
                .entry((*fixture, ProgrammingOwner::Position.key()))
                .or_insert_with(|| {
                    AttributeValue::Position(std::sync::Arc::new(PositionIntent::target(
                        reference, [0.; 3],
                    )))
                });
        }
    }
    if !angle_edit {
        return;
    }
    let members = members.to_vec();
    if let Some(captured) =
        displayed_readouts(state, session, preload, intent, &members, environment)
    {
        install_captured_position(environment, captured);
    } else if !preload && intent.displayed_source.is_none() {
        install_unpublished_position(state, environment, &members);
    }
}

/// TL-552: before the first accepted Live frame of the running generation, a first Angle edit
/// adopts what that frame will resolve (see `unpublished_position_seeds`) instead of silently
/// changing nothing. A named displayed source never comes here: it holds or resolves exactly.
fn install_unpublished_position(
    state: &AppState,
    environment: &mut ProgrammingValuesEnvironment,
    members: &[FixtureId],
) {
    use light_core::programming::{PositionIntent, ProgrammingOwner};
    for seed in crate::runtime::position_readout::unpublished_position_seeds(state, members) {
        let address = (seed.owner, ProgrammingOwner::Position.key());
        if let Some(requested) = seed.requested {
            environment.current_values.insert(address, requested);
        } else if let Some(pose) = seed.declared {
            environment
                .family_contexts
                .entry(seed.owner)
                .or_default()
                .solved_angles = Some(pose);
            environment.current_values.insert(
                address,
                AttributeValue::Position(std::sync::Arc::new(PositionIntent::angles(
                    pose.pan_degrees,
                    pose.tilt_degrees,
                ))),
            );
        }
    }
}

/// The accepted source a first family edit adopts, from the active show only. A named displayed
/// source is resolved exactly (TL-594) or holds the action quietly with the re-read reason.
fn displayed_readouts(
    state: &AppState,
    session: light_core::SessionId,
    preload: bool,
    intent: &light_application::ProgrammingValueIntent,
    members: &[FixtureId],
    environment: &mut ProgrammingValuesEnvironment,
) -> Option<crate::runtime::position_readout::CapturedPositionReadouts> {
    let captured = match intent.displayed_source {
        // TL-594: exactly the leased source the surface displayed, or a quiet hold.
        Some(displayed) => {
            let captured = crate::runtime::output_readouts::displayed_position_readouts(
                state,
                session,
                preload,
                displayed,
                intent.undo_group.as_deref(),
                members,
            )
            .filter(|captured| captured.scope.show_id == active_show(state));
            if captured.is_none() {
                environment.displayed_source_hold =
                    Some(light_application::ProgrammingValuesHold::DisplayedSourceUnavailable);
            }
            captured
        }
        // No displayed source (OSC, HTTP integrators): the lane's latest accepted source.
        None if preload => pending_position_readouts(state, session, members),
        None => live_position_readouts(state, members),
    }?;
    (captured.scope.show_id == active_show(state)).then_some(captured)
}

/// TL-637 follow-up: a Zoom edit adopts the displayed accepted output's opening in degrees, from
/// the same frame and under the same lease rules as Position. Each captured member's seed is that
/// frame's typed Zoom request, else its output measured through the compiled optics model; a
/// member without one keeps no Zoom seed (never the percentage), so the application holds the
/// whole action quietly. Without a captured source the environment is untouched, as for Position.
fn prepare_zoom_context(
    state: &AppState,
    session: light_core::SessionId,
    preload: bool,
    intent: &light_application::ProgrammingValueIntent,
    members: &[FixtureId],
    environment: &mut ProgrammingValuesEnvironment,
) {
    let key = light_core::programming::ProgrammingOwner::Zoom.key();
    let Some(captured) = displayed_readouts(state, session, preload, intent, members, environment)
    else {
        return;
    };
    for entry in captured.owners {
        let address = (entry.owner, key.clone());
        environment.current_values.remove(&address);
        if let Some(seed) = entry.zoom {
            environment.current_values.insert(address, seed);
        }
    }
}

fn active_show(state: &AppState) -> Option<uuid::Uuid> {
    state.active_show.current().map(|show| show.id.0)
}

/// The latest accepted Live publication only; never consulted for Pending.
fn live_position_readouts(
    state: &AppState,
    members: &[FixtureId],
) -> Option<crate::runtime::position_readout::CapturedPositionReadouts> {
    let source = state.output.latest_visualization_frame()?;
    crate::runtime::position_readout::capture_position_readouts(
        state.output.engine(),
        &source,
        members,
    )
}

/// The session Programmer's accepted Pending pair from the injected source. Without a source
/// (production until TL-548) or without an accepted pair this is None: quiet, no Live read.
fn pending_position_readouts(
    state: &AppState,
    session: light_core::SessionId,
    members: &[FixtureId],
) -> Option<crate::runtime::position_readout::CapturedPositionReadouts> {
    let source = state.output.pending_position_readouts().source()?;
    let programmer = state.programming.get(session)?.id;
    source.capture(programmer, members)
}

/// Seed requested intent and achieved common joints from one captured accepted frame.
fn install_captured_position(
    environment: &mut ProgrammingValuesEnvironment,
    captured: crate::runtime::position_readout::CapturedPositionReadouts,
) {
    use light_core::programming::{PositionIntent, ProgrammingOwner};
    for entry in captured.owners {
        let fixture = entry.owner;
        let address = (fixture, ProgrammingOwner::Position.key());
        // Requested seed and achieved context share the exact accepted source used by the
        // readout builder. Distinct copy commands cannot become a guessed common seed.
        environment.current_values.remove(&address);
        if let Some(value) = &entry.requested {
            environment
                .current_values
                .insert(address.clone(), value.clone());
        }
        let Some(pose) = entry.common_angles() else {
            continue;
        };
        environment
            .family_contexts
            .entry(fixture)
            .or_default()
            .solved_angles = Some(pose);
        // An ordinary native/legacy source has no typed Position request. Its complete command
        // pair is a valid first-edit seed; do not replace an existing typed requested value.
        environment
            .current_values
            .entry(address)
            .or_insert_with(|| {
                AttributeValue::Position(std::sync::Arc::new(PositionIntent::angles(
                    pose.pan_degrees,
                    pose.tilt_degrees,
                )))
            });
    }
}

#[path = "values_environment/color_capture.rs"]
mod color_capture;
#[cfg(test)]
#[path = "values_environment/pending_position_tests.rs"]
mod pending_position_tests;
#[cfg(test)]
#[path = "values_environment/position_capture_tests.rs"]
mod position_capture_tests;
#[cfg(test)]
#[path = "values_environment/zoom_capture_tests.rs"]
mod zoom_capture_tests;
