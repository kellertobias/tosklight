use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ApplicationCommand, CommandFamily,
};
use light_core::{FixtureId, UserId};
use light_programmer::{
    GroupDefinition, HighlightAction, HighlightFixture, HighlightOutputLayer, HighlightRegistry,
    HighlightSelectionWrite, HighlightState, HighlightTransition, ProgrammerSelection,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HighlightActionPublication {
    Standard,
    Compatibility,
}

#[derive(Clone, Debug)]
pub enum HighlightCommand {
    Action {
        action: HighlightAction,
        publication: HighlightActionPublication,
    },
    Status,
    Reconcile {
        source: String,
    },
}

impl HighlightCommand {
    pub const fn action(action: HighlightAction) -> Self {
        Self::Action {
            action,
            publication: HighlightActionPublication::Standard,
        }
    }

    pub const fn compatibility_action(action: HighlightAction) -> Self {
        Self::Action {
            action,
            publication: HighlightActionPublication::Compatibility,
        }
    }

    pub const fn status() -> Self {
        Self::Status
    }

    pub fn reconcile(source: impl Into<String>) -> Self {
        Self::Reconcile {
            source: source.into(),
        }
    }
}

impl ApplicationCommand for HighlightCommand {
    type Value = HighlightResult;

    const FAMILY: CommandFamily = CommandFamily::Output;
}

#[derive(Clone, Debug)]
pub struct HighlightEnvironment {
    pub selection: ProgrammerSelection,
    pub fixtures: Vec<HighlightFixture>,
    pub groups: HashMap<String, GroupDefinition>,
    pub output_suppressed: bool,
}

#[derive(Clone, Debug)]
pub struct HighlightResult {
    pub state: HighlightState,
    pub selection_changed: bool,
}

pub trait HighlightPorts: Send + Sync {
    fn environment(&self, context: &ActionContext) -> Result<HighlightEnvironment, ActionError>;

    /// Applies and durably records one selection write. Returning the authoritative selection
    /// lets the service acknowledge the exact revision it caused.
    fn apply_selection(
        &self,
        context: &ActionContext,
        write: &HighlightSelectionWrite,
    ) -> Result<ProgrammerSelection, ActionError>;

    /// Projects combined Highlight output. Adapters may merge another deliberate transient source
    /// such as Patch Preview before installing it in the engine.
    fn synchronize_output(
        &self,
        context: &ActionContext,
        fixtures: &[FixtureId],
    ) -> Result<(), ActionError>;

    /// Installs the complete Highlight/Low Light projection. The compatibility default keeps
    /// existing adapters source-compatible until they adopt roles and attribute suppression.
    fn synchronize_output_layers(
        &self,
        context: &ActionContext,
        layers: &[HighlightOutputLayer],
    ) -> Result<(), ActionError> {
        self.synchronize_output(
            context,
            &layers
                .iter()
                .map(|layer| layer.fixture_id)
                .collect::<Vec<_>>(),
        )
    }

    fn publish_programmer_changed(&self, context: &ActionContext, command: &HighlightCommand);

    fn publish_highlight_changed(
        &self,
        context: &ActionContext,
        command: &HighlightCommand,
        state: &HighlightState,
    );

    fn publish_feedback(&self, context: &ActionContext);
}

#[derive(Clone)]
pub struct HighlightService {
    registry: Arc<HighlightRegistry>,
}

impl HighlightService {
    pub const fn new(registry: Arc<HighlightRegistry>) -> Self {
        Self { registry }
    }

    pub fn handle(
        &self,
        envelope: ActionEnvelope<HighlightCommand>,
        ports: &dyn HighlightPorts,
    ) -> Result<HighlightResult, ActionError> {
        let user_id = required_user(&envelope.context)?;
        let environment = ports.environment(&envelope.context)?;
        let transition =
            self.transition(&envelope.context, user_id, &environment, &envelope.command);
        let selection_changed = if let Some(write) = transition.working_selection.as_ref() {
            let selection = ports.apply_selection(&envelope.context, write)?;
            self.registry.acknowledge_internal_selection(
                envelope.context.desk_id,
                user_id,
                &selection,
            );
            true
        } else {
            false
        };

        ports.synchronize_output_layers(&envelope.context, &self.registry.output_layers())?;
        if selection_changed && publishes_programmer_change(&envelope.command) {
            ports.publish_programmer_changed(&envelope.context, &envelope.command);
        }
        if publishes_highlight_change(&envelope.command) {
            ports.publish_highlight_changed(
                &envelope.context,
                &envelope.command,
                &transition.state,
            );
        }
        if publishes_feedback(&envelope.command) {
            ports.publish_feedback(&envelope.context);
        }
        Ok(HighlightResult {
            state: transition.state,
            selection_changed,
        })
    }

    /// Read-only projection for bootstrap and feedback. Unlike [`Self::handle`], this deliberately
    /// does not apply a reconciliation write or publish events.
    pub fn snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn HighlightPorts,
    ) -> Result<HighlightState, ActionError> {
        required_user(context)?;
        let environment = ports.environment(context)?;
        Ok(self
            .registry
            .status(
                &environment.selection,
                &environment.fixtures,
                &environment.groups,
                environment.output_suppressed,
            )
            .state)
    }

    fn transition(
        &self,
        context: &ActionContext,
        user_id: UserId,
        environment: &HighlightEnvironment,
        command: &HighlightCommand,
    ) -> HighlightTransition {
        match command {
            HighlightCommand::Action { action, .. } => self.registry.action_guarded(
                context.desk_id,
                user_id,
                *action,
                &environment.selection,
                &environment.fixtures,
                &environment.groups,
                environment.output_suppressed,
            ),
            HighlightCommand::Status | HighlightCommand::Reconcile { .. } => self.registry.status(
                &environment.selection,
                &environment.fixtures,
                &environment.groups,
                environment.output_suppressed,
            ),
        }
    }
}

fn required_user(context: &ActionContext) -> Result<UserId, ActionError> {
    context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Highlight requires an authenticated user",
        )
    })
}

const fn publishes_programmer_change(command: &HighlightCommand) -> bool {
    !matches!(
        command,
        HighlightCommand::Action {
            publication: HighlightActionPublication::Compatibility,
            ..
        }
    )
}

const fn publishes_highlight_change(command: &HighlightCommand) -> bool {
    !matches!(command, HighlightCommand::Status)
}

const fn publishes_feedback(command: &HighlightCommand) -> bool {
    matches!(
        command,
        HighlightCommand::Action {
            publication: HighlightActionPublication::Standard,
            ..
        } | HighlightCommand::Reconcile { .. }
    )
}

#[cfg(test)]
mod tests;
