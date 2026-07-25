use super::*;

pub(super) fn compatibility_action(
    payload: serde_json::Value,
    result: light_application::ProgrammingPreloadLifecycleResult,
) -> WsTypedProgrammingAction {
    let no_change = result.state == light_application::ProgrammingPreloadLifecycleState::NoChange;
    WsTypedProgrammingAction {
        payload,
        interaction_changed: result.interaction_event_sequence.is_some(),
        values_changed: false,
        preload_values_changed: result.values_event_sequence.is_some(),
        preload_queue_changed: result.queue_event_sequence.is_some(),
        replayed: result.replayed || no_change,
    }
}

pub(super) fn compatibility_go_payload(
    result: &light_application::ProgrammingPreloadLifecycleResult,
) -> Result<serde_json::Value, String> {
    let commit = result
        .commit
        .as_ref()
        .ok_or("Preload GO completed without commit metadata")?;
    let actions = commit
        .executed
        .iter()
        .map(|action| playback_action(action, commit))
        .collect::<Vec<_>>();
    let sequences = commit
        .runtime_changes
        .iter()
        .map(|change| change.event_sequence)
        .collect::<Vec<_>>();
    let mut payload = serde_json::json!({
        "active":true,
        "application_timestamp":commit.committed_at,
        "programmer_fade_millis":commit.programmer_fade_millis,
        "playback_actions":actions,
        "playback_event_sequences":sequences,
    });
    if !commit.warnings.is_empty() {
        payload["warnings"] = serde_json::json!(commit.warnings);
    }
    Ok(payload)
}

fn playback_action(
    action: &light_application::ProgrammingPreloadExecutedPlaybackAction,
    commit: &light_application::ProgrammingPreloadCommitResult,
) -> serde_json::Value {
    let mut value = serde_json::json!({
        "playback_number":action.playback_number,
        "action":action_name(action.action),
        "surface":surface_name(action.surface),
        "started_at":commit.committed_at,
        "fallback_millis":commit.programmer_fade_millis,
    });
    if let Some(page) = action.page {
        value["page"] = page.into();
    }
    value
}

pub(super) fn emit_compatibility_go(
    state: &AppState,
    session: &Session,
    payload: &serde_json::Value,
) {
    let mut event = payload.clone();
    event["session_id"] = serde_json::json!(session.id);
    emit(state, "preload_committed", event);
    let actions = &payload["playback_actions"];
    if actions
        .as_array()
        .is_some_and(|actions| !actions.is_empty())
    {
        emit(
            state,
            "playback_changed",
            serde_json::json!({
                "session_id":session.id,
                "source":"preload",
                "application_timestamp":payload["application_timestamp"],
                "actions":actions,
            }),
        );
    }
}

const fn action_name(action: light_application::ProgrammingPreloadPlaybackAction) -> &'static str {
    use light_application::ProgrammingPreloadPlaybackAction as Action;
    match action {
        Action::Toggle => "toggle",
        Action::Go => "go",
        Action::Back => "go-minus",
        Action::Off => "off",
        Action::On => "on",
        Action::TemporaryOn => "temp-on",
        Action::TemporaryOff => "temp-off",
    }
}

const fn surface_name(
    surface: light_application::ProgrammingPreloadPlaybackSurface,
) -> &'static str {
    use light_application::ProgrammingPreloadPlaybackSurface as Surface;
    match surface {
        Surface::Physical => "physical",
        Surface::Virtual => "virtual",
        Surface::Osc => "osc",
        Surface::Matter => "matter",
    }
}
