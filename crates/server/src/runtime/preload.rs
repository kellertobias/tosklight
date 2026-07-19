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
    // Match the normal action and render order from show identity through semantic publication.
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry Preload GO".to_owned())?;
    let _ordered = state.playback_service.operation_lock();
    let _serialized = state.playback_action_lock.lock();
    state
        .programmers
        .with_transaction(session.id, || commit_preload_transaction(state, session))
}

pub(super) fn commit_preload_transaction(
    state: &AppState,
    session: &Session,
) -> Result<serde_json::Value, String> {
    let PreparedPreloadCommit {
        pending,
        committed_at,
        programmer_fade_millis,
        staged_playback,
        staged_actions,
        identities,
        before,
        context,
    } = prepare_preload_commit(state, session)?;
    install_preload_commit(state, session, pending, committed_at, staged_playback)?;
    let event_sequences =
        publish_preload_changes(state, &context, &identities, before, &staged_actions)?;
    emit_preload_exclusions(state, session, &staged_actions);
    let executed = executed_preload_actions(staged_actions, committed_at, programmer_fade_millis);
    let warnings = persist_preload_commit(state, session, !executed.is_empty());
    Ok(preload_commit_response(
        state,
        session,
        committed_at,
        programmer_fade_millis,
        executed,
        event_sequences,
        warnings,
    ))
}

type PlaybackIdentity = light_application::PlaybackRuntimeIdentity;
type PlaybackProjection = light_application::PlaybackRuntimeProjection;

struct PreparedPreloadCommit {
    pending: Vec<light_programmer::PreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
    staged_playback: light_playback::PlaybackEngine,
    staged_actions: Vec<StagedPreloadPlaybackAction>,
    identities: Vec<PlaybackIdentity>,
    before: Vec<(PlaybackIdentity, PlaybackProjection)>,
    context: light_application::ActionContext,
}

fn prepare_preload_commit(
    state: &AppState,
    session: &Session,
) -> Result<PreparedPreloadCommit, String> {
    let pending = state
        .programmers
        .get(session.id)
        .ok_or_else(|| "programmer does not exist".to_owned())?
        .preload_playback_pending;
    let snapshot = state.engine.snapshot();
    let definitions = preload_definitions(&pending, &snapshot)?;
    let committed_at = state.programmers.clock().now();
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    let exclusion_zones = virtual_playback_zone_numbers(state, session.desk.id);
    let identities = preload_identities(&definitions, &exclusion_zones);
    let context = light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::UserInterface,
    );
    let before = read_preload_projections(state, &context, &identities)?;
    let (staged_playback, staged_actions) = stage_preload_playback_batch(
        &state.engine.playback().read(),
        &definitions,
        committed_at,
        programmer_fade_millis,
        &exclusion_zones,
    )?;
    Ok(PreparedPreloadCommit {
        pending,
        committed_at,
        programmer_fade_millis,
        staged_playback,
        staged_actions,
        identities,
        before,
        context,
    })
}

fn preload_definitions(
    pending: &[light_programmer::PreloadPlaybackAction],
    snapshot: &EngineSnapshot,
) -> Result<
    Vec<(
        light_programmer::PreloadPlaybackAction,
        light_playback::PlaybackDefinition,
    )>,
    String,
> {
    pending
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
        .collect()
}

fn preload_identities(
    definitions: &[(
        light_programmer::PreloadPlaybackAction,
        light_playback::PlaybackDefinition,
    )],
    exclusion_zones: &[Vec<u16>],
) -> Vec<PlaybackIdentity> {
    let mut identities = definitions
        .iter()
        .map(|(_, definition)| PlaybackIdentity::Playback(definition.number))
        .chain(
            exclusion_zones
                .iter()
                .flatten()
                .copied()
                .map(PlaybackIdentity::Playback),
        )
        .collect::<Vec<_>>();
    identities.sort_by_key(|identity| match identity {
        PlaybackIdentity::Playback(number) => *number,
        PlaybackIdentity::CueList(_) => 0,
    });
    identities.dedup();
    identities
}

