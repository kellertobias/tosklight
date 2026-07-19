use std::{sync::Arc, time::Duration};

use uuid::Uuid;

use super::super::*;
use super::support::{FakeBackend, QueuedRunner, http_request, playback_command, request, service};
use crate::{ActionContext, ActionErrorKind, ActionSource, FixturePositionCommand, StagePosition};

struct HappyRuntime {
    forged_context: ActionContext,
}

impl MacroRuntime for HappyRuntime {
    fn invoke(
        &self,
        definition: &MacroDefinition,
        invocation: &MacroInvocation,
        host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        assert_eq!(definition, host.definition());
        assert_eq!(definition.id, invocation.id);
        assert_eq!(definition.revision, invocation.revision);
        assert_eq!(self.forged_context.desk_id, Uuid::from_u128(99));
        let fixture = host
            .fixture("fixture-1")?
            .ok_or_else(|| MacroError::action(ActionErrorKind::NotFound, "fixture"))?;
        let group = host
            .group("group-1")?
            .ok_or_else(|| MacroError::action(ActionErrorKind::NotFound, "group"))?;
        assert!(group.fixture_ids.contains(&fixture.id));
        host.change_fixture_position(
            FixturePositionCommand {
                fixture_id: fixture.id,
                position: StagePosition {
                    x_mm: 1_000,
                    ..fixture.position
                },
            },
            MacroHostAction::at_revision(fixture.revision),
        )?;
        assert!(matches!(
            host.wait_for(MacroWaitRequest::OperatorInput {
                prompt: "Continue?".into(),
                kind: OperatorInputKind::Confirmation,
            })?,
            MacroResume::OperatorInput(OperatorInputValue::Confirmed(true))
        ));
        assert!(matches!(
            host.wait_for(MacroWaitRequest::Event(MacroEventFilter::ShowRevision {
                show_id: Uuid::from_u128(44),
                after_revision: 7,
            }))?,
            MacroResume::Event(MacroObservedEvent::ShowRevision { revision: 8, .. })
        ));
        assert_eq!(
            host.wait_for(MacroWaitRequest::Timer {
                delay: Duration::from_millis(25),
            })?,
            MacroResume::TimerElapsed
        );
        host.trigger_playback(playback_command())?;
        assert_eq!(host.http(http_request())?.status, 202);
        Ok(MacroExecutionOutcome {
            completed: true,
            host_actions: 8,
        })
    }
}

struct StalePositionRuntime;

impl MacroRuntime for StalePositionRuntime {
    fn invoke(
        &self,
        _definition: &MacroDefinition,
        _invocation: &MacroInvocation,
        host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        host.change_fixture_position(
            FixturePositionCommand {
                fixture_id: "fixture-1".into(),
                position: StagePosition {
                    x_mm: 9,
                    y_mm: 9,
                    z_mm: 9,
                },
            },
            MacroHostAction::at_revision(1),
        )?;
        unreachable!()
    }
}

fn forged_context() -> ActionContext {
    let mut context = ActionContext::system(Uuid::from_u128(99), ActionSource::System)
        .with_request_id("forged")
        .with_expected_revision(123);
    context.correlation_id = Uuid::from_u128(100);
    context
}

#[test]
fn runtime_uses_shared_services_and_cannot_forge_action_context() {
    let backend = Arc::new(FakeBackend::default());
    backend.queue_resumes([
        MacroResume::OperatorInput(OperatorInputValue::Confirmed(true)),
        MacroResume::Event(MacroObservedEvent::ShowRevision {
            show_id: Uuid::from_u128(44),
            revision: 8,
        }),
        MacroResume::TimerElapsed,
    ]);
    let runner = Arc::new(QueuedRunner::default());
    let service = service(
        Arc::new(HappyRuntime {
            forged_context: forged_context(),
        }),
        backend.clone(),
        runner.clone(),
    );

    let started = service.start(request()).unwrap();
    assert_eq!(started.phase, MacroExecutionPhase::Queued);
    assert_eq!(runner.len(), 1);
    assert_eq!(backend.fixture().revision, 7);
    runner.run_next();

    let finished = service.execution(started.execution_id).unwrap();
    assert!(matches!(
        finished.phase,
        MacroExecutionPhase::Completed(MacroExecutionOutcome {
            host_actions: 8,
            ..
        })
    ));
    assert_eq!(backend.fixture().position.x_mm, 1_000);
    assert_eq!(backend.fixture().revision, 8);
    assert_eq!(backend.playbacks(), [playback_command()]);
    assert_eq!(backend.waits().len(), 3);

    let contexts = backend.contexts();
    assert!(!contexts.is_empty());
    for (_, context) in contexts {
        assert_eq!(context.desk_id, Uuid::from_u128(1));
        assert_eq!(context.user_id, Some(Uuid::from_u128(2)));
        assert_eq!(context.session_id, Some(Uuid::from_u128(3)));
        assert_eq!(context.source, ActionSource::Macro);
        assert_eq!(context.correlation_id, Uuid::from_u128(4));
        assert_ne!(context.request_id.as_deref(), Some("forged"));
    }
}

#[test]
fn action_error_kind_revision_and_retryability_cross_the_macro_boundary() {
    let backend = Arc::new(FakeBackend::default());
    let runner = Arc::new(QueuedRunner::default());
    let service = service(Arc::new(StalePositionRuntime), backend, runner.clone());
    let execution = service.start(request()).unwrap();
    runner.run_next();

    let MacroExecutionPhase::Failed(error) =
        service.execution(execution.execution_id).unwrap().phase
    else {
        panic!("expected a failed Macro execution");
    };
    assert_eq!(
        error.kind,
        MacroErrorKind::Action(ActionErrorKind::Conflict)
    );
    assert_eq!(error.current_revision, Some(7));
    assert!(!error.retryable);
}

#[test]
fn action_error_conversion_preserves_retryability_exactly() {
    let action = crate::ActionError::new(ActionErrorKind::Unavailable, "busy").at_revision(12);
    let error = MacroError::from(action);
    assert_eq!(
        error.kind,
        MacroErrorKind::Action(ActionErrorKind::Unavailable)
    );
    assert_eq!(error.current_revision, Some(12));
    assert!(error.retryable);
}
