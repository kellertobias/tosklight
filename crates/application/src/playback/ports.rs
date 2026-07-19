use super::{
    PlaybackAction, PlaybackDeskProjection, PlaybackDurability, PlaybackExecution,
    PlaybackRuntimeIdentity, PlaybackRuntimeProjection, PlaybackSurface, PlaybackTransitionCause,
    ResolvedPlaybackAddress,
};
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

    /// Reports whether an accepted live-state mutation reached its durable store.
    fn durability(&self) -> PlaybackDurability {
        PlaybackDurability::Durable
    }

    /// Resolves configuration-dependent navigation semantics before execution. Most commands
    /// carry their meaning directly; configured buttons need the authoritative Playback layout.
    fn transition_cause(
        &self,
        _context: &ActionContext,
        _address: ResolvedPlaybackAddress,
        _action: PlaybackAction,
    ) -> Result<Option<PlaybackTransitionCause>, ActionError> {
        Ok(None)
    }

    /// Reads only the addressed immutable runtime/control projection. Implementations must not
    /// persist, compile, or construct a broad legacy Playback snapshot here.
    fn projection(
        &self,
        context: &ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<PlaybackRuntimeProjection, ActionError>;

    /// A Cuelist identity may expand to every assigned Playback which targets it. Results retain
    /// their exact requested identity so the service can reject unrelated repair data.
    fn projections(
        &self,
        context: &ActionContext,
        identities: &[PlaybackRuntimeIdentity],
    ) -> Result<Vec<PlaybackRuntimeProjection>, ActionError> {
        identities
            .iter()
            .copied()
            .map(|identity| self.projection(context, identity))
            .collect()
    }

    fn desk_projection(
        &self,
        context: &ActionContext,
    ) -> Result<Option<PlaybackDeskProjection>, ActionError>;
}
