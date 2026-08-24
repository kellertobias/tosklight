use super::super::capability_resources::ActiveShowPermit;
use super::super::*;

pub(in crate::runtime) fn output_route_action(
    session: &Session,
    show_id: light_core::ShowId,
    route_id: String,
    expected_object_revision: u64,
    mutation: light_application::OutputRouteMutation,
) -> light_application::ActionEnvelope<light_application::MutateOutputRouteCommand> {
    light_application::ActionEnvelope {
        context: light_application::ActionContext::operator(
            session.desk.id,
            session.id.0,
            light_application::ActionSource::Http,
        ),
        command: light_application::MutateOutputRouteCommand {
            show_id,
            route_id,
            expected_object_revision,
            mutation,
        },
    }
}

pub(in crate::runtime) fn output_route_range_action(
    session: &Session,
    show_id: light_core::ShowId,
    range_id: uuid::Uuid,
    first_route: light_show::LosslessBody<light_output::OutputRoute>,
    logical_universe_end: u16,
    destination_universe_end: u16,
) -> light_application::ActionEnvelope<light_application::CreateOutputRouteRangeCommand> {
    light_application::ActionEnvelope {
        context: light_application::ActionContext::operator(
            session.desk.id,
            session.id.0,
            light_application::ActionSource::Http,
        ),
        command: light_application::CreateOutputRouteRangeCommand {
            show_id,
            range_id,
            first_route,
            logical_universe_end,
            destination_universe_end,
        },
    }
}

pub(in crate::runtime) async fn run_output_route_action(
    state: &AppState,
    activation: ActiveShowPermit,
    action: light_application::ActionEnvelope<light_application::MutateOutputRouteCommand>,
) -> Result<(light_application::MutateOutputRouteResult, ActiveShowPermit), ApiError> {
    let worker_state = state.clone();
    let active_show = state.active_show.clone();
    let show_id = action.command.show_id;
    let (result, activation) = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        worker_state.active_show.pause_http_lifecycle_if_armed();
        let ports = ServerActiveShowPorts::new(worker_state.clone());
        let result = active_show
            .mutate_output_route(action, &ports)
            .inspect(|result| {
                emit_migration_object_changes(&worker_state, show_id, &result.migration_changes);
                emit_migrated_route_changes(&worker_state, show_id, &result.migrated_routes);
            });
        (result, activation)
    })
    .await
    .map_err(|error| ApiError::internal(format!("active-show service task failed: {error}")))?;
    Ok((result.map_err(active_show_api_error)?, activation))
}

pub(in crate::runtime) async fn run_output_route_range_action(
    state: &AppState,
    activation: ActiveShowPermit,
    action: light_application::ActionEnvelope<light_application::CreateOutputRouteRangeCommand>,
) -> Result<
    (
        light_application::CreateOutputRouteRangeResult,
        ActiveShowPermit,
    ),
    ApiError,
> {
    let worker_state = state.clone();
    let active_show = state.active_show.clone();
    let show_id = action.command.show_id;
    let (result, activation) = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        worker_state.active_show.pause_http_lifecycle_if_armed();
        let ports = ServerActiveShowPorts::new(worker_state.clone());
        let result = active_show
            .create_output_route_range(action, &ports)
            .inspect(|result| {
                emit_migration_object_changes(&worker_state, show_id, &result.migration_changes);
                emit_migrated_route_changes(&worker_state, show_id, &result.migrated_routes);
            });
        (result, activation)
    })
    .await
    .map_err(|error| ApiError::internal(format!("active-show service task failed: {error}")))?;
    Ok((result.map_err(active_show_api_error)?, activation))
}

pub(in crate::runtime) async fn terminate_changed_route(
    state: &AppState,
    route: Option<&light_output::OutputRoute>,
) {
    if let Some(route) = route {
        state
            .output
            .terminate_routes(std::slice::from_ref(route))
            .await;
    }
}

fn active_show_api_error(error: light_application::ActionError) -> ApiError {
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
