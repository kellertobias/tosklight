//! What a control surface is allowed to do to the desk.
//!
//! The desk has one Programmer and one programming user, who may be standing at any number of
//! surfaces at once — the main window, an optional screen, a browser session, an OSC wing. Every
//! one of those behaves identically, because they are views onto the same Programmer.
//!
//! What still has to differ is whether a surface may *program* at all. The operating case is
//! ordinary: the main operator records a cue while somebody else turns a work light on or runs a
//! virtual playback, and neither disturbs the other. That guest is not a second user with a
//! Programmer of its own — it is the same desk, addressed by a surface that may only operate
//! playback.

use serde::{Deserialize, Serialize};

/// Whether a surface may change programming, or only operate playback.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceCapability {
    /// A surface of the one programming user. Accepts the full command set, including Record,
    /// Update and the Assign keyword, exactly as the main window does.
    #[default]
    Programming,
    /// A guest. May present and operate read-only fixture sheets, Stage views, virtual playbacks,
    /// macros and timecodes, but may not record, update, assign, or otherwise change programming.
    PlaybackOnly,
}

impl SurfaceCapability {
    /// Whether this surface may change programming.
    pub const fn may_program(self) -> bool {
        matches!(self, Self::Programming)
    }

    /// Whether this surface is a non-programming guest.
    pub const fn is_guest(self) -> bool {
        matches!(self, Self::PlaybackOnly)
    }
}
