use super::*;

pub(super) async fn undo_object(
    State(state): State<AppState>,
    Path((id, kind, object_id)): Path<(Uuid, String, String)>,
    headers: HeaderMap,
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
    let activation = state.activation_lock.clone().lock_owned().await;
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id);
    let object_kind = light_application::ActiveShowObjectKind::from_storage_kind(&kind);
    let (revision, event_sequence) = if active && let Some(object_kind) = object_kind {
        let action = undo_active_show_object_action(
            operator_action_context(&session, light_application::ActionSource::Http),
            show_id,
            object_kind,
            object_id.clone(),
            expected,
        );
        let (result, _activation) =
            run_active_show_object_undo_async(&state, activation, action).await?;
        (result.change.object_revision, Some(result.event_sequence))
    } else {
        let revision = ShowStore::open(&entry.path)
            .map_err(ApiError::store)?
            .undo_object(&kind, &object_id, expected)
            .map_err(ApiError::store)?;
        if active {
            install_legacy_undo_runtime(&state, &entry, &kind)?;
        }
        drop(activation);
        (revision, None)
    };
    emit(
        &state,
        "show_object_undone",
        serde_json::json!({"show_id":show_id,"kind":kind,"id":object_id,"revision":revision}),
    );
    Ok(Json(serde_json::json!({
        "revision":revision,
        "event_sequence":event_sequence
    })))
}

fn install_legacy_undo_runtime(
    state: &AppState,
    entry: &ShowEntry,
    kind: &str,
) -> Result<(), ApiError> {
    state
        .engine
        .replace_snapshot(load_engine_snapshot(entry).map_err(ApiError::internal)?)
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
            state
                .engine
                .snapshot()
                .fixtures
                .iter()
                .any(|patched| patched.fixture_id == *fixture && patched.direct_control.is_some())
        });
    }
    Ok(())
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
    let stored_preset = existing
        .as_ref()
        .map(decode_preset_object)
        .transpose()
        .map_err(ApiError::bad_request)?
        .map(|(_, preset)| preset);
    let mut preset = stored_preset
        .clone()
        .unwrap_or_else(|| light_programmer::Preset {
            family: address.family,
            number: address.number,
            ..Default::default()
        });
    let requested = incoming.clone();
    preset.store(incoming, input.mode);
    let body = serialize_preset_request_preserving_extensions(
        existing.as_ref().map(|object| &object.body),
        stored_preset.as_ref(),
        &request_body,
        &requested,
        &preset,
    )
    .map_err(|error| ApiError::internal(error.to_string()))?;
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == show_id);
    let (revision, event_sequence) = if active {
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
        let revision = result
            .changes
            .first()
            .ok_or_else(|| ApiError::internal("Preset Store produced no object change"))?
            .object_revision;
        (revision, Some(result.event_sequence))
    } else {
        backup_show(&state, &entry)?;
        let revision = store
            .put_object("preset", &persisted_key, &body, expected)
            .map_err(ApiError::store)?;
        (revision, None)
    };
    emit(
        &state,
        "preset_stored",
        serde_json::json!({"show_id":show_id,"preset_address":address,"revision":revision,"source_session":session.id}),
    );
    Ok((
        [(header::ETAG, format!("\"{revision}\""))],
        Json(serde_json::json!({
            "revision":revision,
            "event_sequence":event_sequence,
            "preset":preset,
            "source_session":session.id
        })),
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
    let activation = state.activation_lock.clone().lock_owned().await;
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
    let prepared = match input.target.as_str() {
        "preset" => prepare_preload_preset(&store, &input, fixture_values, group_values)?,
        "cue" => prepare_preload_cue(&store, &input, fixture_values, group_values)?,
        _ => return Err(ApiError::bad_request("target must be preset or cue")),
    };
    drop(store);
    let (stored, activation) =
        store_prepared_preload_target(&state, &session, &entry, activation, prepared, expected)
            .await?;
    if use_active_preload {
        let _activation = activation;
        #[cfg(test)]
        {
            let pause = Arc::clone(&state.preload_store_release_lifecycle);
            tokio::task::spawn_blocking(move || pause.pause_if_armed())
                .await
                .expect("Preload Store release pause task failed");
        }
        let context = programming_context(&session, light_application::ActionSource::Http, None);
        run_programming_interaction(
            &state,
            &session,
            &context,
            "http_preload_store",
            ProgrammingLockPolicy::AllowLockedReconciliation,
            || {
                state.programmers.release_preload(session.id);
                persist_programmer(&state, &session)
            },
        )?
        .output?;
    } else {
        drop(activation);
    }
    emit(
        &state,
        "preload_stored",
        serde_json::json!({"session_id":session.id,"target":input.target,"target_id":input.target_id,"revision":stored.revision,"source":if use_active_preload { "active_preload" } else { "pending_preload" }}),
    );
    Ok(Json(serde_json::json!({
        "revision":stored.revision,
        "event_sequence":stored.event_sequence
    })))
}

struct StoredPreloadTarget {
    revision: u64,
    event_sequence: Option<u64>,
}

async fn store_prepared_preload_target(
    state: &AppState,
    session: &Session,
    entry: &ShowEntry,
    activation: tokio::sync::OwnedMutexGuard<()>,
    prepared: PreparedPreloadTarget,
    expected: u64,
) -> Result<(StoredPreloadTarget, tokio::sync::OwnedMutexGuard<()>), ApiError> {
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == entry.id);
    if active {
        let action = active_show_object_action(
            operator_action_context(session, light_application::ActionSource::Http),
            entry.id,
            vec![put_active_show_object(
                prepared.kind,
                prepared.object_id,
                expected,
                prepared.body,
            )],
        );
        let (result, activation) =
            run_active_show_object_action_async(state, activation, action).await?;
        Ok((
            StoredPreloadTarget {
                revision: result
                    .changes
                    .first()
                    .ok_or_else(|| ApiError::internal("Preload Store produced no object change"))?
                    .object_revision,
                event_sequence: Some(result.event_sequence),
            },
            activation,
        ))
    } else {
        backup_show(state, entry)?;
        let revision = ShowStore::open(&entry.path)
            .map_err(ApiError::store)?
            .put_object(
                prepared.kind.as_str(),
                &prepared.object_id,
                &prepared.body,
                expected,
            )
            .map_err(ApiError::store)?;
        Ok((
            StoredPreloadTarget {
                revision,
                event_sequence: None,
            },
            activation,
        ))
    }
}
