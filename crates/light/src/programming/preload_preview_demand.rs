//! Whether the desk's Pending (Preload) preview is wanted, read from the retained Programmer.

use light_core::{ProgrammerId, SessionId};
use light_programmer::ProgrammerRegistry;
use uuid::Uuid;

/// What the retained Programmer asks of the desk's Pending preview.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreloadPreviewDemand {
    /// No retained Programmer, or Preload is neither armed nor holding active values.
    Off,
    /// A non-empty Preload Playback queue cannot be previewed.
    Queue,
    /// Preload is engaged (armed for blind capture, or holding active values).
    Engaged(ProgrammerId),
}

/// Preload engaged = armed (blind capture) or retained active Preload values. The registry is
/// desk-scoped: its Preload readers ignore the session argument.
pub fn preload_preview_demand(programmers: &ProgrammerRegistry) -> PreloadPreviewDemand {
    let desk = SessionId(Uuid::nil());
    let Some(programmer) = programmers.programmer_id() else {
        return PreloadPreviewDemand::Off;
    };
    let armed = programmers
        .capture_mode(desk)
        .is_some_and(|mode| mode.blind);
    if !armed && programmers.has_active_preload(desk) != Some(true) {
        return PreloadPreviewDemand::Off;
    }
    if programmers
        .preload_playback_actions(desk)
        .is_some_and(|queue| !queue.is_empty())
    {
        return PreloadPreviewDemand::Queue;
    }
    PreloadPreviewDemand::Engaged(programmer)
}
