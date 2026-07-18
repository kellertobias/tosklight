use super::*;

pub(super) fn predicted_preload_temp_state(
    state: &AppState,
    session: SessionId,
    number: u16,
) -> bool {
    let mut active = state
        .engine
        .playback()
        .read()
        .runtime_status()
        .into_iter()
        .find(|status| status.playback.playback_number == Some(number))
        .is_some_and(|status| status.temporary_active);
    if let Some(programmer) = state.programmers.get(session) {
        for pending in programmer
            .preload_playback_pending
            .iter()
            .filter(|pending| pending.playback_number == number)
        {
            match pending.action.as_str() {
                "temp-on" => active = true,
                "temp-off" => active = false,
                _ => {}
            }
        }
    }
    active
}

pub(super) fn requested_playback_button_action(
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
) -> Result<Option<light_playback::PlaybackButtonAction>, ApiError> {
    use light_playback::PlaybackButtonAction as Action;
    let mapped = match action {
        "button" => {
            let button = input
                .button
                .ok_or_else(|| ApiError::bad_request("button number is required"))?;
            if button == 0 || button > definition.button_count {
                return Err(ApiError::bad_request(
                    "button is not present on this playback",
                ));
            }
            *definition
                .buttons
                .get(usize::from(button - 1))
                .ok_or_else(|| ApiError::bad_request("button must be within 1-3"))?
        }
        "on" => Action::On,
        "off" => Action::Off,
        "toggle" => Action::Toggle,
        "go" | "go-plus" => Action::Go,
        "go-minus" | "back" => Action::GoMinus,
        "fast-forward" => Action::FastForward,
        "fast-rewind" => Action::FastRewind,
        "flash" => Action::Flash,
        "temp" => Action::Temp,
        "swap" => Action::Swap,
        "select" => Action::Select,
        "select-contents" => Action::SelectContents,
        "select-dereferenced" => Action::SelectDereferenced,
        "learn" => Action::Learn,
        "double" => Action::Double,
        "half" => Action::Half,
        "pause" => Action::Pause,
        "blackout" => Action::Blackout,
        "pause-dynamics" => Action::PauseDynamics,
        "none" => Action::None,
        "master" | "fader" | "go-to" | "load" | "xfade-on" | "xfade-off" => {
            return Ok(None);
        }
        _ => return Err(ApiError::not_found("playback action")),
    };
    Ok(Some(mapped))
}

/// Returns the exact action verb retained by Preload, after resolving a configured physical or
/// virtual button. A configured Temp toggle is canonicalized to its next explicit on/off state;
/// Flash, faders, and implicit fader activation are never representable in the pending list.
#[cfg(test)]
pub(super) fn preload_capture_action(
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
) -> Result<Option<&'static str>, ApiError> {
    preload_capture_action_with_temp_state(definition, action, input, false)
}

pub(super) fn preload_capture_action_with_temp_state(
    definition: &light_playback::PlaybackDefinition,
    action: &str,
    input: &PoolPlaybackInput,
    temp_active: bool,
) -> Result<Option<&'static str>, ApiError> {
    use light_playback::PlaybackButtonAction as Action;
    if action == "temp-on" {
        return Ok(input.pressed.unwrap_or(true).then_some("temp-on"));
    }
    if action == "temp-off" {
        return Ok(Some("temp-off"));
    }
    if !input.pressed.unwrap_or(true) {
        return Ok(None);
    }
    Ok(
        match requested_playback_button_action(definition, action, input)? {
            Some(Action::Toggle) => Some("toggle"),
            Some(Action::Go) => Some("go"),
            Some(Action::GoMinus) => Some("go-minus"),
            Some(Action::Off) => Some("off"),
            Some(Action::On) => Some("on"),
            Some(Action::Temp) => Some(if temp_active { "temp-off" } else { "temp-on" }),
            _ => None,
        },
    )
}

pub(super) fn select_cuelist_contents(
    state: &AppState,
    session: &Session,
    cue_list_id: light_core::CueListId,
) -> Result<(), ApiError> {
    let snapshot = state.engine.snapshot();
    let cue_list = snapshot
        .cue_lists
        .iter()
        .find(|cue_list| cue_list.id == cue_list_id)
        .ok_or_else(|| ApiError::bad_request("playback cue list does not exist"))?;
    let mut fixture_ids = std::collections::HashSet::new();
    let mut group_ids = std::collections::HashSet::new();
    let mut items = Vec::new();
    for cue in &cue_list.cues {
        for change in &cue.changes {
            if fixture_ids.insert(change.fixture_id) {
                items.push(light_programmer::SelectionReference::Fixture {
                    fixture_id: change.fixture_id,
                });
            }
        }
        for change in &cue.group_changes {
            if group_ids.insert(change.group_id.clone()) {
                items.push(light_programmer::SelectionReference::LiveGroup {
                    group_id: change.group_id.clone(),
                });
            }
        }
    }
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let fixtures = light_programmer::resolve_selection_references(&items, &groups);
    state.programmers.select_expression(
        session.id,
        fixtures,
        light_programmer::SelectionExpression::PlaybackContents { items },
    );
    persist_programmer(state, session)?;
    reconcile_highlight_selection(state, session, "playback_contents_selection");
    Ok(())
}

pub(super) fn select_group_playback(
    state: &AppState,
    session: &Session,
    group_id: &str,
    live: bool,
) -> Result<(), ApiError> {
    let groups = state
        .engine
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let fixtures =
        light_programmer::resolve_group(group_id, &groups).map_err(ApiError::bad_request)?;
    if live {
        state.programmers.select_expression(
            session.id,
            fixtures,
            light_programmer::SelectionExpression::LiveGroup {
                group_id: group_id.to_owned(),
                rule: light_programmer::SelectionRule::All,
            },
        );
    } else {
        state.programmers.select(session.id, fixtures);
    }
    persist_programmer(state, session)?;
    reconcile_highlight_selection(state, session, "group_playback_selection");
    Ok(())
}

pub(super) fn set_group_playback_master(
    state: &AppState,
    group_id: &str,
    value: f32,
) -> Result<(), ApiError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ApiError::bad_request("playback master must be within 0-1"));
    }
    let mut next = (*state.engine.snapshot()).clone();
    let group = next
        .groups
        .iter_mut()
        .find(|group| group.id == group_id)
        .ok_or_else(|| ApiError::bad_request("group does not exist"))?;
    group.master = value;
    state
        .engine
        .replace_snapshot(next)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}
