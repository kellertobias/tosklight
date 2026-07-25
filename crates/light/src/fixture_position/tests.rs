use std::sync::Mutex;

use uuid::Uuid;

use super::*;
use crate::ActionSource;

#[derive(Default)]
struct TestPorts {
    contexts: Mutex<Vec<ActionContext>>,
}

impl FixturePositionPorts for TestPorts {
    fn authorize(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn fixture(
        &self,
        _context: &ActionContext,
        fixture_id: &str,
    ) -> Result<Option<FixtureProjection>, ActionError> {
        Ok(Some(FixtureProjection {
            id: fixture_id.into(),
            name: "Key".into(),
            position: StagePosition {
                x_mm: 0,
                y_mm: 0,
                z_mm: 3_000,
            },
            revision: 7,
        }))
    }

    fn set_position(
        &self,
        context: &ActionContext,
        command: &FixturePositionCommand,
        expected_revision: u64,
    ) -> Result<FixturePositionExecution, ActionError> {
        self.contexts.lock().unwrap().push(context.clone());
        assert_eq!(expected_revision, 7);
        Ok(FixturePositionExecution {
            outcome: FixturePositionOutcome {
                fixture_id: command.fixture_id.clone(),
                position: command.position,
            },
            revision: 8,
            event_sequence: Some(11),
        })
    }
}

#[test]
fn shared_service_requires_and_returns_authoritative_revisions() {
    let ports = TestPorts::default();
    let context = ActionContext::system(Uuid::from_u128(1), ActionSource::UserInterface)
        .with_expected_revision(7);
    let outcome = FixturePositionService::default()
        .handle(
            ActionEnvelope {
                context: context.clone(),
                command: FixturePositionCommand {
                    fixture_id: "fixture-1".into(),
                    position: StagePosition {
                        x_mm: 1_000,
                        y_mm: 0,
                        z_mm: 3_000,
                    },
                },
            },
            &ports,
        )
        .unwrap();

    assert_eq!(ports.contexts.lock().unwrap().as_slice(), [context]);
    assert_eq!(outcome.revision, Some(8));
    assert_eq!(outcome.event_sequence, Some(11));
}

#[test]
fn missing_precondition_never_reaches_the_mutation_port() {
    let ports = TestPorts::default();
    let error = FixturePositionService::default()
        .handle(
            ActionEnvelope {
                context: ActionContext::system(Uuid::from_u128(1), ActionSource::Http),
                command: FixturePositionCommand {
                    fixture_id: "fixture-1".into(),
                    position: StagePosition {
                        x_mm: 0,
                        y_mm: 0,
                        z_mm: 0,
                    },
                },
            },
            &ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert!(ports.contexts.lock().unwrap().is_empty());
}
