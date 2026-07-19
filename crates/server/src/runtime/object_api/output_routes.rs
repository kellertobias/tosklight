use super::super::*;

pub(super) fn output_route_action(
    session: &Session,
    show_id: light_core::ShowId,
    route_id: String,
    expected_object_revision: u64,
    mutation: light_application::OutputRouteMutation,
) -> light_application::ActionEnvelope<light_application::MutateOutputRouteCommand> {
    light_application::ActionEnvelope {
        context: light_application::ActionContext::operator(
            session.desk.id,
            session.user.id.0,
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

pub(super) async fn run_output_route_action(
    state: &AppState,
    activation: tokio::sync::OwnedMutexGuard<()>,
    action: light_application::ActionEnvelope<light_application::MutateOutputRouteCommand>,
) -> Result<
    (
        light_application::MutateOutputRouteResult,
        tokio::sync::OwnedMutexGuard<()>,
    ),
    ApiError,
> {
    let worker_state = state.clone();
    let service = state.active_show_service.clone();
    let (result, activation) = tokio::task::spawn_blocking(move || {
        let ports = ServerActiveShowPorts::new(worker_state);
        (service.mutate_output_route(action, &ports), activation)
    })
    .await
    .map_err(|error| ApiError::internal(format!("active-show service task failed: {error}")))?;
    Ok((result.map_err(active_show_api_error)?, activation))
}

pub(super) async fn terminate_changed_route(
    state: &AppState,
    route: Option<&light_output::OutputRoute>,
) {
    if let (Some(output), Some(route)) = (&state.network_output, route) {
        let _ = output
            .terminate_routes(
                std::slice::from_ref(route),
                &mut *state.output_sequences.lock().await,
            )
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
