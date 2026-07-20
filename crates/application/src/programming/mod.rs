mod capture_mode_projection;
mod command;
mod event;
mod group_active_show;
mod group_recording;
mod lifecycle;
mod lifecycle_projection;
mod operation;
mod ports;
mod preload_playback_queue_projection;
mod preload_values_action;
mod preload_values_projection;
mod preset_active_show;
mod preset_recording;
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
pub use group_recording::{
    ProgrammingGroupActiveShowPorts, ProgrammingGroupCommit, ProgrammingGroupCommitResult,
    ProgrammingGroupProjection, ProgrammingGroupRecordOperation, ProgrammingGroupRecordOutcome,
    ProgrammingGroupRecordRequest, ProgrammingGroupRecordResult, ProgrammingGroupRecordingPorts,
    ProgrammingGroupRevisionExpectation,
};
pub use lifecycle::{
    ProgrammingLifecycleCompletion, ProgrammingLifecycleResult, ProgrammingLifecycleTarget,
};
pub use lifecycle_projection::{
    ProgrammingLifecycleChange, ProgrammingLifecycleDelta, ProgrammingLifecycleProgrammer,
    ProgrammingLifecycleProjection, ProgrammingLifecycleSession, ProgrammingLifecycleSnapshot,
};
pub use operation::{
    ProgrammingInteractionResult, ProgrammingSelectionRefreshEvent,
    ProgrammingSelectionRefreshResult, ProgrammingSelectionTarget,
};
pub use ports::{
    ProgrammingExecution, ProgrammingPorts, ProgrammingReconciliation,
    ProgrammingSelectionEnvironment, ProgrammingSelectionQuery, ProgrammingValuesEnvironment,
};
pub use preload_playback_queue_projection::{
    ProgrammingPreloadPlaybackAction, ProgrammingPreloadPlaybackQueueChange,
    ProgrammingPreloadPlaybackQueueItem, ProgrammingPreloadPlaybackQueueProjection,
    ProgrammingPreloadPlaybackQueueSnapshot, ProgrammingPreloadPlaybackSurface,
};
pub use preload_values_action::{
    ProgrammingPreloadValueMutation, ProgrammingPreloadValueTiming,
    ProgrammingPreloadValuesCommand, ProgrammingPreloadValuesOutcome,
    ProgrammingPreloadValuesRequest, ProgrammingPreloadValuesResult,
};
pub use preload_values_projection::{
    ProgrammingPreloadValuesChange, ProgrammingPreloadValuesProjection,
    ProgrammingPreloadValuesSnapshot,
};
pub use preset_recording::{
    ProgrammingPresetActiveShowPorts, ProgrammingPresetCommit, ProgrammingPresetCommitResult,
    ProgrammingPresetProjection, ProgrammingPresetRecordOutcome, ProgrammingPresetRecordRequest,
    ProgrammingPresetRecordResult, ProgrammingPresetRecordingPorts,
    ProgrammingPresetRevisionExpectation,
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
