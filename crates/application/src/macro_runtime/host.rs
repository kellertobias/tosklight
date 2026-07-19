use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use super::{
    CancellationSignal, GroupProjection, MacroAuditEntry, MacroCapability, MacroDefinition,
    MacroError, MacroExecutionId, MacroHostAction, MacroHttpPolicy, MacroHttpRequest,
    MacroHttpResponse, MacroHttpTransportError, MacroHttpTransportResponse, MacroResume,
    MacroWaitRequest,
};
use crate::{
    ActionContext, ActionEnvelope, ActionErrorKind, ActionOutcome, ActionSource,
    FixturePositionCommand, FixturePositionOutcome, FixturePositionPorts, FixturePositionService,
    FixtureProjection, PlaybackCommand, PlaybackPorts, PlaybackResult, PlaybackService,
};

pub trait MacroHostBackend: Send + Sync {
    fn authorize_execution(
        &self,
        context: &ActionContext,
        definition: &MacroDefinition,
    ) -> Result<(), MacroError>;

    fn fixture_position_ports(&self) -> &dyn FixturePositionPorts;
    fn playback_ports(&self) -> &dyn PlaybackPorts;

    fn group(
        &self,
        context: &ActionContext,
        id: &str,
    ) -> Result<Option<GroupProjection>, MacroError>;

    fn wait_for(
        &self,
        context: &ActionContext,
        request: MacroWaitRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<MacroResume, MacroError>;

    fn http_policy(
        &self,
        context: &ActionContext,
        execution_id: MacroExecutionId,
        definition: &MacroDefinition,
    ) -> Result<MacroHttpPolicy, MacroError>;

    /// Raw transport only. The scoped application host owns policy checks and audit lifecycle.
    fn dispatch_http(
        &self,
        context: &ActionContext,
        request: &MacroHttpRequest,
        policy: &MacroHttpPolicy,
        cancellation: &dyn CancellationSignal,
    ) -> Result<MacroHttpTransportResponse, MacroHttpTransportError>;

    fn record_http_audit(&self, entry: MacroAuditEntry) -> Result<(), MacroError>;
}

/// Language-visible capability surface. It deliberately accepts no [`ActionContext`] or envelope.
pub trait MacroHost: Send + Sync {
    fn definition(&self) -> &MacroDefinition;
    fn allows(&self, capability: MacroCapability) -> bool;
    fn fixture(&self, id: &str) -> Result<Option<FixtureProjection>, MacroError>;
    fn group(&self, id: &str) -> Result<Option<GroupProjection>, MacroError>;
    fn change_fixture_position(
        &self,
        command: FixturePositionCommand,
        action: MacroHostAction,
    ) -> Result<ActionOutcome<FixturePositionOutcome>, MacroError>;
    fn wait_for(&self, request: MacroWaitRequest) -> Result<MacroResume, MacroError>;
    fn trigger_playback(&self, command: PlaybackCommand) -> Result<PlaybackResult, MacroError>;
    fn http(&self, request: MacroHttpRequest) -> Result<MacroHttpResponse, MacroError>;
}

pub(crate) trait MacroLifecycleObserver: Send + Sync {
    fn waiting(&self, request: &MacroWaitRequest);
    fn running(&self);
}

pub(crate) struct ScopedMacroHost {
    execution_id: MacroExecutionId,
    definition: MacroDefinition,
    context: ActionContext,
    backend: Arc<dyn MacroHostBackend>,
    fixture_positions: FixturePositionService,
    playbacks: PlaybackService,
    lifecycle: Arc<dyn MacroLifecycleObserver>,
    cancellation: Arc<dyn CancellationSignal>,
    action_sequence: AtomicU64,
}

pub(crate) struct ScopedMacroHostDependencies {
    pub backend: Arc<dyn MacroHostBackend>,
    pub fixture_positions: FixturePositionService,
    pub playbacks: PlaybackService,
    pub lifecycle: Arc<dyn MacroLifecycleObserver>,
    pub cancellation: Arc<dyn CancellationSignal>,
}

impl ScopedMacroHost {
    pub(crate) fn new(
        execution_id: MacroExecutionId,
        definition: MacroDefinition,
        mut context: ActionContext,
        dependencies: ScopedMacroHostDependencies,
    ) -> Self {
        context.source = ActionSource::Macro;
        context.expected_revision = None;
        Self {
            execution_id,
            definition,
            context,
            backend: dependencies.backend,
            fixture_positions: dependencies.fixture_positions,
            playbacks: dependencies.playbacks,
            lifecycle: dependencies.lifecycle,
            cancellation: dependencies.cancellation,
            action_sequence: AtomicU64::new(0),
        }
    }

