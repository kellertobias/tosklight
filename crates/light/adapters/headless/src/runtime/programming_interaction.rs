use super::{ApiError, AppState, Session, command_http::ServerProgrammingPorts};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActionSource, ProgrammingInteractionResult,
    ProgrammingLifecycleCompletion, ProgrammingLifecycleResult, ProgrammingLifecycleTarget,
};

#[derive(Clone, Copy)]
pub(super) enum ProgrammingLockPolicy {
    RequireUnlocked,
    AllowLockedReconciliation,
}

pub(super) fn programming_context(
    session: &Session,
    source: ActionSource,
    request_id: Option<&str>,
) -> ActionContext {
    let context = ActionContext::operator(session.desk.id, session.user.id.0, session.id.0, source);
    match request_id {
        Some(id) => context.with_request_id(id),
        None => context,
    }
}

pub(super) fn run_programming_interaction<T>(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
    source: &'static str,
    lock_policy: ProgrammingLockPolicy,
    operation: impl FnOnce() -> T,
) -> Result<ProgrammingInteractionResult<T>, ApiError> {
    let require_unlocked = matches!(lock_policy, ProgrammingLockPolicy::RequireUnlocked);
    let ports = ServerProgrammingPorts::new(state, session, source, require_unlocked);
    state
        .programming
        .run_external_interaction(context, &ports, operation)
        .map_err(programming_action_error)
}

pub(super) fn try_programming_activation(
    state: &AppState,
) -> Result<tokio::sync::OwnedMutexGuard<()>, String> {
    state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry the Programmer action".to_owned())
}

pub(super) fn run_programmer_lifecycle<T>(
    state: &AppState,
    actor: &Session,
    context: &ActionContext,
    target: ProgrammingLifecycleTarget,
    operation: impl FnOnce() -> ProgrammingLifecycleCompletion<T>,
) -> Result<ProgrammingLifecycleResult<T>, ApiError> {
    let ports = ServerProgrammingPorts::new(state, actor, "http_programmer_lifecycle", false);
    state
        .programming
        .replace_user_programmer(context, &ports, target, operation)
        .map_err(programming_action_error)
}

pub(super) fn programming_action_error(error: ActionError) -> ApiError {
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
