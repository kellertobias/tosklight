use super::*;
use light_core::SessionId;
use light_programmer::{HighlightMode, ProgrammerSelection};
use parking_lot::Mutex;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
struct RecordingPorts {
    calls: Mutex<Vec<&'static str>>,
    environment: Mutex<Option<HighlightEnvironment>>,
}

impl RecordingPorts {
    fn with_selection(fixtures: Vec<FixtureId>) -> Self {
        Self {
            environment: Mutex::new(Some(HighlightEnvironment {
                user_name: Some("Operator".into()),
                selection: ProgrammerSelection {
                    selected: fixtures,
                    expression: None,
                    revision: 1,
                    gesture_open: false,
                },
                fixtures: Vec::new(),
                groups: HashMap::new(),
                output_suppressed: false,
            })),
            ..Self::default()
        }
    }
}

impl HighlightPorts for RecordingPorts {
    fn environment(&self, _context: &ActionContext) -> Result<HighlightEnvironment, ActionError> {
        self.calls.lock().push("environment");
        self.environment
            .lock()
            .clone()
            .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "programmer"))
    }

    fn apply_selection(
        &self,
        _context: &ActionContext,
        write: &HighlightSelectionWrite,
    ) -> Result<ProgrammerSelection, ActionError> {
        self.calls.lock().push("selection");
        Ok(ProgrammerSelection {
            selected: write.selected.clone(),
            expression: write.expression.clone(),
            revision: 2,
            gesture_open: false,
        })
    }

    fn synchronize_output(
        &self,
        _context: &ActionContext,
        _fixtures: &[FixtureId],
    ) -> Result<(), ActionError> {
        self.calls.lock().push("output");
        Ok(())
    }

    fn publish_programmer_changed(&self, _context: &ActionContext, _command: &HighlightCommand) {
        self.calls.lock().push("programmer_event");
    }

    fn publish_highlight_changed(
        &self,
        _context: &ActionContext,
        _command: &HighlightCommand,
        _state: &HighlightState,
    ) {
        self.calls.lock().push("highlight_event");
    }

    fn publish_feedback(&self, _context: &ActionContext) {
        self.calls.lock().push("feedback");
    }
}

fn context(source: crate::ActionSource) -> ActionContext {
    ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        SessionId(Uuid::from_u128(3)).0,
        source,
    )
}

#[test]
fn http_action_orders_output_event_and_feedback_through_one_service() {
    let fixture = FixtureId(Uuid::from_u128(4));
    let ports = RecordingPorts::with_selection(vec![fixture]);
    ports.environment.lock().as_mut().unwrap().fixtures = vec![HighlightFixture {
        fixture_id: fixture,
        name: Some("Fixture".into()),
        number: Some(1),
    }];
    let service = HighlightService::new(Arc::new(HighlightRegistry::default()));

    let result = service
        .handle(
            ActionEnvelope {
                context: context(crate::ActionSource::Http),
                command: HighlightCommand::action(HighlightAction::On),
            },
            &ports,
        )
        .unwrap();

    assert!(result.state.active);
    assert_eq!(result.state.mode, HighlightMode::Selection);
    assert_eq!(
        *ports.calls.lock(),
        ["environment", "output", "highlight_event", "feedback"]
    );
}

#[test]
fn stepping_persists_selection_before_output_and_publication() {
    let first = FixtureId(Uuid::from_u128(4));
    let second = FixtureId(Uuid::from_u128(5));
    let ports = RecordingPorts::with_selection(vec![first, second]);
    ports.environment.lock().as_mut().unwrap().fixtures = vec![
        HighlightFixture {
            fixture_id: first,
            name: Some("First".into()),
            number: Some(1),
        },
        HighlightFixture {
            fixture_id: second,
            name: Some("Second".into()),
            number: Some(2),
        },
    ];
    let service = HighlightService::new(Arc::new(HighlightRegistry::default()));
    service
        .handle(
            ActionEnvelope {
                context: context(crate::ActionSource::Osc),
                command: HighlightCommand::action(HighlightAction::On),
            },
            &ports,
        )
        .unwrap();
    ports.calls.lock().clear();

    let result = service
        .handle(
            ActionEnvelope {
                context: context(crate::ActionSource::Osc),
                command: HighlightCommand::action(HighlightAction::Next),
            },
            &ports,
        )
        .unwrap();

    assert!(result.selection_changed);
    assert_eq!(
        *ports.calls.lock(),
        [
            "environment",
            "selection",
            "output",
            "programmer_event",
            "highlight_event",
            "feedback"
        ]
    );
}

#[test]
fn a_second_surface_joins_the_desks_highlight_rather_than_being_refused() {
    let fixture = FixtureId(Uuid::from_u128(4));
    let ports = RecordingPorts::with_selection(vec![fixture]);
    ports.environment.lock().as_mut().unwrap().fixtures = vec![HighlightFixture {
        fixture_id: fixture,
        name: None,
        number: None,
    }];
    let service = HighlightService::new(Arc::new(HighlightRegistry::default()));
    service
        .handle(
            ActionEnvelope {
                context: context(crate::ActionSource::Http),
                command: HighlightCommand::action(HighlightAction::On),
            },
            &ports,
        )
        .unwrap();

    // A second surface — even one arriving under an identity from before the collapse — is the
    // same operator at the same desk. Highlight is already on; asking again is not a conflict.
    ports.calls.lock().clear();
    let mut second_surface = context(crate::ActionSource::Http);
    second_surface.user_id = Some(Uuid::from_u128(20));
    let state = service
        .handle(
            ActionEnvelope {
                context: second_surface,
                command: HighlightCommand::action(HighlightAction::On),
            },
            &ports,
        )
        .expect("every surface shares the desk's Highlight");

    assert!(state.state.active);
    assert!(state.state.output_enabled);
}
