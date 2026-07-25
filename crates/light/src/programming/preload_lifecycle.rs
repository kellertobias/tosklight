use super::{
    ProgrammingCaptureModeProjection, ProgrammingPreloadPlaybackQueueProjection,
    ProgrammingPreloadValuesProjection,
};
use crate::{
    ActionContext, ActionError, ApplicationCommand, CommandFamily, PlaybackRuntimeProjection,
};
use chrono::{DateTime, Utc};
use light_core::ShowId;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPreloadRevisionExpectation {
    Exact(u64),
    Current,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingPreloadLifecycleAction {
    Enter,
    Go {
        show_id: ShowId,
        expected_show_revision: ProgrammingPreloadRevisionExpectation,
        expected_playback_event_sequence: ProgrammingPreloadRevisionExpectation,
    },
    ClearPending,
    Release,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingPreloadLifecycleRequest {
    pub expected_capture_mode_revision: ProgrammingPreloadRevisionExpectation,
    pub expected_values_revision: ProgrammingPreloadRevisionExpectation,
    pub expected_queue_revision: ProgrammingPreloadRevisionExpectation,
    pub expected_selection_revision: ProgrammingPreloadRevisionExpectation,
    pub action: ProgrammingPreloadLifecycleAction,
}

impl ApplicationCommand for ProgrammingPreloadLifecycleRequest {
    type Value = ProgrammingPreloadLifecycleResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPreloadRuntimeChange {
    pub projection: PlaybackRuntimeProjection,
    pub event_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingPreloadExecutedPlaybackAction {
    pub playback_number: u16,
    pub page: Option<u8>,
    pub action: super::ProgrammingPreloadPlaybackAction,
    pub surface: super::ProgrammingPreloadPlaybackSurface,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPreloadCommitResult {
    pub show_id: ShowId,
    pub show_revision: u64,
    pub playback_event_sequence_before: u64,
    pub playback_event_sequence_after: u64,
    pub committed_at: DateTime<Utc>,
    pub programmer_fade_millis: u64,
    pub executed_playback_actions: usize,
    pub executed: Vec<ProgrammingPreloadExecutedPlaybackAction>,
    pub runtime_changes: Vec<ProgrammingPreloadRuntimeChange>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPreloadLifecycleState {
    Changed,
    NoChange,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPreloadLifecycleResult {
    pub context: ActionContext,
    pub request_id: String,
    pub replayed: bool,
    pub state: ProgrammingPreloadLifecycleState,
    pub active: bool,
    pub capture_mode: Arc<ProgrammingCaptureModeProjection>,
    pub capture_mode_event_sequence: Option<u64>,
    pub values_revision: u64,
    pub values_projection: Option<Arc<ProgrammingPreloadValuesProjection>>,
    pub values_event_sequence: Option<u64>,
    pub queue_revision: u64,
    pub queue_projection: Option<Arc<ProgrammingPreloadPlaybackQueueProjection>>,
    pub queue_event_sequence: Option<u64>,
    pub interaction_event_sequence: Option<u64>,
    pub selection_revision: u64,
    pub commit: Option<ProgrammingPreloadCommitResult>,
    pub warning: Option<String>,
}

pub trait ProgrammingPreloadLifecyclePorts: Send + Sync {
    fn authorize_preload_lifecycle(&self, context: &ActionContext) -> Result<(), ActionError>;

    /// Reads the configured Enter policy while the application action owns its user/desk gate.
    fn capture_programmer_on_preload(&self, context: &ActionContext) -> bool;

    /// Commits through the server's one prepared Playback batch and one final install.
    fn commit_preload(
        &self,
        context: &ActionContext,
        request: &ProgrammingPreloadLifecycleRequest,
    ) -> Result<ProgrammingPreloadCommitResult, ActionError>;

    fn reconcile_preload_capture(&self, context: &ActionContext);

    fn persist_preload_lifecycle(
        &self,
        context: &ActionContext,
        operation: &'static str,
    ) -> Option<String>;
}
