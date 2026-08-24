//! The one interaction context a desk has.
//!
//! ToskLight is a lighting desk, not a shared document. Whoever is standing at it — through the
//! main window, an optional screen, a browser screen, an OSC client, an attached hardware surface,
//! the keyboard, or a native extension — is operating the same command line, the same selection,
//! and the same Programmer values. There is no second operator with a private copy to reconcile
//! against, and there never was one in the room.
//!
//! This used to also hold the identity the one Programmer was stored under, because persisted
//! shows, wire contracts and saved hardware configuration all named a user. None of them does.

use light_core::SessionId;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct DeskAuthority {
    command_context: Arc<RwLock<Option<SessionId>>>,
}

impl DeskAuthority {
    /// The interaction context every surface of this desk shares.
    ///
    /// Command line text, the active command target, selection gestures and Align state belong to
    /// the desk rather than to a connection. A second screen shows the command line the operator is
    /// typing on the first, because it is the same command line — that is what makes this one desk
    /// rather than several sharing a show.
    pub fn command_context(&self, presented: SessionId) -> SessionId {
        if let Some(context) = *self.command_context.read() {
            return context;
        }
        let mut context = self.command_context.write();
        *context.get_or_insert(presented)
    }

    /// The interaction context this desk has settled on, if any surface has connected yet.
    ///
    /// Read-only on purpose: asking which context the desk has must never create one.
    /// `None` means no surface has connected, so there is no command line to refresh.
    pub fn settled_command_context(&self) -> Option<SessionId> {
        *self.command_context.read()
    }

    /// Pin the interaction context, as startup does once persistence has chosen one.
    pub fn pin_command_context(&self, context: SessionId) {
        *self.command_context.write() = Some(context);
    }

    /// Forget the desk, as a reset does. The next session to arrive settles it again.
    pub fn release(&self) {
        *self.command_context.write() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_surface_types_on_the_same_command_line() {
        let authority = DeskAuthority::default();
        let main_window = SessionId::new();
        let context = authority.command_context(main_window);
        assert_eq!(context, main_window);
        assert_eq!(authority.command_context(SessionId::new()), context);
        assert_eq!(authority.command_context(SessionId::new()), context);
    }

    #[test]
    fn a_pinned_command_context_outranks_the_first_connection() {
        let authority = DeskAuthority::default();
        authority.command_context(SessionId::new());
        let restored = SessionId::new();
        authority.pin_command_context(restored);
        assert_eq!(authority.command_context(SessionId::new()), restored);
    }

    #[test]
    fn a_released_authority_settles_again_on_the_next_session() {
        let authority = DeskAuthority::default();
        authority.command_context(SessionId::new());
        authority.release();
        assert_eq!(authority.settled_command_context(), None);
        let next = SessionId::new();
        assert_eq!(authority.command_context(next), next);
    }
}
