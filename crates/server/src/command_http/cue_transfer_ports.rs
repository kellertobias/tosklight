//! Authenticated Cue-transfer ports over the shared active-show lifecycle.

use super::super::{
    AppState, ProgrammingInstallOwner, ProgrammingOwnerGesturePolicy,
    ProgrammingOwnerHighlightPolicy, ServerActiveShowPorts, ServerActiveShowUnitOfWork, Session,
    emit, read_desk_lock,
};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowPorts,
    ProgrammingCueTransferPorts,
};
use light_core::{SessionId, ShowId};
use light_engine::{EngineSnapshot, PreparedEngineSnapshot};
use light_show::PortableShowObjectUndo;

#[derive(Clone)]
pub(crate) struct ServerProgrammingCueTransferPorts {
    state: AppState,
    session: Session,
    active: ServerActiveShowPorts,
    within_interaction: bool,
}

impl ServerProgrammingCueTransferPorts {
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

    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let (Some(user_id), Some(session_id)) = (context.user_id, context.session_id) else {
            return Err(unauthorized(
                "Cue transfer requires an authenticated operator",
            ));
        };
        let live = self
            .state
            .sessions
            .read()
            .get(&SessionId(session_id))
            .is_some_and(|session| session.token == self.session.token);
        if !live {
            return Err(unauthorized("Cue transfer session is no longer active"));
        }
        if context.desk_id != self.session.desk.id
            || user_id != self.session.user.id.0
            || session_id != self.session.id.0
        {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "Cue transfer authority does not match the authenticated session",
            ));
        }
        if read_desk_lock(&self.state, context.desk_id).locked {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "desk is locked",
            ));
        }
        Ok(())
    }

    fn publish_compatibility(&self, changes: &[ActiveShowObjectChange]) {
        if !self.within_interaction {
            return;
        }
        let Some(show_id) = self.state.active_show.read().as_ref().map(|show| show.id) else {
            return;
        };
        for change in changes {
            emit(
                &self.state,
                "show_object_changed",
                serde_json::json!({
                    "show_id":show_id,
                    "kind":change.kind.as_str(),
                    "id":change.object_id,
                    "revision":change.object_revision,
                    "deleted":change.deleted,
                }),
            );
        }
    }
}

impl ActiveShowPorts for ServerProgrammingCueTransferPorts {
    type UnitOfWork = ServerActiveShowUnitOfWork;
    type PreparedRuntime = PreparedEngineSnapshot;

    fn authorize_mutation(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize(context)
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

    fn reconcile_object_changes(&self, changes: &[ActiveShowObjectChange]) {
        self.active.reconcile_object_changes(changes);
    }
}

impl ProgrammingCueTransferPorts for ServerProgrammingCueTransferPorts {
    fn authorize_cue_transfer(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize(context)
    }

    fn reconcile_cue_transfer(&self, changes: &[ActiveShowObjectChange]) {
        self.publish_compatibility(changes);
    }

    fn persist_cue_transfer(&self, context: &ActionContext) -> Option<String> {
        if self.within_interaction {
            return None;
        }
        super::events::persist_with_warning(
            &self.state,
            &self.session,
            "http_cue_transfer",
            context.request_id.as_deref(),
            "programmer.cue_transfer",
        )
    }
}

fn unauthorized(message: &'static str) -> ActionError {
    ActionError::new(ActionErrorKind::Unauthorized, message)
}
