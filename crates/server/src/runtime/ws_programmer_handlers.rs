use super::*;

pub(super) fn ws_programmer_group_set(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        group_id: String,
        attribute: String,
        value: serde_json::Value,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let value = if let Some(value) = input.value.as_f64() {
        light_core::AttributeValue::Normalized(value as f32)
    } else {
        serde_json::from_value::<light_core::AttributeValue>(input.value)
            .map_err(|error| format!("group value is invalid: {error}"))?
    };
    match &value {
        light_core::AttributeValue::Normalized(value)
            if !value.is_finite() || !(0.0..=1.0).contains(value) =>
        {
            return Err("value must be within 0-1".into());
        }
        light_core::AttributeValue::Spread(points)
            if points.len() < 2
                || points
                    .iter()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) =>
        {
            return Err("spread requires at least two values within 0-1".into());
        }
        light_core::AttributeValue::Normalized(_) | light_core::AttributeValue::Spread(_) => {}
        _ => return Err("group value must be normalized or spread".into()),
    }
    if !state
        .engine
        .snapshot()
        .groups
        .iter()
        .any(|group| group.id == input.group_id)
    {
        return Err("group does not exist".into());
    }
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    state.programmers.set_group_faded_with_timing(
        session.id,
        input.group_id,
        light_core::AttributeKey(input.attribute),
        value,
        Some(programmer_fade_millis),
        None,
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_programmer_group_release(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        group_id: String,
        attribute: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    if !state
        .engine
        .snapshot()
        .groups
        .iter()
        .any(|group| group.id == input.group_id)
    {
        return Err("group does not exist".into());
    }
    let released = state.programmers.release_group_attribute(
        session.id,
        &input.group_id,
        &light_core::AttributeKey(input.attribute),
    );
    if released {
        persist_programmer(state, session).map_err(|e| e.message)?;
    }
    Ok(serde_json::json!({"released":released,"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_programmer_priority(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        priority: i16,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    if !state.programmers.set_priority(session.id, input.priority) {
        return Err("programmer does not exist".into());
    }
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_programmer_set(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    let input: ProgrammerSet =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    if !input.value.is_finite() || !(0.0..=1.0).contains(&input.value) {
        return Err("value must be within 0-1".into());
    }
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    state.programmers.set_faded_with_timing(
        session.id,
        input.fixture_id,
        light_core::AttributeKey(input.attribute),
        light_core::AttributeValue::Normalized(input.value),
        Some(programmer_fade_millis),
        None,
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_programmer_set_many(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    let input: ProgrammerSetMany =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let snapshot = state.engine.snapshot();
    let fixture_exists = |fixture_id: light_core::FixtureId| {
        snapshot.fixtures.iter().any(|fixture| {
            fixture.fixture_id == fixture_id
                || fixture
                    .logical_heads
                    .iter()
                    .any(|head| head.fixture_id == fixture_id)
        })
    };
    let mut assignments = Vec::with_capacity(input.assignments.len());
    for assignment in input.assignments {
        if assignment.attribute.trim().is_empty() {
            return Err("attribute is required".into());
        }
        if !assignment.value.is_finite() || !(0.0..=1.0).contains(&assignment.value) {
            return Err("value must be within 0-1".into());
        }
        if !fixture_exists(assignment.fixture_id) {
            return Err("fixture does not exist".into());
        }
        assignments.push((
            assignment.fixture_id,
            light_core::AttributeKey(assignment.attribute),
            light_core::AttributeValue::Normalized(assignment.value),
        ));
    }
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    state.programmers.set_many_faded_with_timing(
        session.id,
        assignments,
        Some(programmer_fade_millis),
        None,
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) fn ws_programmer_set_value(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        fixture_id: light_core::FixtureId,
        attribute: String,
        value: light_core::AttributeValue,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    if input.attribute.trim().is_empty() {
        return Err("attribute is required".into());
    }
    validate_programmer_attribute_value(&input.value)?;
    if !state.engine.snapshot().fixtures.iter().any(|fixture| {
        fixture.fixture_id == input.fixture_id
            || fixture
                .logical_heads
                .iter()
                .any(|head| head.fixture_id == input.fixture_id)
    }) {
        return Err("fixture does not exist".into());
    }
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    state.programmers.set_faded_with_timing(
        session.id,
        input.fixture_id,
        light_core::AttributeKey(input.attribute),
        input.value,
        Some(programmer_fade_millis),
        None,
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(serde_json::json!({"programmer":state.programmers.get(session.id)}))
}

pub(super) struct WsControlActionResult {
    pub(super) payload: serde_json::Value,
    pub(super) transient_changed: bool,
}

pub(super) fn ws_programmer_control_action(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<WsControlActionResult, String> {
    #[derive(Deserialize)]
    struct Input {
        fixture_id: light_core::FixtureId,
        action_id: Uuid,
        active: bool,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let snapshot = state.engine.snapshot();
    let (assignments, pulse_duration, kind) = control_action_programmer_values(
        &snapshot,
        input.fixture_id,
        input.action_id,
        input.active,
    )?;
    let transient_source = format!("fixture-control:{}:{}", input.fixture_id.0, input.action_id);
    let (transient_generation, transient_changed) = match (kind, input.active) {
        (light_fixture::ControlActionKind::Latched, _) => {
            state.programmers.set_many(session.id, assignments);
            persist_programmer(state, session).map_err(|e| e.message)?;
            (None, false)
        }
        (_, true) => {
            let generation = state.programmers.set_transient_action(
                session.id,
                transient_source.clone(),
                assignments,
            );
            (generation, generation.is_some())
        }
        (_, false) => {
            let changed =
                state
                    .programmers
                    .release_transient_action(session.id, &transient_source, None);
            (None, changed)
        }
    };
    if let (Some(duration_millis), Some(generation)) = (pulse_duration, transient_generation) {
        let state = state.clone();
        let session = session.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(duration_millis)).await;
            if !state.programmers.release_transient_action(
                session.id,
                &transient_source,
                Some(generation),
            ) {
                return;
            }
            emit(
                &state,
                "programmer_changed",
                serde_json::json!({
                    "session_id":session.id,
                    "command":"programmer.control_action",
                    "action_id":input.action_id,
                    "active":false,
                    "timed_pulse_complete":true,
                }),
            );
        });
    }
    Ok(WsControlActionResult {
        payload: serde_json::json!({
            "action_id":input.action_id,
            "active":input.active,
            "kind":kind,
            "pulse_duration_millis":pulse_duration,
            "programmer":state.programmers.get(session.id),
        }),
        transient_changed,
    })
}

pub(super) fn ws_preset_generate_fixture_values(
    state: &AppState,
    _session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        fixture_ids: Vec<light_core::FixtureId>,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    generate_profile_presets(state, input.fixture_ids)
}

pub(super) fn ws_programmer_release(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        fixture_id: light_core::FixtureId,
        attribute: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    if !state.engine.snapshot().fixtures.iter().any(|fixture| {
        fixture.fixture_id == input.fixture_id
            || fixture
                .logical_heads
                .iter()
                .any(|head| head.fixture_id == input.fixture_id)
    }) {
        return Err("fixture does not exist".into());
    }
    let released = state.programmers.release_fixture_attribute(
        session.id,
        input.fixture_id,
        &light_core::AttributeKey(input.attribute),
    );
    if released {
        persist_programmer(state, session).map_err(|e| e.message)?;
    }
    Ok(serde_json::json!({"released":released,"programmer":state.programmers.get(session.id)}))
}
