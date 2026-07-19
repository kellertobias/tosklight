use crate::{ActionContext, ActionError};

use super::{
    OutputRuntimeCommand, OutputRuntimeDurability, OutputRuntimeIdentity, OutputRuntimeProjection,
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
    ) -> Result<OutputRuntimeDurability, ActionError>;
}
