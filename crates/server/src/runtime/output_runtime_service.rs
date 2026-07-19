use super::{ApiError, AppState, Session, emit, persist_output_runtime};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, OutputLevel, OutputRuntimeCommand,
    OutputRuntimeDurability, OutputRuntimeIdentity, OutputRuntimePorts, OutputRuntimeProjection,
    OutputRuntimeResult, OutputRuntimeScope, OutputRuntimeSnapshot,
};

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

pub(super) fn execute(
    state: &AppState,
    session: Option<&Session>,
    context: ActionContext,
    command: OutputRuntimeCommand,
) -> Result<OutputRuntimeResult, ApiError> {
    let ports = ServerOutputRuntimePorts { state, session };
    state
        .output_runtime_service
        .handle(ActionEnvelope { context, command }, &ports)
        .map_err(action_error)
}

pub(super) fn execute_while_show_stable(
    state: &AppState,
    session: Option<&Session>,
    context: ActionContext,
    command: OutputRuntimeCommand,
) -> Result<OutputRuntimeResult, ApiError> {
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| ApiError::conflict("active show transition is in progress"))?;
    execute(state, session, context, command)
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
        .output_runtime_service
        .snapshot(&context, identity, &ports)
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
                .read()
                .as_ref()
                .map(|show| show.id.0)
                .unwrap_or_default(),
            show_revision: self.state.engine.snapshot().revision,
        };
        let control = self.state.output_control.lock();
        Ok(OutputRuntimeProjection {
            scope,
            identity,
            grand_master: control.options.grand_master,
            blackout: control.options.blackout,
        })
    }

    fn apply(
        &self,
        context: &ActionContext,
        command: OutputRuntimeCommand,
    ) -> Result<OutputRuntimeDurability, ActionError> {
        {
            let mut control = self.state.output_control.lock();
            if let Some(level) = command.grand_master {
                control.options.grand_master = level.value();
            }
            if let Some(blackout) = command.blackout {
                control.options.blackout = blackout;
            }
        }
        if let Err(error) = persist_output_runtime(self.state) {
            tracing::error!(error=%error.message, "global output runtime persistence is pending");
            emit(
                self.state,
                "output_persistence_pending",
                serde_json::json!({
                    "correlation_id": context.correlation_id,
                    "error": error.message,
                }),
            );
            return Ok(OutputRuntimeDurability::PersistencePending);
        }
        Ok(OutputRuntimeDurability::Durable)
    }
}

fn action_error(error: ActionError) -> ApiError {
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
