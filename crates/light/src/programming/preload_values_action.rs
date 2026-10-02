use super::ProgrammingPreloadValuesProjection;
use crate::{ActionContext, ApplicationCommand, CommandFamily, ProgrammingValueIntent};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::{borrow::Cow, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProgrammingPreloadValueTiming {
    pub fade: bool,
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingPreloadValueMutation {
    SetFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammingPreloadValueTiming,
    },
    ReleaseFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
    },
    SetGroup {
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammingPreloadValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: AttributeKey,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingPreloadValuesCommand {
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
        timing: ProgrammingPreloadValueTiming,
    },
    ReleaseFixture {
        fixture_id: FixtureId,
        attribute: AttributeKey,
    },
    SetGroup {
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammingPreloadValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: AttributeKey,
    },
    Batch {
        mutations: Vec<ProgrammingPreloadValueMutation>,
    },
}

impl ProgrammingPreloadValuesCommand {
    pub fn mutations(&self) -> Cow<'_, [ProgrammingPreloadValueMutation]> {
        match self {
            Self::ApplyIntent { .. } | Self::FinishGesture { .. } => Cow::Borrowed(&[]),
            Self::SetFixture {
                fixture_id,
                attribute,
                value,
                timing,
            } => Cow::Owned(vec![ProgrammingPreloadValueMutation::SetFixture {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
                value: value.clone(),
                timing: *timing,
            }]),
            Self::ReleaseFixture {
                fixture_id,
                attribute,
            } => Cow::Owned(vec![ProgrammingPreloadValueMutation::ReleaseFixture {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
            }]),
            Self::SetGroup {
                group_id,
                attribute,
                value,
                timing,
            } => Cow::Owned(vec![ProgrammingPreloadValueMutation::SetGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: value.clone(),
                timing: *timing,
            }]),
            Self::ReleaseGroup {
                group_id,
                attribute,
            } => Cow::Owned(vec![ProgrammingPreloadValueMutation::ReleaseGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
            }]),
            Self::Batch { mutations } => Cow::Borrowed(mutations),
        }
    }

    pub const fn intent(&self) -> Option<&ProgrammingValueIntent> {
        match self {
            Self::ApplyIntent { intent } => Some(intent),
            Self::FinishGesture { .. }
            | Self::SetFixture { .. }
            | Self::ReleaseFixture { .. }
            | Self::SetGroup { .. }
            | Self::ReleaseGroup { .. }
            | Self::Batch { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPreloadValuesRequest {
    pub expected_capture_mode_revision: u64,
    pub command: ProgrammingPreloadValuesCommand,
}

impl ApplicationCommand for ProgrammingPreloadValuesRequest {
    type Value = ProgrammingPreloadValuesResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingPreloadValuesOutcome {
    Changed {
        projection: Arc<ProgrammingPreloadValuesProjection>,
        event_sequence: u64,
    },
    NoChange {
        revision: u64,
    },
}

impl ProgrammingPreloadValuesOutcome {
    pub fn revision(&self) -> u64 {
        match self {
            Self::Changed { projection, .. } => projection.revision,
            Self::NoChange { revision } => *revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPreloadValuesResult {
    pub context: ActionContext,
    pub outcome: ProgrammingPreloadValuesOutcome,
    pub capture_mode_revision: u64,
    pub interaction_event_sequence: Option<u64>,
    pub replayed: bool,
    pub warning: Option<String>,
    /// TL-594: set when the action was held quietly instead of applied.
    pub hold: Option<crate::ProgrammingValuesHold>,
    /// TL-554: the semantic starting value the first semantic edit of a Direct value adopted.
    pub color_adoption: Option<crate::ProgrammingColorAdoption>,
}
