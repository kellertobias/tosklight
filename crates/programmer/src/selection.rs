use crate::ProgrammerRegistry;
use crate::groups::{GroupDefinition, resolve_group};
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectionExpression {
    Static,
    LiveGroup {
        group_id: String,
        rule: SelectionRule,
    },
    FrozenGroup {
        group_id: String,
        source_revision: u64,
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
}

/// Desk-local authoritative programmer selection plus the interaction identity that produced it.
/// Attribute/value mutations change `revision` only when they close an open selection gesture.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgrammerSelection {
    pub selected: Vec<FixtureId>,
    pub expression: Option<SelectionExpression>,
    pub revision: u64,
    pub gesture_open: bool,
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
            state.last_activity = self.clock.now();
        }
        let selection = ProgrammerSelection {
            selected,
            expression: Some(expression),
            revision: self.next_selection_revision(),
            gesture_open: false,
        };
        self.selection_contexts.write().insert(
            context,
            SelectionContext {
                selected: selection.selected.clone(),
                expression: selection.expression.clone(),
                revision: selection.revision,
                gesture_open: false,
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
        let (selected, expression) = {
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
            let expression = SelectionExpression::Sources { items };
            selection.selected = selected.clone();
            selection.expression = Some(expression.clone());
            selection.revision = revision;
            selection.gesture_open = true;
            (selected, expression)
        };
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selected = selected;
            state.selection_expression = Some(expression);
            state.last_activity = self.clock.now();
        }
        true
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
                    selection.revision = self.next_selection_revision();
                }
            }
        });
    }
}
