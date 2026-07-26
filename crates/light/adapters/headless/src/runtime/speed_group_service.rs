use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, SpeedBpm, SpeedBpmDelta,
    SpeedGroupAction, SpeedGroupApplication, SpeedGroupCommand, SpeedGroupDurability, SpeedGroupId,
    SpeedGroupPortState, SpeedGroupPorts, SpeedGroupResolvedAction, SpeedGroupResult,
    SpeedGroupSnapshot,
};

use super::{
    ApiError, AppState, Session, application_millis, copy_speed_group_runtime_to_configuration,
    persist_server_configuration, read_desk_lock, refresh_speed_group_engine,
};

pub(super) fn exact_command(
    authority_id: uuid::Uuid,
    revision: u64,
    action: SpeedGroupAction,
) -> SpeedGroupCommand {
    SpeedGroupCommand::exact(authority_id, revision, action)
}

pub(super) fn bpm(value: f64) -> Result<SpeedBpm, ApiError> {
    SpeedBpm::new(value)
        .ok_or_else(|| ApiError::bad_request("BPM must be finite and within 0.1-999"))
}

pub(super) fn delta(value: f64) -> Result<SpeedBpmDelta, ApiError> {
    SpeedBpmDelta::new(value)
        .ok_or_else(|| ApiError::bad_request("relative BPM must be finite and non-zero"))
}

pub(super) fn execute_action(
    state: &AppState,
    session: Option<&Session>,
    context: ActionContext,
    command: SpeedGroupCommand,
) -> Result<SpeedGroupResult, ActionError> {
    execute_with_lock_policy(state, session, context, command, false)
}

pub(super) fn execute_http_action(
    state: &AppState,
    session: &Session,
    context: ActionContext,
    command: SpeedGroupCommand,
) -> Result<SpeedGroupResult, ActionError> {
    execute_with_lock_policy(state, Some(session), context, command, true)
}

fn execute_with_lock_policy(
    state: &AppState,
    session: Option<&Session>,
    context: ActionContext,
    command: SpeedGroupCommand,
    require_unlocked: bool,
) -> Result<SpeedGroupResult, ActionError> {
    let ports = ServerSpeedGroupPorts {
        state,
        session,
        require_unlocked,
    };
    state
        .output
        .handle_speed_group_action(ActionEnvelope { context, command }, &ports)
}

/// Publishes an authority revision for Speed Group runtime changes applied by the v1 surfaces
/// (tap tempo, double, half, pause) so event-driven v2 views observe them.
pub(super) fn record_external_change(state: &AppState, session: &Session, affected: &[usize]) {
    let changed = affected
        .iter()
        .filter_map(|index| SpeedGroupId::new((index + 1) as u8))
        .collect::<Vec<_>>();
    if changed.is_empty() {
        return;
    }
    let ports = ServerSpeedGroupPorts {
        state,
        session: Some(session),
        require_unlocked: false,
    };
    let context = ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::Http,
    );
    if let Err(error) = state.output.record_speed_group_external_change(
        &context,
        &ports,
        &changed,
        application_millis(state),
    ) {
        tracing::warn!(error=%error.message, "Speed Group external change event was not published");
    }
}

pub(super) fn snapshot(
    state: &AppState,
    session: &Session,
    context: ActionContext,
) -> Result<SpeedGroupSnapshot, ApiError> {
    let ports = ServerSpeedGroupPorts {
        state,
        session: Some(session),
        require_unlocked: false,
    };
    state
        .output
        .speed_group_service_snapshot(&context, &ports)
        .map_err(action_error)
}

struct ServerSpeedGroupPorts<'a> {
    state: &'a AppState,
    session: Option<&'a Session>,
    require_unlocked: bool,
}

impl SpeedGroupPorts for ServerSpeedGroupPorts<'_> {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let Some(session_id) = context.session_id else {
            return Ok(());
        };
        let authorized = self.session.is_some_and(|session| {
            session.id.0 == session_id
                && session.desk.id == context.desk_id
                && Some(session.user.id.0) == context.user_id
        });
        if !authorized {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the action context does not match the authenticated operator session",
            ));
        }
        Ok(())
    }

    fn state(&self, _context: &ActionContext) -> Result<SpeedGroupPortState, ActionError> {
        if self.require_unlocked
            && self
                .session
                .is_some_and(|session| read_desk_lock(self.state, session.desk.id).locked)
        {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "desk is locked",
            ));
        }
        Ok(self.state.output.speed_group_port_state())
    }

    fn application_millis(&self, _context: &ActionContext) -> Result<u64, ActionError> {
        Ok(application_millis(self.state))
    }

    fn apply(
        &self,
        _context: &ActionContext,
        action: SpeedGroupResolvedAction,
    ) -> Result<SpeedGroupApplication, ActionError> {
        let affected = apply_runtime(self.state, action)?;
        clear_sound_owners(self.state, &affected);
        let persistence = persist_configuration(self.state);
        refresh_speed_group_engine(self.state);
        match persistence {
            Ok(()) => Ok(SpeedGroupApplication::durable()),
            Err(error) => {
                let warning = format!(
                    "Speed Group configuration persistence is pending: {}",
                    error.message
                );
                tracing::error!(error=%error.message, "Speed Group configuration persistence is pending");
                Ok(SpeedGroupApplication {
                    durability: SpeedGroupDurability::PersistencePending,
                    warning: Some(warning),
                })
            }
        }
    }
}

fn apply_runtime(
    state: &AppState,
    action: SpeedGroupResolvedAction,
) -> Result<Vec<usize>, ActionError> {
    let affected = state.output.apply_resolved_speed_group_action(action)?;
    copy_speed_group_runtime_to_configuration(state, &affected);
    Ok(affected)
}

fn clear_sound_owners(state: &AppState, affected: &[usize]) {
    state.output.clear_sound_capture_owners(affected);
}

fn persist_configuration(state: &AppState) -> Result<(), ApiError> {
    #[cfg(test)]
    {
        state.output.record_speed_group_persistence_attempt()?;
    }
    persist_server_configuration(state)
}

fn speed_error(error: light_control::speed::SpeedError) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
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
