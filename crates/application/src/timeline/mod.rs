//! Neutral operation boundary for future Timecode and managed-media workflows.
//!
//! Decoder, editor, seek, missed-event, and restart policies remain future product decisions.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use uuid::Uuid;

use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, AssetAvailability,
    AssetReference, CancellationSignal, MacroDefinition, MacroError, MacroErrorKind,
    MacroExecutionRequest, MacroExecutionSnapshot, MacroId, MacroInvocation, MacroService,
    MonotonicMoment, MonotonicScheduler, PlaybackCommand, PlaybackPorts, PlaybackResult,
    PlaybackService, SchedulerError, SchedulerErrorKind,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimelineId(pub Uuid);

/// Language-visible execution data. Trusted action identity remains inside [`TimelineService`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimelineExecution {
    pub timeline_id: TimelineId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineExecutionRequest {
    pub timeline_id: TimelineId,
    pub context: ActionContext,
    pub deadline: MonotonicMoment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineErrorKind {
    Cancelled,
    MissingAsset,
    InvalidOperation,
    Scheduler(SchedulerErrorKind),
    Action(ActionErrorKind),
    Macro(MacroErrorKind),
    Host,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineError {
    pub kind: TimelineErrorKind,
    pub message: String,
    pub current_revision: Option<u64>,
    pub retryable: bool,
}

impl TimelineError {
    pub fn new(kind: TimelineErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            current_revision: None,
            retryable: false,
        }
    }
}

impl From<ActionError> for TimelineError {
    fn from(error: ActionError) -> Self {
        Self {
            kind: TimelineErrorKind::Action(error.kind),
            message: error.message,
            current_revision: error.current_revision,
            retryable: error.retryable,
        }
    }
}

impl From<MacroError> for TimelineError {
    fn from(error: MacroError) -> Self {
        Self {
            kind: TimelineErrorKind::Macro(error.kind),
            message: error.message,
            current_revision: error.current_revision,
            retryable: error.retryable,
        }
    }
}

impl From<SchedulerError> for TimelineError {
    fn from(error: SchedulerError) -> Self {
        Self::new(TimelineErrorKind::Scheduler(error.kind), error.message)
    }
}

pub trait TimelineHostBackend: Send + Sync {
    fn asset_availability(
        &self,
        context: &ActionContext,
        asset: AssetReference,
    ) -> Result<AssetAvailability, TimelineError>;

    fn playback_ports(&self) -> &dyn PlaybackPorts;

    fn macro_definition(
        &self,
        context: &ActionContext,
        id: &MacroId,
        revision: u64,
    ) -> Result<MacroDefinition, TimelineError>;
}

/// Future timeline operations receive only typed commands; the host binds trusted action context.
pub trait TimelineHost: Send + Sync {
    fn asset_availability(&self, asset: AssetReference)
    -> Result<AssetAvailability, TimelineError>;
    fn trigger_playback(&self, command: PlaybackCommand) -> Result<PlaybackResult, TimelineError>;
    fn invoke_macro(
        &self,
        invocation: MacroInvocation,
    ) -> Result<MacroExecutionSnapshot, TimelineError>;
}

pub trait TimelineOperation: Send + Sync {
    fn execute(
        &self,
        timeline: &TimelineExecution,
        host: &dyn TimelineHost,
        cancellation: &dyn CancellationSignal,
    ) -> Result<(), TimelineError>;
}

pub trait TimelineRuntime: Send + Sync {
    fn operate(
        &self,
        timeline: &TimelineExecution,
        operation: &dyn TimelineOperation,
        host: &dyn TimelineHost,
        cancellation: &dyn CancellationSignal,
    ) -> Result<(), TimelineError>;
}

#[derive(Clone)]
pub struct TimelineService {
    scheduler: Arc<dyn MonotonicScheduler>,
    playbacks: PlaybackService,
    macros: MacroService,
}

impl TimelineService {
    pub fn new(
        scheduler: Arc<dyn MonotonicScheduler>,
        playbacks: PlaybackService,
        macros: MacroService,
    ) -> Self {
        Self {
            scheduler,
            playbacks,
            macros,
        }
    }

    pub fn operate(
        &self,
        request: TimelineExecutionRequest,
        runtime: &dyn TimelineRuntime,
        operation: &dyn TimelineOperation,
        backend: Arc<dyn TimelineHostBackend>,
        cancellation: &dyn CancellationSignal,
    ) -> Result<(), TimelineError> {
        self.scheduler.wait_until(request.deadline, cancellation)?;
        if cancellation.is_cancelled() {
            return Err(TimelineError::new(
                TimelineErrorKind::Cancelled,
                "timeline operation was cancelled",
            ));
        }
        let timeline = TimelineExecution {
            timeline_id: request.timeline_id,
        };
        let host = ScopedTimelineHost::new(
            request.timeline_id,
            request.context,
            backend,
            self.playbacks.clone(),
            self.macros.clone(),
            cancellation,
        );
        runtime.operate(&timeline, operation, &host, cancellation)
    }
}

struct ScopedTimelineHost<'a> {
    timeline_id: TimelineId,
    context: ActionContext,
    backend: Arc<dyn TimelineHostBackend>,
    playbacks: PlaybackService,
    macros: MacroService,
    cancellation: &'a dyn CancellationSignal,
    action_sequence: AtomicU64,
}

