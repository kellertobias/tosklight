use super::*;

#[derive(Debug)]
pub(super) struct PlaybackDispatchOutcome {
    pub(super) changed: bool,
    pub(super) persistence_pending: bool,
}

/// The one authoritative playback action path for UI, OSC, attached hardware, and deferred
/// preload actions. Desk selection is intentionally context-local; programmer selection remains
/// shared by the registry's user identity.
pub(super) fn dispatch_playback_action(
    state: &AppState,
    session: Option<&Session>,
    desk: Option<&ControlDesk>,
    definition: &light_playback::PlaybackDefinition,
    action_name: &str,
    input: &PoolPlaybackInput,
    source: &str,
) -> Result<PlaybackDispatchOutcome, ApiError> {
    let was_enabled =
        state.engine.playback_runtime().iter().any(|playback| {
            playback.playback_number == Some(definition.number) && playback.enabled
        });
    let changed = dispatch_playback_action_inner(
        state,
        session,
        desk,
        definition,
        action_name,
        input,
        source,
    )?;
    let now_enabled =
        state.engine.playback_runtime().iter().any(|playback| {
            playback.playback_number == Some(definition.number) && playback.enabled
        });
    if changed
        && !was_enabled
        && now_enabled
        && let Some(desk) = desk
    {
        let released = enforce_virtual_playback_exclusions(state, desk.id, definition.number);
        if !released.is_empty() {
            emit(
                state,
                "playback_exclusion_applied",
                serde_json::json!({"desk_id":desk.id,"activated_playback":definition.number,"released_playbacks":released,"source":source}),
            );
        }
    }
    let mut failures = Vec::new();
    if changed {
        if let Err(error) = persist_active_playbacks(state) {
            failures.push(("active_playbacks", error.message));
        }
        if let Err(error) = persist_output_runtime(state) {
            failures.push(("output_runtime", error.message));
        }
    }
    if !failures.is_empty() {
        emit(
            state,
            "playback_persistence_pending",
            serde_json::json!({
                "desk_id": desk.map(|desk| desk.id),
                "session_id": session.map(|session| session.id),
                "playback_number": definition.number,
                "source": source,
                "failures": failures.iter().map(|(domain, error)| serde_json::json!({
                    "domain": domain,
                    "error": error,
                })).collect::<Vec<_>>(),
            }),
        );
    }
    Ok(PlaybackDispatchOutcome {
        changed,
        persistence_pending: !failures.is_empty(),
    })
}

pub(super) fn dispatch_playback_action_inner(
    state: &AppState,
    session: Option<&Session>,
    desk: Option<&ControlDesk>,
    definition: &light_playback::PlaybackDefinition,
    action_name: &str,
    input: &PoolPlaybackInput,
    source: &str,
) -> Result<bool, ApiError> {
    let pressed = input.pressed.unwrap_or(true);
    if matches!(action_name, "master" | "fader") {
        return apply_playback_master(state, definition, input, source);
    }
    if let Some(changed) = apply_direct_playback_action(state, definition, action_name, input)? {
        return Ok(changed);
    }
    let action = requested_playback_button_action(definition, action_name, input)?
        .ok_or_else(|| ApiError::not_found("playback action"))?;
    if !pressed
        && !matches!(
            action,
            light_playback::PlaybackButtonAction::Flash
                | light_playback::PlaybackButtonAction::Swap
        )
    {
        return Ok(false);
    }
    select_playback_target(state, desk, definition, action)?;
    apply_playback_target_action(state, session, definition, action, input, pressed)
}
