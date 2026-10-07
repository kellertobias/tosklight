//! Episode-owned engine caches and compiler scratch, independent of evaluator loans.
use super::{HybridFrameScratch, PreloadFrameState};

/// Retain this value for the paired episode. Both branch caches survive successive evaluator
/// loans; replacing it intentionally starts a new engine token lineage. This owns no engine,
/// resolver, observer, runtime history or physical lane references.
#[derive(Default)]
pub(in crate::runtime) struct RetainedPreloadHybridState {
    pub(super) state: PreloadFrameState,
    pub(super) before_scratch: HybridFrameScratch,
    pub(super) after_scratch: HybridFrameScratch,
}

/// Existing convenience constructors own their state. Production episode owners can lend it
/// synchronously without a self-referential engine or physical resolver allocation.
pub(super) enum EvaluatorState<'a> {
    Owned(Box<RetainedPreloadHybridState>),
    Borrowed(&'a mut RetainedPreloadHybridState),
}

impl EvaluatorState<'_> {
    pub(super) fn as_mut(&mut self) -> &mut RetainedPreloadHybridState {
        match self {
            Self::Owned(state) => state,
            Self::Borrowed(state) => state,
        }
    }
}
