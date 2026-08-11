use super::playback_persistence::{PlaybackPersistenceDomain, PlaybackPersistencePlan};
use super::*;

#[derive(Debug)]
pub(super) struct PlaybackDispatchOutcome {
    pub(super) changed: bool,
    pub(super) addressed_event_required: bool,
    pub(super) persistence_pending: bool,
}

pub(super) struct PlaybackDispatchContext<'a> {
    pub(super) action: &'a light_application::ActionContext,
    pub(super) session: Option<&'a Session>,
    pub(super) desk: Option<&'a ControlDesk>,
    pub(super) source: &'a str,
    pub(super) exclusion_zones: &'a [Vec<u16>],
    pub(super) activation_origin: Option<light_playback::PlaybackActivationOrigin>,
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
    let before_cue = current_cue_id(state, definition);
    let outcome = dispatch_playback_action_inner(state, definition, action_name, input, &context)?;
    if outcome.changed {
        dispatch_entered_cue_actions(state, definition, before_cue)?;
    }
    if !outcome.released_playbacks.is_empty()
        && let Some(desk) = context.desk
    {
        emit(
            state,
            "playback_exclusion_applied",
            serde_json::json!({"desk_id":desk.id,"activated_playback":definition.number,"released_playbacks":outcome.released_playbacks,"source":context.source}),
        );
    }
    let failures = persist_playback_plan(state, outcome.persistence);
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
        addressed_event_required: outcome.addressed_event_required,
        persistence_pending: outcome.persistence_pending || !failures.is_empty(),
    })
}

fn current_cue_id(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
) -> Option<uuid::Uuid> {
    let light_playback::PlaybackTarget::CueList { cue_list_id } = definition.target else {
        return None;
    };
    state
        .output
        .playback_runtime_status_for_cue_list(cue_list_id)
        .and_then(|status| status.playback.current_cue_id)
}

fn dispatch_entered_cue_actions(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
    before: Option<uuid::Uuid>,
) -> Result<(), ApiError> {
    let light_playback::PlaybackTarget::CueList { cue_list_id } = definition.target else {
        return Ok(());
    };
    let Some(current) =
        current_cue_id(state, definition).filter(|current| Some(*current) != before)
    else {
        return Ok(());
    };
    let snapshot = state.output.snapshot();
    let cue = snapshot
        .cue_lists
        .iter()
        .find(|cue_list| cue_list.id == cue_list_id)
        .and_then(|cue_list| cue_list.cues.iter().find(|cue| cue.id == current))
        .ok_or_else(|| ApiError::internal("entered Cue is missing from the runtime snapshot"))?;
    let mut completion = 0;
    for action in &cue.actions {
        completion =
            completion.max(super::timecode_v2::apply_cue_action(state, action)?.unwrap_or(0));
    }
    state
        .output
        .set_cue_external_completion_millis(cue_list_id, completion);
    Ok(())
}

fn persist_playback_plan(
    state: &AppState,
    plan: PlaybackPersistencePlan,
) -> Vec<(&'static str, String)> {
    plan.domains()
        .filter_map(|domain| persist_playback_domain(state, domain).err())
        .collect()
}

fn persist_playback_domain(
    state: &AppState,
    domain: PlaybackPersistenceDomain,
) -> Result<(), (&'static str, String)> {
    let (name, result) = match domain {
        PlaybackPersistenceDomain::ActivePlaybacks => {
            ("active_playbacks", persist_active_playbacks(state))
        }
        PlaybackPersistenceDomain::OutputRuntime => {
            ("output_runtime", persist_output_runtime(state))
        }
    };
    result.map_err(|error| (name, error.message))
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
            context.action,
            context.session,
            definition,
            input,
            context.source,
            context.exclusion_zones,
            context.activation_origin,
        );
    }
    if let Some(outcome) = apply_direct_playback_action(
        state,
        definition,
        action_name,
        input,
        context.exclusion_zones,
        context.activation_origin,
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
    let selection_changed = select_playback_target(state, context.desk, definition, action)?;
    let outcome = apply_playback_target_action(
        state,
        context.action,
        context.session,
        definition,
        action,
        input,
        pressed,
        context.exclusion_zones,
        context.activation_origin,
    )?;
    Ok(outcome.combine(PlaybackTargetOutcome::changed(selection_changed)))
}
