//! Resolve an Aim operand once into requested Position, never achieved channels.
use super::*;
use light_core::programming::{PositionIntent, ProgrammingOwner};

pub(in crate::runtime) fn target_intent_from_frame(
    source: &light_engine::ObservedSourceFrame,
    number: u32,
) -> Result<Option<PositionIntent>, String> {
    light_engine::aim_target_from_geometry(
        &source.snapshot().fixtures, source.points(), source.mounts(), number,
    )
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
