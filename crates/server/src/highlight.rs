//! Authoritative transient Highlight/Step Through state shared by every control surface.

use light_core::{FixtureId, UserId};
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightMode {
    Off,
    Selection,
    Step,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightAction {
    Capture,
    On,
    Off,
    Toggle,
    Next,
    Previous,
}

impl HighlightAction {
    pub const fn osc_dedupe_key(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::On => "on",
            Self::Off => "off",
            Self::Toggle => "toggle",
            Self::Next => "next",
            Self::Previous => "previous",
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
    pub active: bool,
    pub mode: HighlightMode,
    pub output_enabled: bool,
    pub capture_only: bool,
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
pub struct HighlightTransition {
    pub state: HighlightState,
    /// Fixture identities whose raw Highlight Look should be overlaid by the engine.
    pub output_fixtures: Vec<FixtureId>,
    /// When stepping, replace the desk-local working selection with this one fixture.
    pub working_selection: Option<FixtureId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HighlightError {
    EmptySelection,
    OwnedByAnotherUser(UserId),
}

impl std::fmt::Display for HighlightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySelection => formatter
                .write_str("Highlight needs a non-empty current or remembered fixture selection"),
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
    active_fixture: Option<FixtureId>,
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
    /// attached hardware. The guard belongs here so two adapters cannot advance one remembered
    /// selection independently during a single physical press.
    #[allow(clippy::too_many_arguments)]
    pub fn action_guarded(
        &self,
        desk_id: Uuid,
        user_id: UserId,
        user_name: Option<&str>,
        action: HighlightAction,
        current_selection: &[FixtureId],
        valid_fixtures: &[HighlightFixture],
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
            return Ok(self.status(desk_id, user_id, user_name, valid_fixtures, capture_only));
        }
        let transition = self.action(
            desk_id,
            user_id,
            user_name,
            action,
            current_selection,
            valid_fixtures,
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
        current_selection: &[FixtureId],
        valid_fixtures: &[HighlightFixture],
        capture_only: bool,
    ) -> Result<HighlightTransition, HighlightError> {
        // Stage the entire transition and publish it only after every ownership and selection
        // precondition succeeds. In particular, a Blind/Preview operator may already have a
        // prepared capture when another user owns live output; rejecting On/Next must not drop or
        // partially advance that prepared state.
        let mut live_runtime = self.runtime.lock();
        let mut runtime = live_runtime.clone();
        reconcile_runtime(&mut runtime, valid_fixtures);
        let key = (desk_id, user_id);
        let mut operator = runtime.operators.remove(&key).unwrap_or_default();
        if capture_only && operator.output_enabled {
            operator.output_enabled = false;
            if runtime.output_owners.get(&desk_id) == Some(&user_id) {
                runtime.output_owners.remove(&desk_id);
            }
        }
        operator.message = None;
        let mut working_selection = None;

        match action {
            HighlightAction::Capture => {
                let captured = valid_selection(current_selection, valid_fixtures);
                if captured.is_empty() {
                    return Err(HighlightError::EmptySelection);
                }
                operator.remembered = captured;
                operator.active_fixture = None;
                if operator.active && !capture_only {
                    acquire_output_owner(&mut runtime, desk_id, user_id)?;
                    operator.output_enabled = true;
                }
            }
            HighlightAction::On => {
                if operator.remembered.is_empty() {
                    operator.remembered = valid_selection(current_selection, valid_fixtures);
                }
                if operator.remembered.is_empty() {
                    return Err(HighlightError::EmptySelection);
                }
                operator.active = true;
                if capture_only {
                    operator.output_enabled = false;
                    operator.message = Some(
                        "Highlight prepared in Blind/Preview; live output is suppressed".into(),
                    );
                } else {
                    acquire_output_owner(&mut runtime, desk_id, user_id)?;
                    operator.output_enabled = true;
                }
            }
            HighlightAction::Off => {
                operator.active = false;
                operator.output_enabled = false;
                operator.active_fixture = None;
                if runtime.output_owners.get(&desk_id) == Some(&user_id) {
                    runtime.output_owners.remove(&desk_id);
                }
            }
            HighlightAction::Toggle => {
                if operator.active {
                    operator.active = false;
                    operator.output_enabled = false;
                    operator.active_fixture = None;
                    if runtime.output_owners.get(&desk_id) == Some(&user_id) {
                        runtime.output_owners.remove(&desk_id);
                    }
                } else {
                    if operator.remembered.is_empty() {
                        operator.remembered = valid_selection(current_selection, valid_fixtures);
                    }
                    if operator.remembered.is_empty() {
                        return Err(HighlightError::EmptySelection);
                    }
                    operator.active = true;
                    if capture_only {
                        operator.output_enabled = false;
                        operator.message = Some(
                            "Highlight prepared in Blind/Preview; live output is suppressed".into(),
                        );
                    } else {
                        acquire_output_owner(&mut runtime, desk_id, user_id)?;
                        operator.output_enabled = true;
                    }
                }
            }
            HighlightAction::Next => {
                prepare_step(
                    &mut runtime,
                    desk_id,
                    user_id,
                    &mut operator,
                    current_selection,
                    valid_fixtures,
                    capture_only,
                )?;
                let next = match operator
                    .active_fixture
                    .and_then(|active| operator.remembered.iter().position(|id| *id == active))
                {
                    None => Some(0),
                    Some(index) if index + 1 < operator.remembered.len() => Some(index + 1),
                    Some(_) => None,
                };
                if let Some(index) = next {
                    operator.active_fixture = Some(operator.remembered[index]);
                    working_selection = operator.active_fixture;
                } else {
                    operator.message = Some("End of remembered Highlight selection".into());
                }
            }
            HighlightAction::Previous => {
                prepare_step(
                    &mut runtime,
                    desk_id,
                    user_id,
                    &mut operator,
                    current_selection,
                    valid_fixtures,
                    capture_only,
                )?;
                let previous = operator
                    .active_fixture
                    .and_then(|active| operator.remembered.iter().position(|id| *id == active))
                    .and_then(|index| index.checked_sub(1));
                if let Some(index) = previous {
                    operator.active_fixture = Some(operator.remembered[index]);
                    working_selection = operator.active_fixture;
                } else {
                    operator.message = Some("Start of remembered Highlight selection".into());
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

    pub fn status(
        &self,
        desk_id: Uuid,
        user_id: UserId,
        user_name: Option<&str>,
        valid_fixtures: &[HighlightFixture],
        capture_only: bool,
    ) -> HighlightTransition {
        let mut runtime = self.runtime.lock();
        reconcile_runtime(&mut runtime, valid_fixtures);
        let key = (desk_id, user_id);
        let mut operator = runtime.operators.remove(&key).unwrap_or_default();
        if capture_only && operator.output_enabled {
            operator.output_enabled = false;
            if runtime.output_owners.get(&desk_id) == Some(&user_id) {
                runtime.output_owners.remove(&desk_id);
            }
            operator.message =
                Some("Highlight prepared in Blind/Preview; live output is suppressed".into());
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
            working_selection: None,
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

#[allow(clippy::too_many_arguments)]
fn prepare_step(
    runtime: &mut HighlightRuntime,
    desk_id: Uuid,
    user_id: UserId,
    operator: &mut OperatorState,
    current_selection: &[FixtureId],
    valid_fixtures: &[HighlightFixture],
    capture_only: bool,
) -> Result<(), HighlightError> {
    if operator.remembered.is_empty() {
        operator.remembered = valid_selection(current_selection, valid_fixtures);
    }
    if operator.remembered.is_empty() {
        return Err(HighlightError::EmptySelection);
    }
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

fn reconcile(operator: &mut OperatorState, valid_fixtures: &[HighlightFixture]) {
    let valid = valid_fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<HashSet<_>>();
    let previous_index = operator.active_fixture.and_then(|active| {
        operator
            .remembered
            .iter()
            .position(|fixture| *fixture == active)
    });
    operator
        .remembered
        .retain(|fixture| valid.contains(fixture));
    if operator
        .active_fixture
        .is_some_and(|fixture| !valid.contains(&fixture))
    {
        operator.active_fixture = previous_index.and_then(|index| {
            (!operator.remembered.is_empty())
                .then(|| operator.remembered[index.min(operator.remembered.len() - 1)])
        });
        operator.message = Some("Removed fixture skipped in remembered Highlight selection".into());
    }
    if operator.remembered.is_empty() {
        operator.active = false;
        operator.output_enabled = false;
        operator.active_fixture = None;
    }
}

fn reconcile_runtime(runtime: &mut HighlightRuntime, valid_fixtures: &[HighlightFixture]) {
    for operator in runtime.operators.values_mut() {
        reconcile(operator, valid_fixtures);
    }

    let stale_desks = runtime
        .output_owners
        .iter()
        .filter_map(|(desk_id, user_id)| {
            (!runtime
                .operators
                .get(&(*desk_id, *user_id))
                .is_some_and(|operator| operator.output_enabled))
            .then_some(*desk_id)
        })
        .collect::<Vec<_>>();
    for desk_id in stale_desks {
        runtime.output_owners.remove(&desk_id);
    }
}

fn output_fixture_ids(operator: &OperatorState) -> Vec<FixtureId> {
    if !operator.active || !operator.output_enabled {
        return Vec::new();
    }
    operator
        .active_fixture
        .map(|fixture| vec![fixture])
        .unwrap_or_else(|| operator.remembered.clone())
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
    let active_index = operator.active_fixture.and_then(|active| {
        operator
            .remembered
            .iter()
            .position(|fixture| *fixture == active)
    });
    let active_fixture = operator
        .active_fixture
        .and_then(|fixture| by_id.get(&fixture).cloned());
    HighlightState {
        active: operator.active,
        mode: if !operator.active {
            HighlightMode::Off
        } else if operator.active_fixture.is_some() {
            HighlightMode::Step
        } else {
            HighlightMode::Selection
        },
        output_enabled: operator.output_enabled,
        capture_only,
        remembered,
        active_index,
        active_fixture,
        can_previous: active_index.is_some_and(|index| index > 0),
        can_next: match active_index {
            Some(index) => index + 1 < operator.remembered.len(),
            None => operator.active && !operator.remembered.is_empty(),
        },
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

    #[test]
    fn captures_authoritative_order_steps_without_wrap_and_retains_on_off() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3)];
        let selection = fixtures
            .iter()
            .rev()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        let captured = registry
            .action(
                desk,
                user,
                Some("Operator"),
                HighlightAction::Capture,
                &[selection[0], selection[1], selection[0], selection[2]],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(
            captured
                .state
                .remembered
                .iter()
                .map(|fixture| fixture.fixture_id)
                .collect::<Vec<_>>(),
            selection
        );
        assert!(!captured.state.active);
        assert!(captured.output_fixtures.is_empty());
        let on = registry
            .action(
                desk,
                user,
                Some("Operator"),
                HighlightAction::On,
                &[],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(on.output_fixtures, selection);
        let first = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &[],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(first.working_selection, Some(selection[0]));
        registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &[],
                &fixtures,
                false,
            )
            .unwrap();
        registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &[],
                &fixtures,
                false,
            )
            .unwrap();
        let end = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &[],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(end.state.active_index, Some(2));
        assert!(!end.state.can_next);
        assert!(end.state.message.as_deref().unwrap().contains("End"));
        assert_eq!(end.output_fixtures, vec![selection[2]]);
        let previous = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Previous,
                &[selection[2]],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(previous.state.active_index, Some(1));
        let start = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Previous,
                &[selection[1]],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(start.state.active_index, Some(0));
        let start_again = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Previous,
                &[selection[0]],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(start_again.state.active_index, Some(0));
        assert!(!start_again.state.can_previous);
        assert!(
            start_again
                .state
                .message
                .as_deref()
                .unwrap()
                .contains("Start")
        );
        registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Off,
                &[],
                &fixtures,
                false,
            )
            .unwrap();
        let resumed = registry
            .action(desk, user, None, HighlightAction::On, &[], &fixtures, false)
            .unwrap();
        assert_eq!(resumed.state.remembered.len(), 3);
        assert_eq!(resumed.output_fixtures, selection);
    }

    #[test]
    fn removed_active_fixture_is_skipped_and_programming_selection_is_not_recaptured() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3)];
        let selection = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        registry
            .action(
                desk,
                user,
                None,
                HighlightAction::On,
                &selection,
                &fixtures,
                false,
            )
            .unwrap();
        registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &[],
                &fixtures,
                false,
            )
            .unwrap();
        let second = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &[fixtures[0].fixture_id],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(second.state.active_index, Some(1));
        assert_eq!(second.state.remembered.len(), 3);
        assert_eq!(second.output_fixtures, vec![fixtures[1].fixture_id]);
        assert_eq!(second.working_selection, Some(fixtures[1].fixture_id));
        let remaining = vec![fixtures[0].clone(), fixtures[2].clone()];
        let status = registry.status(desk, user, None, &remaining, false);
        assert_eq!(status.state.remembered.len(), 2);
        assert_eq!(status.state.active_fixture, Some(fixtures[2].clone()));
    }

    #[test]
    fn invalidating_the_last_fixture_releases_desk_output_ownership() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let first_user = UserId::new();
        let second_user = UserId::new();
        let first = fixture(1);
        registry
            .action(
                desk,
                first_user,
                None,
                HighlightAction::On,
                &[first.fixture_id],
                std::slice::from_ref(&first),
                false,
            )
            .unwrap();

        let replacement = fixture(2);
        let acquired = registry
            .action(
                desk,
                second_user,
                None,
                HighlightAction::On,
                &[replacement.fixture_id],
                std::slice::from_ref(&replacement),
                false,
            )
            .unwrap();
        assert_eq!(acquired.state.owner_user_id, Some(second_user));
        assert_eq!(acquired.output_fixtures, vec![replacement.fixture_id]);
    }

    #[test]
    fn authoritative_repeat_guard_prevents_cross_surface_double_steps() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3)];
        let selection = fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect::<Vec<_>>();
        registry
            .action(
                desk,
                user,
                None,
                HighlightAction::On,
                &selection,
                &fixtures,
                false,
            )
            .unwrap();

        let software = registry
            .action_guarded(
                desk,
                user,
                None,
                HighlightAction::Next,
                &selection,
                &fixtures,
                false,
            )
            .unwrap();
        let simultaneous_hardware = registry
            .action_guarded(
                desk,
                user,
                None,
                HighlightAction::Next,
                &[selection[0]],
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(software.state.active_index, Some(0));
        assert_eq!(simultaneous_hardware.state.active_index, Some(0));
        assert_eq!(simultaneous_hardware.state.remembered.len(), 3);
    }

    #[test]
    fn different_user_cannot_take_over_live_output_but_blind_can_prepare() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let first = UserId::new();
        let second = UserId::new();
        let fixtures = vec![fixture(1)];
        let selection = [fixtures[0].fixture_id];
        registry
            .action(
                desk,
                first,
                None,
                HighlightAction::On,
                &selection,
                &fixtures,
                false,
            )
            .unwrap();
        assert_eq!(
            registry
                .action(
                    desk,
                    second,
                    None,
                    HighlightAction::On,
                    &selection,
                    &fixtures,
                    false,
                )
                .unwrap_err(),
            HighlightError::OwnedByAnotherUser(first)
        );
        let prepared = registry
            .action(
                desk,
                second,
                None,
                HighlightAction::On,
                &selection,
                &fixtures,
                true,
            )
            .unwrap();
        assert!(prepared.state.active);
        assert!(prepared.state.capture_only);
        assert!(!prepared.state.output_enabled);
        assert!(prepared.output_fixtures.is_empty());
    }

    #[test]
    fn rejected_live_actions_preserve_a_blind_prepared_capture_transactionally() {
        let registry = HighlightRegistry::default();
        let desk = Uuid::new_v4();
        let owner = UserId::new();
        let prepared_user = UserId::new();
        let fixtures = vec![fixture(1), fixture(2), fixture(3)];
        let owner_selection = [fixtures[0].fixture_id];
        let prepared_selection = [fixtures[1].fixture_id, fixtures[2].fixture_id];

        registry
            .action(
                desk,
                owner,
                Some("Owner"),
                HighlightAction::On,
                &owner_selection,
                &fixtures,
                false,
            )
            .unwrap();
        let prepared = registry
            .action(
                desk,
                prepared_user,
                Some("Prepared"),
                HighlightAction::On,
                &prepared_selection,
                &fixtures,
                true,
            )
            .unwrap();
        assert!(prepared.state.active);
        assert_eq!(
            prepared
                .state
                .remembered
                .iter()
                .map(|fixture| fixture.fixture_id)
                .collect::<Vec<_>>(),
            prepared_selection
        );
        assert_eq!(prepared.state.active_fixture, None);

        for action in [HighlightAction::On, HighlightAction::Next] {
            assert_eq!(
                registry
                    .action(
                        desk,
                        prepared_user,
                        Some("Prepared"),
                        action,
                        &[],
                        &fixtures,
                        false,
                    )
                    .unwrap_err(),
                HighlightError::OwnedByAnotherUser(owner)
            );
            let retained = registry.status(desk, prepared_user, Some("Prepared"), &fixtures, true);
            assert!(retained.state.active);
            assert!(!retained.state.output_enabled);
            assert_eq!(retained.state.active_fixture, None);
            assert_eq!(
                retained
                    .state
                    .remembered
                    .iter()
                    .map(|fixture| fixture.fixture_id)
                    .collect::<Vec<_>>(),
                prepared_selection
            );
            assert_eq!(registry.output_fixtures(), owner_selection);
        }
        let owner_status = registry.status(desk, owner, Some("Owner"), &fixtures, false);
        assert_eq!(owner_status.state.owner_user_id, Some(owner));
        assert_eq!(owner_status.output_fixtures, owner_selection);
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
        assert!(!is_duplicate_osc_action(
            None,
            HighlightAction::Previous,
            received_at,
        ));
    }
}
