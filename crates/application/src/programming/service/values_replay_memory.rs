use super::super::{
    ProgrammingPreloadValuesOutcome, ProgrammingPreloadValuesResult, ProgrammingValuesOutcome,
    ProgrammingValuesResult,
};
use crate::ActionContext;
use light_core::AttributeValue;
use light_programmer::{
    PreloadProgrammerFixtureValue, PreloadProgrammerGroupValue, ProgrammerFixtureUpdate,
    ProgrammerGroupUpdate,
};
use std::mem::size_of;

// Each values authority gets its own bounded replay cache. The byte budget counts the complete
// retained outcome projection plus dynamic result/key storage and deliberately overestimates
// shared Arc allocations so cache ownership cannot hide a large retained projection.
const REQUEST_CACHE_LIMIT: usize = 4_096;
const REQUEST_CACHE_BYTE_LIMIT: usize = 16 * 1024 * 1024;
pub(super) const ENTRY_CONTAINER_OVERHEAD: usize = 256;

#[derive(Clone, Copy)]
pub(super) struct ReplayLimits {
    pub(super) entries: usize,
    pub(super) bytes: usize,
}

impl Default for ReplayLimits {
    fn default() -> Self {
        Self {
            entries: REQUEST_CACHE_LIMIT,
            bytes: REQUEST_CACHE_BYTE_LIMIT,
        }
    }
}

pub(super) fn values_result_retained_bytes(result: &ProgrammingValuesResult) -> usize {
    let mut bytes = size_of::<ProgrammingValuesResult>()
        + result_dynamic_bytes(&result.context, &result.warning);
    if let ProgrammingValuesOutcome::Changed { projection, .. } = &result.outcome {
        bytes = bytes.saturating_add(size_of_val(projection.as_ref()) + 2 * size_of::<usize>());
        bytes = bytes.saturating_add(
            projection.fixture_values.capacity() * size_of::<ProgrammerFixtureUpdate>(),
        );
        bytes = bytes.saturating_add(
            projection.group_values.capacity() * size_of::<ProgrammerGroupUpdate>(),
        );
        for value in &projection.fixture_values {
            bytes = bytes
                .saturating_add(value.attribute.0.capacity() + attribute_value_bytes(&value.value));
        }
        for value in &projection.group_values {
            bytes = bytes.saturating_add(
                value.group_id.capacity()
                    + value.attribute.0.capacity()
                    + attribute_value_bytes(&value.value),
            );
        }
    }
    bytes
}

pub(super) fn preload_result_retained_bytes(result: &ProgrammingPreloadValuesResult) -> usize {
    let mut bytes = size_of::<ProgrammingPreloadValuesResult>()
        + result_dynamic_bytes(&result.context, &result.warning);
    if let ProgrammingPreloadValuesOutcome::Changed { projection, .. } = &result.outcome {
        bytes = bytes.saturating_add(size_of_val(projection.as_ref()) + 2 * size_of::<usize>());
        bytes = bytes.saturating_add(
            projection.fixture_values.capacity() * size_of::<PreloadProgrammerFixtureValue>(),
        );
        bytes = bytes.saturating_add(
            projection.group_values.capacity() * size_of::<PreloadProgrammerGroupValue>(),
        );
        for value in &projection.fixture_values {
            bytes = bytes
                .saturating_add(value.attribute.0.capacity() + attribute_value_bytes(&value.value));
        }
        for value in &projection.group_values {
            bytes = bytes.saturating_add(
                value.group_id.capacity()
                    + value.attribute.0.capacity()
                    + attribute_value_bytes(&value.value),
            );
        }
    }
    bytes
}

fn result_dynamic_bytes(context: &ActionContext, warning: &Option<String>) -> usize {
    context.request_id.as_ref().map_or(0, String::capacity)
        + warning.as_ref().map_or(0, String::capacity)
}

fn attribute_value_bytes(value: &AttributeValue) -> usize {
    match value {
        AttributeValue::Spread(values) => values.capacity() * size_of::<f32>(),
        AttributeValue::Discrete(value) => value.capacity(),
        _ => 0,
    }
}
