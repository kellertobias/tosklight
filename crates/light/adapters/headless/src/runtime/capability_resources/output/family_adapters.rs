//! TL-548 C3: the Live all-family adapters shared with the output scheduler.
//!
//! `OutputResource` starts with its own, never opted-in `LiveFamilyAdapters`. Bootstrap replaces
//! it with the scheduler's `Arc` so both Live paths drive one lane set; a test opts in by
//! installing an opted-in resource (`test_state_with_family_adapters`).
use super::*;

impl OutputResource {
    /// Share `adapters` with this resource's Live render path.
    pub(in crate::runtime) fn with_live_family_adapters(
        mut self,
        adapters: Arc<output_scheduler::LiveFamilyAdapters>,
    ) -> Self {
        self.family_adapters = adapters;
        self
    }

    pub(in crate::runtime) fn live_family_adapters(
        &self,
    ) -> &Arc<output_scheduler::LiveFamilyAdapters> {
        &self.family_adapters
    }
}
