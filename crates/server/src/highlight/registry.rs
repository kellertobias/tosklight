use super::model::{
    HighlightAction, HighlightError, HighlightFixture, HighlightTransition, is_duplicate_osc_action,
};
use super::operations::{
    ActionContext, apply_action, output_fixture_ids, reconcile_capture_mode, response,
    restore_live_output,
};
use super::selection::synchronize_actual_selection;
use super::state::{HighlightRuntime, RecentHighlightActions};
use light_core::{FixtureId, UserId};
use light_programmer::{GroupDefinition, ProgrammerSelection};
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
        let key = (desk_id, user_id);
        let mut operator = runtime.operators.remove(&key).unwrap_or_default();
        let mut working_selection =
            synchronize_actual_selection(&mut operator, current_selection, valid_fixtures, groups);
        let context = ActionContext {
            desk_id,
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
        runtime.operators.insert(key, operator);
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
        let key = (desk_id, user_id);
        let mut operator = runtime.operators.remove(&key).unwrap_or_default();
        let working_selection =
            synchronize_actual_selection(&mut operator, current_selection, valid_fixtures, groups);
        let context = ActionContext {
            desk_id,
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
        runtime.operators.insert(key, operator);
        transition
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
    let owner = runtime.output_owners.get(&desk_id).copied();
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
        working_selection,
    }
}
