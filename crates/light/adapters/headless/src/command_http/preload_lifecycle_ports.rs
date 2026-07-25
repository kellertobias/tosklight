use light_application::{
    ActionContext, ActionError, ProgrammingPorts, ProgrammingPreloadCommitResult,
    ProgrammingPreloadLifecyclePorts, ProgrammingPreloadLifecycleRequest,
    ProgrammingReconciliation,
};

use super::programming_ports::ServerProgrammingPorts;

impl ProgrammingPreloadLifecyclePorts for ServerProgrammingPorts<'_> {
    fn authorize_preload_lifecycle(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize(self, context)
    }

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        self.state().configuration.read().preload_programmer_changes
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
        <Self as ProgrammingPorts>::persist(self, context, operation)
    }
}
