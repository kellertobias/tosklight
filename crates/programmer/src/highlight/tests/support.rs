use crate::highlight::{HighlightFixture, HighlightRegistry, HighlightTransition};
use crate::{GroupDefinition, ProgrammerSelection, SelectionExpression};
use light_core::{FixtureId, UserId};
use std::collections::HashMap;
use uuid::Uuid;

pub(super) fn fixture(number: u32) -> HighlightFixture {
    HighlightFixture {
        fixture_id: FixtureId::new(),
        name: Some(format!("Fixture {number}")),
        number: Some(number),
    }
}

pub(super) fn selection(
    selected: Vec<FixtureId>,
    expression: Option<SelectionExpression>,
    revision: u64,
) -> ProgrammerSelection {
    ProgrammerSelection {
        selected,
        expression,
        revision,
    }
}

pub(super) fn no_groups() -> HashMap<String, GroupDefinition> {
    HashMap::new()
}

pub(super) fn apply_write(
    registry: &HighlightRegistry,
    desk: Uuid,
    user: UserId,
    transition: &HighlightTransition,
    revision: u64,
) -> Option<ProgrammerSelection> {
    let write = transition.working_selection.as_ref()?;
    let selection = selection(write.selected.clone(), write.expression.clone(), revision);
    registry.acknowledge_internal_selection(desk, user, &selection);
    Some(selection)
}
