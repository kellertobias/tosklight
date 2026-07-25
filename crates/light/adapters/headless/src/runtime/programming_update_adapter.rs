//! Authenticated Programming Update ports over the shared active-show lifecycle.

use super::{
    AppState, ProgrammingInstallOwner, ProgrammingOwnerGesturePolicy,
    ProgrammingOwnerHighlightPolicy, ServerActiveShowPorts, ServerActiveShowUnitOfWork, Session,
    read_desk_lock,
};
use light_application::programming_update::{
    ActiveCueContext, ProgrammingUpdatePorts, ProgrammingUpdateProjection,
};
use light_application::{ActionContext, ActionError, ActionErrorKind, ActiveShowPorts};
use light_core::{SessionId, ShowId};
use light_engine::{EngineSnapshot, PreparedEngineSnapshot};
use light_show::PortableShowObjectUndo;

#[derive(Clone)]
pub(super) struct ServerProgrammingUpdatePorts {
    state: AppState,
    session: Session,
    active: ServerActiveShowPorts,
    require_unlocked: bool,
    within_interaction: bool,
}

impl ServerProgrammingUpdatePorts {
    pub(super) fn new(
        state: AppState,
        session: Session,
        within_interaction: bool,
        require_unlocked: bool,
    ) -> Self {
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
            require_unlocked,
            within_interaction,
        }
    }

    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let (Some(user_id), Some(session_id)) = (context.user_id, context.session_id) else {
            return Err(ActionError::new(
                ActionErrorKind::Unauthorized,
                "Update requires an authenticated operator",
            ));
        };
        let live = self
            .state
            .sessions
            .read()
            .get(&SessionId(session_id))
            .is_some_and(|session| session.token == self.session.token);
        if !live {
            return Err(ActionError::new(
                ActionErrorKind::Unauthorized,
                "Update session is no longer active",
            ));
        }
        if context.desk_id != self.session.desk.id
            || user_id != self.session.user.id.0
            || session_id != self.session.id.0
        {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "Update authority does not match the authenticated session",
            ));
        }
        if self.require_unlocked && read_desk_lock(&self.state, context.desk_id).locked {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "desk is locked",
            ));
        }
        Ok(())
    }
}

impl ActiveShowPorts for ServerProgrammingUpdatePorts {
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
        let _activation = self
            .state
            .activation_lock
            .clone()
            .try_lock_owned()
            .map_err(|_| ActionError::new(ActionErrorKind::Busy, "the active show is changing"))?;
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

impl ProgrammingUpdatePorts for ServerProgrammingUpdatePorts {
    fn authorize_programming_update(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize(context)
    }

    fn active_update_cue_contexts(
        &self,
        _context: &ActionContext,
    ) -> Result<Vec<ActiveCueContext>, ActionError> {
        Ok(self
            .state
            .engine
            .active_playbacks()
            .into_iter()
            .filter_map(|playback| {
                Some(ActiveCueContext {
                    playback_number: playback.playback_number?,
                    cue_list_id: playback.cue_list_id,
                    cue_id: playback.current_cue_id?,
                    cue_number: playback.current_cue_number?,
                })
            })
            .collect())
    }

    fn reconcile_programming_update(&self, _projection: &ProgrammingUpdateProjection) {}
}
