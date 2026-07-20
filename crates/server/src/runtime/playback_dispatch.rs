use super::*;

#[derive(Debug)]
pub(super) struct PlaybackDispatchOutcome {
    pub(super) changed: bool,
    pub(super) persistence_pending: bool,
}

pub(super) struct PlaybackDispatchContext<'a> {
    pub(super) session: Option<&'a Session>,
    pub(super) desk: Option<&'a ControlDesk>,
    pub(super) source: &'a str,
    pub(super) exclusion_zones: &'a [Vec<u16>],
}

/// The one authoritative playback action path for UI, OSC, attached hardware, and deferred
/// preload actions. Desk selection is intentionally context-local; programmer selection remains
/// shared by the registry's user identity.
pub(super) fn dispatch_playback_action(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    action_name: &str,
    input: &PoolPlaybackInput,
    context: PlaybackDispatchContext<'_>,
) -> Result<PlaybackDispatchOutcome, ApiError> {
    let outcome = dispatch_playback_action_inner(state, definition, action_name, input, &context)?;
    if !outcome.released_playbacks.is_empty()
        && let Some(desk) = context.desk
    {
        emit(
            state,
            "playback_exclusion_applied",
            serde_json::json!({"desk_id":desk.id,"activated_playback":definition.number,"released_playbacks":outcome.released_playbacks,"source":context.source}),
        );
    }
    let mut failures = Vec::new();
    if outcome.changed {
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
                "desk_id": context.desk.map(|desk| desk.id),
                "session_id": context.session.map(|session| session.id),
                "playback_number": definition.number,
                "source": context.source,
                "failures": failures.iter().map(|(domain, error)| serde_json::json!({
                    "domain": domain,
                    "error": error,
                })).collect::<Vec<_>>(),
            }),
        );
    }
    Ok(PlaybackDispatchOutcome {
        changed: outcome.changed,
        persistence_pending: !failures.is_empty(),
    })
}

pub(super) fn dispatch_playback_action_inner(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    action_name: &str,
    input: &PoolPlaybackInput,
    context: &PlaybackDispatchContext<'_>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let pressed = input.pressed.unwrap_or(true);
    if matches!(action_name, "master" | "fader") {
        return apply_playback_master(
            state,
            definition,
            input,
            context.source,
            context.exclusion_zones,
        );
    }
    if let Some(outcome) = apply_direct_playback_action(
        state,
        definition,
        action_name,
        input,
        context.exclusion_zones,
    )? {
        return Ok(outcome);
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
        return Ok(PlaybackTargetOutcome::changed(false));
    }
    select_playback_target(state, context.desk, definition, action)?;
    apply_playback_target_action(
        state,
        context.session,
        definition,
        action,
        input,
        pressed,
        context.exclusion_zones,
    )
}