fn read_preload_projections(
    state: &AppState,
    context: &light_application::ActionContext,
    identities: &[PlaybackIdentity],
) -> Result<Vec<(PlaybackIdentity, PlaybackProjection)>, String> {
    identities
        .iter()
        .copied()
        .map(|identity| {
            playback_service::read_runtime_projection(state, context, identity)
                .map(|projection| (identity, projection))
                .map_err(|error| error.message)
        })
        .collect()
}

fn install_preload_commit(
    state: &AppState,
    session: &Session,
    pending: Vec<light_programmer::PreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    staged_playback: light_playback::PlaybackEngine,
) -> Result<(), String> {
    let playback = state.engine.playback();
    let mut live_playback = playback.write();

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
    Ok(())
}

fn publish_preload_changes(
    state: &AppState,
    context: &light_application::ActionContext,
    identities: &[PlaybackIdentity],
    before: Vec<(PlaybackIdentity, PlaybackProjection)>,
    actions: &[StagedPreloadPlaybackAction],
) -> Result<Vec<u64>, String> {
    let after = read_preload_projections(state, context, identities)?
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
    Ok(before
        .into_iter()
        .filter_map(|(identity, before)| {
            let projection = after.get(&identity)?.clone();
            state.playback_service.publish_committed_change(
                context,
                preload_event_action(actions, identity),
                None,
                before,
                projection,
            )
        })
        .collect())
}

fn emit_preload_exclusions(
    state: &AppState,
    session: &Session,
    actions: &[StagedPreloadPlaybackAction],
) {
    for action in actions
        .iter()
        .filter(|action| !action.released_playbacks.is_empty())
    {
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

fn executed_preload_actions(
    actions: Vec<StagedPreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
) -> Vec<serde_json::Value> {
    actions
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
        .collect()
}

fn persist_preload_commit(
    state: &AppState,
    session: &Session,
    has_playback_actions: bool,
) -> Vec<String> {
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
    if has_playback_actions {
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
    warnings
}

#[allow(clippy::too_many_arguments)]
fn preload_commit_response(
    state: &AppState,
    session: &Session,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
    executed: Vec<serde_json::Value>,
    playback_event_sequences: Vec<u64>,
    warnings: Vec<String>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "session_id":session.id,
        "application_timestamp":committed_at,
        "programmer_fade_millis":programmer_fade_millis,
        "playback_actions":executed,
        "playback_event_sequences":playback_event_sequences,
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
        "playback_event_sequences":payload["playback_event_sequences"],
        "programmer":state.programmers.get(session.id)
    });
    if let Some(warnings) = payload.get("warnings") {
        response["warnings"] = warnings.clone();
    }
    response
}

fn preload_event_action(
    actions: &[StagedPreloadPlaybackAction],
    identity: light_application::PlaybackRuntimeIdentity,
) -> light_application::PlaybackAction {
    let light_application::PlaybackRuntimeIdentity::Playback(number) = identity else {
        return light_application::PlaybackAction::None { pressed: true };
    };
    if actions
        .iter()
        .any(|action| action.released_playbacks.contains(&number))
    {
        return light_application::PlaybackAction::Off { pressed: true };
    }
    match actions
        .iter()
        .find(|action| action.playback_number == number)
        .map(|action| action.action.as_str())
    {
        Some("toggle") => light_application::PlaybackAction::Toggle { pressed: true },
        Some("go") => light_application::PlaybackAction::Go { pressed: true },
        Some("go-minus") => light_application::PlaybackAction::Back { pressed: true },
        Some("off") => light_application::PlaybackAction::Off { pressed: true },
        Some("on") => light_application::PlaybackAction::On { pressed: true },
        Some("temp-on") => light_application::PlaybackAction::Temporary {
            enabled: true,
            pressed: true,
        },
        Some("temp-off") => light_application::PlaybackAction::Temporary {
            enabled: false,
            pressed: false,
        },
        _ => light_application::PlaybackAction::None { pressed: true },
    }
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
