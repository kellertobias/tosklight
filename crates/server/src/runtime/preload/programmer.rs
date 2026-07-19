use super::*;

pub(in crate::runtime) fn validate_programmer_attribute_value(
    value: &light_core::AttributeValue,
) -> Result<(), String> {
    match value {
        light_core::AttributeValue::Normalized(value)
            if !value.is_finite() || !(0.0..=1.0).contains(value) =>
        {
            return Err("normalized value must be within 0-1".into());
        }
        light_core::AttributeValue::Spread(_) => {
            return Err("spread values require a Group programming command".into());
        }
        light_core::AttributeValue::Discrete(value) if value.trim().is_empty() => {
            return Err("discrete value must contain a semantic identifier".into());
        }
        light_core::AttributeValue::ColorXyz(value)
            if !value.x.is_finite()
                || !value.y.is_finite()
                || !value.z.is_finite()
                || value.x < 0.0
                || value.y < 0.0
                || value.z < 0.0 =>
        {
            return Err("XYZ color components must be finite and non-negative".into());
        }
        _ => {}
    }
    Ok(())
}

pub(in crate::runtime) fn profile_head_owner(
    fixture: &light_fixture::PatchedFixture,
    mode: &light_fixture::FixtureMode,
    head_id: Uuid,
) -> Result<light_core::FixtureId, String> {
    let (head_index, head) = mode
        .heads
        .iter()
        .enumerate()
        .find(|(_, head)| head.id == head_id)
        .ok_or("fixture profile channel references a missing head")?;
    if head.master_shared {
        return Ok(fixture.fixture_id);
    }
    fixture
        .logical_heads
        .iter()
        .find(|head| usize::from(head.head_index) == head_index)
        .or_else(|| {
            fixture
                .logical_heads
                .iter()
                .find(|head| usize::from(head.head_index) == head_index + 1)
        })
        .map(|head| head.fixture_id)
        .ok_or_else(|| {
            format!(
                "fixture {} is missing logical head {head_index}",
                fixture.fixture_id.0
            )
        })
}

type ControlActionProgrammerAssignment = (
    light_core::FixtureId,
    light_core::AttributeKey,
    light_core::AttributeValue,
);

pub(in crate::runtime) type ControlActionProgrammerValues = (
    Vec<ControlActionProgrammerAssignment>,
    Option<u64>,
    light_fixture::ControlActionKind,
);

pub(in crate::runtime) fn control_action_programmer_values(
    snapshot: &EngineSnapshot,
    fixture_id: light_core::FixtureId,
    action_id: Uuid,
    active: bool,
) -> Result<ControlActionProgrammerValues, String> {
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| {
            fixture.fixture_id == fixture_id
                || fixture
                    .logical_heads
                    .iter()
                    .any(|head| head.fixture_id == fixture_id)
        })
        .ok_or("fixture does not exist")?;
    let profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .ok_or("fixture does not use a schema-v2 profile")?;
    let mode_id = fixture
        .definition
        .mode_id
        .ok_or("fixture profile mode is unavailable")?;
    let mode = profile
        .mode(mode_id)
        .ok_or("fixture profile mode does not exist")?;
    let action = mode
        .control_actions
        .iter()
        .find(|action| action.id == action_id)
        .ok_or("control action does not exist")?;
    let duration = (active && action.kind == light_fixture::ControlActionKind::TimedPulse)
        .then_some(action.duration_millis.unwrap_or(0));
    let assignments = mode
        .control_action_values(action_id, active)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|(channel_id, value)| {
            let channel = mode
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .ok_or("control action references a missing channel")?;
            Ok((
                profile_head_owner(fixture, mode, channel.head_id)?,
                light_fixture::FixtureMode::control_action_attribute(channel.id),
                light_core::AttributeValue::RawDmxExact(value),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((assignments, duration, action.kind))
}
