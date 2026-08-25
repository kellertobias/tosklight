use crate::{
    ProgrammerFixtureUpdate, ProgrammerGroupUpdate, ProgrammerRegistry, ProgrammerUpdateContent,
    SelectionExpression, SelectionReference,
};
use light_core::{FixtureId, SessionId};
use light_dynamics::DynamicAddressValue;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammerUpdateValue {
    Fixture(ProgrammerFixtureUpdate),
    Group(ProgrammerGroupUpdate),
    Dynamic(DynamicAddressValue),
}

impl ProgrammerUpdateValue {
    pub fn programmer_order(&self) -> u64 {
        match self {
            Self::Fixture(value) => value.programmer_order,
            Self::Group(value) => value.programmer_order,
            Self::Dynamic(value) => value.programmer_order,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProgrammerUpdateValuesCapture {
    pub revision: u64,
    pub values: Vec<ProgrammerUpdateValue>,
}

impl ProgrammerUpdateValuesCapture {
    pub fn content(&self) -> ProgrammerUpdateContent {
        let mut content = ProgrammerUpdateContent::default();
        for value in &self.values {
            match value {
                ProgrammerUpdateValue::Fixture(value) => content.fixture_values.push(value.clone()),
                ProgrammerUpdateValue::Group(value) => content.group_values.push(value.clone()),
                ProgrammerUpdateValue::Dynamic(value) => content.dynamic_values.push(value.clone()),
            }
        }
        content
    }

    pub fn into_content(self) -> ProgrammerUpdateContent {
        let mut content = ProgrammerUpdateContent::default();
        for value in self.values {
            match value {
                ProgrammerUpdateValue::Fixture(value) => content.fixture_values.push(value),
                ProgrammerUpdateValue::Group(value) => content.group_values.push(value),
                ProgrammerUpdateValue::Dynamic(value) => content.dynamic_values.push(value),
            }
        }
        content
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProgrammerUpdateSelectionCapture {
    pub revision: u64,
    pub fixtures: Vec<FixtureId>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProgrammerUpdateMenuCapture {
    pub values_revision: u64,
    pub selection_revision: u64,
    pub values: Vec<ProgrammerUpdateValue>,
    pub selected_fixtures: Vec<FixtureId>,
    pub active_preset_id: Option<String>,
    pub referenced_group_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammerUpdateCaptureError {
    MissingSession,
}

impl ProgrammerRegistry {
    /// Captures only normal recordable values in global Programmer order.
    pub fn capture_update_values(
        &self,
        session: SessionId,
    ) -> Result<ProgrammerUpdateValuesCapture, ProgrammerUpdateCaptureError> {
        let states = self.states.read();
        let state = states
            .get(&self.key(session))
            .ok_or(ProgrammerUpdateCaptureError::MissingSession)?;
        let content = state.update_content_with_selection(&[]);
        let mut values = content
            .fixture_values
            .into_iter()
            .map(ProgrammerUpdateValue::Fixture)
            .chain(
                content
                    .group_values
                    .into_iter()
                    .map(ProgrammerUpdateValue::Group),
            )
            .chain(
                content
                    .dynamic_values
                    .into_iter()
                    .map(ProgrammerUpdateValue::Dynamic),
            )
            .collect::<Vec<_>>();
        values.sort_by(compare_values);
        Ok(ProgrammerUpdateValuesCapture {
            revision: self.normal_values_revision(),
            values,
        })
    }

    /// Captures the exact ordered desk-local selection without shared Programmer state.
    pub fn capture_update_selection(
        &self,
        session: SessionId,
    ) -> Result<ProgrammerUpdateSelectionCapture, ProgrammerUpdateCaptureError> {
        let states = self.states.read();
        states
            .get(&self.key(session))
            .ok_or(ProgrammerUpdateCaptureError::MissingSession)?;
        let selections = self.selection_contexts.read();
        let selection = selections.get(&self.command_context(session));
        Ok(ProgrammerUpdateSelectionCapture {
            revision: selection.map_or(0, |selection| selection.revision),
            fixtures: selection.map_or_else(Vec::new, |selection| selection.selected.clone()),
        })
    }

    /// Captures the complete narrow Update-menu input without cloning compatibility Programmer
    /// state or mixing another desk's selection into this desk's Group candidates.
    pub fn capture_update_menu(
        &self,
        session: SessionId,
    ) -> Result<ProgrammerUpdateMenuCapture, ProgrammerUpdateCaptureError> {
        let states = self.states.read();
        let state = states
            .get(&self.key(session))
            .ok_or(ProgrammerUpdateCaptureError::MissingSession)?;
        let mut values = update_values(state.update_content_with_selection(&[]));
        values.sort_by(compare_values);
        let selections = self.selection_contexts.read();
        let selection = selections.get(&self.command_context(session));
        let selected_fixtures = selection.map_or_else(Vec::new, |value| value.selected.clone());
        let mut group_ids = referenced_groups(
            &values,
            selection.and_then(|value| value.expression.as_ref()),
        );
        group_ids.sort();
        Ok(ProgrammerUpdateMenuCapture {
            values_revision: self.normal_values_revision(),
            selection_revision: selection.map_or(0, |value| value.revision),
            values,
            selected_fixtures,
            active_preset_id: state
                .active_context
                .as_deref()
                .and_then(|context| context.strip_prefix("preset:"))
                .map(str::to_owned),
            referenced_group_ids: group_ids,
        })
    }
}

fn update_values(content: ProgrammerUpdateContent) -> Vec<ProgrammerUpdateValue> {
    content
        .fixture_values
        .into_iter()
        .map(ProgrammerUpdateValue::Fixture)
        .chain(
            content
                .group_values
                .into_iter()
                .map(ProgrammerUpdateValue::Group),
        )
        .chain(
            content
                .dynamic_values
                .into_iter()
                .map(ProgrammerUpdateValue::Dynamic),
        )
        .collect()
}

fn referenced_groups(
    values: &[ProgrammerUpdateValue],
    expression: Option<&SelectionExpression>,
) -> Vec<String> {
    let mut ids = values
        .iter()
        .filter_map(|value| match value {
            ProgrammerUpdateValue::Group(value) => Some(value.group_id.clone()),
            ProgrammerUpdateValue::Fixture(_) | ProgrammerUpdateValue::Dynamic(_) => None,
        })
        .collect::<BTreeSet<_>>();
    collect_expression_groups(expression, &mut ids);
    ids.into_iter().collect()
}

fn collect_expression_groups(expression: Option<&SelectionExpression>, ids: &mut BTreeSet<String>) {
    match expression {
        Some(SelectionExpression::LiveGroup { group_id, .. }) => {
            ids.insert(group_id.clone());
        }
        Some(SelectionExpression::Sources { items }) => collect_reference_groups(items, ids),
        Some(SelectionExpression::Static | SelectionExpression::PlaybackContents { .. }) | None => {
        }
    }
}

fn collect_reference_groups(items: &[SelectionReference], ids: &mut BTreeSet<String>) {
    for item in items {
        if let SelectionReference::LiveGroup { group_id }
        | SelectionReference::RemoveLiveGroup { group_id } = item
        {
            ids.insert(group_id.clone());
        }
    }
}

fn compare_values(left: &ProgrammerUpdateValue, right: &ProgrammerUpdateValue) -> Ordering {
    left.programmer_order()
        .cmp(&right.programmer_order())
        .then_with(|| compare_identity(left, right))
}

fn compare_identity(left: &ProgrammerUpdateValue, right: &ProgrammerUpdateValue) -> Ordering {
    match (left, right) {
        (ProgrammerUpdateValue::Fixture(left), ProgrammerUpdateValue::Fixture(right)) => left
            .fixture_id
            .0
            .cmp(&right.fixture_id.0)
            .then_with(|| left.attribute.cmp(&right.attribute)),
        (ProgrammerUpdateValue::Group(left), ProgrammerUpdateValue::Group(right)) => left
            .group_id
            .cmp(&right.group_id)
            .then_with(|| left.attribute.cmp(&right.attribute)),
        (ProgrammerUpdateValue::Dynamic(left), ProgrammerUpdateValue::Dynamic(right)) => left
            .fixture_id
            .0
            .cmp(&right.fixture_id.0)
            .then_with(|| left.attribute.cmp(&right.attribute)),
        (ProgrammerUpdateValue::Fixture(_), _) => Ordering::Less,
        (_, ProgrammerUpdateValue::Fixture(_)) => Ordering::Greater,
        (ProgrammerUpdateValue::Group(_), ProgrammerUpdateValue::Dynamic(_)) => Ordering::Less,
        (ProgrammerUpdateValue::Dynamic(_), ProgrammerUpdateValue::Group(_)) => Ordering::Greater,
    }
}
