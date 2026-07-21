//! Authenticated whole-Cue deletion ports over the shared active-show lifecycle.

use super::super::{
    AppState, ProgrammingInstallOwner, ProgrammingOwnerGesturePolicy,
    ProgrammingOwnerHighlightPolicy, ServerActiveShowPorts, ServerActiveShowUnitOfWork, Session,
    read_desk_lock,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowPorts, ProgrammingCueDeletionPorts,
};
use light_core::{SessionId, ShowId};
use light_engine::{EngineSnapshot, PreparedEngineSnapshot};
use light_show::PortableShowObjectUndo;

#[derive(Clone)]
pub(crate) struct ServerProgrammingCueDeletionPorts {
    state: AppState,
    session: Session,
    active: ServerActiveShowPorts,
    within_interaction: bool,
}

impl ServerProgrammingCueDeletionPorts {
    pub(crate) fn new(state: AppState, session: Session, within_interaction: bool) -> Self {
        let owner = ProgrammingInstallOwner {
            desk_id: session.desk.id,
            user_id: session.user.id,
            gesture: ProgrammingOwnerGesturePolicy::Preserve,
            highlight: if within_interaction {
                ProgrammingOwnerHighlightPolicy::DeferToOuterInteraction
            } else {
                ProgrammingOwnerHighlightPolicy::Reconcile
            },
        };
        Self {
            active: ServerActiveShowPorts::show_objects_with_programming_owner(
                state.clone(),
                owner,
            ),
            state,
            session,
            within_interaction,
        }
    }

    fn authorize_identity(&self, context: &ActionContext) -> Result<(), ActionError> {
        let (Some(user_id), Some(session_id)) = (context.user_id, context.session_id) else {
            return Err(unauthorized(
                "Cue deletion requires an authenticated operator",
            ));
        };
        let live = self
            .state
            .sessions
            .read()
            .get(&SessionId(session_id))
            .is_some_and(|session| session.token == self.session.token);
        if !live {
            return Err(unauthorized("Cue deletion session is no longer active"));
        }
        if context.desk_id != self.session.desk.id
            || user_id != self.session.user.id.0
            || session_id != self.session.id.0
        {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "Cue deletion authority does not match the authenticated session",
            ));
        }
        Ok(())
    }
}

impl ActiveShowPorts for ServerProgrammingCueDeletionPorts {
    type UnitOfWork = ServerActiveShowUnitOfWork;
    type PreparedRuntime = PreparedEngineSnapshot;

    fn authorize_mutation(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize_identity(context)?;
        if read_desk_lock(&self.state, context.desk_id).locked {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "desk is locked",
            ));
        }
        Ok(())
    }

    fn run_active_show_lifecycle<T>(
        &self,
        _context: &ActionContext,
        _show_id: ShowId,
        operation: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        if self.within_interaction {
            return operation();
        }
        let _activation = self.state.activation_lock.clone().blocking_lock_owned();
        operation()
    }

    fn begin_active_show(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.active.begin_active_show(context, show_id)
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: light_core::Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        self.active
            .prepare_object_undo(unit, kind, object_id, expected_object_revision)
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.active.prepare_runtime(snapshot)
    }

    fn install_runtime(&self, context: &ActionContext, prepared: Self::PreparedRuntime) {
        self.active.install_runtime(context, prepared);
    }
}

impl ProgrammingCueDeletionPorts for ServerProgrammingCueDeletionPorts {
    fn authorize_cue_deletion_identity(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize_identity(context)
    }

    fn current_cue_deletion_page(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<u8, ActionError> {
        self.state
            .desk
            .lock()
            .desk_page(context.desk_id, show_id)
            .map_err(|error| ActionError::new(ActionErrorKind::Unavailable, error.to_string()))
    }

    fn persist_cue_deletion(&self, context: &ActionContext) -> Option<String> {
        if self.within_interaction {
            return None;
        }
        super::events::persist_with_warning(
            &self.state,
            &self.session,
            "http_cue_deletion",
            context.request_id.as_deref(),
            "programmer.cue_deletion",
        )
    }
}

fn unauthorized(message: &'static str) -> ActionError {
    ActionError::new(ActionErrorKind::Unauthorized, message)
}
