use light_application::{
    ActionContext, ActionError, ProgrammingPorts, ProgrammingPreloadCommitResult,
    ProgrammingPreloadLifecyclePorts, ProgrammingPreloadLifecycleRequest,
    ProgrammingReconciliation,
};

use super::programming_ports::ServerProgrammingPorts;

impl ProgrammingPreloadLifecyclePorts for ServerProgrammingPorts<'_> {
    fn authorize_preload_lifecycle(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize_programming_change(self, context)
    }

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        self.state()
            .installation
            .configuration()
            .preload_programmer_changes
    }

    fn commit_preload(
        &self,
        context: &ActionContext,
        request: &ProgrammingPreloadLifecycleRequest,
    ) -> Result<ProgrammingPreloadCommitResult, ActionError> {
        super::super::commit_preload_lifecycle_while_show_stable(
            self.state(),
            self.session(),
            context,
            request,
        )
    }

    fn reconcile_preload_capture(&self, context: &ActionContext) {
        let pinned = self
            .state()
            .programming
            .capture_mode(self.session().id)
            .is_some_and(|mode| mode.blind);
        self.state().output.set_dynamic_definitions_pinned(pinned);
        <Self as ProgrammingPorts>::reconcile(
            self,
            context,
            ProgrammingReconciliation::CaptureModeChanged,
        );
    }

    fn persist_preload_lifecycle(
        &self,
        context: &ActionContext,
        operation: &'static str,
    ) -> Option<String> {
        if matches!(operation, "preload.clear" | "preload.release") {
            // TL-548 C4: clear/release invalidates the Pending episode.
            self.state()
                .programming
                .pending_episodes()
                .trigger(crate::runtime::output_scheduler::PendingTrigger::Clear);
        }
        match operation {
            "preload.clear" => {
                self.state().output.set_dynamic_definitions_pinned(false);
                self.state().output.set_dynamic_definitions_pinned(true);
            }
            "preload.release" => self.state().output.set_dynamic_definitions_pinned(false),
            _ => {}
        }
        <Self as ProgrammingPorts>::persist(self, context, operation)
    }
}
