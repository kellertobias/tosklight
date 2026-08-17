use super::*;

fn preset_object(
    store: &ActiveShowRepository,
    address: light_programmer::PresetAddress,
) -> Result<light_show::VersionedObject, String> {
    let requested = address.storage_key();
    store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            object.id == requested
                || decode_preset_object(object).is_ok_and(|(stored, _)| stored == address)
        })
        .ok_or_else(|| format!("preset {requested} does not exist"))
}

fn preset_destination(
    store: &ActiveShowRepository,
    source: light_programmer::PresetAddress,
    destination: light_programmer::PresetAddress,
) -> Result<light_programmer::PresetAddress, String> {
    if destination.family != source.family {
        return Err("Preset Move and Copy must stay in the source Preset family".into());
    }
    if store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .iter()
        .any(|object| {
            object.id == destination.storage_key()
                || decode_preset_object(object).is_ok_and(|(stored, _)| stored == destination)
        })
    {
        return Err(format!(
            "preset {} already exists",
            destination.storage_key()
        ));
    }
    Ok(destination)
}

pub(super) fn execute_preset_mutation(
    state: &AppState,
    operation: &str,
    body: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let (entry, store) = active_show_store(state)?;
    let at = body.iter().position(|token| token == "AT");
    let source_address = mutation_preset_address(at.map_or(body, |index| &body[..index]))?;
    let source_object = preset_object(&store, source_address)?;
    let mutations = if operation == "DELETE" {
        vec![delete_active_show_object(
            light_application::ActiveShowObjectKind::Preset,
            source_object.id,
            source_object.revision,
        )]
    } else {
        let at = at.ok_or("MOVE and COPY require AT and a destination number")?;
        let destination = preset_destination(
            &store,
            source_address,
            mutation_preset_address(&body[at + 1..])?,
        )?;
        let destination_id = destination.storage_key();
        let mut destination_body = source_object.body.clone();
        destination_body["number"] = serde_json::json!(destination.number);
        let mut mutations = vec![
            put_active_show_object(
                light_application::ActiveShowObjectKind::Preset,
                destination_id,
                0,
                destination_body,
            )
            .map_err(|error| error.message)?,
        ];
        if operation == "MOVE" {
            mutations.push(delete_active_show_object(
                light_application::ActiveShowObjectKind::Preset,
                source_object.id,
                source_object.revision,
            ));
        }
        mutations
    };
    let action = active_show_object_action(context.clone(), entry.id, mutations);
    run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    Ok(1)
}

fn mutation_preset_address(tokens: &[String]) -> Result<light_programmer::PresetAddress, String> {
    let [family, keyword, number] = tokens else {
        return Err("Preset Move and Copy require <family> PRESET <number> on both sides".into());
    };
    if keyword != "PRESET" {
        return Err("Preset Move and Copy require PRESET before the number".into());
    }
    let family = match family.as_str() {
        "ALL" => light_programmer::PresetFamily::Mixed,
        "INTENSITY" => light_programmer::PresetFamily::Intensity,
        "COLOR" => light_programmer::PresetFamily::Color,
        "POSITION" => light_programmer::PresetFamily::Position,
        "BEAM" => light_programmer::PresetFamily::Beam,
        _ => return Err(format!("unknown Preset family {family}")),
    };
    light_programmer::PresetAddress::new(
        family,
        number
            .parse::<u32>()
            .map_err(|_| "Preset number is invalid")?,
    )
}

pub(super) fn delete_group_command(
    state: &AppState,
    body: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    if body.len() != 2 {
        return Err("expected DELETE GROUP <group-number>".into());
    }
    let snapshot = state.output.snapshot();
    let (entry, store) = active_show_store(state)?;
    let id = &body[1];
    if let Some(dependent) = snapshot.groups.iter().find(|group| {
        group
            .derived_from
            .as_ref()
            .is_some_and(|derived| &derived.source_group_id == id)
    }) {
        return Err(format!(
            "cannot delete group {id}; derived group {} depends on it",
            dependent.id
        ));
    }
    if !snapshot.groups.iter().any(|group| &group.id == id) {
        return Err(format!("group {id} does not exist"));
    }
    let object = store
        .objects("group")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == *id)
        .ok_or_else(|| format!("group {id} does not exist"))?;
    let action = active_show_object_action(
        context.clone(),
        entry.id,
        vec![delete_active_show_object(
            light_application::ActiveShowObjectKind::Group,
            object.id,
            object.revision,
        )],
    );
    run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    Ok(1)
}

