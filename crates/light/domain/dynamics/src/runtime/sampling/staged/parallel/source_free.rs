//! Evaluate only cold-proven literal scalar lanes without touching a frame source fork.
//! Unexpected source access discards this speculative result and uses ordinary evaluation.
use super::*;
use crate::{DynamicValue, DynamicValueAddress};
use light_core::AttributeValue;
use std::cell::Cell;

#[derive(Default)]
struct NoQueries(Cell<bool>);
impl DynamicValueSourceResolver for NoQueries {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.0.set(true);
        None
    }
    fn preset(
        &self,
        _: &crate::DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        self.0.set(true);
        None
    }
    fn try_position_current_family(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.0.set(true);
        Ok(None)
    }
    fn authored_occurrence(
        &self,
        _: Uuid,
        _: Uuid,
        _: FixtureId,
        _: Uuid,
    ) -> Option<crate::DynamicSourceOccurrenceId> {
        self.0.set(true);
        None
    }
    fn current_family_occurrence(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Option<crate::DynamicSourceOccurrenceId> {
        self.0.set(true);
        None
    }
}

pub(super) fn evaluate(
    compiled: &crate::CompiledProgrammingLane,
    plan: &PinnedInstance,
    work: &PinnedController,
    pinned: &PinnedLane,
) -> Option<Evaluated> {
    let controller = &plan.frame.controllers[work.controller_index];
    if !compiled.source_free_scalar() || controller.size != 1.0 {
        return None;
    }
    // Authored occurrence was captured when the lane was pinned; the ordinary evaluator
    // still receives that exact context. Its result and optional cache transition are local:
    // an unexpected source request can be discarded before any runtime state is applied.
    let sources = NoQueries::default();
    let evaluated = super::evaluate(compiled, plan, work, pinned, &sources, true);
    (!sources.0.get()).then_some(evaluated)
}
