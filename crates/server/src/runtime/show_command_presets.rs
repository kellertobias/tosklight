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
    entry: &ShowEntry,
    store: &ShowStore,
) -> Result<usize, String> {
    let at = body.iter().position(|token| token == "AT");
    let source_address = command_preset_address(at.map_or(body, |index| &body[..index]))?;
    let source = preset_object(store, source_address)?;
    if operation == "DELETE" {
        store
            .delete_object("preset", &source.id)
            .map_err(|error| error.to_string())?;
    } else {
        let at = at.ok_or("MOVE and COPY require AT and a destination number")?;
        if body.len() != at + 2 {
            return Err("preset destination must contain only its new number".into());
        }
        let destination = preset_destination(store, source_address, &body[at + 1])?;
        let destination_id = destination.storage_key();
        let mut destination_body = source.body.clone();
        destination_body["number"] = serde_json::json!(destination.number);
        store
            .put_object("preset", &destination_id, &destination_body, 0)
            .map_err(|error| error.to_string())?;
        if operation == "MOVE" {
            store
                .delete_object("preset", &source.id)
                .map_err(|error| error.to_string())?;
        }
    }
    refresh_command_show(state, entry)?;
    Ok(1)
}

pub(super) fn delete_group_command(
    state: &AppState,
    body: &[String],
    entry: &ShowEntry,
    store: &ShowStore,
    snapshot: &EngineSnapshot,
) -> Result<usize, String> {
    if body.len() != 2 {
        return Err("expected DELETE GROUP <group-number>".into());
    }
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
    store
        .delete_object("group", id)
        .map_err(|error| error.to_string())?;
    refresh_command_show(state, entry)?;
    Ok(1)
}
