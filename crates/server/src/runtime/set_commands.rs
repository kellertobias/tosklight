use super::*;

pub(super) fn execute_set_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
) -> Result<usize, String> {
    if tokens.first().is_some_and(|token| token == "GROUP") {
        if tokens.len() != 2 {
            return Err("expected SET GROUP <group-number>".into());
        }
        let group_id = &tokens[1];
        if !state
            .engine
            .snapshot()
            .groups
            .iter()
            .any(|group| &group.id == group_id)
        {
            return Err(format!("group {group_id} does not exist"));
        }
        emit(
            state,
            "group_configuration_requested",
            serde_json::json!({"group_id":group_id,"desk_id":session.desk.id}),
        );
        return Ok(0);
    }
    let at = tokens.iter().position(|token| token == "AT");
    if let Some(at) = at {
        let (entry, store) = active_show_store(state)?;
        if tokens.first().is_some_and(|token| token == "GROUP") {
            return Err(
                "playback pages accept Cuelists only; store the group in a Cuelist first".into(),
            );
        } else {
            let playback = tokens
                .first()
                .ok_or("playback number is required")?
                .parse::<u16>()
                .map_err(|_| "playback number is invalid")?;
            if !state
                .engine
                .snapshot()
                .playbacks
                .iter()
                .any(|item| item.number == playback)
            {
                return Err(format!("Cuelist {playback} does not exist"));
            }
            if !state.engine.snapshot().playbacks.iter().any(|item| {
                item.number == playback
                    && matches!(item.target, light_playback::PlaybackTarget::CueList { .. })
            }) {
                return Err(format!(
                    "Cuelist {playback} cannot be assigned to a playback"
                ));
            }
            let (page, slot) = parse_page_slot(&tokens[at + 1..])?;
            assign_page_slot(&store, &state.engine.snapshot(), page, slot, playback)?;
        }
        refresh_command_show(state, &entry)?;
        return Ok(1);
    }
    let snapshot = state.engine.snapshot();
    let (address, used) = parse_playback_address(tokens, false, &snapshot)?;
    if used != tokens.len() {
        return Err("unexpected tokens after playback selection".into());
    }
    emit(
        state,
        "playback_configuration_requested",
        serde_json::json!({"playback":address.playback,"cue":address.cue}),
    );
    Ok(0)
}

pub(super) fn parse_page_slot(tokens: &[String]) -> Result<(u8, u8), String> {
    if tokens.len() != 3 || tokens[1] != "." {
        return Err("expected <page> . <page-playback>".into());
    }
    Ok((
        tokens[0].parse().map_err(|_| "page number is invalid")?,
        tokens[2]
            .parse()
            .map_err(|_| "page playback number is invalid")?,
    ))
}

pub(super) fn assign_page_slot(
    store: &ShowStore,
    snapshot: &EngineSnapshot,
    page: u8,
    slot: u8,
    playback: u16,
) -> Result<(), String> {
    let object = store
        .objects("playback_page")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == page.to_string());
    let mut definition = if let Some(object) = &object {
        serde_json::from_value::<light_playback::PlaybackPage>(object.body.clone())
            .map_err(|error| error.to_string())?
    } else {
        snapshot
            .playback_pages
            .iter()
            .find(|item| item.number == page)
            .cloned()
            .unwrap_or(light_playback::PlaybackPage {
                number: page,
                name: format!("Page {page}"),
                slots: HashMap::new(),
            })
    };
    definition.slots.insert(slot, playback);
    store
        .put_object(
            "playback_page",
            &page.to_string(),
            &serde_json::to_value(definition).map_err(|error| error.to_string())?,
            object.map_or(0, |object| object.revision),
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}
