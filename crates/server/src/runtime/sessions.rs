use super::*;

pub(super) async fn midi_inputs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(
        light_control::available_midi_inputs().map_err(ApiError::internal)?,
    ))
}
pub(super) async fn update_configuration(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut configuration): Json<DeskConfiguration>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    configuration.validate()?;
    let previous = state.configuration.read().clone();
    let now = application_millis(&state);
    {
        let mut controllers = state.speed_groups.lock();
        for index in 0..controllers.len() {
            if configuration.speed_groups_bpm[index] != previous.speed_groups_bpm[index] {
                // A direct value entered through Configuration is the same manual action as the
                // Speed Group UI or OSC surface and therefore takes ownership from Sound.
                unlink_speed_group(&mut controllers, index, now);
                controllers[index]
                    .set_manual_bpm(configuration.speed_groups_bpm[index])
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                controllers[index]
                    .set_speed_master_scale(1.0)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                controllers[index].set_paused_at(false, now);
                configuration.speed_group_sound_to_light[index].enabled = false;
                state.sound_capture_owners.lock()[index] = None;
            } else {
                controllers[index]
                    .set_manual_fallback_bpm(configuration.speed_groups_bpm[index])
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            }
            controllers[index]
                .set_sound_config(configuration.speed_group_sound_to_light[index].clone())
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
    }
    state
        .output_rate
        .store(configuration.frame_rate_hz, Ordering::Relaxed);
    state
        .timecode_router
        .lock()
        .configure(configuration.timecode_sources.clone());
    let requires_restart = configuration.output_bind_ip != previous.output_bind_ip
        || configuration.osc_bind != previous.osc_bind
        || configuration.art_timecode_bind != previous.art_timecode_bind
        || configuration.midi_inputs != previous.midi_inputs
        || configuration.rtp_midi_bind != previous.rtp_midi_bind;
    *state.configuration.write() = configuration.clone();
    if !configuration.patch_preview_highlight_dmx {
        state.patch_preview_highlights.lock().clear();
        sync_highlight_output(&state);
    }
    persist_server_configuration(&state)?;
    refresh_speed_group_engine(&state);
    let matter = refresh_matter_bridge(&state);
    emit(
        &state,
        "server_configuration_changed",
        serde_json::json!({"configuration":configuration,"requires_restart":requires_restart,"matter":&matter}),
    );
    Ok(Json(
        serde_json::json!({"configuration":configuration,"requires_restart":requires_restart,"matter":matter}),
    ))
}
pub(super) async fn create_session(
    State(state): State<AppState>,
    Json(input): Json<CreateSession>,
) -> Result<Json<SessionResponse>, ApiError> {
    let client_id = input
        .client_id
        .or_else(|| {
            input.desk_id.and_then(|desk_id| {
                state
                    .desk
                    .lock()
                    .client_desks()
                    .ok()?
                    .into_iter()
                    .find(|entry| entry.desk.id == desk_id)?
                    .client_id
            })
        })
        .unwrap_or_else(Uuid::new_v4);
    let user = state
        .desk
        .lock()
        .find_user(&input.username)
        .map_err(ApiError::store)?
        .filter(|u| u.enabled)
        .ok_or_else(|| ApiError::not_found("enabled user"))?;
    let desk = state
        .desk
        .lock()
        .resolve_client_desk(client_id, input.desk_id)
        .map_err(ApiError::store)?;
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: Uuid::new_v4().to_string(),
        connected: true,
        desk: desk.clone(),
    };
    let _activation = state.activation_lock.clone().lock_owned().await;
    state.session_clients.write().insert(session.id, client_id);
    state.programmers.start(session.id, user.id);
    attach_session_command_context(&state, &session);
    state.sessions.write().insert(session.id, session.clone());
    persist_programmer(&state, &session)?;
    emit(
        &state,
        "session_started",
        serde_json::json!({"session_id":session.id,"user":user.name}),
    );
    Ok(Json(SessionResponse {
        session_id: session.id,
        client_id,
        token: session.token,
        user,
        desk,
    }))
}
pub(super) async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UserInput>,
) -> Result<(StatusCode, Json<DeskUser>), ApiError> {
    let _session = authenticate(&state, &headers)?;
    let mut user = state
        .desk
        .lock()
        .add_user(&input.name)
        .map_err(ApiError::store)?;
    if !input.enabled {
        user = state
            .desk
            .lock()
            .update_user(user.id, &user.name, false)
            .map_err(ApiError::store)?;
    }
    emit(
        &state,
        "desk_user_changed",
        serde_json::json!({"user":user}),
    );
    Ok((StatusCode::CREATED, Json(user)))
}
pub(super) async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UserInput>,
) -> Result<Json<DeskUser>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let user = state
        .desk
        .lock()
        .update_user(light_core::UserId(id), &input.name, input.enabled)
        .map_err(ApiError::store)?;
    emit(
        &state,
        "desk_user_changed",
        serde_json::json!({"user":user}),
    );
    Ok(Json(user))
}
pub(super) async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let session = authenticate(&state, &headers)?;
    let id = light_core::UserId(id);
    if session.user.id == id {
        return Err(ApiError::conflict(
            "the current session cannot delete its own user",
        ));
    }
    if !state.desk.lock().delete_user(id).map_err(ApiError::store)? {
        return Err(ApiError::not_found("user"));
    }
    state.highlight.clear_user(id);
    sync_highlight_output(&state);
    emit(
        &state,
        "desk_user_deleted",
        serde_json::json!({"user_id":id}),
    );
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn close_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let id = SessionId(id);
    let caller = authenticate(&state, &headers)?;
    if caller.id != id {
        return Err(ApiError::conflict("a session may only disconnect itself"));
    }
    let _activation = state.activation_lock.clone().lock_owned().await;
    let Some(session) = state.sessions.write().remove(&id) else {
        return Err(ApiError::not_found("session"));
    };
    if let Some(client_id) = state.session_clients.write().remove(&id) {
        state
            .desk
            .lock()
            .touch_client(client_id)
            .map_err(ApiError::store)?;
    }
    let same_context_connected = state.sessions.read().values().any(|candidate| {
        candidate.user.id == session.user.id && candidate.desk.id == session.desk.id
    });
    if !same_context_connected {
        state
            .highlight
            .clear_context(session.desk.id, session.user.id);
        sync_highlight_output(&state);
    }
    state.patch_preview_highlights.lock().remove(&id);
    sync_highlight_output(&state);
    file_manager::release_session_input(&state, &session, "session_closed");
    persist_programmer(&state, &session)?;
    state.programmers.disconnect(id);
    emit(
        &state,
        "session_disconnected",
        serde_json::json!({"session_id":id}),
    );
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn remove_client(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let caller = authenticate(&state, &headers)?;
    let target = state
        .desk
        .lock()
        .client_desks()
        .map_err(ApiError::store)?
        .into_iter()
        .find(|entry| entry.desk.id == id)
        .ok_or_else(|| ApiError::not_found("client"))?;
    let target_client_id = target.client_id.unwrap_or(target.desk.id);
    let caller_client_id = state.session_clients.read().get(&caller.id).copied();
    if caller_client_id == Some(target_client_id) || caller.desk.id == id {
        return Err(ApiError::conflict(
            "the current client cannot remove itself",
        ));
    }
    let sessions = state.sessions.read();
    let session_clients = state.session_clients.read();
    if sessions.values().any(|session| {
        session_clients.get(&session.id) == Some(&target_client_id)
            || session.desk.id == target.desk.id
    }) {
        return Err(ApiError::conflict(
            "an actively connected client cannot be removed",
        ));
    }
    drop(sessions);
    drop(session_clients);
    if !state
        .desk
        .lock()
        .remove_client_desk(id)
        .map_err(ApiError::store)?
    {
        return Err(ApiError::not_found("client"));
    }
    state
        .configuration
        .write()
        .update_settings_by_desk
        .remove(&id);
    state.highlight.clear_desk(id);
    sync_highlight_output(&state);
    emit(
        &state,
        "client_removed",
        serde_json::json!({"client_id":target_client_id,"desk_id":id}),
    );
    Ok(StatusCode::NO_CONTENT)
}
