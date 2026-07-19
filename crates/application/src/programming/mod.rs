mod capture_mode_projection;
mod command;
mod event;
mod lifecycle;
mod operation;
mod ports;
mod projection;
mod service;
mod values_action;
mod values_projection;

pub use capture_mode_projection::{
    ProgrammingCaptureModeChange, ProgrammingCaptureModeProjection, ProgrammingCaptureModeSnapshot,
};
pub use command::{
    CueMoveCopyChoice, CueTransferOperation, ExecutionPolicy, ProgrammingAction,
    ProgrammingChoiceOption, ProgrammingChoiceOptionId, ProgrammingCommand, ProgrammingOutcome,
    ProgrammingResult, SelectionGestureSource,
};
pub use event::ProgrammingInteractionChange;
pub use lifecycle::{
    ProgrammingLifecycleCompletion, ProgrammingLifecycleResult, ProgrammingLifecycleTarget,
};
pub use operation::{
    ProgrammingInteractionResult, ProgrammingSelectionRefreshEvent,
    ProgrammingSelectionRefreshResult, ProgrammingSelectionTarget,
};
pub use ports::{
    ProgrammingExecution, ProgrammingPorts, ProgrammingReconciliation,
    ProgrammingSelectionEnvironment, ProgrammingSelectionQuery, ProgrammingValuesEnvironment,
};
pub use projection::{ProgrammingInteractionProjection, ProgrammingLiveSnapshot};
pub use service::ProgrammingService;
pub use values_action::{
    ProgrammingValueMutation, ProgrammingValueTiming, ProgrammingValuesCommand,
    ProgrammingValuesOutcome, ProgrammingValuesRequest, ProgrammingValuesResult,
};
pub use values_projection::{
    ProgrammingValuesChange, ProgrammingValuesProjection, ProgrammingValuesSnapshot,
};

#[cfg(test)]
mod live_state_tests;
#[cfg(test)]
mod tests;
