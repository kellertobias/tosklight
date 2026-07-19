use super::*;

pub(super) async fn undo_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let revision = ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .undo_object(&kind, &object_id, expected)
        .map_err(ApiError::store)?;
    if state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id)
    {
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).map_err(ApiError::internal)?)
            .map_err(|error| ApiError::internal(error.to_string()))?;
        if kind == "patched_fixture" {
            state.media_cache.lock().retain_fixtures(
                &state
                    .engine
                    .snapshot()
                    .fixtures
                    .iter()
                    .filter(|fixture| fixture.direct_control.is_some())
                    .map(|fixture| fixture.fixture_id.0.to_string())
                    .collect(),
            );
            state.media_status.write().retain(|fixture, _| {
                state.engine.snapshot().fixtures.iter().any(|patched| {
                    patched.fixture_id == *fixture && patched.direct_control.is_some()
                })
            });
        }
    }
    emit(
        &state,
        "show_object_undone",
        serde_json::json!({"show_id":show_id,"kind":kind,"id":object_id,"revision":revision}),
    );
    Ok(Json(serde_json::json!({"revision":revision})))
}
pub(super) async fn store_preset(
    State(state): State<AppState>,
    Path((id, preset_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(input): Json<PresetStoreInput>,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let activation = state.activation_lock.clone().lock_owned().await;
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let family_supplied = input.preset.get("family").is_some();
    let request_body = input.preset;
    let mut incoming: light_programmer::Preset = serde_json::from_value(request_body.clone())
        .map_err(|error| ApiError::bad_request(format!("invalid incoming preset: {error}")))?;
    let address = light_programmer::PresetAddress::from_storage_key(&preset_id, incoming.family)
        .map_err(ApiError::bad_request)?;
    if !family_supplied {
        incoming.family = address.family;
    }
    if incoming.number != 0 && incoming.number != address.number {
        return Err(ApiError::bad_request(
            "preset body number does not match its pool-local address",
        ));
    }
    if incoming.family != address.family {
        return Err(ApiError::bad_request(
            "preset body family does not match its pool address",
        ));
    }
    incoming.number = address.number;
    let storage_key = address.storage_key();
    let existing = store
        .objects("preset")
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| {
            object.id == storage_key
                || decode_preset_object(object)
                    .is_ok_and(|(stored_address, _)| stored_address == address)
        });
    let persisted_key = existing
        .as_ref()
        .map(|object| object.id.clone())
        .unwrap_or(storage_key);
    let mut preset = existing
        .as_ref()
        .map(decode_preset_object)
        .transpose()
        .map_err(ApiError::bad_request)?
        .map(|(_, preset)| preset)
        .unwrap_or_else(|| light_programmer::Preset {
            family: address.family,
            number: address.number,
            ..Default::default()
        });
    preset.store(incoming, input.mode);
    let body = serialize_preset_preserving_extensions(&request_body, &preset)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id);
    let revision = if active {
        let action = active_show_object_action(
            operator_action_context(&session, light_application::ActionSource::Http),
            show_id,
            vec![put_active_show_object(
                light_application::ActiveShowObjectKind::Preset,
                persisted_key,
                expected,
                body,
            )],
        );
        let (result, _activation) =
            run_active_show_object_action_async(&state, activation, action).await?;
        result.changes[0].object_revision
    } else {
        backup_show(&state, &entry)?;
        store
            .put_object("preset", &persisted_key, &body, expected)
            .map_err(ApiError::store)?
    };
    emit(
        &state,
        "preset_stored",
        serde_json::json!({"show_id":show_id,"preset_address":address,"revision":revision,"source_session":session.id}),
    );
    Ok((
        [(header::ETAG, format!("\"{revision}\""))],
        Json(serde_json::json!({"revision":revision,"preset":preset,"source_session":session.id})),
    )
        .into_response())
}
pub(super) async fn store_preload(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<PreloadStoreInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let expected = parse_if_match(&headers)?;
    let show_id = light_core::ShowId(id);
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let use_active_preload = programmer.preload_pending.is_empty()
        && programmer.preload_group_pending.is_empty()
        && (!programmer.preload_active.is_empty() || !programmer.preload_group_active.is_empty());
    let fixture_values = if use_active_preload {
        &programmer.preload_active
    } else {
        &programmer.preload_pending
    };
    let group_values = if use_active_preload {
        &programmer.preload_group_active
    } else {
        &programmer.preload_group_pending
    };
    if fixture_values.is_empty() && group_values.is_empty() {
        return Err(ApiError::bad_request(
            "the pending and active preload scenes are empty",
        ));
    }
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let revision = match input.target.as_str() {
        "preset" => store_preload_preset(&store, &input, fixture_values, group_values, expected)?,
        "cue" => store_preload_cue(&store, &input, fixture_values, group_values, expected)?,
        _ => return Err(ApiError::bad_request("target must be preset or cue")),
    };
    if state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id)
    {
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).map_err(ApiError::internal)?)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }
    if use_active_preload {
        state.programmers.release_preload(session.id);
        persist_programmer(&state, &session)?;
    }
    emit(
        &state,
        "preload_stored",
        serde_json::json!({"session_id":session.id,"target":input.target,"target_id":input.target_id,"revision":revision,"source":if use_active_preload { "active_preload" } else { "pending_preload" }}),
    );
    Ok(Json(serde_json::json!({"revision":revision})))
}
