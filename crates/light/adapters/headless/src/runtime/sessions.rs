use super::*;
use crate::tolerant_json::TolerantJson;
use axum::extract::rejection::JsonRejection;
use light_wire::v2::runtime as wire;

pub(super) async fn update_configuration(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut configuration): Json<DeskConfiguration>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    configuration.validate()?;
    let previous = state.installation.configuration();
    if configuration.highlight_look.compatibility
        == light_fixture::HighlightLookCompatibility::Semantic
    {
        configuration.highlight_legacy_overrides_acknowledged |= previous
            .highlight_legacy_overrides_acknowledged
            || previous.highlight_look.compatibility
                != light_fixture::HighlightLookCompatibility::Semantic;
    } else {
        configuration.highlight_legacy_overrides_acknowledged = false;
    }
    let now = application_millis(&state);
    configuration.speed_group_sound_to_light = state.output.configure_speed_groups(
        previous.speed_groups_bpm,
        configuration.speed_groups_bpm,
        configuration.speed_group_sound_to_light,
        now,
    )?;
    state.output.set_frame_rate_hz(configuration.frame_rate_hz);
    state
        .output
        .set_highlight_look(configuration.highlight_look.clone())
        .map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .output
        .configure_timecode(configuration.timecode_sources.clone());
    let requires_restart = configuration.output_bind_ip != previous.output_bind_ip
        || configuration.osc_bind != previous.osc_bind
        || configuration.art_timecode_bind != previous.art_timecode_bind
        || configuration.midi_inputs != previous.midi_inputs
        || configuration.rtp_midi_bind != previous.rtp_midi_bind;
    state
        .installation
        .replace_configuration(configuration.clone());
    if !configuration.patch_preview_highlight_dmx {
        state.highlight.clear_patch_previews();
        sync_highlight_output(&state);
    }
    persist_server_configuration(&state)?;
    refresh_speed_group_engine(&state);
    let matter = refresh_matter_bridge(&state);
    let wire_configuration = wire_configuration_value(&configuration)?;
    emit(
        &state,
        "server_configuration_changed",
        serde_json::json!({"configuration":&wire_configuration,"requires_restart":requires_restart,"matter":&matter}),
    );
    Ok(Json(
        serde_json::json!({"configuration":wire_configuration,"requires_restart":requires_restart,"matter":matter}),
    ))
}
pub(super) async fn create_session(
    State(state): State<AppState>,
    request: Result<TolerantJson<wire::RuntimeSessionCreateRequest>, JsonRejection>,
) -> Result<Json<wire::RuntimeSessionResponse>, ApiError> {
    let TolerantJson(input) = request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    let client_id = input
        .client_id
        .or_else(|| {
            input.desk_id.and_then(|desk_id| {
                state
                    .installation
                    .client_desks()
                    .ok()?
                    .into_iter()
                    .find(|entry| entry.desk.id == desk_id)?
                    .client_id
            })
        })
        .unwrap_or_else(Uuid::new_v4);
    let user = state
        .installation
        .find_user(&input.username)
        .map_err(ApiError::store)?
        .filter(|u| u.enabled)
        .ok_or_else(|| ApiError::not_found("enabled user"))?;
    let desk = state
        .installation
        .resolve_client_desk(client_id, input.desk_id)
        .map_err(ApiError::store)?;
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: Uuid::new_v4().to_string(),
        connected: true,
        desk: desk.clone(),
    };
    let role = input.role.unwrap_or_default();
    let _activation = state.active_show.acquire().await;
    state.sessions.bind_client(session.id, client_id);
    state.sessions.set_role(session.id, role);
    if role.is_read_only() {
        // A read-only visualizer observes the desk. It must not start a programmer, claim the
        // command line, or change desk selection merely by connecting.
        state.sessions.insert_session(session.clone());
    } else {
        let context = programming_context(&session, light_application::ActionSource::Http, None);
        state.programming.run_lifecycle_transition(
            &context,
            user.id,
            || -> Result<(), ApiError> {
                state.programming.start(session.id, user.id);
                attach_session_command_context(&state, &session);
                state.sessions.insert_session(session.clone());
                persist_programmer(&state, &session)
            },
        )?;
    }
    emit(
        &state,
        "session_started",
        serde_json::json!({"session_id":session.id,"user":user.name,"role":role}),
    );
    Ok(Json(wire::RuntimeSessionResponse {
        role,
        session_id: session.id.0,
        client_id,
        token: session.token,
        user: runtime_wire::user(user),
        desk: runtime_wire::desk(desk),
    }))
}
pub(super) async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UserInput>,
) -> Result<(StatusCode, Json<DeskUser>), ApiError> {
    let _session = authenticate(&state, &headers)?;
    let mut user = state
        .installation
        .add_user(&input.name)
        .map_err(ApiError::store)?;
    if !input.enabled {
        user = state
            .installation
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
    let _activation = state.active_show.acquire().await;
    let read_only = state.sessions.role(id).is_read_only();
    let Some(session) = state.sessions.remove_session(id) else {
        return Err(ApiError::not_found("session"));
    };
    if read_only {
        // Nothing was claimed, so nothing has to be released.
        state.integrations.remove_session_suppression(id);
        if let Some(client_id) = state.sessions.unbind_client(id) {
            state
                .installation
                .touch_client(client_id)
                .map_err(ApiError::store)?;
        }
        emit(
            &state,
            "session_disconnected",
            serde_json::json!({"session_id":id}),
        );
        return Ok(StatusCode::NO_CONTENT);
    }
    state.integrations.remove_session_suppression(id);
    if let Some(client_id) = state.sessions.unbind_client(id) {
        state
            .installation
            .touch_client(client_id)
            .map_err(ApiError::store)?;
    }
    let same_context_connected = state.sessions.same_context_connected(&session);
    if !same_context_connected {
        state
            .highlight
            .clear_context(session.desk.id, session.user.id);
        sync_highlight_output(&state);
    }
    state.highlight.remove_patch_preview(id);
    sync_highlight_output(&state);
    file_manager::release_session_input(&state, &session, "session_closed");
    persist_programmer(&state, &session)?;
    let context = programming_context(&session, light_application::ActionSource::Http, None);
    state
        .programming
        .run_lifecycle_transition(&context, session.user.id, || {
            state.programming.disconnect(id);
        });
    emit(
        &state,
        "session_disconnected",
        serde_json::json!({"session_id":id}),
    );
    Ok(StatusCode::NO_CONTENT)
}
