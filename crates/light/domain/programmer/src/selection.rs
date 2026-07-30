use crate::ProgrammerRegistry;
use crate::groups::{GroupDefinition, resolve_group};
use crate::selection_grid::{
    ColumnsFirstTraversal, GridMethodConfiguration, GridTraversalAxis, RowsFirstTraversal,
    SelectionGrid,
};
use light_core::{FixtureId, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectionRule {
    All,
    Odd,
    Even,
    EveryNth { n: usize, offset: usize },
}
impl SelectionRule {
    pub fn validate(&self) -> Result<(), String> {
        if matches!(self, Self::EveryNth { n: 0, .. }) {
            Err("every-Nth selection requires N to be at least 1".into())
        } else {
            Ok(())
        }
    }
}

// @tour ordered-selection:10 Preserve selection intent
// Selection authority retains static members, live Groups, Playback contents, or ordered add and
// remove sources. The expression is richer than the currently highlighted fixture IDs.

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectionExpression {
    Static,
    LiveGroup {
        group_id: String,
        rule: SelectionRule,
    },
    PlaybackContents {
        items: Vec<SelectionReference>,
    },
    /// Ordered operator sources from a mixed command or consecutive surface gestures. References
    /// remain live and add/remove operations are replayed left-to-right whenever Groups change.
    Sources {
        items: Vec<SelectionReference>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectionReference {
    Fixture { fixture_id: FixtureId },
    LiveGroup { group_id: String },
    RemoveFixture { fixture_id: FixtureId },
    RemoveLiveGroup { group_id: String },
}

pub fn apply_selection_rule(fixtures: &[FixtureId], rule: &SelectionRule) -> Vec<FixtureId> {
    fixtures
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, fixture)| {
            let one_based = index + 1;
            let selected = match rule {
                SelectionRule::All => true,
                SelectionRule::Odd => one_based % 2 == 1,
                SelectionRule::Even => one_based % 2 == 0,
                SelectionRule::EveryNth { n, offset } => {
                    *n > 0 && index >= *offset && (index - *offset) % *n == 0
                }
            };
            selected.then_some(fixture)
        })
        .collect()
}

// @tour ordered-selection:30 Replay ordered add and remove sources
// References apply left-to-right, preserving first appearance and deterministic removals.
// Missing live references are skipped rather than becoming invented empty selections.
pub fn resolve_selection_references(
    items: &[SelectionReference],
    groups: &HashMap<String, GroupDefinition>,
) -> Vec<FixtureId> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        match item {
            SelectionReference::Fixture { fixture_id } => {
                if seen.insert(*fixture_id) {
                    selected.push(*fixture_id);
                }
            }
            SelectionReference::LiveGroup { group_id } => {
                if let Ok(fixtures) = resolve_group(group_id, groups) {
                    for fixture_id in fixtures {
                        if seen.insert(fixture_id) {
                            selected.push(fixture_id);
                        }
                    }
                }
            }
            SelectionReference::RemoveFixture { fixture_id } => {
                selected.retain(|candidate| candidate != fixture_id);
                seen.remove(fixture_id);
            }
            SelectionReference::RemoveLiveGroup { group_id } => {
                if let Ok(fixtures) = resolve_group(group_id, groups) {
                    for fixture_id in fixtures {
                        selected.retain(|candidate| *candidate != fixture_id);
                        seen.remove(&fixture_id);
                    }
                }
            }
        }
    }
    selected
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SelectionContext {
    pub(crate) selected: Vec<FixtureId>,
    pub(crate) expression: Option<SelectionExpression>,
    /// Monotonic identity of the last authoritative selection or gesture-boundary operation. This
    /// changes when an operator deliberately re-selects the same members and when a value closes
    /// an open gesture, keeping the complete projected interaction context versioned.
    pub(crate) revision: u64,
    /// True only while consecutive ordinary surface selections are being accumulated. A value
    /// entry or an explicit selection/clear operation closes the gesture.
    pub(crate) gesture_open: bool,
    pub(crate) grid: SelectionGridState,
}

/// Persisted configuration and independent traversal cursors for one ordered selection.
///
/// Grid cells are derived from current Stage positions and are deliberately not persisted.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(default)]
pub struct SelectionGridState {
    pub configuration: GridMethodConfiguration,
    pub rows_first: RowsFirstTraversal,
    pub columns_first: ColumnsFirstTraversal,
}

impl SelectionGridState {
    fn reset_traversals(&mut self) {
        self.rows_first = RowsFirstTraversal::default();
        self.columns_first = ColumnsFirstTraversal::default();
    }
}

/// Desk-local authoritative programmer selection plus the interaction identity that produced it.
/// Attribute/value mutations change `revision` only when they close an open selection gesture.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgrammerSelection {
    pub selected: Vec<FixtureId>,
    pub expression: Option<SelectionExpression>,
    pub revision: u64,
    pub gesture_open: bool,
    pub grid: SelectionGridState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionReplaceError {
    UnknownSession,
    RevisionConflict { expected: u64, actual: u64 },
}

