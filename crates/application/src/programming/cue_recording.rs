use crate::{
    ActionContext, ActionError, ActiveShowObjectKind, ActiveShowPorts, ApplicationCommand,
    CommandFamily, CueNumber, PlaybackCueReference, PlaybackRuntimeProjection,
};
use light_core::{CueListId, Revision, ShowId};
use light_playback::{
    CueChange, CueList, CueListRecordingPlan, CueRecordOperation, CueRecordingContent,
    CueRecordingPlanError, CueRecordingTiming, GroupCueChange,
};
use light_programmer::{CueRecordingCapture, CueRecordingCapturedSource};
use light_show::PortableShowRevision;
use std::sync::Arc;
use uuid::Uuid;

/// Records one action-time Programmer capture into a portable Cuelist transaction.
///
/// Recordable values are deliberately absent. The service captures them after authentication,
/// ownership, replay, and environment checks while holding the user's Programming gate.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueRecordRequest {
    pub show_id: ShowId,
    pub target: ProgrammingCueRecordTarget,
    pub operation: ProgrammingCueRecordOperation,
    pub cue_number: Option<CueNumber>,
    pub timing: ProgrammingCueRecordTiming,
    pub cue_only: bool,
    pub name: Option<String>,
    pub capture_policy: ProgrammingCueCapturePolicy,
    pub activation_policy: ProgrammingCueActivationPolicy,
    pub expected_show_revision: ProgrammingCueShowRevisionExpectation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingCueRecordTarget {
    Pool { playback_number: u16 },
    SelectedPlayback,
    PageSlot { page: u8, slot: u8 },
    CueList { cue_list_id: CueListId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingCueRecordOperation {
    Overwrite,
    Merge,
    Subtract,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProgrammingCueRecordTiming {
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingCueCapturePolicy {
    CurrentCapture,
    PendingOrActivePreload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingCueActivationPolicy {
    Hold,
    GoToIfNormal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingCueShowRevisionExpectation {
    Current,
    Exact(PortableShowRevision),
}

impl ApplicationCommand for ProgrammingCueRecordRequest {
    type Value = ProgrammingCueRecordResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

/// One environment-resolved recording target. Empty page slots are allocated transactionally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingCueResolvedTarget {
    CueList {
        cue_list_id: CueListId,
    },
    Playback {
        playback_number: u16,
        page_slot: Option<ProgrammingCuePageSlot>,
    },
    EmptyPageSlot(ProgrammingCuePageSlot),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingCuePageSlot {
    pub page: u8,
    pub slot: u8,
}

/// Narrow live environment needed to resolve desk-local addressing and Merge-active semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueRecordingEnvironment {
    pub target: ProgrammingCueResolvedTarget,
    pub active_cue: Option<PlaybackCueReference>,
}

/// Private action-time capture passed to one adapter-owned show transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueCommit {
    pub show_id: ShowId,
    pub expected_show_revision: ProgrammingCueShowRevisionExpectation,
    request: ProgrammingCueRecordRequest,
    environment: ProgrammingCueRecordingEnvironment,
    capture: CueRecordingCapture,
}

impl ProgrammingCueCommit {
    pub(crate) fn new(
        request: ProgrammingCueRecordRequest,
        environment: ProgrammingCueRecordingEnvironment,
        capture: CueRecordingCapture,
    ) -> Self {
        Self {
            show_id: request.show_id,
            expected_show_revision: request.expected_show_revision,
            request,
            environment,
            capture,
        }
    }

    pub(crate) const fn request(&self) -> &ProgrammingCueRecordRequest {
        &self.request
    }

    pub(crate) const fn environment(&self) -> &ProgrammingCueRecordingEnvironment {
        &self.environment
    }

    pub(crate) fn plan_existing(
        &self,
        cue_list: &CueList,
    ) -> Result<CueListRecordingPlan, CueRecordingPlanError> {
        cue_list.plan_recording(self.content(), self.domain_operation())
    }

    pub(crate) fn plan_new(
        &self,
        cue_list_id: CueListId,
        name: String,
    ) -> Result<CueListRecordingPlan, CueRecordingPlanError> {
        let creates_first_cue = self.request.operation == ProgrammingCueRecordOperation::Overwrite
            || (self.request.operation == ProgrammingCueRecordOperation::Merge
                && self.request.cue_number.is_none());
        if !creates_first_cue {
            return Err(CueRecordingPlanError::CueDoesNotExist {
                cue_number: self.request.cue_number.map_or(1.0, CueNumber::value),
            });
        }
        CueList::new_recording(
            cue_list_id,
            name,
            self.content(),
            self.request.cue_number.map(CueNumber::value),
        )
    }

    fn domain_operation(&self) -> CueRecordOperation {
        let cue_number = self.request.cue_number.map(CueNumber::value);
        match (self.request.operation, cue_number) {
            (ProgrammingCueRecordOperation::Overwrite, None) => CueRecordOperation::Append,
            (ProgrammingCueRecordOperation::Overwrite, Some(cue_number)) => {
                CueRecordOperation::Overwrite { cue_number }
            }
            (ProgrammingCueRecordOperation::Merge, Some(cue_number)) => {
                CueRecordOperation::Merge { cue_number }
            }
            (ProgrammingCueRecordOperation::Merge, None) => CueRecordOperation::MergeActive {
                active_cue_id: self.environment.active_cue.as_ref().map(|cue| cue.id),
            },
            (ProgrammingCueRecordOperation::Subtract, Some(cue_number)) => {
                CueRecordOperation::Subtract { cue_number }
            }
            (ProgrammingCueRecordOperation::Subtract, None) => {
                unreachable!("Cue subtract is validated before capture")
            }
        }
    }

    fn content(&self) -> CueRecordingContent {
        CueRecordingContent {
            changes: self
                .capture
                .fixture_values
                .iter()
                .map(fixture_change)
                .collect(),
            group_changes: self.capture.group_values.iter().map(group_change).collect(),
            timing: CueRecordingTiming {
                fade_millis: self.request.timing.fade_millis,
                delay_millis: self.request.timing.delay_millis,
            },
            cue_only: self.request.cue_only,
            name: self.request.name.clone(),
        }
    }
}

fn fixture_change(value: &light_programmer::CueRecordingFixtureValue) -> CueChange {
    let mut change = CueChange::set(
        value.fixture_id,
        value.attribute.clone(),
        value.value.clone(),
    );
    change.fade_millis = value.fade_millis;
    change.delay_millis = value.delay_millis;
    change
}

fn group_change(value: &light_programmer::CueRecordingGroupValue) -> GroupCueChange {
    GroupCueChange {
        group_id: value.group_id.clone(),
        attribute: value.attribute.clone(),
        value: Some(value.value.clone()),
        automatic_restore: false,
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

/// Exact losslessly merged portable object returned by the atomic transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueObjectProjection {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub object_revision: Revision,
    pub raw_body: Arc<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueProjections {
    pub show_id: ShowId,
    pub cue_list: ProgrammingCueObjectProjection,
    pub playback: Option<ProgrammingCueObjectProjection>,
    pub page: Option<ProgrammingCueObjectProjection>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgrammingRecordedCue {
    pub id: Uuid,
    pub number: CueNumber,
    pub deleted: bool,
}

/// Adapter completion after one atomic show transaction or a verified no-change.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueCommitResult {
    pub changed: bool,
    pub projections: ProgrammingCueProjections,
    pub recorded_cue: ProgrammingRecordedCue,
    pub show_revision: PortableShowRevision,
    pub event_sequence: Option<u64>,
    /// Concrete Playback which may be taken live after a successful changed normal capture.
    pub concrete_playback_number: Option<u16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueActivationResult {
    pub projection: PlaybackRuntimeProjection,
    pub event_sequence: u64,
}

/// Post-commit take-live result. An already-authoritative runtime may legitimately emit no event.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueActivationCompletion {
    pub projection: PlaybackRuntimeProjection,
    pub event_sequence: Option<u64>,
}

pub trait ProgrammingCueRecordingPorts: Send + Sync {
    fn authorize_cue_recording(&self, context: &ActionContext) -> Result<(), ActionError>;

    fn cue_recording_environment(
        &self,
        context: &ActionContext,
        request: &ProgrammingCueRecordRequest,
    ) -> Result<ProgrammingCueRecordingEnvironment, ActionError>;

    /// Commits one lossless multi-object portable transaction and emits at most one Show event.
    /// Implementations must not re-enter a Programming user or desk gate.
    fn commit_cue(
        &self,
        context: &ActionContext,
        commit: &ProgrammingCueCommit,
    ) -> Result<ProgrammingCueCommitResult, ActionError>;

    /// Attempts to take one just-recorded Cue live after the portable commit. A coherent
    /// already-current runtime may return a completion without an event; adapters audit an
    /// unexpected post-commit activation failure and return `None` rather than unwinding it.
    fn activate_recorded_cue(
        &self,
        context: &ActionContext,
        playback_number: u16,
        cue_number: CueNumber,
    ) -> Option<ProgrammingCueActivationCompletion>;
}

/// Active-show adapter hooks used by the narrow Cue-recording transaction.
pub trait ProgrammingCueActiveShowPorts: ActiveShowPorts {
    fn reconcile_programming_cue(&self, _projections: &ProgrammingCueProjections) {}
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingCueRecordOutcome {
    Changed {
        projections: Arc<ProgrammingCueProjections>,
        recorded_cue: ProgrammingRecordedCue,
        show_revision: PortableShowRevision,
        show_event_sequence: u64,
        runtime: Option<Arc<ProgrammingCueActivationResult>>,
    },
    NoChange {
        projections: Arc<ProgrammingCueProjections>,
        recorded_cue: ProgrammingRecordedCue,
        show_revision: PortableShowRevision,
    },
}

impl ProgrammingCueRecordOutcome {
    pub fn projections(&self) -> &ProgrammingCueProjections {
        match self {
            Self::Changed { projections, .. } | Self::NoChange { projections, .. } => projections,
        }
    }

    pub const fn show_revision(&self) -> PortableShowRevision {
        match self {
            Self::Changed { show_revision, .. } | Self::NoChange { show_revision, .. } => {
                *show_revision
            }
        }
    }

    pub const fn show_event_sequence(&self) -> Option<u64> {
        match self {
            Self::Changed {
                show_event_sequence,
                ..
            } => Some(*show_event_sequence),
            Self::NoChange { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueRecordResult {
    pub context: ActionContext,
    pub request_id: String,
    pub correlation_id: Uuid,
    pub replayed: bool,
    pub captured_source: CueRecordingCapturedSource,
    pub outcome: ProgrammingCueRecordOutcome,
}
