use super::ProgrammingValuesProjection;
use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::sync::Arc;

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

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingValuesCommand {
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
    pub fn mutations(&self) -> Vec<ProgrammingValueMutation> {
        match self {
            Self::SetFixture {
                fixture_id,
                attribute,
                value,
                timing,
            } => vec![ProgrammingValueMutation::SetFixture {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
                value: value.clone(),
                timing: *timing,
            }],
            Self::ReleaseFixture {
                fixture_id,
                attribute,
            } => vec![ProgrammingValueMutation::ReleaseFixture {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
            }],
            Self::SetGroup {
                group_id,
                attribute,
                value,
                timing,
            } => vec![ProgrammingValueMutation::SetGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: value.clone(),
                timing: *timing,
            }],
            Self::ReleaseGroup {
                group_id,
                attribute,
            } => vec![ProgrammingValueMutation::ReleaseGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
            }],
            Self::Batch { mutations } => mutations.clone(),
            Self::Clear => Vec::new(),
        }
    }

    pub const fn is_clear(&self) -> bool {
        matches!(self, Self::Clear)
    }
}

impl ApplicationCommand for ProgrammingValuesCommand {
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
    pub interaction_event_sequence: Option<u64>,
    pub replayed: bool,
    pub warning: Option<String>,
}
