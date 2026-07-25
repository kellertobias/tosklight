use super::*;

pub(in crate::runtime) fn handle_subscription_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> bool {
    if address != "/light/subscribe" && address != "/light/unsubscribe" {
        return false;
    }
    let Some(client_id) = osc_string(arguments.first()) else {
        return true;
    };
    if address == "/light/unsubscribe" {
        unsubscribe_osc_client(state, &client_id);
        return true;
    }
    let Some((desk_alias, port, command_source)) = subscription_target(arguments, source) else {
        return true;
    };
    subscribe_osc_client(state, client_id, &desk_alias, port, command_source);
    true
}

fn osc_string(argument: Option<&OscArgument>) -> Option<String> {
    match argument? {
        OscArgument::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn subscription_target(
    arguments: &[OscArgument],
    source: Option<&str>,
) -> Option<(String, u16, SocketAddr)> {
    let desk_alias = osc_string(arguments.get(1))?;
    let port = match arguments.get(2)? {
        OscArgument::Int(value) => u16::try_from(*value).ok()?,
        _ => return None,
    };
    Some((desk_alias, port, source?.parse().ok()?))
}

fn unsubscribe_osc_client(state: &AppState, client_id: &str) {
    let removed = state.osc_subscribers.lock().remove(client_id);
    if let Some(subscriber) = removed {
        state
            .osc_cue_record_suppression
            .lock()
            .remove_source(subscriber.session_id, subscriber.command_source);
        disconnect_orphaned_osc_session(state, subscriber.session_id);
    }
    emit(
        state,
        "hardware_connection_changed",
        serde_json::json!({"connected":!state.osc_subscribers.lock().is_empty()}),
    );
}

fn subscribe_osc_client(
    state: &AppState,
    client_id: String,
    requested_alias: &str,
    port: u16,
    command_source: SocketAddr,
) {
    let mut target = command_source;
    target.set_port(port);
    let Some(desk) = osc_control_desk(state, requested_alias) else {
        return;
    };
    let desk_alias = desk.osc_alias.clone();
    let existing = state.osc_subscribers.lock().get(&client_id).cloned();
    let attached = attached_osc_session(state, desk.id);
    let reusable = reusable_osc_session(state, &client_id, &desk_alias, existing.as_ref());
    let session_id = resolve_osc_session(
        state,
        &desk,
        &desk_alias,
        existing.as_ref(),
        attached,
        reusable,
    );
    replace_osc_subscriber(
        state,
        client_id,
        OscSubscriber {
            desk_alias,
            target,
            command_source,
            session_id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    emit(
        state,
        "hardware_connection_changed",
        serde_json::json!({"connected":true}),
    );
    send_osc_feedback(state, true);
}

fn attached_osc_session(state: &AppState, desk_id: Uuid) -> Option<Session> {
    let session = state
        .sessions
        .read()
        .values()
        .find(|session| {
            session.connected
                && session.desk.id == desk_id
                && state.programmers.get(session.id).is_some()
        })
        .cloned();
    if let Some(session) = &session {
        attach_session_command_context(state, session);
    }
    session
}

fn reusable_osc_session(
    state: &AppState,
    client_id: &str,
    desk_alias: &str,
    existing: Option<&OscSubscriber>,
) -> Option<Session> {
    existing
        .filter(|subscriber| !subscriber.desk_alias.eq_ignore_ascii_case(desk_alias))
        .filter(|subscriber| {
            !state
                .session_clients
                .read()
                .contains_key(&subscriber.session_id)
        })
        .filter(|subscriber| {
            state.osc_subscribers.lock().iter().all(|(id, peer)| {
                id.as_str() == client_id || peer.session_id != subscriber.session_id
            })
        })
        .and_then(|subscriber| state.sessions.read().get(&subscriber.session_id).cloned())
}

fn resolve_osc_session(
    state: &AppState,
    desk: &ControlDesk,
    desk_alias: &str,
    existing: Option<&OscSubscriber>,
    attached: Option<Session>,
    reusable: Option<Session>,
) -> SessionId {
    existing
        .filter(|subscriber| subscriber.desk_alias.eq_ignore_ascii_case(desk_alias))
        .map(|subscriber| subscriber.session_id)
        .or_else(|| attached.map(|session| session.id))
        .or_else(|| reusable.map(|session| reuse_osc_session(state, desk, session)))
        .unwrap_or_else(|| create_osc_session(state, desk))
}

fn reuse_osc_session(state: &AppState, desk: &ControlDesk, mut session: Session) -> SessionId {
    session.desk = desk.clone();
    let context = programming_context(&session, light_application::ActionSource::Osc, None);
    state
        .programming
        .run_lifecycle_transition(&context, session.user.id, || {
            attach_session_command_context(state, &session);
            state.sessions.write().insert(session.id, session.clone());
        });
    session.id
}

fn create_osc_session(state: &AppState, desk: &ControlDesk) -> SessionId {
    let Some(user) = state
        .desk
        .lock()
        .users()
        .ok()
        .and_then(|users| users.into_iter().find(|user| user.enabled))
    else {
        return SessionId::new();
    };
    let id = SessionId::new();
    let session = Session {
        id,
        user: user.clone(),
        token: Uuid::new_v4().to_string(),
        connected: true,
        desk: desk.clone(),
    };
    let context = programming_context(&session, light_application::ActionSource::Osc, None);
    state
        .programming
        .run_lifecycle_transition(&context, user.id, || {
            state.programmers.start(id, user.id);
            attach_session_command_context(state, &session);
            state.sessions.write().insert(id, session);
        });
    id
}

fn replace_osc_subscriber(state: &AppState, client_id: String, subscriber: OscSubscriber) {
    let session_id = subscriber.session_id;
    let replaced = state.osc_subscribers.lock().insert(client_id, subscriber);
    if let Some(replaced) = replaced {
        state
            .osc_cue_record_suppression
            .lock()
            .remove_source(replaced.session_id, replaced.command_source);
        if replaced.session_id != session_id {
            disconnect_orphaned_osc_session(state, replaced.session_id);
        }
    }
}

pub(in crate::runtime) fn disconnect_orphaned_osc_session(state: &AppState, session_id: SessionId) {
    if state.session_clients.read().contains_key(&session_id)
        || state
            .osc_subscribers
            .lock()
            .values()
            .any(|subscriber| subscriber.session_id == session_id)
    {
        return;
    }
    state
        .osc_cue_record_suppression
        .lock()
        .remove_session(session_id);
    let Some(session) = state.sessions.write().remove(&session_id) else {
        return;
    };
    let context = programming_context(&session, light_application::ActionSource::Osc, None);
    state
        .programming
        .run_lifecycle_transition(&context, session.user.id, || {
            state.programmers.disconnect(session_id);
        });
}