    fn require(&self, capability: MacroCapability) -> Result<(), MacroError> {
        if self.allows(capability) {
            Ok(())
        } else {
            Err(MacroError::action(
                ActionErrorKind::Forbidden,
                format!("Macro capability {capability:?} is not granted"),
            ))
        }
    }

    fn ensure_active(&self) -> Result<(), MacroError> {
        if self.cancellation.is_cancelled() {
            Err(MacroError::new(
                super::MacroErrorKind::Cancelled,
                "Macro execution was cancelled",
            ))
        } else {
            Ok(())
        }
    }

    fn action_context(&self, action: MacroHostAction) -> ActionContext {
        let sequence = self.action_sequence.fetch_add(1, Ordering::Relaxed);
        let mut context = self.context.clone();
        context.request_id = Some(format!("macro:{}:{sequence}", self.execution_id.0));
        context.expected_revision = action.expected_revision;
        context
    }
}

impl MacroHost for ScopedMacroHost {
    fn definition(&self) -> &MacroDefinition {
        &self.definition
    }

    fn allows(&self, capability: MacroCapability) -> bool {
        self.definition.capabilities.contains(&capability)
    }

    fn fixture(&self, id: &str) -> Result<Option<FixtureProjection>, MacroError> {
        self.ensure_active()?;
        self.require(MacroCapability::QueryFixtures)?;
        self.fixture_positions
            .fixture(
                &self.action_context(MacroHostAction::unversioned()),
                id,
                self.backend.fixture_position_ports(),
            )
            .map_err(Into::into)
    }

    fn group(&self, id: &str) -> Result<Option<GroupProjection>, MacroError> {
        self.ensure_active()?;
        self.require(MacroCapability::QueryGroups)?;
        self.backend
            .group(&self.action_context(MacroHostAction::unversioned()), id)
    }

    fn change_fixture_position(
        &self,
        command: FixturePositionCommand,
        action: MacroHostAction,
    ) -> Result<ActionOutcome<FixturePositionOutcome>, MacroError> {
        self.ensure_active()?;
        self.require(MacroCapability::ChangeFixturePosition)?;
        self.fixture_positions
            .handle(
                ActionEnvelope {
                    context: self.action_context(action),
                    command,
                },
                self.backend.fixture_position_ports(),
            )
            .map_err(Into::into)
    }

    fn wait_for(&self, request: MacroWaitRequest) -> Result<MacroResume, MacroError> {
        self.ensure_active()?;
        let capability = wait_capability(&request);
        self.require(capability)?;
        self.lifecycle.waiting(&request);
        let result = self.backend.wait_for(
            &self.action_context(MacroHostAction::unversioned()),
            request,
            self.cancellation.as_ref(),
        );
        self.lifecycle.running();
        result
    }

    fn trigger_playback(&self, command: PlaybackCommand) -> Result<PlaybackResult, MacroError> {
        self.ensure_active()?;
        self.require(MacroCapability::TriggerPlayback)?;
        self.playbacks
            .handle(
                ActionEnvelope {
                    context: self.action_context(MacroHostAction::unversioned()),
                    command,
                },
                self.backend.playback_ports(),
            )
            .map_err(Into::into)
    }

    fn http(&self, request: MacroHttpRequest) -> Result<MacroHttpResponse, MacroError> {
        self.require(MacroCapability::Http)?;
        super::http::execute(
            self.backend.as_ref(),
            &self.action_context(MacroHostAction::unversioned()),
            self.execution_id,
            &self.definition,
            request,
            self.cancellation.as_ref(),
        )
    }
}

fn wait_capability(request: &MacroWaitRequest) -> MacroCapability {
    match request {
        MacroWaitRequest::Timer { .. } => MacroCapability::AwaitTimer,
        MacroWaitRequest::OperatorInput { .. } => MacroCapability::AwaitOperatorInput,
        MacroWaitRequest::Event(_) => MacroCapability::AwaitEvent,
    }
}
