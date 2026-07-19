use super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionSource, ApplicationEvent, EventBus, EventCapability,
    EventFilter, EventObject, EventReplay, ProgrammingEvent, SubscriptionDelivery,
    SubscriptionOptions,
};
use light_core::{FixtureId, SessionId, UserId};
use light_programmer::command_line::{CommandKey, CommandKeyPhase};
use light_programmer::{HighlightRegistry, ProgrammerRegistry, ProgrammerSelection};
use parking_lot::Mutex;
use std::sync::{Arc, mpsc};
use std::thread;
use uuid::Uuid;

#[path = "live_state_tests/routing.rs"]
mod routing;

#[derive(Default)]
struct LivePorts {
    selection: Mutex<Vec<FixtureId>>,
    reconciled_selection: Mutex<Option<Vec<FixtureId>>>,
    reconciliations: Mutex<Vec<ProgrammingReconciliation>>,
    registry: Option<ProgrammerRegistry>,
}

impl ProgrammingPorts for LivePorts {
    fn execute(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        if command == "SELECT"
            && let Some(session) = context.session_id.map(SessionId)
        {
            programmers.select(session, self.selection.lock().iter().copied());
        }
        ProgrammingExecution::Accepted {
            applied: 1,
            warning: None,
        }
    }

    fn persist(&self, _context: &ActionContext, _operation: &'static str) -> Option<String> {
        None
    }

    fn reconcile(&self, context: &ActionContext, reason: ProgrammingReconciliation) {
        self.reconciliations.lock().push(reason);
        let Some(selection) = self.reconciled_selection.lock().clone() else {
            return;
        };
        let (Some(registry), Some(session)) =
            (self.registry.as_ref(), context.session_id.map(SessionId))
        else {
            return;
        };
        registry.select(session, selection);
    }

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}

struct LiveSetup {
    events: EventBus,
    highlight: Arc<HighlightRegistry>,
    service: ProgrammingService,
    context: ActionContext,
    ports: LivePorts,
}

impl LiveSetup {
    fn new(retention: usize) -> Self {
        let registry = ProgrammerRegistry::default();
        let desk = Uuid::new_v4();
        let session = SessionId::new();
        let user = UserId::new();
        registry.start(session, user);
        assert!(registry.attach_command_context(session, SessionId(desk)));
        let events = EventBus::new(retention);
        let highlight = Arc::new(HighlightRegistry::default());
        let service =
            ProgrammingService::new(registry.clone(), events.clone(), Arc::clone(&highlight));
        Self {
            events,
            highlight,
            service,
            context: ActionContext::operator(desk, user.0, session.0, ActionSource::UserInterface),
            ports: LivePorts {
                registry: Some(registry),
                ..LivePorts::default()
            },
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
            .unwrap()
    }

    fn press(&self, key: CommandKey) -> ProgrammingResult {
        self.handle(ProgrammingCommand::ApplyKey {
            key,
            phase: CommandKeyPhase::Press,
            execute_policy: ExecutionPolicy::AtomicProgrammer,
        })
    }

    fn command_filter(&self) -> EventFilter {
        EventFilter::for_desk(self.context.desk_id)
            .with_object(EventObject::programming_command_line(self.context.desk_id))
    }

    fn selection_filter(&self) -> EventFilter {
        EventFilter::for_desk(self.context.desk_id)
            .with_object(EventObject::programming_selection(self.context.desk_id))
    }
}

#[test]
fn handle_publishes_one_sparse_authoritative_change_per_interaction() {
    let setup = LiveSetup::new(8);
    let first = setup.press(CommandKey::Digit(1));
    assert_eq!(first.interaction_event_sequence, Some(1));

    let fixtures = [FixtureId::new(), FixtureId::new()];
    setup.ports.selection.lock().extend(fixtures);
    let selected = setup.handle(ProgrammingCommand::Execute {
        command: Some("SELECT".into()),
        policy: ExecutionPolicy::AtomicProgrammer,
    });
    assert_eq!(selected.interaction_event_sequence, Some(2));

    let EventReplay::Events(events) = setup
        .events
        .replay(0, &EventFilter::for_desk(setup.context.desk_id))
    else {
        panic!("retained interaction events should be replayable")
    };
    assert_eq!(events.len(), 2);
    let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(command_change)) =
        &events[0].payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    assert!(command_change.command_line().is_some());
    assert!(command_change.selection().is_none());
    let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
        &events[1].payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    assert_eq!(change.desk_id(), setup.context.desk_id);
    assert_eq!(change.selection().unwrap().selected, fixtures);
    let EventReplay::Events(selection_events) = setup.events.replay(0, &setup.selection_filter())
    else {
        panic!("selection changes should be independently routable")
    };
    assert_eq!(selection_events.len(), 1);
    let EventReplay::Events(command_events) = setup.events.replay(0, &setup.command_filter())
    else {
        panic!("command-line changes should be independently routable")
    };
    assert_eq!(command_events.len(), 1);
    assert_eq!(
        events[1].correlation_id,
        Some(selected.context.correlation_id)
    );
}

