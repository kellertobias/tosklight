use super::*;
use crate::{ActionContext, ActionEnvelope, ActionErrorKind, ActionSource};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, UserId};
use light_programmer::ProgrammerRegistry;
use light_programmer::command_line::{CommandKey, CommandKeyPhase};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use uuid::Uuid;

#[derive(Default)]
struct TestPorts {
    executions: AtomicUsize,
    persisted: Mutex<Vec<&'static str>>,
}

impl ProgrammingPorts for TestPorts {
    fn execute(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        self.executions.fetch_add(1, Ordering::Relaxed);
        let Some(session) = context.session_id.map(SessionId) else {
            return ProgrammingExecution::Rejected {
                error: "missing session".into(),
            };
        };
        if command == "REJECT" {
            return ProgrammingExecution::Rejected {
                error: "rejected".into(),
            };
        }
        programmers.update_command_line(session, |current| (String::new(), current.target, true));
        ProgrammingExecution::Accepted {
            applied: 1,
            warning: None,
        }
    }

    fn persist(&self, _context: &ActionContext, operation: &'static str) -> Option<String> {
        self.persisted.lock().push(operation);
        None
    }

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}

struct Harness {
    service: ProgrammingService,
    registry: ProgrammerRegistry,
    context: ActionContext,
    ports: TestPorts,
}

impl Harness {
    fn new(source: ActionSource) -> Self {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let user = UserId::new();
        let desk = Uuid::new_v4();
        registry.start(session, user);
        assert!(registry.attach_command_context(session, SessionId(desk)));
        Self {
            service: ProgrammingService::new(registry.clone()),
            registry,
            context: ActionContext::operator(desk, user.0, session.0, source),
            ports: TestPorts::default(),
        }
    }

    fn handle(&self, command: ProgrammingCommand) -> ProgrammingResult {
        self.service
            .handle(
                ActionEnvelope {
                    context: self.context.clone(),
                    command,
                },
                &self.ports,
            )
            .expect("programming command succeeds")
    }

    fn press(&self, key: CommandKey) -> ProgrammingResult {
        self.handle(ProgrammingCommand::ApplyKey {
            key,
            phase: CommandKeyPhase::Press,
            execute_policy: ExecutionPolicy::AtomicProgrammer,
        })
    }
}

#[test]
fn identical_surface_key_traces_share_one_editor() {
    let sources = [
        ActionSource::UserInterface,
        ActionSource::Keyboard,
        ActionSource::Osc,
        ActionSource::Http,
    ];
    let lines = sources.map(|source| {
        let harness = Harness::new(source);
        for key in [
            CommandKey::Digit(1),
            CommandKey::Plus,
            CommandKey::Group,
            CommandKey::Digit(2),
        ] {
            harness.press(key);
        }
        harness
            .registry
            .command_line_state(SessionId(harness.context.session_id.unwrap()))
            .unwrap()
            .visible_text()
            .to_owned()
    });
    assert!(lines.iter().all(|line| line == &lines[0]));
    assert_eq!(lines[0], "F1 + G2");
}

#[test]
fn clear_is_staged_and_resets_the_command_line_after_selection() {
    let harness = Harness::new(ActionSource::Http);
    let session = SessionId(harness.context.session_id.unwrap());
    let fixture = FixtureId::new();
    harness.registry.select(session, [fixture]);
    harness.registry.set(
        session,
        fixture,
        AttributeKey("intensity".into()),
        AttributeValue::Normalized(0.5),
    );
    harness.press(CommandKey::Digit(1));

    let first = harness.handle(ProgrammingCommand::ClearStep);
    assert!(matches!(
        first.outcome,
        ProgrammingOutcome::Accepted {
            action: ProgrammingAction::ClearedSelection,
            ..
        }
    ));
    assert!(first.command_line.pristine);
    assert!(
        harness
            .registry
            .selection(session)
            .unwrap()
            .selected
            .is_empty()
    );

    let second = harness.handle(ProgrammingCommand::ClearStep);
    assert!(matches!(
        second.outcome,
        ProgrammingOutcome::Accepted {
            action: ProgrammingAction::ClearedValues,
            ..
        }
    ));
    assert!(harness.registry.get(session).unwrap().values.is_empty());

    let third = harness.handle(ProgrammingCommand::ClearStep);
    assert!(matches!(
        third.outcome,
        ProgrammingOutcome::Accepted {
            action: ProgrammingAction::NoChange,
            ..
        }
    ));
    assert_eq!(
        harness.ports.persisted.lock().as_slice(),
        ["programmer.clear_selection", "programmer.clear_values"]
    );
}

