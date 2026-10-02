//! Resolve an Aim operand once into requested Position, never achieved channels.
use super::*;
use light_core::programming::{PositionIntent, ProgrammingOwner, TargetReference};

pub(in crate::runtime) fn target_intent_from_frame(
    source: &light_engine::ObservedSourceFrame,
    number: u32,
) -> Result<Option<PositionIntent>, String> {
    let snapshot = source.snapshot();
    let target = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(number))
        .ok_or_else(|| format!("no fixture numbered {number}"))?;
    // Point channels may belong to logical heads, but references always name the root UUID.
    if source
        .points()
        .iter()
        .any(|point| point.fixture_id == target.fixture_id)
    {
        return Ok(Some(PositionIntent::target(
            TargetReference::Point {
                point_id: target.fixture_id.0,
            },
            [0.0; 3],
        )));
    }
    // The scalar mount projection may fall back to saved placement for a missing Point.
    // That fallback must not freeze an explicitly tracked relation into a fixed Origin target.
    if target
        .position_master
        .is_some_and(|id| !source.points().iter().any(|point| point.fixture_id.0 == id))
    {
        return Ok(None);
    }
    let Some(mount) = source.mounts().mount(target.fixture_id.0) else {
        return Ok(None);
    };
    let Some(world) = mount.world_from_fixture else {
        return Ok(None);
    };
    let world = world.point([0.0; 3]);
    let (reference, offset) = if let Some(point) = mount
        .position_master
        .and_then(|id| source.points().iter().find(|point| point.fixture_id == id))
    {
        let Some(rotation) =
            light_core::spatial::RigidTransform::euler_xyz(point.rotation_degrees.map(f64::from))
        else {
            return Ok(None);
        };
        let local = std::array::from_fn(|axis| {
            world[axis]
                - f64::from(point.origin_metres[axis])
                - f64::from(point.offset_metres[axis])
        });
        (
            TargetReference::Point {
                point_id: point.fixture_id.0,
            },
            rotation.inverse().direction(local),
        )
    } else {
        (TargetReference::Origin, world)
    };
    let intent = PositionIntent::target(reference, offset.map(|value| value as f32));
    intent.validate().map_err(|error| error.to_string())?;
    Ok(Some(intent))
}

pub(in crate::runtime) fn aim_target_intent(
    state: &AppState,
    number: u32,
) -> Result<Option<PositionIntent>, String> {
    target_intent_from_frame(&state.output.engine().observe_source_frame(&[]), number)
}

/// A runtime-only materialization. The original Preset/raw body stays available for selection,
/// persistence and transport. Semantic Aim is universal, so live Group ownership survives.
pub(in crate::runtime) fn resolve_aim_preset(
    state: &AppState,
    preset: &light_programmer::Preset,
) -> Result<light_programmer::Preset, String> {
    let Some(number) = preset.aim_at_fixture_number else {
        return Ok(preset.clone());
    };
    let mut resolved = preset.clone();
    resolved.values.clear();
    resolved.group_values.clear();
    resolved.universal_values.clear();
    resolved.aim_at_fixture_number = None;
    if state.output.supported_programming_contract()
        >= light_core::programming::PROGRAMMING_CONTRACT_VERSION
    {
        if let Some(intent) = aim_target_intent(state, number)? {
            resolved.universal_values.insert(
                ProgrammingOwner::Position.key(),
                light_core::AttributeValue::Position(Arc::new(intent)),
            );
        }
    } else {
        let targets = state
            .output
            .snapshot()
            .fixtures
            .iter()
            .flat_map(super::super::selectable_fixture_ids)
            .collect::<Vec<_>>();
        for (fixture, attribute, value) in super::aim_selection(state, &targets, number)? {
            resolved
                .values
                .entry(fixture)
                .or_default()
                .insert(attribute, value);
        }
    }
    Ok(resolved)
}

/// Commit requested Position through the existing whole-family batch, inside the entered
/// command's staged transaction. Preload remains a separate lane; Blind is recordable Normal.
pub(in crate::runtime) fn apply_position_mutations(
    state: &AppState,
    session: &Session,
    mutations: &[light_programmer::NormalProgrammerValueMutation],
    preset_context: Option<String>,
) -> Result<(), String> {
    let current = state
        .programming
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let registry = state.programming.programmers();
    if current.blind && current.preload_capture_programmer {
        registry.apply_preload_values(
            session.id,
            &light_application::preload_preset_mutations(mutations),
        );
    } else if let Some(context) = preset_context {
        registry
            .apply_normal_preset_recall(session.id, mutations, context)
            .ok_or("programmer does not exist")?;
    } else {
        registry.apply_normal_values(session.id, mutations);
    }
    Ok(())
}

pub(in crate::runtime) fn apply_semantic_aim(
    state: &AppState,
    session: &Session,
    assignments: Vec<(
        light_core::FixtureId,
        light_core::AttributeKey,
        light_core::AttributeValue,
    )>,
    timing: CommandTiming,
    target_number: u32,
) -> Result<(), String> {
    let selection = state
        .programming
        .selection(session.id)
        .ok_or("programmer does not exist")?;
    if selection
        .expression
        .as_ref()
        .is_some_and(|expression| !expression.live_group_owners().is_empty())
    {
        let value = if let Some((_, _, value)) = assignments.first() {
            value.clone()
        } else {
            let Some(intent) = aim_target_intent(state, target_number)? else {
                return Ok(());
            };
            light_core::AttributeValue::Position(Arc::new(intent))
        };
        let attribute = ProgrammingOwner::Position.key();
        let mut preset = light_programmer::Preset {
            family: light_programmer::PresetFamily::Position,
            ..Default::default()
        };
        preset.universal_values.insert(attribute, value);
        let snapshot = state.output.snapshot();
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect();
        let mut mutations = light_application::plan_preset_selection_values(
            &selection,
            &preset,
            &groups,
            &HashMap::new(),
            0,
        )
        .map_err(|error| error.message)?;
        for mutation in &mut mutations {
            if let light_programmer::NormalProgrammerValueMutation::SetFixture {
                timing: planned,
                ..
            }
            | light_programmer::NormalProgrammerValueMutation::SetGroup {
                timing: planned,
                ..
            } = mutation
            {
                *planned = light_programmer::NormalProgrammerValueTiming {
                    fade: timing.fade,
                    fade_millis: timing.fade_millis,
                    delay_millis: timing.delay_millis,
                };
            }
        }
        return apply_position_mutations(state, session, &mutations, None);
    }
    let mutations = assignments
        .into_iter()
        .map(|(fixture_id, attribute, value)| {
            light_programmer::NormalProgrammerValueMutation::SetFixture {
                fixture_id,
                attribute,
                value,
                timing: light_programmer::NormalProgrammerValueTiming {
                    fade: timing.fade,
                    fade_millis: timing.fade_millis,
                    delay_millis: timing.delay_millis,
                },
            }
        })
        .collect::<Vec<_>>();
    apply_position_mutations(state, session, &mutations, None)
}
