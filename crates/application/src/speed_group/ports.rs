use crate::{ActionContext, ActionError};

use super::{SpeedGroupApplication, SpeedGroupPortState, SpeedGroupResolvedAction};

pub trait SpeedGroupPorts: Send + Sync {
    fn authorize(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn state(&self, context: &ActionContext) -> Result<SpeedGroupPortState, ActionError>;

    fn application_millis(&self, context: &ActionContext) -> Result<u64, ActionError>;

    /// Applies one already-resolved mutation, refreshes runtime consumers, and attempts exactly one
    /// configuration persistence write.
    fn apply(
        &self,
        context: &ActionContext,
        action: SpeedGroupResolvedAction,
    ) -> Result<SpeedGroupApplication, ActionError>;
}
