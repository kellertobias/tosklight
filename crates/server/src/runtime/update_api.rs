use super::*;

pub(super) async fn list_programmers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<light_programmer::ProgrammerState>>, ApiError> {
    let actor = authenticate(&state, &headers)?;
    // Filter session ownership before cloning any complete compatibility row. New clients use
    // narrow scoped projections; this endpoint remains only for authenticated migration callers.
    let programmers = state.programmers.active_for_user_sessions(actor.user.id);
    Ok(Json(programmers))
}

pub(super) fn update_settings_for(state: &AppState, desk_id: Uuid) -> update::UpdateSettings {
    state
        .configuration
        .read()
        .update_settings_by_desk
        .get(&desk_id)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn command_line_arms_update(command_line: &str) -> bool {
    command_line
        .split_whitespace()
        .next()
        .is_some_and(|token| token.eq_ignore_ascii_case("UPDATE"))
}

pub(super) fn emit_update_armed_transition(
    state: &AppState,
    session: &Session,
    was_armed: bool,
    is_armed: bool,
    source: &str,
) {
    if was_armed == is_armed {
        return;
    }
    emit(
        state,
        "update_armed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "armed":is_armed,
            "source":source,
        }),
    );
}

pub(super) async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<update::UpdateSettings>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(update_settings_for(&state, session.desk.id)))
}

pub(super) async fn put_update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(settings): Json<update::UpdateSettings>,
) -> Result<Json<update::UpdateSettings>, ApiError> {
    let session = authenticate(&state, &headers)?;
    state
        .configuration
        .write()
        .update_settings_by_desk
        .insert(session.desk.id, settings.clone());
    persist_server_configuration(&state)?;
    emit(
        &state,
        "update_settings_changed",
        serde_json::json!({"desk_id":session.desk.id,"settings":settings}),
    );
    Ok(Json(settings))
}

pub(super) fn active_update_cue_contexts(state: &AppState) -> Vec<update::ActiveCueContext> {
    state
        .engine
        .active_playbacks()
        .into_iter()
        .filter_map(|playback| {
            Some(update::ActiveCueContext {
                playback_number: playback.playback_number?,
                cue_list_id: playback.cue_list_id,
                cue_id: playback.current_cue_id?,
                cue_number: playback.current_cue_number?,
            })
        })
        .collect()
}

pub(super) fn parse_update_cue_list_id(
    target: &UpdateApiTarget,
) -> Result<light_core::CueListId, ApiError> {
    let id = target
        .object_id
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("Cue Update requires a Cuelist object_id"))?;
    Ok(light_core::CueListId(Uuid::parse_str(id).map_err(
        |_| ApiError::bad_request("Cue Update object_id is not a Cuelist UUID"),
    )?))
}

pub(super) fn resolve_update_cue_target(
    target: &UpdateApiTarget,
    active: &[update::ActiveCueContext],
) -> Result<update::ResolvedCueTarget, ApiError> {
    if target.validate_active_context {
        let playback_number = target.playback_number.ok_or_else(|| {
            ApiError::bad_request("a live Update target requires playback_number")
        })?;
        let cue_list_id = parse_update_cue_list_id(target)?;
        let context = active
            .iter()
            .find(|context| context.playback_number == playback_number)
            .ok_or_else(|| {
                ApiError::conflict("the touched playback is no longer active; preview Update again")
            })?;
        if context.cue_list_id != cue_list_id
            || target.cue_id.is_some_and(|cue_id| context.cue_id != cue_id)
            || target
                .cue_number
                .is_some_and(|number| context.cue_number != number)
        {
            return Err(ApiError::conflict(
                "the touched playback/Cue context changed; preview Update again",
            ));
        }
        return Ok(update::ResolvedCueTarget::from(context));
    }
    let request = if let Some(cue_id) = target.cue_id {
        if let Some(object_id) = target.object_id.as_deref() {
            let cue_list_id = light_core::CueListId(Uuid::parse_str(object_id).map_err(|_| {
                ApiError::bad_request("Cue Update object_id is not a Cuelist UUID")
            })?);
            update::CueTargetRequest::Explicit(update::ResolvedCueTarget {
                cue_list_id,
                playback_number: target.playback_number,
                cue_id,
                cue_number: target.cue_number.unwrap_or_default(),
            })
        } else {
            let context = active
                .iter()
                .find(|context| {
                    context.cue_id == cue_id
                        && target
                            .playback_number
                            .is_none_or(|number| context.playback_number == number)
                })
                .ok_or_else(|| ApiError::bad_request("explicit Cue context is no longer active"))?;
            update::CueTargetRequest::Explicit(update::ResolvedCueTarget::from(context))
        }
    } else if let Some(playback_number) = target.playback_number {
        update::CueTargetRequest::ActivePlayback { playback_number }
    } else {
        update::CueTargetRequest::PoolCueList {
            cue_list_id: parse_update_cue_list_id(target)?,
        }
    };
    update::resolve_cue_target(&request, active).map_err(update_api_error)
}

pub(super) fn update_api_error(error: update::UpdateError) -> ApiError {
    match error {
        update::UpdateError::StaleRevision { .. } => ApiError::conflict(error.to_string()),
        update::UpdateError::MissingTarget { .. } => ApiError::not_found(error.to_string()),
        _ => ApiError::bad_request(error.to_string()),
    }
}

#[cfg(test)]
pub(super) fn stored_update_object(
    store: &ShowStore,
    kind: &str,
    id: &str,
) -> Result<light_show::VersionedObject, ApiError> {
    store
        .objects(kind)
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| object.id == id)
        .ok_or_else(|| ApiError::not_found(format!("{kind} {id}")))
}

#[cfg(test)]
pub(super) fn preview_update_request(
    state: &AppState,
    session: &Session,
    request: &UpdateApiRequest,
) -> Result<UpdatePreviewResponse, ApiError> {
    preview_update_application(state, session, request)
}
