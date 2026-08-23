use super::{AppState, Session};
use light_application::{ActionContext, ActionError, ActionErrorKind, DynamicsPorts};
use light_dynamics::{DynamicRuntimeError, DynamicStartRequest};
use light_engine::EngineSnapshot;
use std::sync::Arc;
use uuid::Uuid;

pub(super) struct ServerDynamicsPorts<'a> {
    pub(super) state: &'a AppState,
    pub(super) session: &'a Session,
}

pub(super) fn controller_for_runtime_instance(
    state: &AppState,
    session: &Session,
    runtime_or_controller_id: Uuid,
) -> Result<Uuid, String> {
    let runtime = state.output.dynamic_runtime_snapshot();
    if runtime.instances.iter().any(|instance| {
        instance
            .controllers
            .iter()
            .any(|controller| controller.id == runtime_or_controller_id)
    }) {
        return Ok(runtime_or_controller_id);
    }
    let instance = runtime
        .instances
        .iter()
        .find(|instance| instance.id == runtime_or_controller_id)
        .ok_or_else(|| "Dynamic runtime instance does not exist".to_owned())?;
    let programmer_id = state
        .programming
        .get(session.id)
        .map(|programmer| programmer.id.0);
    if let Some(controller) = instance.controllers.iter().find(|controller| {
        matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Programmer {
                programmer_id: source
            } if Some(source) == programmer_id
        )
    }) {
        return Ok(controller.id);
    }
    match instance.controllers.as_slice() {
        [controller] => Ok(controller.id),
        [] => Err("Dynamic runtime instance has no active controller".to_owned()),
        _ => Err(
            "Dynamic runtime instance has multiple controllers; use its exact running source"
                .to_owned(),
        ),
    }
}

impl DynamicsPorts for ServerDynamicsPorts<'_> {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        if context.desk_id != self.session.desk.id
            || context.session_id != Some(self.session.id.0)
            || context.user_id != Some(self.session.user.id.0)
        {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the action context does not match the authenticated operator session",
            ));
        }
        if super::read_desk_lock(self.state).locked {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "desk is locked",
            ));
        }
        Ok(())
    }

    fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.state.output.snapshot()
    }

    fn now_millis(&self) -> u64 {
        u64::try_from(self.state.output.application_time().timestamp_millis()).unwrap_or_default()
    }

    fn runtime_controller_is_completed(&self, controller_id: Uuid) -> bool {
        self.state
            .output
            .dynamic_runtime_snapshot()
            .instances
            .iter()
            .any(|instance| {
                instance.completed
                    && instance
                        .controllers
                        .iter()
                        .any(|controller| controller.id == controller_id)
            })
    }

    fn start_runtime(&self, request: DynamicStartRequest) -> Result<Uuid, DynamicRuntimeError> {
        self.state.output.start_dynamic(request)
    }

    fn off_runtime_controller(
        &self,
        controller_id: Uuid,
        now_millis: u64,
        release_delay_millis: u64,
        release_duration_millis: u64,
    ) -> Result<(Uuid, bool), DynamicRuntimeError> {
        self.state.output.off_dynamic_controller(
            controller_id,
            now_millis,
            release_delay_millis,
            release_duration_millis,
        )
    }

    fn update_runtime_controller(
        &self,
        controller_id: Uuid,
        size: Option<f32>,
        speed_multiplier: Option<f32>,
        phase_offset_degrees: Option<f32>,
    ) -> Result<(), DynamicRuntimeError> {
        self.state.output.update_dynamic_controller(
            controller_id,
            size,
            speed_multiplier,
            phase_offset_degrees,
        )
    }

    fn publish_runtime_change(
        &self,
        context: &ActionContext,
        change: light_application::DynamicRuntimeChange,
    ) {
        self.state
            .events
            .publish(light_application::EventDraft::dynamic_runtime_changed(
                Some(context),
                change,
            ));
    }
}
