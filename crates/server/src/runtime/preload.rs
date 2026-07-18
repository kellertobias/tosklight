use super::*;

#[derive(Debug)]
pub(super) struct StagedPreloadPlaybackAction {
    pub(super) playback_number: u16,
    pub(super) action: String,
    pub(super) surface: String,
    pub(super) released_playbacks: Vec<u16>,
}

pub(super) fn apply_preload_playback_verb(
    playback: &mut light_playback::PlaybackEngine,
    number: u16,
    action: &str,
) -> Result<(), String> {
    match action {
        "toggle" => playback.toggle(number).map(|_| ()),
        "go" => playback.go_playback(number).map(|_| ()),
        "go-minus" => playback.back_playback(number).map(|_| ()),
        "off" => playback.off(number).map(|_| ()),
        "on" => playback.on(number).map(|_| ()),
        "temp-on" => playback.set_temp_button(number, true),
        "temp-off" => playback.set_temp_button(number, false),
        _ => Err(format!("unsupported queued Preload action {action}")),
    }
}

/// Build one complete Preload playback result without changing the live engine. A rejected verb,
/// stale definition, or timing error therefore discards only this clone, even when it follows
/// actions that would otherwise have succeeded.
pub(super) fn stage_preload_playback_batch(
    current: &light_playback::PlaybackEngine,
    definitions: &[(
        light_programmer::PreloadPlaybackAction,
        light_playback::PlaybackDefinition,
    )],
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
    exclusion_zones: &[Vec<u16>],
) -> Result<
    (
        light_playback::PlaybackEngine,
        Vec<StagedPreloadPlaybackAction>,
    ),
    String,
> {
    let mut staged = current.clone();
    let mut actions = Vec::with_capacity(definitions.len());
    for (pending, definition) in definitions {
        let previous = staged
            .runtime()
            .into_iter()
            .find(|playback| playback.playback_number == Some(definition.number))
            .map(|playback| (playback.enabled, playback.master));
        let was_enabled = previous.is_some_and(|(enabled, _)| enabled);

        apply_preload_playback_verb(&mut staged, definition.number, &pending.action)?;
        let now_enabled = staged.runtime().into_iter().any(|playback| {
            playback.playback_number == Some(definition.number) && playback.enabled
        });
        let released_playbacks = if !was_enabled && now_enabled {
            enforce_virtual_playback_exclusions_on(&mut staged, exclusion_zones, definition.number)
        } else {
            Vec::new()
        };
        staged.apply_preload_timing(
            definition.number,
            &pending.action,
            committed_at,
            programmer_fade_millis,
            previous,
        )?;
        actions.push(StagedPreloadPlaybackAction {
            playback_number: definition.number,
            action: pending.action.clone(),
            surface: pending.surface.clone(),
            released_playbacks,
        });
    }
    Ok((staged, actions))
}

pub(super) fn record_preload_persistence_failure(
    state: &AppState,
    session: &Session,
    domain: &str,
    error: ApiError,
) -> String {
    let warning = format!(
        "Preload committed but {domain} persistence failed: {}",
        error.message
    );
    emit(
        state,
        "preload_persistence_failed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "domain":domain,
            "source":"preload",
            "accepted":true,
            "error":error.message,
        }),
    );
    warning
}

pub(super) fn commit_preload(
    state: &AppState,
    session: &Session,
) -> Result<serde_json::Value, String> {
    // Use the same lock ordering as normal playback actions: playback serialization first, then
    // the user's reentrant Programmer transaction gate. This keeps queued actions stable while
    // the candidate playback engine is validated.
    let _serialized = state.playback_action_lock.lock();
    state
        .programmers
        .with_transaction(session.id, || commit_preload_transaction(state, session))
}

