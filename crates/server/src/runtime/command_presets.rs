use super::*;

/// Return the same normalized first command token used by execution, after removing valid timing
/// clauses. Transport adapters use this to enforce capability ownership without maintaining a
/// second, subtly different command parser.
pub(super) fn normalized_programmer_command_family(
    command_line: &str,
) -> Result<Option<String>, String> {
    tokenize_programmer_command(command_line).map(|(tokens, _)| tokens.into_iter().next())
}

pub(super) fn active_show_store(state: &AppState) -> Result<(ShowEntry, ShowStore), String> {
    let entry = state
        .active_show
        .read()
        .clone()
        .ok_or("no active show is loaded")?;
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    Ok((entry, store))
}

pub(super) fn refresh_command_show(state: &AppState, entry: &ShowEntry) -> Result<(), String> {
    let snapshot = load_engine_snapshot(entry)?;
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    state
        .engine
        .replace_snapshot(snapshot)
        .map_err(|error| error.to_string())?;
    state.programmers.refresh_live_selections(&groups);
    let mut reconciled = HashSet::new();
    for session in state.sessions.read().values().cloned().collect::<Vec<_>>() {
        if reconciled.insert((session.desk.id, session.user.id)) {
            reconcile_highlight_selection(state, &session, "show_selection_refresh");
        }
    }
    Ok(())
}

pub(super) fn emit_command_object_changed(
    state: &AppState,
    entry: &ShowEntry,
    kind: &str,
    id: &str,
    revision: u64,
) {
    emit(
        state,
        "show_object_changed",
        serde_json::json!({"show_id":entry.id,"kind":kind,"id":id,"revision":revision}),
    );
}

pub(super) fn decode_preset_object(
    object: &light_show::VersionedObject,
) -> Result<(light_programmer::PresetAddress, light_programmer::Preset), String> {
    let mut preset: light_programmer::Preset = serde_json::from_value(object.body.clone())
        .map_err(|error| format!("invalid stored preset: {error}"))?;
    let address = preset.reconcile_address(&object.id)?;
    Ok((address, preset))
}

pub(super) fn serialize_preset_preserving_extensions(
    original: &serde_json::Value,
    preset: &light_programmer::Preset,
) -> Result<serde_json::Value, serde_json::Error> {
    let canonical = serde_json::to_value(preset)?;
    let mut merged = original.clone();
    let Some(merged_fields) = merged.as_object_mut() else {
        return Ok(canonical);
    };
    let Some(canonical_fields) = canonical.as_object() else {
        return Ok(canonical);
    };
    for (key, value) in canonical_fields {
        merged_fields.insert(key.clone(), value.clone());
    }
    Ok(merged)
}

pub(super) fn apply_command_preset(
    state: &AppState,
    session: &Session,
    id: &str,
    selected: &[light_core::FixtureId],
) -> Result<(), String> {
    let (_, store) = active_show_store(state)?;
    let requested_address = light_programmer::PresetAddress::parse(id)?;
    let object = store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            object.id == id
                || decode_preset_object(object)
                    .is_ok_and(|(address, _)| address == requested_address)
        })
        .ok_or_else(|| format!("preset {id} does not exist"))?;
    let (_, preset) = decode_preset_object(&object)?;
    let groups = state
        .engine
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let current_expression = state
        .programmers
        .get(session.id)
        .and_then(|programmer| programmer.selection_expression);
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    let live_group_targets = match current_expression {
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
    for fixture in selected {
        if let Some(attributes) = preset.values.get(fixture) {
            for (attribute, value) in attributes {
                state.programmers.set_faded_with_timing(
                    session.id,
                    *fixture,
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
            if light_programmer::resolve_group(group_id, &groups)
                .is_ok_and(|members| members.contains(fixture))
            {
                for (attribute, value) in attributes {
                    state.programmers.set_faded_with_timing(
                        session.id,
                        *fixture,
                        attribute.clone(),
                        value.clone(),
                        Some(programmer_fade_millis),
                        None,
                    );
                }
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
        Some(Some(format!("preset:{id}"))),
    );
    Ok(())
}

pub(super) fn command_preset_address(
    tokens: &[String],
) -> Result<light_programmer::PresetAddress, String> {
    if tokens.len() != 3 || tokens[1] != "." {
        return Err("expected <preset-type> . <preset-number>".into());
    }
    light_programmer::PresetAddress::parse(&format!("{}.{}", tokens[0], tokens[2]))
}

pub(super) fn command_preset_id(tokens: &[String]) -> Result<String, String> {
    Ok(command_preset_address(tokens)?.storage_key())
}

pub(super) fn command_preset_family(id: &str) -> Result<light_programmer::PresetFamily, String> {
    Ok(light_programmer::PresetAddress::parse(id)?.family)
}
