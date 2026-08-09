fn decode_dynamic(value: serde_json::Value) -> Result<light_dynamics::DynamicDefinition, ApiError> {
    serde_json::from_value(value)
        .map_err(|error| ApiError::bad_request(format!("invalid Dynamic definition: {error}")))
}

fn load_dynamic(
    state: &AppState,
    show_id: light_core::ShowId,
    id: Uuid,
) -> Result<(u64, light_dynamics::DynamicDefinition), ApiError> {
    let entry = active_entry(state, show_id)?;
    let (_, object) = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .object_with_portable_revision("dynamic", &id.to_string())
        .map_err(ApiError::store)?;
    let object = object.ok_or_else(|| ApiError::not_found("Dynamic does not exist"))?;
    Ok((object.revision, decode_dynamic(object.body)?))
}

fn ensure_dynamic_pool_slot_free(
    state: &AppState,
    show_id: light_core::ShowId,
    pool_number: u16,
    except: Option<Uuid>,
) -> Result<(), ApiError> {
    if !(1..=9_999).contains(&pool_number) {
        return Err(ApiError::bad_request(
            "Dynamic pool number must be between 1 and 9999",
        ));
    }
    let entry = active_entry(state, show_id)?;
    let (_, objects) = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .objects_with_portable_revision("dynamic")
        .map_err(ApiError::store)?;
    for object in objects {
        let definition = decode_dynamic(object.body)?;
        if definition.pool_number == pool_number && Some(definition.id) != except {
            return Err(ApiError::conflict(format!(
                "Dynamic pool slot {pool_number} is already occupied"
            )));
        }
    }
    Ok(())
}
