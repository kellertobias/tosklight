use super::ProgrammingService;
use crate::programming::{ProgrammingInteractionResult, ProgrammingPorts};
use crate::{ActionContext, ActionError};
use light_core::SessionId;

impl ProgrammingService {
    /// Publish only the normal-value metadata difference after a committed replacement.
    /// The caller owns the active-show Programmer mutation boundary through capture, persist,
    /// attachment and publication. No desk gate is acquired and no operator session is created
    /// or impersonated; the original action context also supports system-owned replacement.
    pub fn publish_replacement_migration_values(
        &self,
        context: &ActionContext,
        before: &light_programmer::ProgrammerState,
    ) -> Option<u64> {
        let after = self
            .programmers
            .active()
            .into_iter()
            .find(|state| state.id == before.id)?;
        let content = |state: &light_programmer::ProgrammerState| {
            let (fixture_values, group_values, dynamic_values) = state.update_projection_content();
            super::super::values_projection::ProgrammingValuesContent {
                fixture_values,
                group_values,
                dynamic_values,
            }
        };
        let before = content(before);
        let after = content(&after);
        if before == after {
            return None;
        }
        let revision = self.programmers.advance_normal_values_revision();
        self.publish_values(context, Some(after.change(&before, revision)))
    }

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
        self.with_programmer_and_desk_gate(context.desk_id, || {
            ports.authorize_programming_change(context)?;
            self.capture_external_interaction(context, session, operation)
        })
    }

    fn capture_external_interaction<T>(
        &self,
        context: &ActionContext,
        session: SessionId,
        operation: impl FnOnce() -> T,
    ) -> Result<ProgrammingInteractionResult<T>, ActionError> {
        let lifecycle_before = self.active_lifecycle_programmer();
        let before = super::Snapshot::read(&self.programmers, context.desk_id, session)?;
        let output = operation();
        let after = super::Snapshot::read(&self.programmers, context.desk_id, session)?;
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
                self.capture_mode_change(before.capture_mode, after.capture_mode),
            ),
            values_event_sequence: self.publish_values(
                context,
                self.values_change(&before.values_content, &after.values_content)?,
            ),
            preload_values_event_sequence: self.publish_preload_values(
                context,
                self.preload_values_change(
                    session,
                    before.preload_values_generation,
                    after.preload_values_generation,
                )?,
            ),
            preload_playback_queue_event_sequence: self.publish_preload_playback_queue(
                context,
                self.preload_playback_queue_change(
                    session,
                    before.preload_playback_queue_generation,
                    after.preload_playback_queue_generation,
                )?,
            ),
        };
        self.publish_lifecycle(context, lifecycle_before);
        Ok(result)
    }
}
