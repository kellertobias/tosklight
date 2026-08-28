//! What a zone does when somebody walks into it.
//!
//! It runs a Macro, and that is the whole design. The operator already has a way to say "turn
//! this playback on" — the Macro editor, with the command line they use everywhere else — and a
//! zone that runs one is a zone whose behaviour they can read, test by pressing it, and change
//! without learning a second vocabulary. A leaving macro is how "and turn it off again" is
//! configured, and leaving it unset is how a zone that should not is.
//!
//! The Macro runs as the desk, on a connected session, exactly as an OSC surface's does. A desk
//! with no session connected runs nothing: there is nobody the action would belong to, and the
//! marker will still be there when somebody opens the desk.

use uuid::Uuid;

use super::super::AppState;
use super::zones::ZoneTransition;

pub(in crate::runtime) fn run(state: &AppState, zone_id: Uuid, transition: ZoneTransition) {
    let configuration = state.psn.configuration();
    let Some(zone) = configuration.zones.iter().find(|zone| zone.id == zone_id) else {
        return;
    };
    let macro_id = match transition {
        ZoneTransition::Entered => zone.enter_macro_id,
        ZoneTransition::Left => zone.leave_macro_id,
    };
    let Some(macro_id) = macro_id else {
        return;
    };
    if let Err(error) = super::super::macros_v2::start_macro_from_tracking(state, macro_id) {
        tracing::warn!(
            zone = %zone.name,
            %macro_id,
            error = %error.message,
            "a tracking zone could not run its Macro"
        );
    }
}
