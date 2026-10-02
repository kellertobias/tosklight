use crate::{ActionContext, ActionError};

use super::{
    OutputRuntimeApplication, OutputRuntimeCommand, OutputRuntimeIdentity, OutputRuntimeProjection,
};

pub trait OutputRuntimePorts: Send + Sync {
    fn authorize(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn projection(
        &self,
        context: &ActionContext,
        identity: OutputRuntimeIdentity,
    ) -> Result<OutputRuntimeProjection, ActionError>;

    /// Applies one already-validated global-output mutation and reports its persistence status.
    fn apply(
        &self,
        context: &ActionContext,
        command: OutputRuntimeCommand,
    ) -> Result<OutputRuntimeApplication, ActionError>;

    /// Records a fresh, authorized, expectation-valid command whose supplied fields already equal
    /// the current projection. It must not change values, revision, events or persistence; only
    /// write identity of explicitly supplied fields may be refreshed. Cached replays and rejected
    /// commands never reach this hook.
    fn reassert(&self, _context: &ActionContext, _command: OutputRuntimeCommand) {}
}
