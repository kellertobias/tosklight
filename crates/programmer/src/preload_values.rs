use crate::fixture_value_batch::{
    FixtureValueBatch, FixtureValueIndex, FixtureValueTiming, restamp_transient_values,
};
use crate::{GroupProgrammerValue, ProgrammerRegistry};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, TimedValue};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PreloadProgrammerValueTiming {
    pub fade: bool,
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PreloadProgrammerValueMutation {
    SetFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: PreloadProgrammerValueTiming,
    },
    ReleaseFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
    },
    SetGroup {
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: PreloadProgrammerValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: AttributeKey,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreloadProgrammerFixtureValue {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    pub programmer_order: u64,
    pub fade: bool,
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreloadProgrammerGroupValue {
    pub group_id: String,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    pub programmer_order: u64,
    pub fade: bool,
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreloadProgrammerValuesContent {
    pub fixture_values: Vec<PreloadProgrammerFixtureValue>,
    pub group_values: Vec<PreloadProgrammerGroupValue>,
}

impl ProgrammerRegistry {
    /// Apply one ordered pending-Preload values action.
    ///
    /// The caller owns capture-mode authorization and provides unique addresses in operator
    /// order. The complete batch shares one Undo checkpoint, timestamp, generation advance, and
    /// application projection event.
    pub fn apply_preload_values(
        &self,
        session: SessionId,
        mutations: &[PreloadProgrammerValueMutation],
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        if !state.blind || !state.preload_capture_programmer {
            return false;
        }
        let fixture_index = FixtureValueIndex::new(&state.preload_pending);
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
        let touched = fixture_batch.commit(&mut state.preload_pending);
        restamp_transient_values(self, state, &touched, changed_at);
        state.last_activity = changed_at;
        let user_id = state.user_id;
        drop(states);
        self.mark_preload_values_changed(user_id);
        true
    }

    pub fn preload_pending_values(
        &self,
        session: SessionId,
    ) -> Option<PreloadProgrammerValuesContent> {
        let key = self.key(session);
        let states = self.states.read();
        Some(content(states.get(&key)?))
    }
}

fn content(state: &crate::ProgrammerState) -> PreloadProgrammerValuesContent {
    let mut fixture_values = state
        .preload_pending
        .iter()
        .map(fixture_value)
        .collect::<Vec<_>>();
    fixture_values.sort_by(|left, right| {
        left.programmer_order
            .cmp(&right.programmer_order)
            .then_with(|| left.fixture_id.0.cmp(&right.fixture_id.0))
            .then_with(|| left.attribute.cmp(&right.attribute))
    });
    let mut group_values = state
        .preload_group_pending
        .iter()
        .flat_map(|(group_id, attributes)| {
            attributes
                .iter()
                .map(move |(attribute, value)| group_value(group_id, attribute, value))
        })
        .collect::<Vec<_>>();
    group_values.sort_by(|left, right| {
        left.programmer_order
            .cmp(&right.programmer_order)
            .then_with(|| left.group_id.cmp(&right.group_id))
            .then_with(|| left.attribute.cmp(&right.attribute))
    });
    PreloadProgrammerValuesContent {
        fixture_values,
        group_values,
    }
}

fn fixture_value(value: &TimedValue) -> PreloadProgrammerFixtureValue {
    PreloadProgrammerFixtureValue {
        fixture_id: value.fixture_id,
        attribute: value.attribute.clone(),
        value: value.value.clone(),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn group_value(
    group_id: &str,
    attribute: &AttributeKey,
    value: &GroupProgrammerValue,
) -> PreloadProgrammerGroupValue {
    PreloadProgrammerGroupValue {
        group_id: group_id.to_owned(),
        attribute: attribute.clone(),
        value: value.value.clone(),
        programmer_order: value.programmer_order,
        fade: value.fade,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn mutation_changes(
    state: &crate::ProgrammerState,
    fixture_index: &FixtureValueIndex<'_>,
    mutation: &PreloadProgrammerValueMutation,
) -> bool {
    match mutation {
        PreloadProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => fixture_index
            .get(*fixture_id, attribute)
            .is_none_or(|stored| !fixture_value_matches(stored, value, *timing)),
        PreloadProgrammerValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => fixture_index.get(*fixture_id, attribute).is_some(),
        PreloadProgrammerValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => state
            .preload_group_pending
            .get(group_id)
            .and_then(|values| values.get(attribute))
            .is_none_or(|stored| !group_value_matches(stored, value, *timing)),
        PreloadProgrammerValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => state
            .preload_group_pending
            .get(group_id)
            .is_some_and(|values| values.contains_key(attribute)),
    }
}

fn fixture_value_matches(
    stored: &TimedValue,
    value: &AttributeValue,
    timing: PreloadProgrammerValueTiming,
) -> bool {
    stored.value == *value
        && stored.fade == timing.fade
        && stored.fade_millis == timing.fade_millis
        && stored.delay_millis == timing.delay_millis
}

fn group_value_matches(
    stored: &GroupProgrammerValue,
    value: &AttributeValue,
    timing: PreloadProgrammerValueTiming,
) -> bool {
    stored.value == *value
        && stored.fade == timing.fade
        && stored.fade_millis == timing.fade_millis
        && stored.delay_millis == timing.delay_millis
}

fn apply_mutation(
    registry: &ProgrammerRegistry,
    state: &mut crate::ProgrammerState,
    mutation: &PreloadProgrammerValueMutation,
    changed_at: chrono::DateTime<chrono::Utc>,
    fixture_batch: &mut FixtureValueBatch,
) {
    match mutation {
        PreloadProgrammerValueMutation::SetFixture {
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
        PreloadProgrammerValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => fixture_batch.release(*fixture_id, attribute),
        PreloadProgrammerValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => set_group(
            registry, state, group_id, attribute, value, *timing, changed_at,
        ),
        PreloadProgrammerValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => release_group(state, group_id, attribute),
    }
}

fn fixture_timing(timing: PreloadProgrammerValueTiming) -> FixtureValueTiming {
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
    timing: PreloadProgrammerValueTiming,
    changed_at: chrono::DateTime<chrono::Utc>,
) {
    state
        .preload_group_pending
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
    if let Some(values) = state.preload_group_pending.get_mut(group_id) {
        values.remove(attribute);
        if values.is_empty() {
            state.preload_group_pending.remove(group_id);
        }
    }
}
