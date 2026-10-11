use super::playback_persistence::{PlaybackPersistenceDomain, PlaybackPersistencePlan};
use super::*;

#[path = "playback_dispatch/solo_releases.rs"]
mod solo_releases;

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
    pub(super) control_source: &'a light_playback::PlaybackControlSource,
    pub(super) activation_origin: Option<light_playback::PlaybackActivationOrigin>,
}

/// The one authoritative playback action path for UI, OSC, attached hardware, and deferred
/// preload actions. Desk selection is intentionally context-local; programmer selection remains
/// shared by the registry's user identity.
pub(super) fn dispatch_playback_action(
    state: &AppState,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    action_name: &str,
    input: &PoolPlaybackInput,
    context: PlaybackDispatchContext<'_>,
) -> Result<PlaybackDispatchOutcome, ApiError> {
    let scoped_source = context
        .control_source
        .scoped_to_desk(context.action.desk_id)
        .map_err(ApiError::bad_request)?;
    let context = PlaybackDispatchContext {
        control_source: &scoped_source,
        ..context
    };
    let before_cue = current_cue_id(state, definition);
    let enabled_peers = solo_releases::enabled_peers(state, address);
    let mut outcome =
        dispatch_playback_action_inner(state, address, definition, action_name, input, &context)?;
    outcome
        .released_playbacks
        .extend(solo_releases::released_peers(state, enabled_peers));
    outcome.released_playbacks.sort_unstable();
    outcome.released_playbacks.dedup();
    if outcome.changed {
        dispatch_entered_cue_actions(state, definition, before_cue)?;
    }
    if !outcome.released_playbacks.is_empty()
        && let Some(desk) = context.desk
    {
        emit(
            state,
            "playback_exclusion_applied",
            serde_json::json!({"desk_id":desk.id,"activated_playback":address,"released_playbacks":outcome.released_playbacks,"source":context.source}),
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
                "playback_address": address,
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
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    action_name: &str,
    input: &PoolPlaybackInput,
    context: &PlaybackDispatchContext<'_>,
) -> Result<PlaybackTargetOutcome, ApiError> {
    let pressed = input.pressed.unwrap_or(true);
    if matches!(action_name, "master" | "fader" | "master-absolute") {
        return apply_playback_master(
            state,
            context.action,
            context.session,
            address,
            definition,
            input,
            action_name == "master-absolute",
            context.control_source,
            context.activation_origin,
        );
    }
    if let Some(outcome) = apply_direct_playback_action(
        state,
        address,
        definition,
        action_name,
        input,
        context.control_source,
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
    let selection_changed =
        select_playback_target(state, context.desk, address, definition, action)?;
    let outcome = apply_playback_target_action(
        state,
        context.action,
        context.session,
        address,
        definition,
        action,
        input,
        pressed,
        context.control_source,
        context.activation_origin,
    )?;
    Ok(outcome.combine(PlaybackTargetOutcome::changed(selection_changed)))
}

#[cfg(test)]
#[path = "playback_dispatch/solo_audit_tests.rs"]
mod solo_audit_tests;
