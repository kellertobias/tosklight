use crate::{GroupProgrammerValue, ProgrammerRegistry};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, TimedValue};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NormalProgrammerValueTiming {
    pub fade: bool,
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NormalProgrammerValueMutation {
    SetFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: NormalProgrammerValueTiming,
    },
    ReleaseFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
    },
    SetGroup {
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: NormalProgrammerValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: AttributeKey,
    },
}

impl ProgrammerRegistry {
    /// Apply one normal, recordable Programmer action independently of Preload capture mode.
    ///
    /// Callers provide unique addresses in operator order. The complete batch shares one Undo
    /// checkpoint, timestamp, generation advance, and application-level projection event.
    pub fn apply_normal_values(
        &self,
        session: SessionId,
        mutations: &[NormalProgrammerValueMutation],
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        if !mutations
            .iter()
            .any(|mutation| mutation_changes(state, mutation))
        {
            return false;
        }
        state.checkpoint();
        let changed_at = self.clock.now();
        let mut touched = HashSet::new();
        for mutation in mutations {
            if mutation_changes(state, mutation) {
                apply_mutation(self, state, mutation, changed_at, &mut touched);
            }
        }
        restamp_transient_values(self, state, &touched, changed_at);
        state.last_activity = changed_at;
        let user_id = state.user_id;
        drop(states);
        self.mark_normal_values_changed(user_id);
        true
    }

    /// Clear only recordable fixture and Group values. Preload and transient actions are not part
    /// of this boundary and remain untouched.
    pub fn clear_normal_values(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        if state.values.is_empty() && state.group_values.is_empty() {
            return false;
        }
        state.checkpoint();
        state.values.clear();
        state.group_values.clear();
        state.last_activity = self.clock.now();
        let user_id = state.user_id;
        drop(states);
        self.mark_normal_values_changed(user_id);
        true
    }
}

fn mutation_changes(
    state: &crate::ProgrammerState,
    mutation: &NormalProgrammerValueMutation,
) -> bool {
    match mutation {
        NormalProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => state
            .values
            .iter()
            .find(|stored| stored.fixture_id == *fixture_id && stored.attribute == *attribute)
            .is_none_or(|stored| !fixture_value_matches(stored, value, *timing)),
        NormalProgrammerValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => state
            .values
            .iter()
            .any(|value| value.fixture_id == *fixture_id && value.attribute == *attribute),
        NormalProgrammerValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => state
            .group_values
            .get(group_id)
            .is_some_and(|values| values.contains_key(attribute)),
        NormalProgrammerValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => state
            .group_values
            .get(group_id)
            .and_then(|values| values.get(attribute))
            .is_none_or(|stored| !group_value_matches(stored, value, *timing)),
    }
}

fn fixture_value_matches(
    stored: &TimedValue,
    value: &AttributeValue,
    timing: NormalProgrammerValueTiming,
) -> bool {
    stored.value == *value
        && stored.fade == timing.fade
        && stored.fade_millis == timing.fade_millis
        && stored.delay_millis == timing.delay_millis
}

fn group_value_matches(
    stored: &GroupProgrammerValue,
    value: &AttributeValue,
    timing: NormalProgrammerValueTiming,
) -> bool {
    stored.value == *value
        && stored.fade == timing.fade
        && stored.fade_millis == timing.fade_millis
        && stored.delay_millis == timing.delay_millis
}

fn apply_mutation(
    registry: &ProgrammerRegistry,
    state: &mut crate::ProgrammerState,
    mutation: &NormalProgrammerValueMutation,
    changed_at: chrono::DateTime<chrono::Utc>,
    touched: &mut HashSet<(FixtureId, AttributeKey)>,
) {
    match mutation {
        NormalProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => set_fixture(
            registry,
            state,
            *fixture_id,
            attribute,
            value,
            *timing,
            changed_at,
            touched,
        ),
        NormalProgrammerValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => state
            .values
            .retain(|value| value.fixture_id != *fixture_id || value.attribute != *attribute),
        NormalProgrammerValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => set_group(
            registry, state, group_id, attribute, value, *timing, changed_at,
        ),
        NormalProgrammerValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => release_group(state, group_id, attribute),
    }
}

#[allow(clippy::too_many_arguments)]
fn set_fixture(
    registry: &ProgrammerRegistry,
    state: &mut crate::ProgrammerState,
    fixture_id: FixtureId,
    attribute: &AttributeKey,
    value: &AttributeValue,
    timing: NormalProgrammerValueTiming,
    changed_at: chrono::DateTime<chrono::Utc>,
    touched: &mut HashSet<(FixtureId, AttributeKey)>,
) {
    state
        .values
        .retain(|existing| existing.fixture_id != fixture_id || existing.attribute != *attribute);
    state.values.push(TimedValue {
        fixture_id,
        attribute: attribute.clone(),
        value: value.clone(),
        priority: state.priority,
        changed_at,
        programmer_order: registry.next_programmer_order(),
        merge_mode: light_core::MergeMode::Ltp,
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    });
    touched.insert((fixture_id, attribute.clone()));
}

fn set_group(
    registry: &ProgrammerRegistry,
    state: &mut crate::ProgrammerState,
    group_id: &str,
    attribute: &AttributeKey,
    value: &AttributeValue,
    timing: NormalProgrammerValueTiming,
    changed_at: chrono::DateTime<chrono::Utc>,
) {
    state
        .group_values
        .entry(group_id.to_owned())
        .or_default()
        .insert(
            attribute.clone(),
            GroupProgrammerValue {
                value: value.clone(),
                changed_at,
                programmer_order: registry.next_programmer_order(),
                fade: timing.fade,
                fade_millis: timing.fade_millis,
                delay_millis: timing.delay_millis,
            },
        );
}

fn release_group(state: &mut crate::ProgrammerState, group_id: &str, attribute: &AttributeKey) {
    if let Some(values) = state.group_values.get_mut(group_id) {
        values.remove(attribute);
        if values.is_empty() {
            state.group_values.remove(group_id);
        }
    }
}

fn restamp_transient_values(
    registry: &ProgrammerRegistry,
    state: &mut crate::ProgrammerState,
    touched: &HashSet<(FixtureId, AttributeKey)>,
    changed_at: chrono::DateTime<chrono::Utc>,
) {
    for value in state
        .transient_values
        .iter_mut()
        .flat_map(|action| action.values.iter_mut())
        .filter(|value| touched.contains(&(value.fixture_id, value.attribute.clone())))
    {
        value.changed_at = changed_at;
        value.programmer_order = registry.next_programmer_order();
    }
}
