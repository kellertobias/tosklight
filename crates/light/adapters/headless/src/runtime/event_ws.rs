use super::*;

pub(super) async fn audit_events(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Event>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(
        state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.revision > query.after)
            .cloned()
            .collect(),
    ))
}

pub(super) const COMMAND_HISTORY_LIMIT: usize = 50;

pub(super) async fn command_history(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CommandHistoryEntry>>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let entries = state
        .command_history
        .lock()
        .get(&session.desk.id)
        .map(|history| history.iter().cloned().collect())
        .unwrap_or_default();
    Ok(Json(entries))
}
pub(super) async fn clear_programmer(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let actor = authenticate(&state, &headers)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let session_id = SessionId(id);
    let Some(programmer) = state.programmers.get(session_id) else {
        return Ok(StatusCode::NOT_FOUND);
    };
    let user_id = programmer.user_id;
    let connected = state
        .sessions
        .read()
        .values()
        .filter(|candidate| candidate.user.id == user_id)
        .cloned()
        .collect::<Vec<_>>();
    let target = light_application::ProgrammingLifecycleTarget::new(
        user_id,
        session_id,
        connected.iter().map(|session| session.desk.id).collect(),
    );
    let context = programming_context(&actor, light_application::ActionSource::Http, None);
    let lifecycle = run_programmer_lifecycle(&state, &actor, &context, target, || {
        replace_programmer_authority(&state, session_id, user_id, &connected)
    })?;
    let status = lifecycle.output?;
    if status != StatusCode::NO_CONTENT {
        return Ok(status);
    }
    emit(
        &state,
        "programmer_cleared",
        serde_json::json!({"session_id":id}),
    );
    Ok(status)
}

fn replace_programmer_authority(
    state: &AppState,
    session_id: SessionId,
    user_id: light_core::UserId,
    connected: &[Session],
) -> light_application::ProgrammingLifecycleCompletion<Result<StatusCode, ApiError>> {
    let mut replacement_session_id = None;
    let output = (|| {
        if !state.programmers.clear(session_id) {
            return Ok(StatusCode::NOT_FOUND);
        }
        if let Err(error) = state.desk.lock().delete_session(session_id) {
            tracing::error!(%error, "failed to remove persisted programmer");
            return Ok(StatusCode::INTERNAL_SERVER_ERROR);
        }
        for session in connected {
            state.programmers.start(session.id, user_id);
            replacement_session_id.get_or_insert(session.id);
            attach_session_command_context(state, session);
            persist_programmer(state, session)?;
        }
        Ok(StatusCode::NO_CONTENT)
    })();
    light_application::ProgrammingLifecycleCompletion::new(output, replacement_session_id)
}
pub(super) async fn update_master(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<MasterInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let command = output_runtime_service::command(input.grand_master, input.blackout)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let context = light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::Http,
    );
    let outcome = output_runtime_service::execute(&state, Some(&session), context, command)?;
    let result = serde_json::json!({
        "grand_master":outcome.projection.grand_master,
        "blackout":outcome.projection.blackout
    });
    if outcome.outcome == light_application::OutputRuntimeOutcome::Applied {
        emit(
            &state,
            "master_changed",
            serde_json::json!({"session_id":session.id,"state":result}),
        );
    }
    Ok(Json(result))
}
