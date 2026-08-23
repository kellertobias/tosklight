use super::capability_resources::ActiveShowPermit;
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
) -> Result<light_application::ActiveShowObjectMutation, ApiError> {
    let body = light_application::ActiveShowObjectBody::decode(kind, body).map_err(|error| {
        ApiError::bad_request(format!("invalid {} body: {error}", kind.as_str()))
    })?;
    Ok(light_application::ActiveShowObjectMutation {
        kind,
        object_id: object_id.into(),
        expected_object_revision,
        mutation: light_application::ActiveShowObjectMutationKind::Put {
            body: Box::new(body),
        },
    })
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

#[cfg(test)]
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

/// Runs while the caller holds an active-show coordinator permit, keeping the active identity
/// stable through the infallible runtime installation.
pub(super) fn run_active_show_object_action(
    state: &AppState,
    action: light_application::ActionEnvelope<light_application::MutateActiveShowObjectsCommand>,
) -> Result<light_application::MutateActiveShowObjectsResult, ApiError> {
    let ports = ServerActiveShowPorts::show_objects(state.clone());
    run_active_show_object_action_with_ports(state, action, &ports)
}

/// Runs a show mutation from inside the desk's existing Programming interaction, so the outer
/// boundary remains the sole publisher of the resulting selection change.
pub(super) fn run_active_show_object_action_in_programming_interaction(
    state: &AppState,
    action: light_application::ActionEnvelope<light_application::MutateActiveShowObjectsCommand>,
) -> Result<light_application::MutateActiveShowObjectsResult, ApiError> {
    let owner = ProgrammingInstallOwner {
        gesture: super::ProgrammingOwnerGesturePolicy::Preserve,
        highlight: super::ProgrammingOwnerHighlightPolicy::DeferToOuterInteraction,
    };
    let ports = ServerActiveShowPorts::show_objects_with_programming_owner(state.clone(), owner);
    run_active_show_object_action_with_ports(state, action, &ports)
}

fn run_active_show_object_action_with_ports(
    state: &AppState,
    action: light_application::ActionEnvelope<light_application::MutateActiveShowObjectsCommand>,
    ports: &ServerActiveShowPorts,
) -> Result<light_application::MutateActiveShowObjectsResult, ApiError> {
    let show_id = action.command.show_id;
    let result = state
        .active_show
        .mutate_objects(action, ports)
        .map_err(active_show_object_api_error)?;
    emit_migration_object_changes(state, show_id, &result.migration_changes);
    emit_migrated_route_changes(state, show_id, &result.migrated_routes);
    Ok(result)
}

/// Publishes the legacy per-object event for compatibility-migration write-backs that rode along
/// a committed mutation, so their revision bumps never stay silent for connected clients.
pub(super) fn emit_migration_object_changes(
    state: &AppState,
    show_id: light_core::ShowId,
    changes: &[light_application::ActiveShowObjectChange],
) {
    for change in changes {
        emit(
            state,
            "show_object_changed",
            serde_json::json!({
                "show_id": show_id,
                "kind": change.kind.as_str(),
                "id": change.object_id,
                "revision": change.object_revision,
                "source": "migration",
            }),
        );
    }
}

/// Publishes the legacy per-object event for route-kind migration write-backs.
pub(super) fn emit_migrated_route_changes(
    state: &AppState,
    show_id: light_core::ShowId,
    changes: &[light_application::OutputRouteChange],
) {
    for change in changes {
        emit(
            state,
            "show_object_changed",
            serde_json::json!({
                "show_id": show_id,
                "kind": "route",
                "id": change.route_id,
                "revision": change.object_revision,
                "source": "migration",
            }),
        );
    }
}

pub(super) async fn run_active_show_object_action_async(
    state: &AppState,
    activation: ActiveShowPermit,
    action: light_application::ActionEnvelope<light_application::MutateActiveShowObjectsCommand>,
) -> Result<
    (
        light_application::MutateActiveShowObjectsResult,
        ActiveShowPermit,
    ),
    ApiError,
> {
    let worker_state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        worker_state.active_show.pause_http_lifecycle_if_armed();
        (
            run_active_show_object_action(&worker_state, action),
            activation,
        )
    })
    .await
    .map_err(|error| ApiError::internal(format!("active-show service task failed: {error}")))?;
    Ok((result.0?, result.1))
}

/// Runs while the caller holds an active-show coordinator permit, keeping the active identity
/// stable through the infallible runtime installation.
#[cfg(test)]
pub(super) async fn run_active_show_object_undo_async(
    state: &AppState,
    activation: ActiveShowPermit,
    action: light_application::ActionEnvelope<light_application::UndoActiveShowObjectCommand>,
) -> Result<
    (
        light_application::UndoActiveShowObjectResult,
        ActiveShowPermit,
    ),
    ApiError,
> {
    let worker_state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let ports = ServerActiveShowPorts::show_objects(worker_state.clone());
        let show_id = action.command.show_id;
        let undone = worker_state
            .active_show
            .undo_object(action, &ports)
            .map_err(active_show_object_api_error)
            .inspect(|result| {
                emit_migration_object_changes(&worker_state, show_id, &result.migration_changes);
                emit_migrated_route_changes(&worker_state, show_id, &result.migrated_routes);
            });
        (undone, activation)
    })
    .await
    .map_err(|error| ApiError::internal(format!("active-show service task failed: {error}")))?;
    Ok((result.0?, result.1))
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
