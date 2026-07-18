use super::{PlaybackAction, PlaybackExecution, PlaybackSurface, ResolvedPlaybackAddress};
use crate::{ActionContext, ActionError};

/// Transitional capabilities retained by the server while Stage 3 moves legacy playback
/// mutations behind a transport-independent ordering and addressing boundary.
pub trait PlaybackPorts: Send + Sync {
    fn authorize(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn current_page(&self, context: &ActionContext) -> Result<u8, ActionError>;

    fn playback_at(&self, page: u8, slot: u8) -> Result<Option<u16>, ActionError>;

    fn execute(
        &self,
        context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError>;
}
