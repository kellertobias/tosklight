use super::*;

fn cue_update_request(
    state: &AppState,
    session: &Session,
    body: &[String],
    snapshot: &EngineSnapshot,
    settings: update::UpdateSettings,
) -> Result<UpdateApiRequest, String> {
    let show = state.active_show.read().clone().ok_or("no show is open")?;
    let current_page = state
        .desk
        .lock()
        .desk_page(session.desk.id, show.id)
        .unwrap_or(1);
    let address = parse_update_playback_address(body, current_page, snapshot)?;
    let definition = snapshot
        .playbacks
        .iter()
        .find(|definition| definition.number == address.playback)
        .ok_or_else(|| format!("playback {} does not exist", address.playback))?;
    let light_playback::PlaybackTarget::CueList { cue_list_id } = &definition.target else {
        return Err(format!(
            "playback {} is not assigned to a Cuelist",
            address.playback
        ));
    };
    let explicit = address
        .cue
        .map(|number| {
            snapshot
                .cue_lists
                .iter()
                .find(|list| list.id == *cue_list_id)
                .and_then(|list| list.cues.iter().find(|cue| cue.number == number))
                .map(|cue| (cue.id, cue.number))
                .ok_or_else(|| format!("Cue {number} does not exist"))
        })
        .transpose()?;
    Ok(UpdateApiRequest {
        target: UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(cue_list_id.0.to_string()),
            playback_number: Some(address.playback),
            cue_id: explicit.map(|cue| cue.0),
            cue_number: explicit.map(|cue| cue.1),
            validate_active_context: false,
        },
        mode: update::UpdateMode::Cue(settings.cue_mode),
        expected_revision: None,
        expected_programmer_revision: None,
        expected_show_revision: None,
    })
}

fn object_update_request(
    body: &[String],
    settings: update::UpdateSettings,
) -> Result<UpdateApiRequest, String> {
    let (family, object_id, mode) = if body.first().is_some_and(|token| token == "GROUP") {
        if body.len() != 2 {
            return Err("expected UPDATE GROUP <group-number>".into());
        }
        (
            UpdateApiTargetFamily::Group,
            body[1].clone(),
            update::UpdateMode::ExistingContent(settings.group_mode),
        )
    } else {
        (
            UpdateApiTargetFamily::Preset,
            command_preset_id(body)?,
            update::UpdateMode::ExistingContent(settings.preset_mode),
        )
    };
    Ok(UpdateApiRequest {
        target: UpdateApiTarget {
            family,
            object_id: Some(object_id),
            playback_number: None,
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        },
        mode,
        expected_revision: None,
        expected_programmer_revision: None,
        expected_show_revision: None,
    })
}

pub(super) fn execute_update_show_command(
    state: &AppState,
    session: &Session,
    body: &[String],
    snapshot: &EngineSnapshot,
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let settings = update_settings_for(state, session.desk.id);
    let request = if body.first().is_some_and(|token| token == "SET") {
        cue_update_request(state, session, body, snapshot, settings)?
    } else {
        object_update_request(body, settings)?
    };
    perform_update_from(state, session, &request, context)
        .map(|result| result.changed_count)
        .map_err(|error| error.message)
}
