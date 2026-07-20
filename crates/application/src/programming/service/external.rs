use super::ProgrammingService;
use crate::programming::{ProgrammingInteractionResult, ProgrammingPorts};
use crate::{ActionContext, ActionError};
use light_core::{SessionId, UserId};

impl ProgrammingService {
    /// Serializes adapter-owned Programming mutations with typed commands on the same desk.
    ///
    /// Authorization runs under the desk gate. The closure must finish validation, mutation,
    /// persistence, and reconciliation without deleting the session or re-entering this desk's
    /// Programming gate. The boundary captures final state even when the closure returns an error
    /// as its output, then publishes the sparse authoritative change before releasing the gate.
    pub fn run_external_interaction<T>(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        operation: impl FnOnce() -> T,
    ) -> Result<ProgrammingInteractionResult<T>, ActionError> {
        let session = super::context_session(context)?;
        let user_id = super::context_user(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            self.capture_external_interaction(context, session, user_id, operation)
        })
    }

    fn capture_external_interaction<T>(
        &self,
        context: &ActionContext,
        session: SessionId,
        user_id: UserId,
        operation: impl FnOnce() -> T,
    ) -> Result<ProgrammingInteractionResult<T>, ActionError> {
        let lifecycle_before = self.active_lifecycle_programmer(user_id);
        let before = super::Snapshot::read(&self.programmers, context.desk_id, session, user_id)?;
        let output = operation();
        let after = super::Snapshot::read(&self.programmers, context.desk_id, session, user_id)?;
        let result = ProgrammingInteractionResult {
            output,
            event_sequence: self.publish_interaction(
                context,
                super::interaction_change(
                    &self.programmers,
                    context.desk_id,
                    session,
                    &before,
                    &after,
                ),
            ),
            capture_mode_event_sequence: self.publish_capture_mode(
                context,
                self.capture_mode_change(user_id, before.capture_mode, after.capture_mode),
            ),
            values_event_sequence: self.publish_values(
                context,
                self.values_change(
                    user_id,
                    session,
                    before.values_generation,
                    after.values_generation,
                )?,
            ),
            preload_values_event_sequence: self.publish_preload_values(
                context,
                self.preload_values_change(
                    user_id,
                    session,
                    before.preload_values_generation,
                    after.preload_values_generation,
                )?,
            ),
        };
        self.publish_lifecycle_for_context(context, lifecycle_before);
        Ok(result)
    }
}
