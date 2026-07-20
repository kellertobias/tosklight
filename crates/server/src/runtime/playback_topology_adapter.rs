//! Authenticated active-show and compatibility ports for Playback topology.

use super::{AppState, ServerActiveShowPorts, ServerActiveShowUnitOfWork, Session, emit};
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowPorts,
    PlaybackTopologyPorts,
};
use light_core::{SessionId, ShowId};
use light_engine::{EngineSnapshot, PreparedEngineSnapshot};
use light_show::PortableShowObjectUndo;

#[derive(Clone)]
pub(super) struct ServerPlaybackTopologyPorts {
    state: AppState,
    active: ServerActiveShowPorts,
    session: Session,
    show_id: ShowId,
}

impl ServerPlaybackTopologyPorts {
    pub(super) fn new(state: AppState, session: Session, show_id: ShowId) -> Self {
        Self {
            active: ServerActiveShowPorts::show_objects(state.clone()),
            state,
            session,
            show_id,
        }
    }

    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let (Some(user_id), Some(session_id)) = (context.user_id, context.session_id) else {
            return Err(ActionError::new(
                ActionErrorKind::Unauthorized,
                "Playback topology requires an authenticated operator",
            ));
        };
        let session_is_live = self
            .state
            .sessions
            .read()
            .get(&SessionId(session_id))
            .is_some_and(|session| session.token == self.session.token);
        if !session_is_live {
            return Err(ActionError::new(
                ActionErrorKind::Unauthorized,
                "Playback topology session is no longer active",
            ));
        }
        if context.desk_id != self.session.desk.id
            || user_id != self.session.user.id.0
            || session_id != self.session.id.0
        {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "Playback topology authority does not match the authenticated session",
            ));
        }
        Ok(())
    }

    fn publish_compatibility(&self, changes: &[ActiveShowObjectChange]) {
        for change in changes {
            emit(
                &self.state,
                "show_object_changed",
                serde_json::json!({
                    "show_id": self.show_id,
                    "kind": change.kind.as_str(),
                    "id": change.object_id,
                    "revision": change.object_revision,
                    "deleted": change.deleted,
                }),
            );
        }
    }
}

impl ActiveShowPorts for ServerPlaybackTopologyPorts {
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

impl PlaybackTopologyPorts for ServerPlaybackTopologyPorts {
    fn authorize_playback_topology(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize(context)
    }

    /// v1 notifications mirror exact mutations but do not publish another application event.
    fn reconcile_playback_topology(&self, changes: &[ActiveShowObjectChange]) {
        self.publish_compatibility(changes);
    }
}
