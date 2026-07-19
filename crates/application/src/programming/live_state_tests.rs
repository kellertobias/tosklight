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

#[derive(Default)]
struct LivePorts {
    selection: Mutex<Vec<FixtureId>>,
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
            ports: LivePorts::default(),
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

    fn filter(&self) -> EventFilter {
        EventFilter::for_desk(self.context.desk_id)
            .with_object(EventObject::programming_interaction(self.context.desk_id))
    }
}

#[test]
fn handle_publishes_one_authoritative_interaction_projection_per_change() {
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

    let EventReplay::Events(events) = setup.events.replay(0, &setup.filter()) else {
        panic!("retained interaction events should be replayable")
    };
    assert_eq!(events.len(), 2);
    let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
        &events[1].payload
    else {
        panic!("expected a typed Programming interaction event")
    };
    assert_eq!(change.projection.desk_id, setup.context.desk_id);
    assert_eq!(change.projection.selection.selected, fixtures);
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
fn interaction_routes_are_exactly_desk_and_object_scoped() {
    let setup = LiveSetup::new(8);
    setup.press(CommandKey::Digit(1));

    let object = EventObject::programming_interaction(setup.context.desk_id);
    assert_eq!(object.capability, EventCapability::Desk);
    assert_eq!(
        object.id,
        format!("programming-interaction:{}", setup.context.desk_id)
    );

    let matching = setup.events.replay(0, &setup.filter());
    assert!(matches!(matching, EventReplay::Events(events) if events.len() == 1));

    let programmer_scope =
        EventFilter::for_desk(setup.context.desk_id).with_capability(EventCapability::Programmer);
    assert!(matches!(
        setup.events.replay(0, &programmer_scope),
        EventReplay::Events(events) if events.is_empty()
    ));

    let other_desk = Uuid::new_v4();
    let wrong_desk = EventFilter::for_desk(other_desk)
        .with_object(EventObject::programming_interaction(setup.context.desk_id));
    assert!(matches!(
        setup.events.replay(0, &wrong_desk),
        EventReplay::Events(events) if events.is_empty()
    ));

    let wrong_object = EventFilter::for_desk(setup.context.desk_id)
        .with_object(EventObject::programming_interaction(other_desk));
    assert!(matches!(
        setup.events.replay(0, &wrong_object),
        EventReplay::Events(events) if events.is_empty()
    ));
}

#[test]
fn snapshot_cursor_repairs_a_gap_without_missing_the_next_change() {
    let setup = LiveSetup::new(1);
    setup.press(CommandKey::Digit(1));
    setup.press(CommandKey::Digit(2));
    let subscription = setup.events.subscribe(
        setup.filter(),
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
        let projection = ProgrammingInteractionProjection {
            desk_id: self.context.desk_id,
            command_line: Default::default(),
            selection: ProgrammerSelection::default(),
        };
        ProgrammingOperation::with_events(
            "committed",
            vec![crate::EventDraft::programming_interaction_changed(
                &self.context,
                projection,
            )],
        )
    }
}
