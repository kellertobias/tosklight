use super::ProgrammingValuesProjection;
use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::{borrow::Cow, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProgrammingValueTiming {
    pub fade: bool,
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingValueMutation {
    SetFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammingValueTiming,
    },
    ReleaseFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
    },
    SetGroup {
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammingValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: AttributeKey,
    },
}

/// One operator value gesture before application-owned activation expansion.
///
/// The ordered targets and initiating operation are transport facts. Any linked attributes are
/// resolved later from one `ProgrammingValuesEnvironment`, inside the Programmer transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValueIntent {
    pub fixture_ids: Vec<FixtureId>,
    pub group_id: Option<String>,
    pub attribute: AttributeKey,
    pub operation: ProgrammingValueOperation,
    pub undo_group: Option<String>,
    pub timing: ProgrammingValueTiming,
    /// TL-594: the accepted source the surface displayed. Present, first-edit adoption uses
    /// only that leased source and holds quietly when it is gone. Absent (OSC, HTTP
    /// integrators), adoption keeps reading the latest accepted source of the lane.
    pub displayed_source: Option<ProgrammingDisplayedSource>,
    /// TL-554: Direct reference head and explicit semantic starting colour, when supplied.
    pub color_adoption: super::ProgrammingColorAdoptionRequest,
}

/// Which lane's accepted source a displayed-source lease names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingDisplayedLane {
    Normal,
    Preload,
}

/// An opaque, session-scoped lease of one source the server delivered to that session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingDisplayedSource {
    pub lane: ProgrammingDisplayedLane,
    pub lease: u64,
}

/// Why a values action was held quietly: no mutation, revision or Undo step. The surface
/// re-reads and retries from fresh state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingValuesHold {
    /// The named displayed source is unknown, expired, of another lane, or no longer current.
    DisplayedSourceUnavailable,
    /// TL-554: a Direct (native) edit has no verified reference head, original model or
    /// published premaster output to adopt.
    NativeColorUnavailable,
    /// TL-554: a Direct value's visible appearance is unknown; the first semantic edit needs
    /// the operator's explicit starting colour.
    ExplicitColorStartRequired,
    /// TL-637 follow-up: a Zoom edit has no seed in degrees: no authored Zoom and no opening
    /// measurable in a known convention on the displayed output (unknown or unsupported model).
    ZoomUnavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingValueOperation {
    AbsoluteSet(AttributeValue),
    RelativeStep(f32),
    /// Component operations produce one complete owner, adopted independently per target.
    ComponentEdits(Vec<light_core::programming::ComponentEdit>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingValuesCommand {
    /// Retire only the matching runtime capture and Undo gesture; never change values.
    FinishGesture {
        attribute: AttributeKey,
        undo_group: String,
    },
    ApplyIntent {
        intent: ProgrammingValueIntent,
    },
    SetFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammingValueTiming,
    },
    ReleaseFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
    },
    SetGroup {
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammingValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: AttributeKey,
    },
    Batch {
        mutations: Vec<ProgrammingValueMutation>,
    },
    Clear,
}

impl ProgrammingValuesCommand {
    pub fn mutations(&self) -> Cow<'_, [ProgrammingValueMutation]> {
        match self {
            Self::ApplyIntent { .. } | Self::FinishGesture { .. } => Cow::Borrowed(&[]),
            Self::SetFixture {
                fixture_id,
                attribute,
                value,
                timing,
            } => Cow::Owned(vec![ProgrammingValueMutation::SetFixture {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
                value: value.clone(),
                timing: *timing,
            }]),
            Self::ReleaseFixture {
                fixture_id,
                attribute,
            } => Cow::Owned(vec![ProgrammingValueMutation::ReleaseFixture {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
            }]),
            Self::SetGroup {
                group_id,
                attribute,
                value,
                timing,
            } => Cow::Owned(vec![ProgrammingValueMutation::SetGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: value.clone(),
                timing: *timing,
            }]),
            Self::ReleaseGroup {
                group_id,
                attribute,
            } => Cow::Owned(vec![ProgrammingValueMutation::ReleaseGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
            }]),
            Self::Batch { mutations } => Cow::Borrowed(mutations),
            Self::Clear => Cow::Borrowed(&[]),
        }
    }

    pub const fn is_clear(&self) -> bool {
        matches!(self, Self::Clear)
    }

    pub const fn intent(&self) -> Option<&ProgrammingValueIntent> {
        match self {
            Self::ApplyIntent { intent } => Some(intent),
            _ => None,
        }
    }
}

/// One normal-values action plus its atomic capture-mode precondition.
///
/// The normal-values revision remains in `ActionContext.expected_revision`; keeping the related
/// capture revision here avoids making generic action metadata feature-specific.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValuesRequest {
    pub expected_capture_mode_revision: u64,
    pub command: ProgrammingValuesCommand,
}

impl ApplicationCommand for ProgrammingValuesRequest {
    type Value = ProgrammingValuesResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingValuesOutcome {
    Changed {
        projection: Arc<ProgrammingValuesProjection>,
        event_sequence: u64,
    },
    NoChange {
        revision: u64,
    },
}

impl ProgrammingValuesOutcome {
    pub fn revision(&self) -> u64 {
        match self {
            Self::Changed { projection, .. } => projection.revision,
            Self::NoChange { revision } => *revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValuesResult {
    pub context: ActionContext,
    pub outcome: ProgrammingValuesOutcome,
    pub capture_mode_revision: u64,
    pub interaction_event_sequence: Option<u64>,
    pub replayed: bool,
    pub warning: Option<String>,
    /// TL-594: set when the action was held quietly instead of applied.
    pub hold: Option<ProgrammingValuesHold>,
    /// TL-554: the semantic starting value the first semantic edit of a Direct value adopted.
    pub color_adoption: Option<super::ProgrammingColorAdoption>,
}
