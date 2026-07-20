use crate::{ProgrammerRegistry, ProgrammerState};
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use std::collections::{HashMap, HashSet};

pub(crate) struct FixtureValueIndex<'a> {
    values: HashMap<(FixtureId, &'a AttributeKey), &'a TimedValue>,
}

impl<'a> FixtureValueIndex<'a> {
    pub(crate) fn new(values: &'a [TimedValue]) -> Self {
        Self {
            values: values
                .iter()
                .map(|value| ((value.fixture_id, &value.attribute), value))
                .collect(),
        }
    }

    pub(crate) fn get(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&TimedValue> {
        self.values.get(&(fixture_id, attribute)).copied()
    }
}

#[derive(Default)]
pub(crate) struct FixtureValueBatch {
    additions: Vec<TimedValue>,
    replaced: FixtureAddresses,
    touched: FixtureAddresses,
}

impl FixtureValueBatch {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set(
        &mut self,
        registry: &ProgrammerRegistry,
        priority: i16,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
        value: &AttributeValue,
        timing: FixtureValueTiming,
        changed_at: DateTime<Utc>,
    ) {
        self.replace_once(fixture_id, attribute);
        self.touched.insert(fixture_id, attribute);
        self.additions.push(TimedValue {
            fixture_id,
            attribute: attribute.clone(),
            value: value.clone(),
            priority,
            changed_at,
            programmer_order: registry.next_programmer_order(),
            merge_mode: MergeMode::Ltp,
            fade: timing.fade,
            fade_millis: timing.fade_millis,
            delay_millis: timing.delay_millis,
        });
    }

    pub(crate) fn release(&mut self, fixture_id: FixtureId, attribute: &AttributeKey) {
        self.replace_once(fixture_id, attribute);
    }

    pub(crate) fn commit(mut self, values: &mut Vec<TimedValue>) -> FixtureAddresses {
        values.retain(|value| !self.replaced.contains(value.fixture_id, &value.attribute));
        values.append(&mut self.additions);
        self.touched
    }

    fn replace_once(&mut self, fixture_id: FixtureId, attribute: &AttributeKey) {
        debug_assert!(
            self.replaced.insert(fixture_id, attribute),
            "Programmer value batches must contain unique addresses"
        );
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FixtureValueTiming {
    pub(crate) fade: bool,
    pub(crate) fade_millis: Option<u64>,
    pub(crate) delay_millis: Option<u64>,
}

#[derive(Default)]
pub(crate) struct FixtureAddresses(HashMap<FixtureId, HashSet<AttributeKey>>);

impl FixtureAddresses {
    fn insert(&mut self, fixture_id: FixtureId, attribute: &AttributeKey) -> bool {
        self.0
            .entry(fixture_id)
            .or_default()
            .insert(attribute.clone())
    }

    fn contains(&self, fixture_id: FixtureId, attribute: &AttributeKey) -> bool {
        self.0
            .get(&fixture_id)
            .is_some_and(|attributes| attributes.contains(attribute))
    }
}

pub(crate) fn restamp_transient_values(
    registry: &ProgrammerRegistry,
    state: &mut ProgrammerState,
    touched: &FixtureAddresses,
    changed_at: DateTime<Utc>,
) {
    for value in state
        .transient_values
        .iter_mut()
        .flat_map(|action| action.values.iter_mut())
        .filter(|value| touched.contains(value.fixture_id, &value.attribute))
    {
        value.changed_at = changed_at;
        value.programmer_order = registry.next_programmer_order();
    }
}
