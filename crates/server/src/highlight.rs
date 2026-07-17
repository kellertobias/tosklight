//! Authoritative transient Highlight and programmer-selection stepping shared by every control
//! surface.

use light_core::{FixtureId, UserId};
use light_programmer::{
    GroupDefinition, ProgrammerSelection, SelectionExpression, apply_selection_rule, resolve_group,
    resolve_selection_references,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// OSC buttons commonly repeat a press while a physical contact settles. Treat aliases for the
/// same authoritative action as one press inside this window, while allowing another action or a
/// deliberate later press through immediately.
pub const OSC_REPEAT_GUARD: Duration = Duration::from_millis(150);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HighlightFixture {
    pub fixture_id: FixtureId,
    pub name: Option<String>,
    pub number: Option<u32>,
}

/// Selection stepping is independent of whether HIGH is active. `Selection` means the complete
/// remembered source is the actual programmer selection; `Step` means one item is selected.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightMode {
    Selection,
    Step,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightAction {
    On,
    Off,
    Toggle,
    Next,
    Previous,
    All,
}

impl HighlightAction {
    pub const fn osc_dedupe_key(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Toggle => "toggle",
            Self::Next => "next",
            Self::Previous => "previous",
            Self::All => "all",
        }
    }
}

pub fn is_duplicate_osc_action(
    previous: Option<(&str, Instant)>,
    action: HighlightAction,
    received_at: Instant,
) -> bool {
    previous.is_some_and(|(previous, previous_at)| {
        previous == action.osc_dedupe_key()
            && received_at.saturating_duration_since(previous_at) < OSC_REPEAT_GUARD
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct HighlightState {
    /// HIGH state only. This is deliberately independent of `mode` and selection emptiness.
    pub active: bool,
    pub mode: HighlightMode,
    pub output_enabled: bool,
    /// Compatibility name for the Blind/Preview output-suppression state.
    pub capture_only: bool,
    /// Current valid resolution of the remembered live selection source.
    pub remembered: Vec<HighlightFixture>,
    pub active_index: Option<usize>,
    pub active_fixture: Option<HighlightFixture>,
    pub can_previous: bool,
    pub can_next: bool,
    pub owner_user_id: Option<UserId>,
    pub owner_user_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HighlightSelectionWrite {
    pub selected: Vec<FixtureId>,
    pub expression: Option<SelectionExpression>,
}

#[derive(Clone, Debug)]
pub struct HighlightTransition {
    pub state: HighlightState,
    /// Fixture identities whose raw Highlight Look should be overlaid by the engine.
    pub output_fixtures: Vec<FixtureId>,
    /// Authoritative actual programmer selection requested by PREV, NEXT, ALL, or reconciliation
    /// after an item disappeared. Attribute values are never touched by this write.
    pub working_selection: Option<HighlightSelectionWrite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HighlightError {
    OwnedByAnotherUser(UserId),
}

impl std::fmt::Display for HighlightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OwnedByAnotherUser(_) => {
                formatter.write_str("Highlight output is active for another user on this desk")
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct OperatorState {
    active: bool,
    output_enabled: bool,
    remembered: Vec<FixtureId>,
    remembered_expression: Option<SelectionExpression>,
    stepping: bool,
    active_fixture: Option<FixtureId>,
    /// Revision of the actual programmer selection last observed or explicitly acknowledged as
    /// our own PREV/NEXT/ALL write.
    observed_selection_revision: Option<u64>,
    message: Option<String>,
}

#[derive(Clone, Default)]
struct HighlightRuntime {
    operators: HashMap<(Uuid, UserId), OperatorState>,
    output_owners: HashMap<Uuid, UserId>,
}

type OperatorKey = (Uuid, UserId);
type RecentHighlightActions = HashMap<OperatorKey, (&'static str, Instant)>;

#[derive(Default)]
pub struct HighlightRegistry {
    runtime: Mutex<HighlightRuntime>,
    recent_actions: Mutex<RecentHighlightActions>,
}

impl HighlightRegistry {
    /// Apply an operator-facing action with one repeat guard shared by REST, software, OSC, and
    /// attached hardware. The guard belongs here so two adapters cannot advance the actual
    /// selection independently during a single physical press.
    #[allow(clippy::too_many_arguments)]
    pub fn action_guarded(
        &self,
        desk_id: Uuid,
        user_id: UserId,
        user_name: Option<&str>,
        action: HighlightAction,
        current_selection: &ProgrammerSelection,
        valid_fixtures: &[HighlightFixture],
        groups: &HashMap<String, GroupDefinition>,
        capture_only: bool,
    ) -> Result<HighlightTransition, HighlightError> {
        let received_at = Instant::now();
        let key = (desk_id, user_id);
        let mut recent_actions = self.recent_actions.lock();
        if is_duplicate_osc_action(
            recent_actions
                .get(&key)
                .map(|(previous, previous_at)| (*previous, *previous_at)),
            action,
            received_at,
        ) {
            drop(recent_actions);
            return Ok(self.status(
                desk_id,
                user_id,
                user_name,
                current_selection,
                valid_fixtures,
                groups,
                capture_only,
            ));
        }
        let transition = self.action(
            desk_id,
            user_id,
            user_name,
            action,
            current_selection,
            valid_fixtures,
            groups,
            capture_only,
        )?;
        recent_actions.insert(key, (action.osc_dedupe_key(), received_at));
        Ok(transition)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn action(
        &self,
        desk_id: Uuid,
        user_id: UserId,
        user_name: Option<&str>,
        action: HighlightAction,
        current_selection: &ProgrammerSelection,
        valid_fixtures: &[HighlightFixture],
        groups: &HashMap<String, GroupDefinition>,
        capture_only: bool,
    ) -> Result<HighlightTransition, HighlightError> {
        // Stage the whole transition so an ownership failure cannot partially toggle HIGH or move
        // the programmer selection.
        let mut live_runtime = self.runtime.lock();
        let mut runtime = live_runtime.clone();
        let key = (desk_id, user_id);
        let mut operator = runtime.operators.remove(&key).unwrap_or_default();
        let mut working_selection =
            synchronize_actual_selection(&mut operator, current_selection, valid_fixtures, groups);
        reconcile_capture_mode(&mut runtime, desk_id, user_id, &mut operator, capture_only);
        operator.message = None;

        match action {
            HighlightAction::On => {
                enable_highlight(&mut runtime, desk_id, user_id, &mut operator, capture_only)?;
            }
            HighlightAction::Off => {
                disable_highlight(&mut runtime, desk_id, user_id, &mut operator);
            }
            HighlightAction::Toggle => {
                if operator.active {
                    disable_highlight(&mut runtime, desk_id, user_id, &mut operator);
                } else {
                    enable_highlight(&mut runtime, desk_id, user_id, &mut operator, capture_only)?;
                }
            }
            HighlightAction::Next | HighlightAction::Previous => {
                operator.remembered = resolve_remembered(&operator, valid_fixtures, groups);
                if operator.remembered.is_empty() {
                    operator.stepping = true;
                    operator.active_fixture = None;
                    working_selection = Some(HighlightSelectionWrite {
                        selected: Vec::new(),
                        expression: Some(SelectionExpression::Static),
                    });
                    operator.message = Some("The remembered selection has no valid items".into());
                } else {
                    let index = if operator.stepping {
                        operator
                            .active_fixture
                            .and_then(|active| {
                                operator
                                    .remembered
                                    .iter()
                                    .position(|fixture| *fixture == active)
                            })
                            .map(|index| match action {
                                HighlightAction::Next => (index + 1) % operator.remembered.len(),
                                HighlightAction::Previous => {
                                    (index + operator.remembered.len() - 1)
                                        % operator.remembered.len()
                                }
                                _ => unreachable!(),
                            })
                            .unwrap_or_else(|| {
                                if action == HighlightAction::Next {
                                    0
                                } else {
                                    operator.remembered.len() - 1
                                }
                            })
                    } else if action == HighlightAction::Next {
                        0
                    } else {
                        operator.remembered.len() - 1
                    };
                    let fixture = operator.remembered[index];
                    operator.stepping = true;
                    operator.active_fixture = Some(fixture);
                    working_selection = Some(HighlightSelectionWrite {
                        selected: vec![fixture],
                        expression: Some(SelectionExpression::Static),
                    });
                }
            }
            HighlightAction::All => {
                operator.remembered = resolve_remembered(&operator, valid_fixtures, groups);
                if operator.stepping {
                    operator.stepping = false;
                    operator.active_fixture = None;
                    working_selection = Some(HighlightSelectionWrite {
                        selected: operator.remembered.clone(),
                        expression: operator.remembered_expression.clone(),
                    });
                }
            }
        }

        let output_fixtures = output_fixture_ids(&operator);
        let owner = runtime.output_owners.get(&desk_id).copied();
        let state = response(
            &operator,
            valid_fixtures,
            capture_only,
            owner,
            (owner == Some(user_id))
                .then(|| user_name.map(str::to_owned))
                .flatten(),
        );
        runtime.operators.insert(key, operator);
        *live_runtime = runtime;
        Ok(HighlightTransition {
            state,
            output_fixtures,
            working_selection,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn status(
        &self,
        desk_id: Uuid,
        user_id: UserId,
        user_name: Option<&str>,
        current_selection: &ProgrammerSelection,
        valid_fixtures: &[HighlightFixture],
        groups: &HashMap<String, GroupDefinition>,
        capture_only: bool,
    ) -> HighlightTransition {
        let mut runtime = self.runtime.lock();
        let key = (desk_id, user_id);
        let mut operator = runtime.operators.remove(&key).unwrap_or_default();
        let working_selection =
            synchronize_actual_selection(&mut operator, current_selection, valid_fixtures, groups);
        reconcile_capture_mode(&mut runtime, desk_id, user_id, &mut operator, capture_only);
        if operator.active && !capture_only && !operator.output_enabled {
            if runtime
                .output_owners
                .get(&desk_id)
                .is_none_or(|owner| *owner == user_id)
            {
                runtime.output_owners.insert(desk_id, user_id);
                operator.output_enabled = true;
                operator.message = None;
            } else {
                operator.message =
                    Some("Highlight output is active for another user on this desk".into());
            }
        }
        let owner = runtime.output_owners.get(&desk_id).copied();
        let output_fixtures = output_fixture_ids(&operator);
        let state = response(
            &operator,
            valid_fixtures,
            capture_only,
            owner,
            (owner == Some(user_id))
                .then(|| user_name.map(str::to_owned))
                .flatten(),
        );
        runtime.operators.insert(key, operator);
        HighlightTransition {
            state,
            output_fixtures,
            working_selection,
        }
    }

    /// Acknowledge the programmer selection revision written by one of this registry's own
    /// PREV/NEXT/ALL transitions. The next status call therefore does not mistake it for an
    /// external operator selection, while any later external write (even identical membership)
    /// has a new revision and resets the step basis.
    pub fn acknowledge_internal_selection(
        &self,
        desk_id: Uuid,
        user_id: UserId,
        selection: &ProgrammerSelection,
    ) {
        if let Some(operator) = self.runtime.lock().operators.get_mut(&(desk_id, user_id)) {
            operator.observed_selection_revision = Some(selection.revision);
        }
    }

    pub fn clear_desk(&self, desk_id: Uuid) {
        let mut runtime = self.runtime.lock();
        runtime.operators.retain(|(desk, _), _| *desk != desk_id);
        runtime.output_owners.remove(&desk_id);
        drop(runtime);
        self.recent_actions
            .lock()
            .retain(|(desk, _), _| *desk != desk_id);
    }

    pub fn clear_context(&self, desk_id: Uuid, user_id: UserId) {
        let mut runtime = self.runtime.lock();
        runtime.operators.remove(&(desk_id, user_id));
        if runtime.output_owners.get(&desk_id) == Some(&user_id) {
            runtime.output_owners.remove(&desk_id);
        }
        drop(runtime);
        self.recent_actions.lock().remove(&(desk_id, user_id));
    }

    pub fn clear_user(&self, user_id: UserId) {
        let mut runtime = self.runtime.lock();
        runtime.operators.retain(|(_, user), _| *user != user_id);
        runtime.output_owners.retain(|_, owner| *owner != user_id);
        drop(runtime);
        self.recent_actions
            .lock()
            .retain(|(_, user), _| *user != user_id);
    }

    pub fn clear_all(&self) {
        let mut runtime = self.runtime.lock();
        runtime.operators.clear();
        runtime.output_owners.clear();
        drop(runtime);
        self.recent_actions.lock().clear();
    }

    /// Combined output for every isolated desk context. Programmer layers already merge across
    /// connected desks; Highlight follows the same rule while each desk retains independent step
    /// state and ownership.
    pub fn output_fixtures(&self) -> Vec<FixtureId> {
        let runtime = self.runtime.lock();
        let mut seen = HashSet::new();
        runtime
            .operators
            .values()
            .flat_map(output_fixture_ids)
            .filter(|fixture| seen.insert(*fixture))
            .collect()
    }
}

fn acquire_output_owner(
    runtime: &mut HighlightRuntime,
    desk_id: Uuid,
    user_id: UserId,
) -> Result<(), HighlightError> {
    if let Some(owner) = runtime.output_owners.get(&desk_id)
        && *owner != user_id
    {
        return Err(HighlightError::OwnedByAnotherUser(*owner));
    }
    runtime.output_owners.insert(desk_id, user_id);
    Ok(())
}

fn enable_highlight(
    runtime: &mut HighlightRuntime,
    desk_id: Uuid,
    user_id: UserId,
    operator: &mut OperatorState,
    capture_only: bool,
) -> Result<(), HighlightError> {
    operator.active = true;
    if capture_only {
        operator.output_enabled = false;
        operator.message =
            Some("Highlight prepared in Blind/Preview; live output is suppressed".into());
    } else {
        acquire_output_owner(runtime, desk_id, user_id)?;
        operator.output_enabled = true;
    }
    Ok(())
}

fn disable_highlight(
    runtime: &mut HighlightRuntime,
    desk_id: Uuid,
    user_id: UserId,
    operator: &mut OperatorState,
) {
    operator.active = false;
    operator.output_enabled = false;
    if runtime.output_owners.get(&desk_id) == Some(&user_id) {
        runtime.output_owners.remove(&desk_id);
    }
}

fn reconcile_capture_mode(
    runtime: &mut HighlightRuntime,
    desk_id: Uuid,
    user_id: UserId,
    operator: &mut OperatorState,
    capture_only: bool,
) {
    if capture_only && operator.output_enabled {
        operator.output_enabled = false;
        if runtime.output_owners.get(&desk_id) == Some(&user_id) {
            runtime.output_owners.remove(&desk_id);
        }
        operator.message =
            Some("Highlight prepared in Blind/Preview; live output is suppressed".into());
    }
}

fn valid_selection(selection: &[FixtureId], valid_fixtures: &[HighlightFixture]) -> Vec<FixtureId> {
    let valid = valid_fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    selection
        .iter()
        .copied()
        .filter(|fixture| valid.contains(fixture) && seen.insert(*fixture))
        .collect()
}

fn resolve_expression(
    expression: &SelectionExpression,
    snapshot: &[FixtureId],
    groups: &HashMap<String, GroupDefinition>,
) -> Vec<FixtureId> {
    match expression {
        SelectionExpression::LiveGroup { group_id, rule } => resolve_group(group_id, groups)
            .map(|fixtures| apply_selection_rule(&fixtures, rule))
            .unwrap_or_default(),
        SelectionExpression::PlaybackContents { items }
        | SelectionExpression::Sources { items } => resolve_selection_references(items, groups),
        // A frozen selection and an explicitly static selection retain the exact resolved order
        // present when stepping began.
        SelectionExpression::Static | SelectionExpression::FrozenGroup { .. } => snapshot.to_vec(),
    }
}

fn resolve_remembered(
    operator: &OperatorState,
    valid_fixtures: &[HighlightFixture],
    groups: &HashMap<String, GroupDefinition>,
) -> Vec<FixtureId> {
    let resolved = operator
        .remembered_expression
        .as_ref()
        .map(|expression| resolve_expression(expression, &operator.remembered, groups))
        .unwrap_or_else(|| operator.remembered.clone());
    valid_selection(&resolved, valid_fixtures)
}

fn reset_basis(
    operator: &mut OperatorState,
    current_selection: &ProgrammerSelection,
    valid_fixtures: &[HighlightFixture],
    groups: &HashMap<String, GroupDefinition>,
) {
    let snapshot = valid_selection(&current_selection.selected, valid_fixtures);
    let remembered = current_selection
        .expression
        .as_ref()
        .map(|expression| resolve_expression(expression, &snapshot, groups))
        .unwrap_or(snapshot);
    operator.remembered = valid_selection(&remembered, valid_fixtures);
    operator.remembered_expression = current_selection.expression.clone();
    operator.stepping = false;
    operator.active_fixture = None;
    operator.message = None;
    operator.observed_selection_revision = Some(current_selection.revision);
}

fn synchronize_actual_selection(
    operator: &mut OperatorState,
    current_selection: &ProgrammerSelection,
    valid_fixtures: &[HighlightFixture],
    groups: &HashMap<String, GroupDefinition>,
) -> Option<HighlightSelectionWrite> {
    if operator.observed_selection_revision != Some(current_selection.revision) {
        reset_basis(operator, current_selection, valid_fixtures, groups);
        return None;
    }

    let previous = operator.remembered.clone();
    let previous_index = operator
        .active_fixture
        .and_then(|active| previous.iter().position(|candidate| *candidate == active));
    operator.remembered = resolve_remembered(operator, valid_fixtures, groups);
    if !operator.stepping {
        return None;
    }
    if operator
        .active_fixture
        .is_some_and(|active| operator.remembered.contains(&active))
    {
        return None;
    }
    operator.active_fixture = previous_index.and_then(|index| {
        (!operator.remembered.is_empty())
            .then(|| operator.remembered[index.min(operator.remembered.len() - 1)])
    });
    operator.message = Some("Removed selection item skipped while stepping".into());
    Some(HighlightSelectionWrite {
        selected: operator.active_fixture.into_iter().collect(),
        expression: Some(SelectionExpression::Static),
    })
}

fn output_fixture_ids(operator: &OperatorState) -> Vec<FixtureId> {
    if !operator.active || !operator.output_enabled {
        return Vec::new();
    }
    if operator.stepping {
        operator.active_fixture.into_iter().collect()
    } else {
        operator.remembered.clone()
    }
}

fn response(
    operator: &OperatorState,
    fixtures: &[HighlightFixture],
    capture_only: bool,
    owner_user_id: Option<UserId>,
    owner_user_name: Option<String>,
) -> HighlightState {
    let by_id = fixtures
        .iter()
        .map(|fixture| (fixture.fixture_id, fixture.clone()))
        .collect::<HashMap<_, _>>();
    let remembered = operator
        .remembered
        .iter()
        .filter_map(|fixture| by_id.get(fixture).cloned())
        .collect::<Vec<_>>();
    let active_index = if operator.stepping {
        operator.active_fixture.and_then(|active| {
            operator
                .remembered
                .iter()
                .position(|fixture| *fixture == active)
        })
    } else {
        None
    };
    let active_fixture = operator
        .stepping
        .then_some(operator.active_fixture)
        .flatten()
        .and_then(|fixture| by_id.get(&fixture).cloned());
    let can_step = !operator.remembered.is_empty();
    HighlightState {
        active: operator.active,
        mode: if operator.stepping {
            HighlightMode::Step
        } else {
            HighlightMode::Selection
        },
        output_enabled: operator.output_enabled,
        capture_only,
        remembered,
        active_index,
        active_fixture,
        can_previous: can_step,
        can_next: can_step,
        owner_user_id,
        owner_user_name,
        message: operator.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(number: u32) -> HighlightFixture {
        HighlightFixture {
            fixture_id: FixtureId::new(),
            name: Some(format!("Fixture {number}")),
            number: Some(number),
        }
    }

    fn selection(
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

    fn no_groups() -> HashMap<String, GroupDefinition> {
        HashMap::new()
    }

    fn apply_write(
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

    #[test]
    fn prev_next_all_write_real_selection_wrap_and_never_activate_high() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3), fixture(4)];
        let ids = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        let groups = no_groups();
        let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);

        let next = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &complete,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        assert!(!next.state.active);
        assert_eq!(
            next.working_selection.as_ref().unwrap().selected,
            vec![ids[0]]
        );
        let mut actual = apply_write(&registry, desk, user, &next, 2).unwrap();
        for expected in [ids[1], ids[2], ids[3], ids[0]] {
            let next = registry
                .action(
                    desk,
                    user,
                    None,
                    HighlightAction::Next,
                    &actual,
                    &fixtures,
                    &groups,
                    false,
                )
                .unwrap();
            assert_eq!(
                next.working_selection.as_ref().unwrap().selected,
                vec![expected]
            );
            actual = apply_write(&registry, desk, user, &next, actual.revision + 1).unwrap();
        }

        let all = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::All,
                &actual,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        assert_eq!(all.state.mode, HighlightMode::Selection);
        assert_eq!(all.working_selection.as_ref().unwrap().selected, ids);
        actual = apply_write(&registry, desk, user, &all, actual.revision + 1).unwrap();
        let previous = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Previous,
                &actual,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        assert_eq!(
            previous.working_selection.as_ref().unwrap().selected,
            vec![ids[3]]
        );
        assert!(previous.state.can_next && previous.state.can_previous);
    }

    #[test]
    fn high_is_independent_accepts_empty_selection_and_follows_later_external_selection() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2)];
        let groups = no_groups();
        let empty = selection(Vec::new(), Some(SelectionExpression::Static), 1);
        let on = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::On,
                &empty,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        assert!(on.state.active);
        assert!(on.state.output_enabled);
        assert!(on.output_fixtures.is_empty());

        let selected = selection(
            vec![fixtures[1].fixture_id],
            Some(SelectionExpression::Static),
            2,
        );
        let followed = registry.status(desk, user, None, &selected, &fixtures, &groups, false);
        assert!(followed.state.active);
        assert_eq!(followed.state.mode, HighlightMode::Selection);
        assert_eq!(followed.output_fixtures, vec![fixtures[1].fixture_id]);
    }

    #[test]
    fn external_same_membership_revision_resets_step_but_value_changes_do_not() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3)];
        let ids = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        let groups = no_groups();
        let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);
        let first = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &complete,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        let stepped = apply_write(&registry, desk, user, &first, 2).unwrap();

        // Programmer values may change repeatedly while the selection revision is unchanged.
        let unchanged = registry.status(desk, user, None, &stepped, &fixtures, &groups, false);
        assert_eq!(unchanged.state.mode, HighlightMode::Step);

        // A deliberate external selection operation has a new revision even if it resolves to the
        // same singleton ID, so it becomes a new complete basis.
        let external_same = selection(stepped.selected.clone(), stepped.expression.clone(), 3);
        let reset = registry.status(desk, user, None, &external_same, &fixtures, &groups, false);
        assert_eq!(reset.state.mode, HighlightMode::Selection);
        let next = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &external_same,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        assert_eq!(next.working_selection.unwrap().selected, vec![ids[0]]);
    }

    #[test]
    fn all_reresolves_the_live_group_source_after_membership_changes() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3), fixture(4)];
        let ids = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        let mut groups = HashMap::from([(
            "1".into(),
            GroupDefinition {
                id: "1".into(),
                fixtures: ids[..3].to_vec(),
                ..Default::default()
            },
        )]);
        let complete = selection(
            ids[..3].to_vec(),
            Some(SelectionExpression::LiveGroup {
                group_id: "1".into(),
                rule: light_programmer::SelectionRule::All,
            }),
            1,
        );
        let first = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &complete,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        let stepped = apply_write(&registry, desk, user, &first, 2).unwrap();
        groups.get_mut("1").unwrap().fixtures = vec![ids[3], ids[1]];
        let all = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::All,
                &stepped,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        assert_eq!(
            all.working_selection.as_ref().unwrap().selected,
            vec![ids[3], ids[1]]
        );
        assert!(matches!(
            all.working_selection.unwrap().expression,
            Some(SelectionExpression::LiveGroup { ref group_id, .. }) if group_id == "1"
        ));
    }

    #[test]
    fn removed_items_keep_live_sequence_deterministic_and_high_active_when_empty() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3)];
        let ids = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        let groups = no_groups();
        let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);
        registry
            .action(
                desk,
                user,
                None,
                HighlightAction::On,
                &complete,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        let first = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &complete,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        let stepped = apply_write(&registry, desk, user, &first, 2).unwrap();
        let remaining = vec![fixtures[1].clone(), fixtures[2].clone()];
        let reconciled = registry.status(desk, user, None, &stepped, &remaining, &groups, false);
        assert_eq!(
            reconciled.working_selection.as_ref().unwrap().selected,
            vec![ids[1]]
        );
        assert_eq!(reconciled.output_fixtures, vec![ids[1]]);

        let corrected = apply_write(&registry, desk, user, &reconciled, 3).unwrap();
        let only_active = vec![fixtures[1].clone()];
        let inactive_removed =
            registry.status(desk, user, None, &corrected, &only_active, &groups, false);
        assert_eq!(inactive_removed.state.remembered.len(), 1);
        assert!(inactive_removed.working_selection.is_none());
        let wrapped = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &corrected,
                &only_active,
                &groups,
                false,
            )
            .unwrap();
        assert_eq!(wrapped.working_selection.unwrap().selected, vec![ids[1]]);

        let none = registry.status(desk, user, None, &corrected, &[], &groups, false);
        assert!(none.state.active);
        assert!(none.state.output_enabled);
        assert!(none.output_fixtures.is_empty());
    }

    #[test]
    fn authoritative_repeat_guard_prevents_cross_surface_double_steps() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3)];
        let ids = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        let groups = no_groups();
        let complete = selection(ids, Some(SelectionExpression::Static), 1);
        let software = registry
            .action_guarded(
                desk,
                user,
                None,
                HighlightAction::Next,
                &complete,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        let simultaneous_hardware = registry
            .action_guarded(
                desk,
                user,
                None,
                HighlightAction::Next,
                &complete,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        assert_eq!(software.state.active_index, Some(0));
        assert_eq!(simultaneous_hardware.state.active_index, Some(0));
    }

    #[test]
    fn osc_repeat_guard_normalizes_aliases_and_has_an_exact_boundary() {
        let received_at = Instant::now();
        let previous_at = received_at - Duration::from_millis(149);
        assert!(is_duplicate_osc_action(
            Some(("previous", previous_at)),
            HighlightAction::Previous,
            received_at,
        ));
        assert!(!is_duplicate_osc_action(
            Some(("next", previous_at)),
            HighlightAction::Previous,
            received_at,
        ));
        assert!(!is_duplicate_osc_action(
            Some(("previous", received_at - Duration::from_millis(150))),
            HighlightAction::Previous,
            received_at,
        ));
    }
}
