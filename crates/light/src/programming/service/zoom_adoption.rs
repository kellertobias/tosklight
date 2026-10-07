//! TL-637 follow-up: the first sample of a Zoom component gesture needs a Zoom seed in degrees.
//!
//! Runs once per gesture, right after the transport captured its family contexts, under the
//! Programmer/desk mutation boundary and before anything is planned. Each target's seed is its
//! authored Programmer Zoom, else the opening the transport adopted from the displayed accepted
//! output (`current_values`). When any target has no Zoom seed in degrees (no compiled optics
//! model, no known convention, an unmeasurable output or a legacy percentage), the whole action
//! holds quietly with `ZoomUnavailable`: no partial edit, no revision, no Undo step, no error.
use crate::{
    ProgrammingValueIntent, ProgrammingValueOperation, ProgrammingValuesEnvironment,
    ProgrammingValuesHold,
};
use light_core::programming::ProgrammingOwner;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::collections::HashMap;

type FixtureValues = HashMap<(FixtureId, AttributeKey), AttributeValue>;

pub(super) fn prepare_zoom_adoption(
    intent: &ProgrammingValueIntent,
    members: &[FixtureId],
    environment: &mut ProgrammingValuesEnvironment,
    active: &FixtureValues,
) {
    let ProgrammingValueOperation::ComponentEdits(edits) = &intent.operation else {
        return;
    };
    if environment.displayed_source_hold.is_some()
        || edits
            .first()
            .is_none_or(|edit| edit.owner() != ProgrammingOwner::Zoom)
    {
        return;
    }
    let seeded = members.iter().all(|fixture| {
        let address = (*fixture, intent.attribute.clone());
        matches!(
            active
                .get(&address)
                .or_else(|| environment.current_values.get(&address))
                .or_else(|| environment.default_values.get(&address)),
            Some(AttributeValue::Zoom(_))
        )
    });
    if !seeded {
        environment.displayed_source_hold = Some(ProgrammingValuesHold::ZoomUnavailable);
    }
}