pub(super) fn execute_pool_object_transfer(
    state: &AppState,
    operation: &str,
    body: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let at = body
        .iter()
        .position(|token| token == "AT")
        .ok_or("MOVE and COPY require AT and a destination object")?;
    let source = &body[..at];
    let destination = &body[at + 1..];
    let source_family = source.first().ok_or("source object is required")?;
    let destination_family = destination
        .first()
        .ok_or("destination object is required")?;
    if source_family != destination_family {
        return Err(format!(
            "cannot {operation} {source_family} to incompatible {destination_family}"
        ));
    }
    match source_family.as_str() {
        "GROUP" => transfer_group(state, operation, source, destination, context),
        "CUELIST" => transfer_cuelist(state, operation, source, destination, context),
        family => Err(format!("{family} objects do not support Move or Copy")),
    }
}

fn numbered_object(tokens: &[String], family: &str) -> Result<String, String> {
    let [actual, number] = tokens else {
        return Err(format!("{family} address must be {family} <number>"));
    };
    if actual != family {
        return Err(format!("expected {family}, got {actual}"));
    }
    let parsed = number
        .parse::<u16>()
        .map_err(|_| format!("{family} number is invalid"))?;
    if parsed == 0 || parsed > light_playback::MAX_PLAYBACKS {
        return Err(format!("{family} number must be within 1-1000"));
    }
    Ok(parsed.to_string())
}

fn transfer_group(
    state: &AppState,
    operation: &str,
    source: &[String],
    destination: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let source_id = numbered_object(source, "GROUP")?;
    let destination_id = numbered_object(destination, "GROUP")?;
    let (entry, store) = active_show_store(state)?;
    let groups = store.objects("group").map_err(|error| error.to_string())?;
    let source = groups
        .iter()
        .find(|object| object.id == source_id)
        .ok_or_else(|| format!("Group {source_id} does not exist"))?;
    if groups.iter().any(|object| object.id == destination_id) {
        return Err(format!("Group {destination_id} already exists"));
    }
    if operation == "MOVE" {
        for kind in ["group", "cue_list", "playback", "dynamic"] {
            let referenced = store
                .objects(kind)
                .map_err(|error| error.to_string())?
                .iter()
                .any(|object| {
                    !(kind == "group" && object.id == source_id)
                        && json_has_group_reference(&object.body, &source_id)
                });
            if referenced {
                return Err(format!(
                    "cannot move Group {source_id}; a {kind} object depends on it"
                ));
            }
        }
    }
    let mut destination_body = source.body.clone();
    destination_body["id"] = serde_json::json!(destination_id);
    let mut mutations = vec![
        put_active_show_object(
            light_application::ActiveShowObjectKind::Group,
            destination_id,
            0,
            destination_body,
        )
        .map_err(|error| error.message)?,
    ];
    if operation == "MOVE" {
        mutations.push(delete_active_show_object(
            light_application::ActiveShowObjectKind::Group,
            source.id.clone(),
            source.revision,
        ));
    }
    let action = active_show_object_action(context.clone(), entry.id, mutations);
    run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    Ok(1)
}

fn json_has_group_reference(value: &serde_json::Value, group_id: &str) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            (matches!(key.as_str(), "group_id" | "source_group_id")
                && value.as_str() == Some(group_id))
                || json_has_group_reference(value, group_id)
        }),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| json_has_group_reference(value, group_id)),
        _ => false,
    }
}

