//! The one Programmer a desk has.
//!
//! ToskLight is a lighting desk, not a shared document. Whoever is standing at it — through the
//! main window, an optional screen, a browser screen, an OSC client, an attached hardware surface,
//! the keyboard, or a native extension — is operating the same command line, the same selection,
//! and the same Programmer values. There is no second operator with a private copy to reconcile
//! against, and there never was one in the room.
//!
//! The registry still stores its state under a `UserId` because persisted shows, wire contracts,
//! and saved hardware configuration all name one. That identity is now a single desk-wide value
//! rather than a way to pick between Programmers: every session that starts, whatever identity it
//! arrives holding, is bound to the authority named here.

use light_core::{SessionId, UserId};
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The identity the desk's one Programmer is stored under.
///
/// Deliberately not a constant. A desk database written before this collapse already holds a
/// Programmer under some identity, and the desk has to keep operating that one rather than wake up
/// beside it under a fresh name. Startup pins the identity migration chose; a desk with nothing
/// persisted adopts the first identity a session presents and keeps it.
#[derive(Clone, Default)]
pub struct DeskAuthority {
    /// Whether this desk has been collapsed onto one Programmer yet.
    ///
    /// The collapse is a migration, not a constant, and it arrives in stages: the authority, the
    /// persistence that chooses which Programmer survives, and the operator surfaces that stop
    /// offering a choice. Until every stage is in place a desk keeps its previous behaviour
    /// exactly, so this switch is the one place that says which model is running.
    collapsed: Arc<AtomicBool>,
    user_id: Arc<RwLock<Option<UserId>>>,
    command_context: Arc<RwLock<Option<SessionId>>>,
}

impl DeskAuthority {
    /// Collapse this desk onto one Programmer.
    ///
    /// From here every session binds to one authority and one interaction context, whatever
    /// identity or context it presents.
    pub fn collapse(&self) {
        self.collapsed.store(true, Ordering::Release);
    }

    /// Whether this desk is running the one-Programmer model.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed.load(Ordering::Acquire)
    }

    /// Pin the identity the desk's Programmer is stored under.
    ///
    /// Called by startup once persistence has decided which of a legacy database's Programmers is
    /// the canonical one. Replaces whatever was adopted before it, because a deliberate choice
    /// outranks a guess made from whichever connection happened to arrive first.
    pub fn pin(&self, user_id: UserId) {
        *self.user_id.write() = Some(user_id);
    }

    /// The identity this desk's Programmer is stored under, if one has been settled on.
    pub fn pinned(&self) -> Option<UserId> {
        *self.user_id.read()
    }

    /// The identity a session must operate, whatever identity it arrived holding.
    ///
    /// The first identity to ask becomes the desk's, so a fresh installation settles on one
    /// without needing startup to invent a name for it. Every identity after that is answered with
    /// the one already in use — which is the whole point: a second connection joins the desk
    /// rather than opening a desk of its own.
    pub fn resolve(&self, presented: UserId) -> UserId {
        if !self.is_collapsed() {
            return presented;
        }
        if let Some(pinned) = *self.user_id.read() {
            return pinned;
        }
        let mut user_id = self.user_id.write();
        // Checked again under the write lock: two connections arriving together must not each
        // decide they were first.
        *user_id.get_or_insert(presented)
    }

    /// What a presented identity means on this desk, without settling the desk on it.
    ///
    /// An older client, a saved hardware configuration, or a stored URL may still name an identity
    /// from before this desk had only one. That identity is not foreign — there is nothing for it
    /// to be foreign to — so it reads as the desk's own. Read-only on purpose: answering a question
    /// about an identity must never decide which identity the desk has.
    pub fn normalize(&self, presented: UserId) -> UserId {
        if !self.is_collapsed() {
            return presented;
        }
        self.pinned().unwrap_or(presented)
    }

    /// The interaction context every surface of this desk shares.
    ///
    /// Command line text, the active command target, selection gestures and Align state belong to
    /// the desk rather than to a connection. A second screen shows the command line the operator is
    /// typing on the first, because it is the same command line — that is what makes this one desk
    /// rather than several sharing a show.
    pub fn command_context(&self, presented: SessionId) -> SessionId {
        if !self.is_collapsed() {
            return presented;
        }
        if let Some(context) = *self.command_context.read() {
            return context;
        }
        let mut context = self.command_context.write();
        *context.get_or_insert(presented)
    }

    /// Pin the interaction context, as startup does once persistence has chosen one.
    pub fn pin_command_context(&self, context: SessionId) {
        *self.command_context.write() = Some(context);
    }

    /// Forget the desk, as a reset does. The next session to arrive settles it again.
    pub fn release(&self) {
        *self.user_id.write() = None;
        *self.command_context.write() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_desk_that_has_not_collapsed_yet_keeps_every_identity_it_is_given() {
        let authority = DeskAuthority::default();
        let first = UserId::new();
        let second = UserId::new();
        assert_eq!(authority.resolve(first), first);
        assert_eq!(authority.resolve(second), second);
        assert_eq!(authority.normalize(second), second);
        assert_eq!(authority.pinned(), None);
    }

    fn collapsed() -> DeskAuthority {
        let authority = DeskAuthority::default();
        authority.collapse();
        authority
    }

    #[test]
    fn the_first_identity_to_arrive_becomes_the_desks() {
        let authority = collapsed();
        let first = UserId::new();
        assert_eq!(authority.resolve(first), first);
        assert_eq!(authority.pinned(), Some(first));
    }

    #[test]
    fn a_later_identity_joins_the_desk_rather_than_opening_another() {
        let authority = collapsed();
        let operator = authority.resolve(UserId::new());
        assert_eq!(authority.resolve(UserId::new()), operator);
        assert_eq!(authority.resolve(UserId::new()), operator);
    }

    #[test]
    fn a_pinned_identity_outranks_whichever_connection_arrived_first() {
        let authority = collapsed();
        authority.resolve(UserId::new());
        let migrated = UserId::new();
        authority.pin(migrated);
        assert_eq!(authority.resolve(UserId::new()), migrated);
    }

    #[test]
    fn a_released_authority_settles_again_on_the_next_session() {
        let authority = collapsed();
        authority.resolve(UserId::new());
        authority.command_context(SessionId::new());
        authority.release();
        assert_eq!(authority.pinned(), None);
        let next = UserId::new();
        assert_eq!(authority.resolve(next), next);
    }

    #[test]
    fn a_legacy_identity_reads_as_the_desks_own_without_claiming_it() {
        let authority = collapsed();
        let legacy = UserId::new();
        assert_eq!(
            authority.normalize(legacy),
            legacy,
            "with nothing settled there is nothing to normalise to"
        );
        assert_eq!(authority.pinned(), None, "asking must not settle the desk");

        let operator = authority.resolve(UserId::new());
        assert_eq!(authority.normalize(legacy), operator);
    }

    #[test]
    fn every_surface_types_on_the_same_command_line() {
        let authority = collapsed();
        let main_window = SessionId::new();
        let context = authority.command_context(main_window);
        assert_eq!(context, main_window);
        assert_eq!(authority.command_context(SessionId::new()), context);
        assert_eq!(authority.command_context(SessionId::new()), context);
    }

    #[test]
    fn a_pinned_command_context_outranks_the_first_connection() {
        let authority = collapsed();
        authority.command_context(SessionId::new());
        let restored = SessionId::new();
        authority.pin_command_context(restored);
        assert_eq!(authority.command_context(SessionId::new()), restored);
    }
}