pub(super) fn commit_preload_transaction(
    state: &AppState,
    session: &Session,
) -> Result<serde_json::Value, String> {
    let pending = state
        .programmers
        .get(session.id)
        .ok_or_else(|| "programmer does not exist".to_owned())?
        .preload_playback_pending;
    let snapshot = state.engine.snapshot();
    let definitions = pending
        .iter()
        .map(|action| {
            snapshot
                .playbacks
                .iter()
                .find(|definition| definition.number == action.playback_number)
                .cloned()
                .map(|definition| (action.clone(), definition))
                .ok_or_else(|| format!("playback {} no longer exists", action.playback_number))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let committed_at = state.programmers.clock().now();
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    let exclusion_zones = virtual_playback_zone_numbers(state, session.desk.id);
    let playback = state.engine.playback();
    let mut live_playback = playback.write();
    let (staged_playback, staged_actions) = stage_preload_playback_batch(
        &live_playback,
        &definitions,
        committed_at,
        programmer_fade_millis,
        &exclusion_zones,
    )?;

    // Nothing live has changed before this point. The Programmer transaction restores its exact
    // checkpoint if the queue somehow differs despite holding the per-user mutation gate.
    state
        .programmers
        .activate_preload_at(session.id, committed_at);
    let drained = state.programmers.take_preload_playback_actions(session.id);
    if drained != pending {
        return Err("the Preload queue changed while GO was being prepared".into());
    }

    // Publishing the already validated clone is the only live playback mutation and cannot fail.
    // Engine resolution acquires Playback before reading Programmer sources, so retaining this
    // write guard across Programmer activation also exposes the combined result at one render
    // boundary instead of allowing a torn frame between the two domains.
    *live_playback = staged_playback;
    drop(live_playback);

    for action in &staged_actions {
        if !action.released_playbacks.is_empty() {
            emit(
                state,
                "playback_exclusion_applied",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "activated_playback":action.playback_number,
                    "released_playbacks":action.released_playbacks,
                    "source":"preload",
                }),
            );
        }
    }
    let executed = staged_actions
        .into_iter()
        .map(|action| {
            serde_json::json!({
                "playback_number":action.playback_number,
                "action":action.action,
                "surface":action.surface,
                "started_at":committed_at,
                "fallback_millis":programmer_fade_millis
            })
        })
        .collect::<Vec<_>>();

    // Persistence is deliberately downstream of the commit point. A disk/store error is an
    // accepted operation with an explicit warning and audit event, never a false rejection after
    // the live Programmer and Playback states have changed.
    let mut warnings = Vec::new();
    if let Err(error) = persist_programmer(state, session) {
        warnings.push(record_preload_persistence_failure(
            state,
            session,
            "programmer",
            error,
        ));
    }
    if !executed.is_empty() {
        if let Err(error) = persist_active_playbacks(state) {
            warnings.push(record_preload_persistence_failure(
                state,
                session,
                "active playbacks",
                error,
            ));
        }
        if let Err(error) = persist_output_runtime(state) {
            warnings.push(record_preload_persistence_failure(
                state,
                session,
                "output runtime",
                error,
            ));
        }
    }

    let mut payload = serde_json::json!({
        "session_id":session.id,
        "application_timestamp":committed_at,
        "programmer_fade_millis":programmer_fade_millis,
        "playback_actions":executed
    });
    if !warnings.is_empty() {
        payload["warnings"] = serde_json::json!(warnings);
    }
    emit(state, "preload_committed", payload.clone());
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"preload_committed_at":committed_at}),
    );
    if !executed.is_empty() {
        emit(
            state,
            "playback_changed",
            serde_json::json!({"session_id":session.id,"source":"preload","application_timestamp":committed_at,"actions":executed}),
        );
    }
    let mut response = serde_json::json!({
        "active":true,
        "application_timestamp":committed_at,
        "programmer_fade_millis":programmer_fade_millis,
        "playback_actions":payload["playback_actions"],
        "programmer":state.programmers.get(session.id)
    });
    if let Some(warnings) = payload.get("warnings") {
        response["warnings"] = warnings.clone();
    }
    Ok(response)
}

pub(super) fn validate_programmer_attribute_value(
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

pub(super) fn profile_head_owner(
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

pub(super) type ControlActionProgrammerAssignment = (
    light_core::FixtureId,
    light_core::AttributeKey,
    light_core::AttributeValue,
);

pub(super) type ControlActionProgrammerValues = (
    Vec<ControlActionProgrammerAssignment>,
    Option<u64>,
    light_fixture::ControlActionKind,
);

pub(super) fn control_action_programmer_values(
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
