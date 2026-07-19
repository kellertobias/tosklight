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
pub(super) async fn set_programmer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ProgrammerSet>,
) -> Result<StatusCode, ApiError> {
    let session = authenticate(&state, &headers)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let context = programming_context(&session, light_application::ActionSource::Http, None);
    run_programming_interaction(
        &state,
        &session,
        &context,
        "http",
        ProgrammingLockPolicy::RequireUnlocked,
        || set_programmer_value(&state, &session, input),
    )?
    .output
}

fn set_programmer_value(
    state: &AppState,
    session: &Session,
    input: ProgrammerSet,
) -> Result<StatusCode, ApiError> {
    let before = selection_revision(state, session);
    state.programmers.set(
        session.id,
        input.fixture_id,
        light_core::AttributeKey(input.attribute),
        light_core::AttributeValue::Normalized(input.value),
    );
    if before != selection_revision(state, session) {
        reconcile_highlight_selection(state, session, "legacy_programmer_set");
    }
    persist_programmer(state, session)?;
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id}),
    );
    Ok(StatusCode::NO_CONTENT)
}

fn selection_revision(state: &AppState, session: &Session) -> Option<u64> {
    state
        .programmers
        .interaction_version(session.id)
        .map(|version| version.selection_revision)
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
pub(super) async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let protocols = headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let token = protocols
        .split(',')
        .map(str::trim)
        .find_map(|protocol| protocol.strip_prefix("light.token."))
        .ok_or_else(|| ApiError::unauthorized("WebSocket session token protocol is missing"))?;
    let session = authenticate_token(&state, token)?;
    Ok(ws
        .protocols(["light.v1"])
        .on_upgrade(move |socket| handle_socket(socket, state, session))
        .into_response())
}
pub(super) async fn handle_socket(mut socket: WebSocket, state: AppState, session: Session) {
    {
        let mut connections = state.ws_connections.lock();
        *connections.entry(session.id).or_insert(0) += 1;
    }
    state.programmers.connect(session.id);
    let _ = persist_programmer(&state, &session);
    let mut receiver = state.events.subscribe();
    loop {
        tokio::select! { event = receiver.recv() => match event { Ok(event) => { let Ok(json)=serde_json::to_string(&event) else { continue; }; if socket.send(Message::Text(json.into())).await.is_err() { break; } }, Err(_) => break }, incoming = socket.recv() => match incoming { Some(Ok(Message::Close(_))) | None => break, Some(Ok(Message::Ping(v))) => { let _ = socket.send(Message::Pong(v)).await; }, Some(Ok(Message::Text(text))) => { let response = match serde_json::from_str::<WsCommand>(&text) { Ok(command) => dispatch_ws_command(&state, &session, command), Err(error) => WsResponse { protocol_version: 1, request_id: String::new(), ok: false, revision: state.engine.snapshot().revision, payload: None, error: Some(format!("invalid command envelope: {error}")) } }; let Ok(json)=serde_json::to_string(&response) else { continue; }; if socket.send(Message::Text(json.into())).await.is_err() { break; } }, _ => {} } }
    }
    finish_event_socket(&state, &session);
}

pub(super) fn finish_event_socket(state: &AppState, session: &Session) {
    let disconnected = {
        let mut connections = state.ws_connections.lock();
        if let Some(count) = connections.get_mut(&session.id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                connections.remove(&session.id);
                true
            } else {
                false
            }
        } else {
            false
        }
    };
    if disconnected {
        // An event socket is only one transport attached to the authenticated
        // control-desk session. Short-lived command sockets and browser
        // reconnects must retain the Desk's input context; only close_session
        // ends that session and releases its owned context.
        let _ = persist_programmer(state, session);
    }
}

pub(super) fn lock_live_input(
    state: &AppState,
    session: &Session,
    key: String,
) -> Result<(), String> {
    let now = Instant::now();
    let mut locks = state.input_locks.lock();
    locks.retain(|_, (_, expires)| *expires > now);
    if let Some((owner, _)) = locks.get(&key)
        && *owner != session.user.id
    {
        return Err(format!(
            "input {key} is currently controlled by another user"
        ));
    }
    locks.insert(key, (session.user.id, now + Duration::from_secs(1)));
    Ok(())
}
