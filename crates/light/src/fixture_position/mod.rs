//! Shared typed boundary for querying and revisioning fixture stage positions.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionOutcome, ApplicationCommand,
    CommandFamily,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StagePosition {
    pub x_mm: i32,
    pub y_mm: i32,
    pub z_mm: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureProjection {
    pub id: String,
    pub name: String,
    pub position: StagePosition,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixturePositionCommand {
    pub fixture_id: String,
    pub position: StagePosition,
}

impl ApplicationCommand for FixturePositionCommand {
    type Value = FixturePositionOutcome;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixturePositionOutcome {
    pub fixture_id: String,
    pub position: StagePosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixturePositionExecution {
    pub outcome: FixturePositionOutcome,
    pub revision: u64,
    pub event_sequence: Option<u64>,
}

pub trait FixturePositionPorts: Send + Sync {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError>;

    fn fixture(
        &self,
        context: &ActionContext,
        fixture_id: &str,
    ) -> Result<Option<FixtureProjection>, ActionError>;

    fn set_position(
        &self,
        context: &ActionContext,
        command: &FixturePositionCommand,
        expected_revision: u64,
    ) -> Result<FixturePositionExecution, ActionError>;
}

/// Ordered application service shared by UI, HTTP, Timecode, and Macro adapters.
#[derive(Clone, Default)]
pub struct FixturePositionService {
    operation: Arc<Mutex<()>>,
}

impl FixturePositionService {
    pub fn fixture(
        &self,
        context: &ActionContext,
        fixture_id: &str,
        ports: &dyn FixturePositionPorts,
    ) -> Result<Option<FixtureProjection>, ActionError> {
        ports.authorize(context)?;
        ports.fixture(context, fixture_id)
    }

    pub fn handle(
        &self,
        envelope: ActionEnvelope<FixturePositionCommand>,
        ports: &dyn FixturePositionPorts,
    ) -> Result<ActionOutcome<FixturePositionOutcome>, ActionError> {
        let expected_revision = envelope.context.expected_revision.ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Invalid,
                "fixture position changes require an expected revision",
            )
        })?;
        let _ordered = self.operation.lock();
        ports.authorize(&envelope.context)?;
        let execution =
            ports.set_position(&envelope.context, &envelope.command, expected_revision)?;
        let mut outcome = ActionOutcome::new(execution.outcome).at_revision(execution.revision);
        if let Some(sequence) = execution.event_sequence {
            outcome = outcome.with_event_sequence(sequence);
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests;
