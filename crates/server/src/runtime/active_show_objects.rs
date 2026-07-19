use super::*;

pub(super) fn active_show_object_action(
    context: light_application::ActionContext,
    show_id: light_core::ShowId,
    mutations: Vec<light_application::ActiveShowObjectMutation>,
) -> light_application::ActionEnvelope<light_application::MutateActiveShowObjectsCommand> {
    light_application::ActionEnvelope {
        context,
        command: light_application::MutateActiveShowObjectsCommand { show_id, mutations },
    }
}

pub(super) fn operator_action_context(
    session: &Session,
    source: light_application::ActionSource,
) -> light_application::ActionContext {
    light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        source,
    )
}

pub(super) fn put_active_show_object(
    kind: light_application::ActiveShowObjectKind,
    object_id: impl Into<String>,
    expected_object_revision: u64,
    body: serde_json::Value,
) -> light_application::ActiveShowObjectMutation {
    light_application::ActiveShowObjectMutation {
        kind,
        object_id: object_id.into(),
        expected_object_revision,
        mutation: light_application::ActiveShowObjectMutationKind::Put { body },
    }
}

pub(super) fn delete_active_show_object(
    kind: light_application::ActiveShowObjectKind,
    object_id: impl Into<String>,
    expected_object_revision: u64,
) -> light_application::ActiveShowObjectMutation {
    light_application::ActiveShowObjectMutation {
        kind,
        object_id: object_id.into(),
        expected_object_revision,
        mutation: light_application::ActiveShowObjectMutationKind::Delete,
    }
}

pub(super) fn undo_active_show_object_action(
    context: light_application::ActionContext,
    show_id: light_core::ShowId,
    kind: light_application::ActiveShowObjectKind,
    object_id: impl Into<String>,
    expected_object_revision: u64,
) -> light_application::ActionEnvelope<light_application::UndoActiveShowObjectCommand> {
    light_application::ActionEnvelope {
        context,
        command: light_application::UndoActiveShowObjectCommand {
            show_id,
            kind,
            object_id: object_id.into(),
            expected_object_revision,
        },
    }
}

/// Runs while the caller holds `activation_lock`, keeping the active identity stable through the
/// infallible runtime installation.
pub(super) fn run_active_show_object_action(
    state: &AppState,
    action: light_application::ActionEnvelope<light_application::MutateActiveShowObjectsCommand>,
) -> Result<light_application::MutateActiveShowObjectsResult, ApiError> {
    let ports = ServerActiveShowPorts::show_objects(state.clone());
    state
        .active_show_service
        .mutate_objects(action, &ports)
        .map_err(active_show_object_api_error)
}

pub(super) async fn run_active_show_object_action_async(
    state: &AppState,
    activation: tokio::sync::OwnedMutexGuard<()>,
    action: light_application::ActionEnvelope<light_application::MutateActiveShowObjectsCommand>,
) -> Result<
    (
        light_application::MutateActiveShowObjectsResult,
        tokio::sync::OwnedMutexGuard<()>,
    ),
    ApiError,
> {
    let worker_state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        worker_state.active_show_http_lifecycle.pause_if_armed();
        (
            run_active_show_object_action(&worker_state, action),
            activation,
        )
    })
    .await
    .map_err(|error| ApiError::internal(format!("active-show service task failed: {error}")))?;
    Ok((result.0?, result.1))
}

/// Runs while the caller holds `activation_lock`, keeping the active identity stable through the
/// infallible runtime installation.
pub(super) async fn run_active_show_object_undo_async(
    state: &AppState,
    activation: tokio::sync::OwnedMutexGuard<()>,
    action: light_application::ActionEnvelope<light_application::UndoActiveShowObjectCommand>,
) -> Result<
    (
        light_application::UndoActiveShowObjectResult,
        tokio::sync::OwnedMutexGuard<()>,
    ),
    ApiError,
> {
    let worker_state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let ports = ServerActiveShowPorts::show_objects(worker_state.clone());
        (
            worker_state
                .active_show_service
                .undo_object(action, &ports)
                .map_err(active_show_object_api_error),
            activation,
        )
    })
    .await
    .map_err(|error| ApiError::internal(format!("active-show service task failed: {error}")))?;
    Ok((result.0?, result.1))
}

pub(super) fn reconcile_group_projections(state: &AppState) {
    let groups = state
        .engine
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    state.programmers.refresh_live_selections(&groups);
    let mut reconciled = HashSet::new();
    for session in state.sessions.read().values().cloned().collect::<Vec<_>>() {
        if reconciled.insert((session.desk.id, session.user.id)) {
            reconcile_highlight_selection(state, &session, "show_selection_refresh");
        }
    }
}

fn active_show_object_api_error(error: light_application::ActionError) -> ApiError {
    let status = match error.kind {
        light_application::ActionErrorKind::Invalid => StatusCode::BAD_REQUEST,
        light_application::ActionErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        light_application::ActionErrorKind::Forbidden => StatusCode::FORBIDDEN,
        light_application::ActionErrorKind::NotFound => StatusCode::NOT_FOUND,
        light_application::ActionErrorKind::Conflict | light_application::ActionErrorKind::Busy => {
            StatusCode::CONFLICT
        }
        light_application::ActionErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        light_application::ActionErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiError {
        status,
        message: error.message,
    }
}