#[test]
fn request_replay_is_exactly_once_and_retains_original_context() {
    let mut harness = Harness::new(ActionSource::Http);
    harness.context = harness.context.clone().with_request_id("request-1");
    let command = ProgrammingCommand::ApplyKey {
        key: CommandKey::Digit(1),
        phase: CommandKeyPhase::Press,
        execute_policy: ExecutionPolicy::AtomicProgrammer,
    };
    let first = harness.handle(command.clone());
    let replay = harness.handle(command);
    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(first.context, replay.context);
    assert_eq!(first.command_line.revision, replay.command_line.revision);

    let conflict = harness.service.handle(
        ActionEnvelope {
            context: harness.context.clone(),
            command: ProgrammingCommand::ApplyKey {
                key: CommandKey::Digit(2),
                phase: CommandKeyPhase::Press,
                execute_policy: ExecutionPolicy::AtomicProgrammer,
            },
        },
        &harness.ports,
    );
    assert_eq!(conflict.unwrap_err().kind, ActionErrorKind::Conflict);
}

#[test]
fn desk_ordering_uses_one_lock_per_live_desk() {
    let harness = Harness::new(ActionSource::Osc);
    let same = harness.service.desk_lock(harness.context.desk_id);
    let same_again = harness.service.desk_lock(harness.context.desk_id);
    let other = harness.service.desk_lock(Uuid::new_v4());
    assert!(std::sync::Arc::ptr_eq(&same, &same_again));
    assert!(!std::sync::Arc::ptr_eq(&same, &other));
}

struct OrderingPorts {
    commands: Mutex<Vec<String>>,
    first_started: mpsc::Sender<()>,
    release_first: Mutex<mpsc::Receiver<()>>,
}

impl ProgrammingPorts for OrderingPorts {
    fn execute(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        self.commands.lock().push(command.to_owned());
        if command == "FIRST" {
            self.first_started.send(()).unwrap();
            self.release_first.lock().recv().unwrap();
        }
        if let Some(session) = context.session_id.map(SessionId) {
            programmers
                .update_command_line(session, |current| (String::new(), current.target, true));
        }
        ProgrammingExecution::Accepted {
            applied: 0,
            warning: None,
        }
    }

    fn persist(&self, _context: &ActionContext, _operation: &'static str) -> Option<String> {
        None
    }

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}

#[test]
fn same_desk_requests_execute_in_arrival_order() {
    let harness = Harness::new(ActionSource::Http);
    let service = harness.service.clone();
    let context = harness.context.clone();
    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let ports = Arc::new(OrderingPorts {
        commands: Mutex::new(Vec::new()),
        first_started: first_started_tx,
        release_first: Mutex::new(release_rx),
    });

    let first_service = service.clone();
    let first_context = context.clone();
    let first_ports = Arc::clone(&ports);
    let first = thread::spawn(move || {
        first_service.handle(
            ActionEnvelope {
                context: first_context,
                command: ProgrammingCommand::Execute {
                    command: Some("FIRST".into()),
                    policy: ExecutionPolicy::AtomicProgrammer,
                },
            },
            first_ports.as_ref(),
        )
    });
    first_started_rx.recv().unwrap();

    let (second_attempt_tx, second_attempt_rx) = mpsc::channel();
    let second_ports = Arc::clone(&ports);
    let second = thread::spawn(move || {
        second_attempt_tx.send(()).unwrap();
        service.handle(
            ActionEnvelope {
                context,
                command: ProgrammingCommand::Execute {
                    command: Some("SECOND".into()),
                    policy: ExecutionPolicy::AtomicProgrammer,
                },
            },
            second_ports.as_ref(),
        )
    });
    second_attempt_rx.recv().unwrap();
    assert_eq!(ports.commands.lock().as_slice(), ["FIRST"]);

    release_tx.send(()).unwrap();
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
    assert_eq!(ports.commands.lock().as_slice(), ["FIRST", "SECOND"]);
}

#[test]
fn rejected_supplied_execution_retains_the_command() {
    let harness = Harness::new(ActionSource::Http);
    let result = harness.handle(ProgrammingCommand::Execute {
        command: Some("REJECT".into()),
        policy: ExecutionPolicy::AtomicProgrammer,
    });
    assert!(matches!(
        result.outcome,
        ProgrammingOutcome::Rejected { .. }
    ));
    assert_eq!(result.command_line.visible_text(), "REJECT");
}
