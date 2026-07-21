//! Transport-independent application contracts and use-case infrastructure.
//!
//! Domain crates return typed state transitions. Application services publish those transitions
//! here, while server and desktop adapters translate them into their public wire contracts.

pub mod action;
pub mod active_show;
pub mod event;
pub mod fixture_position;
pub mod lossless_json;
pub mod macro_runtime;
pub mod managed_assets;
pub mod mvr_import;
pub mod output_runtime;
pub mod playback;
pub mod playback_topology;
pub mod programming;
pub mod scheduling;
pub mod selective_import;
pub mod show_compiler;
pub mod show_patch;
pub mod speed_group;
pub mod timeline;

pub use action::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionOutcome, ActionSource,
    ApplicationCommand, CommandFamily,
};
pub use active_show::{
    ActiveShowObjectChange, ActiveShowObjectKind, ActiveShowObjectMutation,
    ActiveShowObjectMutationKind, ActiveShowObjectsChange, ActiveShowPorts, ActiveShowService,
    ActiveShowUnitOfWork, BackupIdentity, MutateActiveShowObjectsCommand,
    MutateActiveShowObjectsResult, MutateOutputRouteCommand, MutateOutputRouteResult,
    OutputRouteChange, OutputRouteMutation, UndoActiveShowObjectCommand,
    UndoActiveShowObjectResult,
};
pub use event::{
    ApplicationEvent, DeliveryPolicy, DeskEvent, EventBus, EventCapability, EventClass, EventDraft,
    EventEnvelope, EventFilter, EventObject, EventReplay, EventSource, EventSubscription,
    OutputEvent, PlaybackEvent, ProgrammingEvent, ReplaceableEventRateLimit, SequenceGap,
    ShowEvent, SubscriptionDelivery, SubscriptionOptions,
};
pub use fixture_position::{
    FixturePositionCommand, FixturePositionExecution, FixturePositionOutcome, FixturePositionPorts,
    FixturePositionService, FixtureProjection, StagePosition,
};
pub use macro_runtime::{
    CancellationSignal, GroupProjection, MacroAuditEntry, MacroAuditedAction, MacroCapability,
    MacroDefinition, MacroDependency, MacroError, MacroErrorKind, MacroEventFilter,
    MacroExecutionId, MacroExecutionOutcome, MacroExecutionPhase, MacroExecutionRequest,
    MacroExecutionSnapshot, MacroHost, MacroHostAction, MacroHostBackend, MacroHttpAuditEvent,
    MacroHttpFailureKind, MacroHttpPolicy, MacroHttpRequest, MacroHttpResponse, MacroHttpTerminal,
    MacroHttpTransportError, MacroHttpTransportErrorKind, MacroHttpTransportResponse, MacroId,
    MacroInvocation, MacroLanguageId, MacroObservedEvent, MacroResume, MacroRuntime, MacroService,
    MacroTask, MacroTaskRunner, MacroValue, MacroWaitRequest, MacroWaitState, OperatorInputKind,
    OperatorInputValue,
};
pub use managed_assets::{
    AssetAvailability, AssetChunkSink, AssetChunkSource, AssetCleanupReport, AssetDescriptor,
    AssetError, AssetErrorKind, AssetExportManifest, AssetExportReport, AssetExportSink, AssetId,
    AssetNamespace, AssetReference, AssetRevision, AssetStreamReport, AssetValidation,
    CleanupAssetsRequest, CopyAssetRequest, ExportAssetsRequest, ImportAssetRequest,
    ManagedAssetStore,
};
pub use mvr_import::{
    ActiveMvrImportResult, ApplyActiveMvrImportCommand, MvrImportResolution, MvrImportService,
    PreparedActiveMvrImport, resolve_mvr_definition,
};
pub use output_runtime::{
    OutputLevel, OutputRuntimeApplication, OutputRuntimeChange, OutputRuntimeCommand,
    OutputRuntimeDurability, OutputRuntimeExpectation, OutputRuntimeIdentity, OutputRuntimeOutcome,
    OutputRuntimePorts, OutputRuntimeProjection, OutputRuntimeResult, OutputRuntimeScope,
    OutputRuntimeService, OutputRuntimeSnapshot,
};
pub use playback::{
    AutomaticPlaybackProjection, CueListRuntimeProjection, CueNumber, GrandMasterRuntimeProjection,
    MAX_PLAYBACK_GROUP_ID_BYTES, ManualXFadeDirection, PendingPlaybackAction, PlaybackAction,
    PlaybackAddress, PlaybackCommand, PlaybackCueReference, PlaybackCueTransition,
    PlaybackDeskProjection, PlaybackDurability, PlaybackExecution, PlaybackGroupId,
    PlaybackGroupIdError, PlaybackLevel, PlaybackOperation, PlaybackOperationResult,
    PlaybackOutcome, PlaybackPorts, PlaybackResult, PlaybackRuntimeChange, PlaybackRuntimeIdentity,
    PlaybackRuntimeProjection, PlaybackRuntimeSnapshot, PlaybackService, PlaybackShowScope,
    PlaybackSurface, PlaybackTargetProjection, PlaybackTransitionCause, PlaybackUnitOfWork,
    ResolvedPlaybackAddress, SoundLossReason, SoundStatus, SpeedGroupRuntimeProjection,
    SpeedSource, automatic_playback_events, committed_playback_effect_event,
    committed_playback_event, publish_automatic_playback_events,
};
pub use playback_topology::{
    PlaybackTopologyAction, PlaybackTopologyCommand, PlaybackTopologyObjectProjection,
    PlaybackTopologyOutcome, PlaybackTopologyPorts, PlaybackTopologyResolution,
    PlaybackTopologyResult, PlaybackTopologyService,
};
pub use programming::update as programming_update;
pub use programming::{
    CueMoveCopyChoice, CueTransferOperation, ExecutionPolicy, GroupManagementActiveShowPorts,
    GroupManagementCommit, GroupManagementCommitResult, GroupManagementOperation,
    GroupManagementOutcome, GroupManagementPorts, GroupManagementProjection,
    GroupManagementRequest, GroupManagementResult, GroupManagementSelection, GroupPropertiesUpdate,
    GroupSourceExpectation, ProgrammingAction, ProgrammingCaptureModeChange,
    ProgrammingCaptureModeProjection, ProgrammingCaptureModeSnapshot, ProgrammingChoiceOption,
    ProgrammingChoiceOptionId, ProgrammingCommand, ProgrammingCueActivationCompletion,
    ProgrammingCueActivationPolicy, ProgrammingCueActivationResult, ProgrammingCueActiveShowPorts,
    ProgrammingCueCapturePolicy, ProgrammingCueCommit, ProgrammingCueCommitResult,
    ProgrammingCueDeletionAddress, ProgrammingCueDeletionAuthority,
    ProgrammingCueDeletionExpectation, ProgrammingCueDeletionObjectProjection,
    ProgrammingCueDeletionOutcome, ProgrammingCueDeletionPorts, ProgrammingCueDeletionRequest,
    ProgrammingCueDeletionResult, ProgrammingCueDeletionState, ProgrammingCueObjectProjection,
    ProgrammingCuePageSlot, ProgrammingCueProjections, ProgrammingCueRecordOperation,
    ProgrammingCueRecordOutcome, ProgrammingCueRecordRequest, ProgrammingCueRecordResult,
    ProgrammingCueRecordTarget, ProgrammingCueRecordTiming, ProgrammingCueRecordingEnvironment,
    ProgrammingCueRecordingPorts, ProgrammingCueResolvedTarget,
    ProgrammingCueShowRevisionExpectation, ProgrammingCueTransferAddress,
    ProgrammingCueTransferChoiceRequest, ProgrammingCueTransferEndpoint,
    ProgrammingCueTransferMode, ProgrammingCueTransferObjectProjection,
    ProgrammingCueTransferOutcome, ProgrammingCueTransferPorts, ProgrammingCueTransferRequest,
    ProgrammingCueTransferResult, ProgrammingCueTransferSummary, ProgrammingDeletedCue,
    ProgrammingExecution, ProgrammingGroupActiveShowPorts, ProgrammingGroupCommit,
    ProgrammingGroupCommitResult, ProgrammingGroupProjection, ProgrammingGroupRecordOperation,
    ProgrammingGroupRecordOutcome, ProgrammingGroupRecordRequest, ProgrammingGroupRecordResult,
    ProgrammingGroupRecordingPorts, ProgrammingGroupRevisionExpectation,
    ProgrammingInteractionChange, ProgrammingInteractionProjection, ProgrammingInteractionResult,
    ProgrammingLifecycleChange, ProgrammingLifecycleCompletion, ProgrammingLifecycleDelta,
    ProgrammingLifecycleProgrammer, ProgrammingLifecycleProjection, ProgrammingLifecycleResult,
    ProgrammingLifecycleSession, ProgrammingLifecycleSnapshot, ProgrammingLifecycleTarget,
    ProgrammingLiveSnapshot, ProgrammingOutcome, ProgrammingPorts, ProgrammingPreloadCommitResult,
    ProgrammingPreloadExecutedPlaybackAction, ProgrammingPreloadLifecycleAction,
    ProgrammingPreloadLifecyclePorts, ProgrammingPreloadLifecycleRequest,
    ProgrammingPreloadLifecycleResult, ProgrammingPreloadLifecycleState,
    ProgrammingPreloadPlaybackAction, ProgrammingPreloadPlaybackQueueChange,
    ProgrammingPreloadPlaybackQueueItem, ProgrammingPreloadPlaybackQueueProjection,
    ProgrammingPreloadPlaybackQueueSnapshot, ProgrammingPreloadPlaybackSurface,
    ProgrammingPreloadRevisionExpectation, ProgrammingPreloadRuntimeChange,
    ProgrammingPreloadValueMutation, ProgrammingPreloadValueTiming, ProgrammingPreloadValuesChange,
    ProgrammingPreloadValuesCommand, ProgrammingPreloadValuesOutcome,
    ProgrammingPreloadValuesProjection, ProgrammingPreloadValuesRequest,
    ProgrammingPreloadValuesResult, ProgrammingPreloadValuesSnapshot,
    ProgrammingPresetActiveShowPorts, ProgrammingPresetCommit, ProgrammingPresetCommitResult,
    ProgrammingPresetProjection, ProgrammingPresetRecallEnvironment,
    ProgrammingPresetRecallOutcome, ProgrammingPresetRecallPorts, ProgrammingPresetRecallRequest,
    ProgrammingPresetRecallResult, ProgrammingPresetRecallRevisionExpectation,
    ProgrammingPresetRecordOutcome, ProgrammingPresetRecordRequest, ProgrammingPresetRecordResult,
    ProgrammingPresetRecordingPorts, ProgrammingPresetRevisionExpectation,
    ProgrammingPriorityActionState, ProgrammingPriorityChange, ProgrammingPriorityProjection,
    ProgrammingPriorityRequest, ProgrammingPriorityResult, ProgrammingPriorityRevisionExpectation,
    ProgrammingPrioritySnapshot, ProgrammingRecalledPresetProjection, ProgrammingReconciliation,
    ProgrammingRecordedCue, ProgrammingResult, ProgrammingSelectionEnvironment,
    ProgrammingSelectionQuery, ProgrammingSelectionRefreshEvent, ProgrammingSelectionRefreshResult,
    ProgrammingSelectionTarget, ProgrammingService, ProgrammingValueMutation,
    ProgrammingValueTiming, ProgrammingValuesChange, ProgrammingValuesCommand,
    ProgrammingValuesEnvironment, ProgrammingValuesOutcome, ProgrammingValuesProjection,
    ProgrammingValuesRequest, ProgrammingValuesResult, ProgrammingValuesSnapshot,
    SelectionGestureSource,
};
pub use scheduling::{
    MonotonicClock, MonotonicMoment, MonotonicScheduler, SchedulerError, SchedulerErrorKind,
    WallClock, deadline_after,
};
pub use selective_import::{
    AppliedImportObject, ApplySelectiveShowImportCommand, ImportBlocker, ImportConflict,
    ImportConflictResolution, ImportDependency, ImportDependencyDisposition, ImportIdentityFormat,
    ImportManagedAssetAction, ImportManagedAssetPreview, ImportObjectAction,
    ImportObjectDescriptor, ImportObjectReference, ImportOwnedIdentity, ImportProfileAction,
    ImportProfileConflictResolution, ImportProfileKey, ImportProfilePreview,
    ImportReferenceLocation, SelectiveShowImportChange, SelectiveShowImportPorts,
    SelectiveShowImportPreview, SelectiveShowImportRequest, SelectiveShowImportResult,
    SelectiveShowImportService, SelectiveShowObjectChange, SelectiveShowProfileChange,
};
pub use show_compiler::{PreparedShowCandidate, prepare_show_candidate};
pub use show_patch::{
    PatchChange, PatchFixtureCandidate, PatchFixtureProjection, PatchFixturesCommand,
    PatchFixturesResult, PatchModeProjection, PatchProfileRevisionProjection, PatchSnapshot,
    ShowPatchPorts, ShowPatchService,
};
pub use speed_group::{
    SPEED_GROUP_COUNT, SpeedBpm, SpeedBpmDelta, SpeedGroupAction, SpeedGroupApplication,
    SpeedGroupAuthorityProjection, SpeedGroupChange, SpeedGroupCommand, SpeedGroupDurability,
    SpeedGroupExpectation, SpeedGroupId, SpeedGroupOutcome, SpeedGroupPortState, SpeedGroupPorts,
    SpeedGroupProjection, SpeedGroupResolvedAction, SpeedGroupResult, SpeedGroupService,
    SpeedGroupSnapshot,
};
pub use timeline::{
    TimelineError, TimelineErrorKind, TimelineExecution, TimelineExecutionRequest, TimelineHost,
    TimelineHostBackend, TimelineId, TimelineOperation, TimelineRuntime, TimelineService,
};
