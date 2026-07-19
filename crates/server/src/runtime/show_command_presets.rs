use super::*;

fn preset_object(
    store: &ShowStore,
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
    store: &ShowStore,
    source: light_programmer::PresetAddress,
    token: &str,
) -> Result<light_programmer::PresetAddress, String> {
    let destination = light_programmer::PresetAddress::new(
        source.family,
        token
            .parse::<u32>()
            .map_err(|_| "preset destination is invalid")?,
    )?;
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
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry the Preset command".to_owned())?;
    let (entry, store) = active_show_store(state)?;
    let at = body.iter().position(|token| token == "AT");
    let source_address = command_preset_address(at.map_or(body, |index| &body[..index]))?;
    let source_object = preset_object(&store, source_address)?;
    let mutations = if operation == "DELETE" {
        vec![delete_active_show_object(
            light_application::ActiveShowObjectKind::Preset,
            source_object.id,
            source_object.revision,
        )]
    } else {
        let at = at.ok_or("MOVE and COPY require AT and a destination number")?;
        if body.len() != at + 2 {
            return Err("preset destination must contain only its new number".into());
        }
        let destination = preset_destination(&store, source_address, &body[at + 1])?;
        let destination_id = destination.storage_key();
        let mut destination_body = source_object.body.clone();
        destination_body["number"] = serde_json::json!(destination.number);
        let mut mutations = vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Preset,
            destination_id,
            0,
            destination_body,
        )];
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
    run_active_show_object_action(state, action).map_err(|error| error.message)?;
    Ok(1)
}

pub(super) fn delete_group_command(
    state: &AppState,
    body: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    if body.len() != 2 {
        return Err("expected DELETE GROUP <group-number>".into());
    }
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry Delete".to_owned())?;
    let snapshot = state.engine.snapshot();
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
    run_active_show_object_action(state, action).map_err(|error| error.message)?;
    Ok(1)
}
