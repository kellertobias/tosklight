use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, SpeedBpm, SpeedBpmDelta,
    SpeedGroupAction, SpeedGroupApplication, SpeedGroupCommand, SpeedGroupDurability, SpeedGroupId,
    SpeedGroupPortState, SpeedGroupPorts, SpeedGroupProjection, SpeedGroupResolvedAction,
    SpeedGroupResult, SpeedGroupSnapshot,
};

use super::{
    ApiError, AppState, Session, application_millis, copy_speed_group_runtime_to_configuration,
    persist_server_configuration, read_desk_lock, refresh_speed_group_engine,
    synchronize_speed_groups, unlink_speed_group,
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
        .speed_group_service
        .handle(ActionEnvelope { context, command }, &ports)
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
        .speed_group_service
        .snapshot(&context, &ports)
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
        let controllers = self.state.speed_groups.lock();
        let owners = self.state.sound_capture_owners.lock();
        let groups = controllers
            .iter()
            .enumerate()
            .map(|(index, controller)| projection(index, controller))
            .collect();
        let manual_control_clean = controllers
            .iter()
            .enumerate()
            .filter(|(index, controller)| {
                controller.manual_entry_is_current(controller.manual_bpm())
                    && owners[*index].is_none()
            })
            .filter_map(|(index, _)| SpeedGroupId::new((index + 1) as u8))
            .collect();
        Ok(SpeedGroupPortState {
            groups,
            manual_control_clean,
        })
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

fn projection(
    index: usize,
    controller: &light_control::speed::SpeedGroupController,
) -> SpeedGroupProjection {
    let snapshot = controller.snapshot(0);
    SpeedGroupProjection {
        group: SpeedGroupId::new((index + 1) as u8).expect("fixed Speed Group index"),
        manual_bpm: snapshot.manual_bpm,
        paused: snapshot.paused,
        speed_master_scale: snapshot.speed_master_scale,
        synchronized_with: snapshot.synchronized_with.and_then(SpeedGroupId::new),
        phase_origin_millis: snapshot.phase_origin_millis,
    }
}

fn apply_runtime(
    state: &AppState,
    action: SpeedGroupResolvedAction,
) -> Result<Vec<usize>, ActionError> {
    let mut controllers = state.speed_groups.lock();
    let affected = match action {
        SpeedGroupResolvedAction::SetManualBpm {
            group,
            bpm,
            applied_at_millis,
        } => {
            let index = group.index();
            unlink_speed_group(&mut controllers, index, applied_at_millis);
            controllers[index]
                .set_manual_bpm(bpm)
                .map_err(speed_error)?;
            controllers[index]
                .set_speed_master_scale(1.0)
                .map_err(speed_error)?;
            controllers[index].set_paused_at(false, applied_at_millis);
            vec![index]
        }
        SpeedGroupResolvedAction::Synchronize {
            source,
            target,
            applied_at_millis,
        } => {
            synchronize_speed_groups(
                &mut controllers,
                source.index(),
                target.index(),
                applied_at_millis,
            )
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.message))?;
            vec![source.index(), target.index()]
        }
    };
    copy_speed_group_runtime_to_configuration(state, &controllers, &affected);
    Ok(affected)
}

fn clear_sound_owners(state: &AppState, affected: &[usize]) {
    let mut owners = state.sound_capture_owners.lock();
    for &index in affected {
        owners[index] = None;
    }
}

fn persist_configuration(state: &AppState) -> Result<(), ApiError> {
    #[cfg(test)]
    {
        use std::sync::atomic::Ordering;
        state
            .speed_group_persistence_attempts
            .fetch_add(1, Ordering::SeqCst);
        if state.speed_group_persistence_failure.load(Ordering::SeqCst) {
            return Err(ApiError::internal("forced Speed Group persistence failure"));
        }
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
