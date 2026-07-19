use super::*;

pub(super) fn ws_preset_apply(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        #[serde(default)]
        preset_id: Option<String>,
        #[serde(default)]
        family: Option<light_programmer::PresetFamily>,
        #[serde(default)]
        number: Option<u32>,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let requested_address = match (input.family, input.number, input.preset_id.as_deref()) {
        (Some(family), Some(number), _) => light_programmer::PresetAddress::new(family, number)?,
        (_, _, Some(id)) => light_programmer::PresetAddress::parse(id)?,
        _ => return Err("preset.apply requires family and number".into()),
    };
    let storage_key = requested_address.storage_key();
    let active = state
        .active_show
        .read()
        .clone()
        .ok_or("no active show is loaded")?;
    let object = ShowStore::open(&active.path)
        .map_err(|e| e.to_string())?
        .objects("preset")
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|object| {
            object.id == storage_key
                || decode_preset_object(object)
                    .is_ok_and(|(address, _)| address == requested_address)
        })
        .ok_or("preset does not exist")?;
    let (stored_address, preset) = decode_preset_object(&object)?;
    if stored_address != requested_address {
        return Err("stored preset address does not match the requested pool entry".into());
    }
    let group_map = state
        .engine
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let current = state
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    if current.selected.is_empty() {
        return Err("preset recall requires a current selection".into());
    }
    let live_group_targets = match current.selection_expression.clone() {
        Some(light_programmer::SelectionExpression::LiveGroup {
            group_id,
            rule: light_programmer::SelectionRule::All,
        }) => vec![group_id],
        Some(light_programmer::SelectionExpression::Sources { items })
            if items.iter().all(|item| {
                matches!(item, light_programmer::SelectionReference::LiveGroup { .. })
            }) =>
        {
            items
                .into_iter()
                .filter_map(|item| match item {
                    light_programmer::SelectionReference::LiveGroup { group_id } => Some(group_id),
                    _ => None,
                })
                .collect()
        }
        _ => Vec::new(),
    };
    for fixture_id in &current.selected {
        if let Some(attributes) = preset.values.get(fixture_id) {
            for (attribute, value) in attributes {
                state.programmers.set_faded_with_timing(
                    session.id,
                    *fixture_id,
                    attribute.clone(),
                    value.clone(),
                    Some(programmer_fade_millis),
                    None,
                );
            }
        }
        for (group_id, attributes) in preset
            .group_values
            .iter()
            .filter(|(group_id, _)| !live_group_targets.contains(group_id))
        {
            if !light_programmer::resolve_group(group_id, &group_map)
                .is_ok_and(|members| members.contains(fixture_id))
            {
                continue;
            }
            for (attribute, value) in attributes {
                state.programmers.set_faded_with_timing(
                    session.id,
                    *fixture_id,
                    attribute.clone(),
                    value.clone(),
                    Some(programmer_fade_millis),
                    None,
                );
            }
        }
    }
    for group_id in live_group_targets {
        let Some(attributes) = preset.group_values.get(&group_id) else {
            continue;
        };
        for (attribute, value) in attributes {
            state.programmers.set_group_faded_with_timing(
                session.id,
                group_id.clone(),
                attribute.clone(),
                value.clone(),
                Some(programmer_fade_millis),
                None,
            );
        }
    }
    state.programmers.set_modes(
        session.id,
        None,
        None,
        None,
        Some(Some(format!("preset:{}", storage_key))),
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    Ok(
        serde_json::json!({"applied":current.selected.len(),"programmer":state.programmers.get(session.id)}),
    )
}

pub(super) fn ws_programmer_mode(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)]
    struct Input {
        blind: Option<bool>,
        preview: Option<bool>,
        highlight: Option<bool>,
        active_context: Option<Option<String>>,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    state.programmers.set_modes(
        session.id,
        input.blind,
        input.preview,
        None,
        input.active_context,
    );
    persist_programmer(state, session).map_err(|e| e.message)?;
    let mut highlight_state = None;
    if let Some(enabled) = input.highlight {
        let programmer = state
            .programmers
            .get(session.id)
            .ok_or("programmer does not exist")?;
        let selection = state
            .programmers
            .selection(session.id)
            .ok_or("programmer selection does not exist")?;
        let snapshot = state.engine.snapshot();
        let fixtures = highlight_fixture_summaries(&snapshot.fixtures);
        let groups = highlight_groups(&snapshot);
        let transition = state
            .highlight
            .action_guarded(
                session.desk.id,
                session.user.id,
                Some(&session.user.name),
                if enabled {
                    HighlightAction::On
                } else {
                    HighlightAction::Off
                },
                &selection,
                &fixtures,
                &groups,
                programmer.blind || programmer.preview,
            )
            .map_err(|error| error.to_string())?;
        apply_highlight_selection_write(state, session, transition.working_selection.as_ref())
            .map_err(|error| error.message)?;
        sync_highlight_output(state);
        emit(
            state,
            "highlight_changed",
            serde_json::json!({"desk_id":session.desk.id,"user_id":session.user.id,"state":&transition.state}),
        );
        highlight_state = Some(transition.state);
    }
    Ok(serde_json::json!({"updated":true,"highlight":highlight_state}))
}