#[test]
fn unchanged_command_does_not_publish_and_replay_keeps_the_original_cursor() {
    let mut setup = LiveSetup::new(8);
    setup.context = setup.context.clone().with_request_id("release-1");
    let command = ProgrammingCommand::ApplyKey {
        key: CommandKey::Digit(1),
        phase: CommandKeyPhase::Release,
        execute_policy: ExecutionPolicy::AtomicProgrammer,
    };
    let first = setup.handle(command.clone());
    let replay = setup.handle(command);

    assert_eq!(first.interaction_event_sequence, None);
    assert_eq!(replay.interaction_event_sequence, None);
    assert!(replay.replayed);
    assert_eq!(setup.events.latest_sequence(), 0);
}

#[test]
fn snapshot_cursor_repairs_a_gap_without_missing_the_next_change() {
    let setup = LiveSetup::new(1);
    setup.press(CommandKey::Digit(1));
    setup.press(CommandKey::Digit(2));
    let subscription = setup.events.subscribe(
        setup.command_filter(),
        SubscriptionOptions {
            after_sequence: Some(0),
            ..SubscriptionOptions::default()
        },
    );
    assert!(matches!(
        subscription.try_next(),
        Some(SubscriptionDelivery::Gap(_))
    ));

    let snapshot = setup
        .service
        .snapshot(&setup.context, &setup.ports)
        .unwrap();
    assert_eq!(snapshot.event_sequence, 2);
    assert_eq!(snapshot.interaction.command_line.visible_text(), "F12");
    subscription
        .repair_from_snapshot(snapshot.event_sequence)
        .unwrap();

    let result = setup.press(CommandKey::Plus);
    assert_eq!(result.interaction_event_sequence, Some(3));
    assert!(matches!(
        subscription.try_next(),
        Some(SubscriptionDelivery::Event(event)) if event.sequence == 3
    ));
}

#[test]
fn final_change_is_captured_after_selection_reconciliation() {
    let setup = LiveSetup::new(8);
    let initial = [FixtureId::new(), FixtureId::new()];
    let reconciled = vec![initial[1]];
    setup.ports.selection.lock().extend(initial);
    *setup.ports.reconciled_selection.lock() = Some(reconciled.clone());

    let result = setup.handle(ProgrammingCommand::Execute {
        command: Some("SELECT".into()),
        policy: ExecutionPolicy::AtomicProgrammer,
    });
    let EventReplay::Events(events) = setup.events.replay(0, &setup.selection_filter()) else {
        panic!("the final selection change should be replayable")
    };
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
        &events[0].payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    assert_eq!(change.selection().unwrap().selected, reconciled);
    assert_eq!(
        result.selection_revision,
        change.selection().unwrap().revision
    );
    assert_eq!(
        *setup.ports.reconciliations.lock(),
        vec![ProgrammingReconciliation::SelectionChanged]
    );
}

#[test]
fn preload_capture_reconciliation_is_included_in_the_same_final_event() {
    let setup = LiveSetup::new(8);
    let fixture = FixtureId::new();
    *setup.ports.reconciled_selection.lock() = Some(vec![fixture]);

    let result = setup.handle(ProgrammingCommand::Preload {
        capture_programmer: false,
    });
    let EventReplay::Events(events) = setup.events.replay(0, &setup.selection_filter()) else {
        panic!("the reconciled PRELOAD selection should be replayable")
    };
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
        &events[0].payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    assert_eq!(change.selection().unwrap().selected, vec![fixture]);
    assert_eq!(result.interaction_event_sequence, Some(events[0].sequence));
    assert_eq!(
        *setup.ports.reconciliations.lock(),
        vec![ProgrammingReconciliation::CaptureModeChanged]
    );
}

#[test]
fn unit_of_work_and_handle_share_the_private_desk_gate() {
    let setup = LiveSetup::new(8);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let service = setup.service.clone();
    let operation_context = setup.context.clone();
    let operation = thread::spawn(move || {
        service.run_unit_of_work(BlockingOperation {
            context: operation_context,
            entered: entered_tx,
            release: release_rx,
        })
    });
    entered_rx.recv().unwrap();

    let worker_service = setup.service.clone();
    let worker_context = setup.context.clone();
    let worker = thread::spawn(move || {
        worker_service.handle(
            ActionEnvelope {
                context: worker_context,
                command: ProgrammingCommand::ApplyKey {
                    key: CommandKey::Digit(1),
                    phase: CommandKeyPhase::Press,
                    execute_policy: ExecutionPolicy::AtomicProgrammer,
                },
            },
            &LivePorts::default(),
        )
    });
    assert_eq!(setup.events.latest_sequence(), 0);
    release_tx.send(()).unwrap();

    let operation = operation.join().unwrap();
    let result = worker.join().unwrap().unwrap();
    assert_eq!(operation.event_sequences, vec![1]);
    assert_eq!(result.interaction_event_sequence, Some(2));
}

#[test]
fn service_shares_the_injected_highlight_registry() {
    let setup = LiveSetup::new(8);
    assert!(Arc::ptr_eq(
        setup.service.highlight_registry(),
        &setup.highlight
    ));
}

struct BlockingOperation {
    context: ActionContext,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

impl ProgrammingUnitOfWork for BlockingOperation {
    type Output = &'static str;

    fn desk_id(&self) -> Uuid {
        self.context.desk_id
    }

    fn execute(self) -> ProgrammingOperation<Self::Output> {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        let change = ProgrammingInteractionChange::from_components(
            self.context.desk_id,
            Some(Default::default()),
            None,
        )
        .unwrap();
        ProgrammingOperation::with_events(
            "committed",
            vec![crate::EventDraft::programming_interaction_changed(
                &self.context,
                change,
            )],
        )
    }
}
