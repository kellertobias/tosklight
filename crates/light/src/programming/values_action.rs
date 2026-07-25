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
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingValueOperation {
    AbsoluteSet(AttributeValue),
    RelativeStep(f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingValuesCommand {
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
            Self::ApplyIntent { .. } => Cow::Borrowed(&[]),
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
}
