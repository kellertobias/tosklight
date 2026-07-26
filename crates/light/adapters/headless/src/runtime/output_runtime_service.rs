use super::{ApiError, AppState, Session, emit, persist_output_runtime};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, OutputLevel,
    OutputRuntimeApplication, OutputRuntimeCommand, OutputRuntimeDurability, OutputRuntimeIdentity,
    OutputRuntimePorts, OutputRuntimeProjection, OutputRuntimeResult, OutputRuntimeScope,
    OutputRuntimeSnapshot,
};
use uuid::Uuid;

pub(super) fn command(
    grand_master: Option<f32>,
    blackout: Option<bool>,
) -> Result<OutputRuntimeCommand, ApiError> {
    let grand_master = grand_master
        .map(|level| {
            OutputLevel::new(level)
                .ok_or_else(|| ApiError::bad_request("grand_master must be within 0-1"))
        })
        .transpose()?;
    Ok(OutputRuntimeCommand::new(grand_master, blackout))
}

pub(super) fn exact_command(
    show_id: Uuid,
    revision: u64,
    grand_master: Option<f32>,
    blackout: Option<bool>,
) -> Result<OutputRuntimeCommand, ApiError> {
    if grand_master.is_none() && blackout.is_none() {
        return Err(ApiError::bad_request(
            "at least one of grand_master or blackout is required",
        ));
    }
    let command = command(grand_master, blackout)?;
    Ok(OutputRuntimeCommand::exact(
        show_id,
        revision,
        command.grand_master,
        command.blackout,
    ))
}

pub(super) fn execute(
    state: &AppState,
    session: Option<&Session>,
    context: ActionContext,
    command: OutputRuntimeCommand,
) -> Result<OutputRuntimeResult, ApiError> {
    execute_action(state, session, context, command).map_err(action_error)
}

pub(super) fn execute_action(
    state: &AppState,
    session: Option<&Session>,
    context: ActionContext,
    command: OutputRuntimeCommand,
) -> Result<OutputRuntimeResult, ActionError> {
    let ports = ServerOutputRuntimePorts { state, session };
    state
        .output
        .handle_runtime_action(ActionEnvelope { context, command }, &ports)
}

pub(super) fn snapshot(
    state: &AppState,
    session: &Session,
    context: ActionContext,
    identity: OutputRuntimeIdentity,
) -> Result<OutputRuntimeSnapshot, ApiError> {
    let ports = ServerOutputRuntimePorts {
        state,
        session: Some(session),
    };
    state
        .output
        .runtime_snapshot(&context, identity, &ports)
        .map_err(action_error)
}

struct ServerOutputRuntimePorts<'a> {
    state: &'a AppState,
    session: Option<&'a Session>,
}

impl OutputRuntimePorts for ServerOutputRuntimePorts<'_> {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let Some(session_id) = context.session_id else {
            return Ok(());
        };
        self.session
            .filter(|session| session.id.0 == session_id)
            .map(|_| ())
            .ok_or_else(|| ActionError::new(ActionErrorKind::Unauthorized, "invalid session"))
    }

    fn projection(
        &self,
        _context: &ActionContext,
        identity: OutputRuntimeIdentity,
    ) -> Result<OutputRuntimeProjection, ActionError> {
        let scope = OutputRuntimeScope {
            show_id: self
                .state
                .active_show
                .current()
                .as_ref()
                .map(|show| show.id.0)
                .unwrap_or_default(),
        };
        let control = self.state.output.control_projection();
        Ok(OutputRuntimeProjection {
            scope,
            identity,
            revision: control.revision,
            grand_master: control.grand_master,
            blackout: control.blackout,
        })
    }

    fn apply(
        &self,
        context: &ActionContext,
        command: OutputRuntimeCommand,
    ) -> Result<OutputRuntimeApplication, ActionError> {
        self.state.output.apply_runtime_control(
            command.grand_master.map(OutputLevel::value),
            command.blackout,
        )?;
        if let Err(error) = persist_output_runtime(self.state) {
            let warning = format!(
                "global output runtime persistence is pending: {}",
                error.message
            );
            tracing::error!(error=%error.message, "global output runtime persistence is pending");
            emit(
                self.state,
                "output_persistence_pending",
                serde_json::json!({
                    "correlation_id": context.correlation_id,
                    "error": error.message,
                }),
            );
            return Ok(OutputRuntimeApplication {
                durability: OutputRuntimeDurability::PersistencePending,
                warning: Some(warning),
            });
        }
        Ok(OutputRuntimeApplication::durable())
    }
}

pub(super) fn action_error(error: ActionError) -> ApiError {
    match error.kind {
        ActionErrorKind::Invalid => ApiError::bad_request(error.message),
        ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
        ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
        ActionErrorKind::NotFound => ApiError::not_found(error.message),
        ActionErrorKind::Conflict | ActionErrorKind::Busy => ApiError::conflict(error.message),
        ActionErrorKind::Unavailable => ApiError::unavailable(error.message),
        ActionErrorKind::Internal => ApiError::internal(error.message),
    }
}