fn transfer_cuelist(
    state: &AppState,
    operation: &str,
    source: &[String],
    destination: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let source_number = numbered_object(source, "CUELIST")?;
    let destination_number = numbered_object(destination, "CUELIST")?;
    let (entry, store) = active_show_store(state)?;
    let playbacks = store
        .objects("playback")
        .map_err(|error| error.to_string())?;
    let source_playback = playbacks
        .iter()
        .find(|object| object.id == source_number)
        .ok_or_else(|| format!("Cuelist {source_number} does not exist"))?;
    if playbacks
        .iter()
        .any(|object| object.id == destination_number)
    {
        return Err(format!("Cuelist {destination_number} already exists"));
    }
    let definition: light_playback::PlaybackDefinition =
        serde_json::from_value(source_playback.body.clone())
            .map_err(|error| format!("Cuelist {source_number} playback is invalid: {error}"))?;
    let light_playback::PlaybackTarget::CueList { cue_list_id } = definition.target else {
        return Err(format!("playback {source_number} is not a Cuelist"));
    };
    let cue_lists = store
        .objects("cue_list")
        .map_err(|error| error.to_string())?;
    let source_list = cue_lists
        .iter()
        .find(|object| object.id == cue_list_id.0.to_string())
        .ok_or_else(|| format!("Cuelist {source_number} body does not exist"))?;

    let mut destination_playback = source_playback.body.clone();
    destination_playback["number"] = serde_json::json!(
        destination_number
            .parse::<u16>()
            .map_err(|_| "Cuelist destination is invalid")?
    );
    let mut mutations = Vec::new();
    if operation == "COPY" {
        let new_list_id = light_core::CueListId::new();
        let mut destination_list = source_list.body.clone();
        destination_list["id"] = serde_json::json!(new_list_id);
        remap_copied_cue_ids(&mut destination_list)?;
        destination_playback["target"]["cue_list_id"] = serde_json::json!(new_list_id);
        mutations.push(
            put_active_show_object(
                light_application::ActiveShowObjectKind::CueList,
                new_list_id.0.to_string(),
                0,
                destination_list,
            )
            .map_err(|error| error.message)?,
        );
    }
    mutations.push(
        put_active_show_object(
            light_application::ActiveShowObjectKind::Playback,
            destination_number.clone(),
            0,
            destination_playback,
        )
        .map_err(|error| error.message)?,
    );
    if operation == "MOVE" {
        for page in store
            .objects("playback_page")
            .map_err(|error| error.to_string())?
        {
            let mut body = page.body.clone();
            let mut changed = false;
            if let Some(slots) = body
                .get_mut("slots")
                .and_then(serde_json::Value::as_object_mut)
            {
                for value in slots.values_mut() {
                    if value
                        .as_u64()
                        .is_some_and(|number| number.to_string() == source_number)
                    {
                        *value = serde_json::json!(
                            destination_number
                                .parse::<u16>()
                                .map_err(|_| "Cuelist destination is invalid")?
                        );
                        changed = true;
                    }
                }
            }
            if changed {
                mutations.push(
                    put_active_show_object(
                        light_application::ActiveShowObjectKind::PlaybackPage,
                        page.id,
                        page.revision,
                        body,
                    )
                    .map_err(|error| error.message)?,
                );
            }
        }
        mutations.push(delete_active_show_object(
            light_application::ActiveShowObjectKind::Playback,
            source_playback.id.clone(),
            source_playback.revision,
        ));
    }
    let action = active_show_object_action(context.clone(), entry.id, mutations);
    run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    Ok(1)
}

fn remap_copied_cue_ids(body: &mut serde_json::Value) -> Result<(), String> {
    let cues = body
        .get_mut("cues")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("Cuelist cues are invalid")?;
    let mut replacements = std::collections::HashMap::new();
    for cue in cues.iter_mut() {
        let old = cue
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("Cue id is invalid")?
            .to_owned();
        let new = uuid::Uuid::new_v4().to_string();
        cue["id"] = serde_json::json!(new);
        replacements.insert(old, new);
    }
    for cue in cues {
        remap_cue_reference(cue.get_mut("trigger"), &replacements);
        if let Some(actions) = cue
            .get_mut("actions")
            .and_then(serde_json::Value::as_array_mut)
        {
            for action in actions {
                remap_cue_reference(Some(action), &replacements);
            }
        }
    }
    Ok(())
}

fn remap_cue_reference(
    value: Option<&mut serde_json::Value>,
    replacements: &std::collections::HashMap<String, String>,
) {
    let Some(value) = value else { return };
    let Some(old) = value.get("cue_id").and_then(serde_json::Value::as_str) else {
        return;
    };
    let Some(new) = replacements.get(old) else {
        return;
    };
    value["cue_id"] = serde_json::json!(new);
}
