use super::*;
use crate::{ActionContext, ActionEnvelope, ActionErrorKind, ActionSource};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, UserId};
use light_programmer::command_line::{CommandKey, CommandKeyPhase};
use light_programmer::{HighlightRegistry, ProgrammerRegistry};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use uuid::Uuid;

#[derive(Default)]
struct TestPorts {
    executions: AtomicUsize,
    persisted: Mutex<Vec<&'static str>>,
    persistence_warning: Mutex<Option<String>>,
    selection_environment: Mutex<ProgrammingSelectionEnvironment>,
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
        if command.starts_with("COPY SET") {
            return ProgrammingExecution::ChoiceRequired {
                pending_choice: CueMoveCopyChoice {
                    choice_id: uuid::Uuid::from_u128(1),
                    show_id: uuid::Uuid::from_u128(2),
                    show_revision: 3,
                    operation: CueTransferOperation::Copy,
                    command: command.into(),
                    options: vec![ProgrammingChoiceOption {
                        id: ProgrammingChoiceOptionId::Plain,
                        label: "Plain Copy".into(),
                        command: command.replacen("COPY", "COPY PLAIN", 1),
                    }],
                    cancel_label: "Cancel".into(),
                },
            };
        }
        programmers.update_command_line(session, |current| (String::new(), current.target, true));
        ProgrammingExecution::Accepted {
            applied: 1,
            warning: None,
            replayed: false,
        }
    }

    fn persist(&self, _context: &ActionContext, operation: &'static str) -> Option<String> {
        self.persisted.lock().push(operation);
        self.persistence_warning.lock().clone()
    }

    fn selection_environment(
        &self,
        _context: &ActionContext,
        _query: &ProgrammingSelectionQuery,
    ) -> Result<ProgrammingSelectionEnvironment, crate::ActionError> {
        Ok(self.selection_environment.lock().clone())
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

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
            service: ProgrammingService::new(
                registry.clone(),
                crate::EventBus::default(),
                Arc::new(HighlightRegistry::default()),
            ),
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
        [
            "programmer.command_line",
            "programmer.clear_selection",
            "programmer.clear_values"
        ]
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
    assert_eq!(
        harness.ports.persisted.lock().as_slice(),
        ["programmer.command_line"]
    );

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
fn request_replay_retains_an_edit_persistence_warning() {
    let mut harness = Harness::new(ActionSource::Http);
    harness.context = harness.context.clone().with_request_id("failed-save");
    *harness.ports.persistence_warning.lock() = Some("programmer save failed".into());
    let command = ProgrammingCommand::ApplyKey {
        key: CommandKey::Digit(1),
        phase: CommandKeyPhase::Press,
        execute_policy: ExecutionPolicy::AtomicProgrammer,
    };

    let first = harness.handle(command.clone());
    let replay = harness.handle(command);

    for result in [first, replay] {
        assert!(matches!(
            result.outcome,
            ProgrammingOutcome::Accepted {
                action: ProgrammingAction::Edited,
                warning: Some(ref warning),
                ..
            } if warning == "programmer save failed"
        ));
    }
    assert_eq!(
        harness.ports.persisted.lock().as_slice(),
        ["programmer.command_line"]
    );
}

#[test]
fn choice_required_is_explicit_revisioned_and_replay_cannot_restore_it() {
    let harness = Harness::new(ActionSource::Http);
    let command = "COPY SET 1 CUE 1 AT SET 2 CUE 2";
    let typed = ProgrammingCommand::Execute {
        command: Some(command.into()),
        policy: ExecutionPolicy::AtomicProgrammer,
    };
    let sequence_before = harness.service.events().latest_sequence();
    let first_context = harness.context.clone().with_request_id("choice-1");
    let first = harness
        .service
        .handle(
            ActionEnvelope {
                context: first_context.clone(),
                command: typed.clone(),
            },
            &harness.ports,
        )
        .unwrap();
    assert!(matches!(
        first.outcome,
        ProgrammingOutcome::ChoiceRequired { .. }
    ));
    assert_eq!(first.command_line.visible_text(), command);
    assert_eq!(first.command_line.revision, 1);
    assert!(first.command_line.pending_choice.is_some());
    assert_eq!(first.interaction_event_sequence, Some(sequence_before + 1));

    let repeated = harness.handle(typed.clone());
    assert_eq!(repeated.command_line.revision, first.command_line.revision);
    assert!(repeated.interaction_event_sequence.is_none());
    assert_eq!(
        harness.service.events().latest_sequence(),
        sequence_before + 1
    );

    let reset = harness
        .service
        .handle(
            ActionEnvelope {
                context: harness.context.clone().with_request_id("cancel-1"),
                command: ProgrammingCommand::ReplaceCommandLine {
                    text: String::new(),
                    expected_revision: repeated.command_line.revision,
                },
            },
            &harness.ports,
        )
        .unwrap();
    assert!(reset.command_line.pending_choice.is_none());
    assert_eq!(reset.command_line.revision, first.command_line.revision + 1);
    assert_eq!(
        harness.service.events().latest_sequence(),
        sequence_before + 2
    );

    let replay = harness
        .service
        .handle(
            ActionEnvelope {
                context: first_context,
                command: typed,
            },
            &harness.ports,
        )
        .unwrap();
    assert!(replay.replayed);
    assert!(matches!(
        replay.outcome,
        ProgrammingOutcome::ChoiceRequired { .. }
    ));
    assert_eq!(replay.command_line, reset.command_line);
    assert!(replay.command_line.pending_choice.is_none());
    assert_eq!(
        harness.service.events().latest_sequence(),
        sequence_before + 2
    );
}

#[test]
fn accepted_choice_selection_clears_the_command_and_choice_atomically() {
    let harness = Harness::new(ActionSource::UserInterface);
    let pending = harness.handle(ProgrammingCommand::Execute {
        command: Some("COPY SET 1 CUE 1 AT SET 2 CUE 2".into()),
        policy: ExecutionPolicy::Compatibility,
    });
    let sequence = harness.service.events().latest_sequence();

    let accepted = harness.handle(ProgrammingCommand::Execute {
        command: Some("COPY PLAIN SET 1 CUE 1 AT SET 2 CUE 2".into()),
        policy: ExecutionPolicy::Compatibility,
    });

    assert!(matches!(
        accepted.outcome,
        ProgrammingOutcome::Accepted { .. }
    ));
    assert_eq!(accepted.command_line.visible_text(), "FIXTURE");
    assert!(accepted.command_line.pristine);
    assert!(accepted.command_line.pending_choice.is_none());
    assert_eq!(
        accepted.command_line.revision,
        pending.command_line.revision + 1
    );
    assert_eq!(accepted.interaction_event_sequence, Some(sequence + 1));
    assert_eq!(harness.service.events().latest_sequence(), sequence + 1);
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
            replayed: false,
        }
    }

    fn persist(&self, _context: &ActionContext, _operation: &'static str) -> Option<String> {
        None
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

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

#[test]
fn selection_replacement_is_revisioned_expands_heads_and_returns_authority() {
    let harness = Harness::new(ActionSource::UserInterface);
    let session = SessionId(harness.context.session_id.unwrap());
    let parent = FixtureId::new();
    let first_head = FixtureId::new();
    let second_head = FixtureId::new();
    let mut environment = harness.ports.selection_environment.lock();
    environment
        .selectable_fixtures
        .insert(parent, vec![first_head, second_head]);
    environment
        .selectable_fixtures
        .insert(first_head, vec![first_head]);
    drop(environment);
    let expected_revision = harness.registry.selection(session).unwrap().revision;

    let result = harness.handle(ProgrammingCommand::ReplaceSelection {
        fixtures: vec![parent, first_head],
        expected_revision,
    });

    assert!(matches!(
        result.outcome,
        ProgrammingOutcome::Accepted {
            action: ProgrammingAction::SelectionReplaced,
            applied: Some(2),
            ..
        }
    ));
    let selection = result.selection.unwrap();
    assert_eq!(selection.selected, vec![first_head, second_head]);
    assert_eq!(
        selection.expression,
        Some(light_programmer::SelectionExpression::Static)
    );
    assert!(!selection.gesture_open);
    assert_eq!(result.interaction_event_sequence, Some(1));
    assert_eq!(
        harness.ports.persisted.lock().as_slice(),
        ["programmer.selection.replace"]
    );
}

#[test]
fn stale_selection_replacement_does_not_mutate_persist_or_publish() {
    let harness = Harness::new(ActionSource::UserInterface);
    let session = SessionId(harness.context.session_id.unwrap());
    let fixture = FixtureId::new();
    harness
        .ports
        .selection_environment
        .lock()
        .selectable_fixtures
        .insert(fixture, vec![fixture]);
    let expected_revision = harness.registry.selection(session).unwrap().revision;
    harness.handle(ProgrammingCommand::ReplaceSelection {
        fixtures: vec![fixture],
        expected_revision,
    });

    let rejected = harness.service.handle(
        ActionEnvelope {
            context: harness.context.clone(),
            command: ProgrammingCommand::ReplaceSelection {
                fixtures: Vec::new(),
                expected_revision,
            },
        },
        &harness.ports,
    );

    let error = rejected.unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(
        error.current_revision,
        harness.registry.selection(session).map(|s| s.revision)
    );
    assert_eq!(
        harness.registry.selection(session).unwrap().selected,
        vec![fixture]
    );
    assert_eq!(
        harness.ports.persisted.lock().as_slice(),
        ["programmer.selection.replace"]
    );
    assert_eq!(harness.service.events().latest_sequence(), 2);
}

#[test]
fn selection_gestures_and_rules_preserve_ordered_group_semantics() {
    let harness = Harness::new(ActionSource::UserInterface);
    let first = FixtureId::new();
    let second = FixtureId::new();
    let third = FixtureId::new();
    let mut environment = harness.ports.selection_environment.lock();
    for fixture in [first, second, third] {
        environment
            .selectable_fixtures
            .insert(fixture, vec![fixture]);
    }
    environment.groups.insert(
        "1".into(),
        light_programmer::GroupDefinition {
            id: "1".into(),
            fixtures: vec![second, first, third],
            ..Default::default()
        },
    );
    drop(environment);

    harness.handle(ProgrammingCommand::ApplySelectionGesture {
        source: SelectionGestureSource::Fixture { fixture_id: first },
        remove: false,
    });
    let group = harness.handle(ProgrammingCommand::ApplySelectionGesture {
        source: SelectionGestureSource::LiveGroup {
            group_id: "1".into(),
        },
        remove: false,
    });
    let selected = group.selection.unwrap();
    assert_eq!(selected.selected, vec![first, second, third]);
    assert!(selected.gesture_open);
    assert!(matches!(
        selected.expression,
        Some(light_programmer::SelectionExpression::Sources { .. })
    ));

    let ruled = harness.handle(ProgrammingCommand::ApplySelectionRule {
        rule: light_programmer::SelectionRule::Even,
    });
    let selection = ruled.selection.unwrap();
    assert_eq!(selection.selected, vec![second]);
    assert_eq!(
        selection.expression,
        Some(light_programmer::SelectionExpression::Static)
    );
    assert!(!selection.gesture_open);
}

#[test]
fn live_and_frozen_group_selection_use_the_compiled_show_revision() {
    let harness = Harness::new(ActionSource::Http);
    let session = SessionId(harness.context.session_id.unwrap());
    let fixtures = vec![FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let mut environment = harness.ports.selection_environment.lock();
    environment.show_revision = 42;
    environment.groups.insert(
        "7".into(),
        light_programmer::GroupDefinition {
            id: "7".into(),
            fixtures: fixtures.clone(),
            ..Default::default()
        },
    );
    drop(environment);

    let live_revision = harness.registry.selection(session).unwrap().revision;
    let live = harness.handle(ProgrammingCommand::SelectGroup {
        group_id: "7".into(),
        frozen: false,
        rule: light_programmer::SelectionRule::Odd,
        expected_revision: live_revision,
    });
    assert_eq!(
        live.selection.as_ref().unwrap().selected,
        vec![fixtures[0], fixtures[2]]
    );
    assert!(matches!(
        live.selection.unwrap().expression,
        Some(light_programmer::SelectionExpression::LiveGroup { .. })
    ));

    let frozen_revision = harness.registry.selection(session).unwrap().revision;
    let frozen = harness.handle(ProgrammingCommand::SelectGroup {
        group_id: "7".into(),
        frozen: true,
        rule: light_programmer::SelectionRule::All,
        expected_revision: frozen_revision,
    });
    assert_eq!(frozen.selection.as_ref().unwrap().selected, fixtures);
    assert_eq!(
        frozen.selection.unwrap().expression,
        Some(light_programmer::SelectionExpression::FrozenGroup {
            group_id: "7".into(),
            source_revision: 42,
        })
    );
}
