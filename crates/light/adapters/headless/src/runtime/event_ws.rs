use super::*;

pub(super) async fn audit_events(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Event>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(state.events.audit_after(query.after)))
}

pub(super) const COMMAND_HISTORY_LIMIT: usize = 50;

pub(super) async fn command_history(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CommandHistoryEntry>>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let entries = state.programming.command_history(session.desk.id);
    Ok(Json(entries))
}
pub(super) async fn clear_programmer(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let actor = authenticate(&state, &headers)?;
    let _activation = state.active_show.acquire().await;
    let session_id = SessionId(id);
    if state.programming.get(session_id).is_none() {
        return Ok(StatusCode::NOT_FOUND);
    }
    // Every connected surface operates the desk's one Programmer, so every one of them is a
    // candidate to carry it after the replacement.
    let connected = state.sessions.sessions();
    let target = light_application::ProgrammingLifecycleTarget::new(
        session_id,
        connected.iter().map(|session| session.desk.id).collect(),
    );
    let context = programming_context(&actor, light_application::ActionSource::Http, None);
    let lifecycle = run_programmer_lifecycle(&state, &actor, &context, target, || {
        replace_programmer_authority(&state, session_id, &connected)
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
    connected: &[Session],
) -> light_application::ProgrammingLifecycleCompletion<Result<StatusCode, ApiError>> {
    let mut replacement_session_id = None;
    let output = (|| {
        if !state.programming.clear(session_id) {
            return Ok(StatusCode::NOT_FOUND);
        }
        if let Err(error) = state.installation.delete_session(session_id) {
            tracing::error!(%error, "failed to remove persisted programmer");
            return Ok(StatusCode::INTERNAL_SERVER_ERROR);
        }
        for session in connected {
            state.programming.start(session.id);
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
    let _activation = state.active_show.acquire().await;
    let context = light_application::ActionContext::operator(
        session.desk.id,
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
