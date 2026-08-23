use super::ProgrammingService;
use crate::{
    ActionContext, ActionError, ProgrammingPorts, ProgrammingShowUndoTarget,
    programming::show_history::history_identity,
};
use light_core::{SessionId, UserId};

impl ProgrammingService {
    pub fn remember_selective_import(
        &self,
        context: &ActionContext,
        target: crate::SelectiveShowImportUndoTarget,
    ) -> Result<(), ActionError> {
        let (session_id, user_id) = history_identity(context)?;
        // Whatever identity arrived, this session operates the desk's one Programmer.
        let user_id = self.programmers.desk_user_for(session_id, user_id);
        if target.objects.is_empty() {
            return Ok(());
        }
        self.remember_show_mutation(
            session_id,
            user_id,
            context.desk_id,
            ProgrammingShowUndoTarget {
                show_id: target.show_id,
                objects: Vec::new(),
                portable_objects: target.objects,
            },
        );
        Ok(())
    }

    pub(super) fn remember_show_mutation(
        &self,
        session_id: SessionId,
        user_id: UserId,
        desk_id: uuid::Uuid,
        target: ProgrammingShowUndoTarget,
    ) {
        let Some(undo_depth) = self.programmers.undo_depth(session_id) else {
            debug_assert!(false, "successful show mutation lost its Programmer");
            return;
        };
        self.programmers.clear_redo(session_id);
        self.show_history
            .lock()
            .record(user_id, desk_id, undo_depth, target);
    }

    pub(super) fn undo_show_mutation(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<bool, ActionError> {
        let (session_id, user_id) = history_identity(context)?;
        // Whatever identity arrived, this session operates the desk's one Programmer.
        let user_id = self.programmers.desk_user_for(session_id, user_id);
        let undo_depth = self
            .programmers
            .undo_depth(session_id)
            .ok_or_else(super::support::unknown_programmer)?;
        let Some(target) = self
            .show_history
            .lock()
            .next(user_id, context.desk_id, undo_depth)
        else {
            return Ok(false);
        };
        let current_revision = ports.undo_show_recording(context, &target)?;
        self.show_history
            .lock()
            .finish(user_id, context.desk_id, &target, current_revision);
        self.programmers.clear_redo(session_id);
        Ok(true)
    }
}
