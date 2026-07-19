//! Transport-independent application contracts and use-case infrastructure.
//!
//! Domain crates return typed state transitions. Application services publish those transitions
//! here, while server and desktop adapters translate them into their public wire contracts.

pub mod action;
pub mod event;
pub mod playback;
pub mod programming;
pub mod show_compiler;
pub mod show_patch;

pub use action::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionOutcome, ActionSource,
    ApplicationCommand, CommandFamily,
};
pub use event::{
    ApplicationEvent, CueReference, DeliveryPolicy, EventBus, EventCapability, EventClass,
    EventDraft, EventEnvelope, EventFilter, EventObject, EventReplay, EventSource,
    EventSubscription, PlaybackCueTransition, PlaybackEvent, PlaybackTransitionCause,
    ReplaceableEventRateLimit, SequenceGap, ShowEvent, SubscriptionDelivery, SubscriptionOptions,
};
pub use playback::{
    CueNumber, PendingPlaybackAction, PlaybackAction, PlaybackAddress, PlaybackCommand,
    PlaybackExecution, PlaybackLevel, PlaybackOutcome, PlaybackPorts, PlaybackResult,
    PlaybackService, PlaybackSurface, ResolvedPlaybackAddress, automatic_playback_events,
    publish_automatic_playback_events,
};
pub use programming::{
    CueMoveCopyChoice, CueTransferOperation, ExecutionPolicy, ProgrammingAction,
    ProgrammingChoiceOption, ProgrammingChoiceOptionId, ProgrammingCommand, ProgrammingExecution,
    ProgrammingOutcome, ProgrammingPorts, ProgrammingResult, ProgrammingService,
};
pub use show_compiler::{PreparedShowCandidate, prepare_show_candidate};
pub use show_patch::{
    ActiveShowUnitOfWork, BackupIdentity, PatchChange, PatchFixtureCandidate,
    PatchFixtureProjection, PatchFixturesCommand, PatchFixturesResult, PatchModeProjection,
    PatchProfileRevisionProjection, PatchSnapshot, ShowPatchPorts, ShowPatchService,
};
