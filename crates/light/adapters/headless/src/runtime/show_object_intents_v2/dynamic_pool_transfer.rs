use super::*;

pub(in crate::runtime) fn copy_dynamic_command(
    state: &AppState,
    tokens: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let [source_kind, source, at, destination_kind, destination] = tokens else {
        return Err("expected COPY DYNAMIC <source> AT DYNAMIC <destination>".into());
    };
    if source_kind != "DYNAMIC" || at != "AT" || destination_kind != "DYNAMIC" {
        return Err("expected COPY DYNAMIC <source> AT DYNAMIC <destination>".into());
    }
    let pool_number = |value: &str| {
        value
            .parse::<u16>()
            .ok()
            .filter(|number| (1..=9999).contains(number))
            .ok_or_else(|| "Dynamic pool number must be between 1 and 9999".to_owned())
    };
    let source = pool_number(source)?;
    let destination = pool_number(destination)?;
    let entry = state.active_show.current().ok_or("no show is open")?;
    let (_, objects) = ActiveShowRepository::open(&entry.path)
        .map_err(|error| error.to_string())?
        .objects_with_portable_revision("dynamic")
        .map_err(|error| error.to_string())?;
    let mut found = None;
    for object in objects {
        let definition = decode_dynamic(object.body).map_err(|error| error.message)?;
        if definition.pool_number == source {
            found = Some((definition.id, object.revision));
            break;
        }
    }
    let (id, expected_revision) =
        found.ok_or_else(|| format!("Dynamic {source} does not exist"))?;
    let request = wire::DynamicPoolActionRequest {
        request_id: context
            .request_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        expected_revision,
        pool_number: destination,
    };
    let (_, mutation) = prepare_dynamic_pool_mutation(state, id, entry.id, &request, true)
        .map_err(|error| error.message)?;
    let action = active_show_object_action(context.clone(), entry.id, vec![mutation]);
    let result = run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    for change in &result.changes {
        emit_command_object_changed(
            state,
            &entry,
            change.kind.as_str(),
            &change.object_id,
            change.object_revision,
        );
    }
    Ok(1)
}

pub(in crate::runtime) fn prepare_dynamic_pool_mutation(
    state: &AppState,
    id: Uuid,
    show_id: light_core::ShowId,
    request: &wire::DynamicPoolActionRequest,
    copy: bool,
) -> Result<(Uuid, light_application::ActiveShowObjectMutation), ApiError> {
    ensure_dynamic_pool_slot_free(
        state,
        show_id,
        request.pool_number,
        if copy { None } else { Some(id) },
    )?;
    let mut raw_body = load_body(state, show_id, "dynamic", &id.to_string())?;
    let (object_revision, mut definition) = load_dynamic(state, show_id, id)?;
    if object_revision != request.expected_revision {
        return Err(ApiError::conflict(format!(
            "Dynamic revision conflict: expected {}, current {object_revision}",
            request.expected_revision
        )));
    }
    let (target_id, expected_revision) = if copy {
        let changed_lanes = definition.reidentify(Uuid::new_v4());
        rewrite_dynamic_lane_ids(&mut raw_body, &changed_lanes);
        definition.name = format!("{} Copy", definition.name);
        definition.revision = 1;
        (definition.id, 0)
    } else {
        definition.revision = definition.revision.saturating_add(1);
        (id, request.expected_revision)
    };
    definition.pool_number = request.pool_number;
    let body = if copy {
        let mut body = raw_body;
        let object = body
            .as_object_mut()
            .ok_or_else(|| ApiError::internal("stored Dynamic is not an object"))?;
        object.insert("id".into(), target_id.to_string().into());
        object.insert("pool_number".into(), request.pool_number.into());
        object.insert("revision".into(), 1.into());
        object.insert("name".into(), definition.name.clone().into());
        body
    } else {
        serde_json::to_value(definition).map_err(|error| ApiError::internal(error.to_string()))?
    };
    let mutation = put_active_show_object(
        light_application::ActiveShowObjectKind::Dynamic,
        target_id.to_string(),
        expected_revision,
        body,
    )?;
    Ok((target_id, mutation))
}