impl ProgrammerRegistry {
    pub fn replace_selection_if_revision(
        &self,
        session: SessionId,
        expected_revision: u64,
        fixtures: impl IntoIterator<Item = FixtureId>,
        expression: SelectionExpression,
    ) -> Result<ProgrammerSelection, SelectionReplaceError> {
        self.replace_selection_with_grid_if_revision(
            session,
            expected_revision,
            fixtures,
            expression,
            GridMethodConfiguration::default(),
        )
    }

    pub fn replace_selection_with_grid_if_revision(
        &self,
        session: SessionId,
        expected_revision: u64,
        fixtures: impl IntoIterator<Item = FixtureId>,
        expression: SelectionExpression,
        configuration: GridMethodConfiguration,
    ) -> Result<ProgrammerSelection, SelectionReplaceError> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains_key(&session) {
            return Err(SelectionReplaceError::UnknownSession);
        }
        let context = self.command_context(session);
        let actual_revision = self
            .selection_contexts
            .read()
            .get(&context)
            .map_or(0, |selection| selection.revision);
        if actual_revision != expected_revision {
            return Err(SelectionReplaceError::RevisionConflict {
                expected: expected_revision,
                actual: actual_revision,
            });
        }
        let mut seen = HashSet::new();
        let selected = fixtures
            .into_iter()
            .filter(|fixture| seen.insert(*fixture))
            .collect::<Vec<_>>();
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selected = selected.clone();
            state.selection_expression = Some(expression.clone());
            state.selection_grid = SelectionGridState {
                configuration,
                ..SelectionGridState::default()
            };
            state.last_activity = self.clock.now();
        }
        let selection = ProgrammerSelection {
            selected,
            expression: Some(expression),
            revision: self.next_selection_revision(),
            gesture_open: false,
            grid: SelectionGridState {
                configuration,
                ..SelectionGridState::default()
            },
        };
        self.selection_contexts.write().insert(
            context,
            SelectionContext {
                selected: selection.selected.clone(),
                expression: selection.expression.clone(),
                revision: selection.revision,
                gesture_open: false,
                grid: selection.grid,
            },
        );
        Ok(selection)
    }

    pub fn select(&self, session: SessionId, fixtures: impl IntoIterator<Item = FixtureId>) -> u64 {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let mut seen = HashSet::new();
        let selected = fixtures
            .into_iter()
            .filter(|fixture| seen.insert(*fixture))
            .collect::<Vec<_>>();
        let expression = Some(SelectionExpression::Static);
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            // Keep a serializable projection for legacy persistence. Reads are projected from the
            // desk-local selection context below.
            state.selected = selected.clone();
            state.selection_expression = expression.clone();
            state.selection_grid = SelectionGridState::default();
            state.last_activity = self.clock.now();
        }
        let revision = self.next_selection_revision();
        self.selection_contexts.write().insert(
            self.command_context(session),
            SelectionContext {
                selected,
                expression,
                revision,
                gesture_open: false,
                grid: SelectionGridState::default(),
            },
        );
        revision
    }
    pub fn select_expression(
        &self,
        session: SessionId,
        fixtures: Vec<FixtureId>,
        expression: SelectionExpression,
    ) -> u64 {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selected = fixtures.clone();
            state.selection_expression = Some(expression.clone());
            state.selection_grid = SelectionGridState::default();
            state.last_activity = self.clock.now();
        }
        let revision = self.next_selection_revision();
        self.selection_contexts.write().insert(
            self.command_context(session),
            SelectionContext {
                selected: fixtures,
                expression: Some(expression),
                revision,
                gesture_open: false,
                grid: SelectionGridState::default(),
            },
        );
        revision
    }

    /// Apply one ordinary UI selection gesture. Consecutive calls on the same desk accumulate;
    /// selection on another desk is independent even when both sessions share programmer values.
    pub fn apply_selection_gesture(
        &self,
        session: SessionId,
        references: Vec<SelectionReference>,
        groups: &HashMap<String, GroupDefinition>,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains_key(&session) {
            return false;
        }
        let context = self.command_context(session);
        let revision = self.next_selection_revision();
        let (selected, expression, grid) = {
            let mut selections = self.selection_contexts.write();
            let selection = selections.entry(context).or_default();
            let mut items = if selection.gesture_open {
                match selection.expression.clone() {
                    Some(SelectionExpression::Sources { items }) => items,
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            items.extend(references);
            let selected = resolve_selection_references(&items, groups);
            let configuration =
                common_grid_configuration(&items, groups, selection.grid.configuration);
            let expression = SelectionExpression::Sources { items };
            selection.selected = selected.clone();
            selection.expression = Some(expression.clone());
            selection.revision = revision;
            selection.gesture_open = true;
            selection.grid.configuration = configuration;
            selection.grid.reset_traversals();
            (selected, expression, selection.grid)
        };
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selected = selected;
            state.selection_expression = Some(expression);
            state.selection_grid = grid;
            state.last_activity = self.clock.now();
        }
        true
    }

    pub fn set_selection_grid_configuration_if_revision(
        &self,
        session: SessionId,
        expected_revision: u64,
        configuration: GridMethodConfiguration,
    ) -> Result<ProgrammerSelection, SelectionReplaceError> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains_key(&session) {
            return Err(SelectionReplaceError::UnknownSession);
        }
        let context = self.command_context(session);
        let actual_revision = self
            .selection_contexts
            .read()
            .get(&context)
            .map_or(0, |selection| selection.revision);
        if actual_revision != expected_revision {
            return Err(SelectionReplaceError::RevisionConflict {
                expected: expected_revision,
                actual: actual_revision,
            });
        }
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selection_grid.configuration = configuration;
            state.selection_grid.reset_traversals();
            state.last_activity = self.clock.now();
        }
        let revision = self.next_selection_revision();
        let mut selections = self.selection_contexts.write();
        let selection = selections.entry(context).or_default();
        selection.grid.configuration = configuration;
        selection.grid.reset_traversals();
        selection.revision = revision;
        selection.gesture_open = false;
        Ok(ProgrammerSelection {
            selected: selection.selected.clone(),
            expression: selection.expression.clone(),
            revision,
            gesture_open: false,
            grid: selection.grid,
        })
    }

    pub fn cycle_selection_grid_method(&self, session: SessionId) -> Option<ProgrammerSelection> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let current = self.selection(session)?;
        let mut configuration = current.grid.configuration;
        configuration.method = configuration.method.next();
        self.set_selection_grid_configuration_if_revision(session, current.revision, configuration)
            .ok()
    }

    pub fn reorder_selection_from_grid(
        &self,
        session: SessionId,
        grid: &SelectionGrid,
        axis: GridTraversalAxis,
    ) -> Option<ProgrammerSelection> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains_key(&session) {
            return None;
        }
        let context = self.command_context(session);
        let (selected, grid_state) = {
            let mut selections = self.selection_contexts.write();
            let selection = selections.get_mut(&context)?;
            let expected = selection.selected.iter().copied().collect::<HashSet<_>>();
            let actual = grid
                .cells
                .iter()
                .map(|cell| cell.fixture_id)
                .collect::<HashSet<_>>();
            if expected.len() != selection.selected.len()
                || actual.len() != grid.cells.len()
                || actual != expected
            {
                return None;
            }
            let selected = match axis {
                GridTraversalAxis::Rows => {
                    let selected = grid.rows_first(selection.grid.rows_first);
                    selection.grid.rows_first = selection.grid.rows_first.next();
                    selected
                }
                GridTraversalAxis::Columns => {
                    let selected = grid.columns_first(selection.grid.columns_first);
                    selection.grid.columns_first = selection.grid.columns_first.next();
                    selected
                }
            };
            selection.selected.clone_from(&selected);
            selection.expression = Some(SelectionExpression::Static);
            selection.revision = self.next_selection_revision();
            selection.gesture_open = false;
            (selected, selection.grid)
        };
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selected.clone_from(&selected);
            state.selection_expression = Some(SelectionExpression::Static);
            state.selection_grid = grid_state;
            state.last_activity = self.clock.now();
        }
        self.selection(session)
    }

    pub fn refresh_live_selections(&self, groups: &HashMap<String, GroupDefinition>) {
        self.with_all_mutation_gates(|| {
            for selection in self.selection_contexts.write().values_mut() {
                let resolved = match selection.expression.clone() {
                    Some(SelectionExpression::LiveGroup { group_id, rule }) => {
                        resolve_group(&group_id, groups)
                            .ok()
                            .map(|fixtures| apply_selection_rule(&fixtures, &rule))
                    }
                    Some(
                        SelectionExpression::PlaybackContents { items }
                        | SelectionExpression::Sources { items },
                    ) => Some(resolve_selection_references(&items, groups)),
                    _ => None,
                };
                if let Some(resolved) = resolved
                    && selection.selected != resolved
                {
                    selection.selected = resolved;
                    selection.grid.reset_traversals();
                    selection.revision = self.next_selection_revision();
                }
            }
        });
    }
}

fn common_grid_configuration(
    references: &[SelectionReference],
    groups: &HashMap<String, GroupDefinition>,
    current: GridMethodConfiguration,
) -> GridMethodConfiguration {
    let mut common = None;
    for reference in references {
        let configuration = match reference {
            SelectionReference::LiveGroup { group_id } => {
                let Some(group) = groups.get(group_id) else {
                    return GridMethodConfiguration::default();
                };
                group.grid
            }
            SelectionReference::Fixture { .. }
            | SelectionReference::RemoveFixture { .. }
            | SelectionReference::RemoveLiveGroup { .. } => continue,
        };
        match common {
            None => common = Some(configuration),
            Some(current) if current == configuration => {}
            Some(_) => return GridMethodConfiguration::default(),
        }
    }
    common.unwrap_or(current)
}
