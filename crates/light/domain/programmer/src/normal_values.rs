use crate::fixture_value_batch::{
    FixtureValueBatch, FixtureValueIndex, FixtureValueTiming, restamp_transient_values,
};
use crate::{GroupProgrammerValue, ProgrammerRegistry};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, TimedValue};

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NormalPresetRecallTransition {
    pub values_changed: bool,
    pub active_context_changed: bool,
}

impl NormalPresetRecallTransition {
    pub const fn changed(self) -> bool {
        self.values_changed || self.active_context_changed
    }
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
        let fixture_index = FixtureValueIndex::new(&state.values);
        let changed = mutations
            .iter()
            .map(|mutation| mutation_changes(state, &fixture_index, mutation))
            .collect::<Vec<_>>();
        drop(fixture_index);
        if !changed.iter().any(|changed| *changed) {
            return false;
        }
        state.checkpoint();
        let changed_at = self.clock.now();
        let mut fixture_batch = FixtureValueBatch::default();
        for (mutation, changed) in mutations.iter().zip(changed) {
            if changed {
                apply_mutation(self, state, mutation, changed_at, &mut fixture_batch);
            }
        }
        let touched = fixture_batch.commit(&mut state.values);
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

    /// Applies one planned Preset recall as a single normal Programmer transaction.
    ///
    /// The caller supplies unique addresses in final Programmer order. Values and active context
    /// share one checkpoint and timestamp; only a real values transition advances the retained
    /// normal-values generation.
    pub fn apply_normal_preset_recall(
        &self,
        session: SessionId,
        mutations: &[NormalProgrammerValueMutation],
        active_context: String,
    ) -> Option<NormalPresetRecallTransition> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let state = states.get_mut(&self.key(session))?;
        let fixture_index = FixtureValueIndex::new(&state.values);
        let changed = mutations
            .iter()
            .map(|mutation| mutation_changes(state, &fixture_index, mutation))
            .collect::<Vec<_>>();
        drop(fixture_index);
        let transition = NormalPresetRecallTransition {
            values_changed: changed.iter().any(|changed| *changed),
            active_context_changed: state.active_context.as_deref() != Some(&active_context),
        };
        if !transition.changed() {
            return Some(transition);
        }
        state.checkpoint();
        let changed_at = self.clock.now();
        let mut fixture_batch = FixtureValueBatch::default();
        for (mutation, changed) in mutations.iter().zip(changed) {
            if changed {
                apply_mutation(self, state, mutation, changed_at, &mut fixture_batch);
            }
        }
        let touched = fixture_batch.commit(&mut state.values);
        restamp_transient_values(self, state, &touched, changed_at);
        state.active_context = Some(active_context);
        state.last_activity = changed_at;
        let user_id = state.user_id;
        drop(states);
        if transition.values_changed {
            self.mark_normal_values_changed(user_id);
        }
        Some(transition)
    }
}

fn mutation_changes(
    state: &crate::ProgrammerState,
    fixture_index: &FixtureValueIndex<'_>,
    mutation: &NormalProgrammerValueMutation,
) -> bool {
    match mutation {
        NormalProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => fixture_index
            .get(*fixture_id, attribute)
            .is_none_or(|stored| !fixture_value_matches(stored, value, *timing)),
        NormalProgrammerValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => fixture_index.get(*fixture_id, attribute).is_some(),
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
    fixture_batch: &mut FixtureValueBatch,
) {
    match mutation {
        NormalProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => fixture_batch.set(
            registry,
            state.priority,
            *fixture_id,
            attribute,
            value,
            fixture_timing(*timing),
            changed_at,
        ),
        NormalProgrammerValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => fixture_batch.release(*fixture_id, attribute),
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

fn fixture_timing(timing: NormalProgrammerValueTiming) -> FixtureValueTiming {
    FixtureValueTiming {
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    }
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