impl<'a> ScopedTimelineHost<'a> {
    fn new(
        timeline_id: TimelineId,
        mut context: ActionContext,
        backend: Arc<dyn TimelineHostBackend>,
        playbacks: PlaybackService,
        macros: MacroService,
        cancellation: &'a dyn CancellationSignal,
    ) -> Self {
        context.source = ActionSource::Timecode;
        context.expected_revision = None;
        Self {
            timeline_id,
            context,
            backend,
            playbacks,
            macros,
            cancellation,
            action_sequence: AtomicU64::new(0),
        }
    }

    fn ensure_active(&self) -> Result<(), TimelineError> {
        if self.cancellation.is_cancelled() {
            Err(TimelineError::new(
                TimelineErrorKind::Cancelled,
                "timeline operation was cancelled",
            ))
        } else {
            Ok(())
        }
    }

    fn action_context(&self) -> ActionContext {
        let sequence = self.action_sequence.fetch_add(1, Ordering::Relaxed);
        let mut context = self.context.clone();
        context.request_id = Some(format!("timeline:{}:{sequence}", self.timeline_id.0));
        context
    }
}

impl TimelineHost for ScopedTimelineHost<'_> {
    fn asset_availability(
        &self,
        asset: AssetReference,
    ) -> Result<AssetAvailability, TimelineError> {
        self.ensure_active()?;
        self.backend
            .asset_availability(&self.action_context(), asset)
    }

    fn trigger_playback(&self, command: PlaybackCommand) -> Result<PlaybackResult, TimelineError> {
        self.ensure_active()?;
        self.playbacks
            .handle(
                ActionEnvelope {
                    context: self.action_context(),
                    command,
                },
                self.backend.playback_ports(),
            )
            .map_err(Into::into)
    }

    fn invoke_macro(
        &self,
        invocation: MacroInvocation,
    ) -> Result<MacroExecutionSnapshot, TimelineError> {
        self.ensure_active()?;
        let context = self.action_context();
        let definition =
            self.backend
                .macro_definition(&context, &invocation.id, invocation.revision)?;
        if definition.id != invocation.id || definition.revision != invocation.revision {
            return Err(TimelineError::new(
                TimelineErrorKind::InvalidOperation,
                "resolved Macro definition does not match the timeline reference",
            ));
        }
        self.ensure_active()?;
        self.macros
            .start(MacroExecutionRequest {
                definition,
                context,
                arguments: invocation.arguments,
            })
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
