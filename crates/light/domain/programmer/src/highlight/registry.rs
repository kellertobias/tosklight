use super::model::{
    HighlightAction, HighlightError, HighlightFixture, HighlightOutputLayer, HighlightTransition,
    is_duplicate_osc_action,
};
use super::operations::{
    ActionContext, apply_action, output_fixture_ids, output_layers, reconcile_capture_mode,
    response, restore_live_output,
};
use super::selection::synchronize_actual_selection;
use super::state::{HighlightRuntime, OperatorState, RecentHighlightActions};
use crate::{GroupDefinition, ProgrammerSelection};
use light_core::{AttributeKey, FixtureId, UserId};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

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
        let mut operator = std::mem::take(&mut runtime.operator);
        let mut working_selection =
            synchronize_actual_selection(&mut operator, current_selection, valid_fixtures, groups);
        let context = ActionContext {
            user_id,
            valid_fixtures,
            groups,
            capture_only,
        };
        reconcile_capture_mode(&mut runtime, &mut operator, &context);
        operator.message = None;
        if let Some(action_selection) = apply_action(&mut runtime, &mut operator, action, &context)?
        {
            working_selection = Some(action_selection);
        }
        let transition = build_transition(
            &runtime,
            &operator,
            user_id,
            user_name,
            valid_fixtures,
            capture_only,
            working_selection,
            desk_id,
        );
        runtime.operator = operator;
        *live_runtime = runtime;
        Ok(transition)
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
        let mut operator = std::mem::take(&mut runtime.operator);
        let working_selection =
            synchronize_actual_selection(&mut operator, current_selection, valid_fixtures, groups);
        let context = ActionContext {
            user_id,
            valid_fixtures,
            groups,
            capture_only,
        };
        reconcile_capture_mode(&mut runtime, &mut operator, &context);
        restore_live_output(&mut runtime, &mut operator, &context);
        let transition = build_transition(
            &runtime,
            &operator,
            user_id,
            user_name,
            valid_fixtures,
            capture_only,
            working_selection,
            desk_id,
        );
        runtime.operator = operator;
        transition
    }

    /// Acknowledge the programmer selection revision written by one of this registry's own
    /// PREV/NEXT/ALL transitions. The next status call therefore does not mistake it for an
    /// external operator selection. While HIGH remains active, later external revisions are
    /// observed without replacing the frozen activation basis.
    pub fn acknowledge_internal_selection(
        &self,
        desk_id: Uuid,
        user_id: UserId,
        selection: &ProgrammerSelection,
    ) {
        let _ = (desk_id, user_id);
        self.runtime.lock().operator.observed_selection_revision = Some(selection.revision);
    }

    /// Suppress temporary look attributes for exact fixture addresses explicitly authored in the
    /// normal Programmer during the current Highlight activation.
    pub fn mark_explicit_fixture_attributes(
        &self,
        desk_id: Uuid,
        user_id: UserId,
        touched: impl IntoIterator<Item = (FixtureId, AttributeKey)>,
    ) -> bool {
        let _ = (desk_id, user_id);
        let mut runtime = self.runtime.lock();
        let operator = &mut runtime.operator;
        if !operator.active {
            return false;
        }
        let focused = if operator.stepping {
            operator.active_fixture.into_iter().collect::<HashSet<_>>()
        } else {
            operator.remembered.iter().copied().collect()
        };
        let mut changed = false;
        for (fixture_id, attribute) in touched {
            if focused.contains(&fixture_id) {
                changed |= operator
                    .explicit_attributes
                    .entry(fixture_id)
                    .or_default()
                    .insert(attribute);
            }
        }
        changed
    }

    pub fn clear_desk(&self, desk_id: Uuid) {
        self.clear_all_but_repeat_guard();
        self.recent_actions
            .lock()
            .retain(|(desk, _), _| *desk != desk_id);
    }

    pub fn clear_context(&self, desk_id: Uuid, user_id: UserId) {
        self.clear_all_but_repeat_guard();
        self.recent_actions.lock().remove(&(desk_id, user_id));
    }

    pub fn clear_user(&self, user_id: UserId) {
        self.clear_all_but_repeat_guard();
        self.recent_actions
            .lock()
            .retain(|(_, user), _| *user != user_id);
    }

    pub fn clear_all(&self) {
        self.clear_all_but_repeat_guard();
        self.recent_actions.lock().clear();
    }

    /// Forget the desk's Highlight.
    ///
    /// There is one, so every clearing path clears the same thing. What still differs between
    /// them is which surfaces' repeat guards go with it.
    fn clear_all_but_repeat_guard(&self) {
        let mut runtime = self.runtime.lock();
        runtime.operator = OperatorState::default();
        runtime.output_owner = None;
    }

    /// Compatibility projection containing only full Highlight identities. New output adapters
    /// should install [`Self::output_layers`] so Low Light and explicit-attribute suppression are
    /// preserved.
    pub fn output_fixtures(&self) -> Vec<FixtureId> {
        let mut seen = HashSet::new();
        output_fixture_ids(&self.runtime.lock().operator)
            .into_iter()
            .filter(|fixture| seen.insert(*fixture))
            .collect()
    }

    /// The desk's Highlight output. Highlight wins over Low Light.
    ///
    /// This used to combine layers across desk contexts, resolving overlaps between them. One
    /// desk has one Highlight, so there is nothing to combine.
    pub fn output_layers(&self) -> Vec<HighlightOutputLayer> {
        let mut layers = output_layers(&self.runtime.lock().operator);
        layers.sort_by_key(|layer| layer.fixture_id.0);
        layers
    }
}

#[allow(clippy::too_many_arguments)]
fn build_transition(
    runtime: &HighlightRuntime,
    operator: &super::state::OperatorState,
    user_id: UserId,
    user_name: Option<&str>,
    valid_fixtures: &[HighlightFixture],
    capture_only: bool,
    working_selection: Option<super::model::HighlightSelectionWrite>,
    desk_id: Uuid,
) -> HighlightTransition {
    let _ = desk_id;
    let owner = runtime.output_owner;
    HighlightTransition {
        state: response(
            operator,
            valid_fixtures,
            capture_only,
            owner,
            (owner == Some(user_id))
                .then(|| user_name.map(str::to_owned))
                .flatten(),
        ),
        output_fixtures: output_fixture_ids(operator),
        output_layers: output_layers(operator),
        working_selection,
    }
}
