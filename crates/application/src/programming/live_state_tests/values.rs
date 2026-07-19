use super::*;
use crate::{DeliveryPolicy, EventClass, EventSource};
use light_core::{AttributeKey, AttributeValue};
use std::time::Duration;

use super::super::values_projection::{projection_read_count, reset_projection_read_count};

#[test]
fn command_line_only_actions_do_not_materialize_the_values_projection() {
    let setup = LiveSetup::new(8);
    reset_projection_read_count();

    let result = setup.press(CommandKey::Digit(1));

    assert_eq!(result.values_event_sequence, None);
    assert_eq!(projection_read_count(), 0);
}

#[test]
fn one_external_action_publishes_one_full_deterministic_values_projection() {
    let setup = LiveSetup::new(8);
    let registry = setup.ports.registry.as_ref().unwrap();
    let session = SessionId(setup.context.session_id.unwrap());
    let user_id = UserId(setup.context.user_id.unwrap());
    let fixture_a = FixtureId::new();
    let fixture_b = FixtureId::new();
    reset_projection_read_count();

    let completed = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            registry.set_faded_with_timing(
                session,
                fixture_b,
                AttributeKey("tilt".into()),
                AttributeValue::Normalized(0.25),
                Some(1_000),
                Some(250),
            );
            registry.set(
                session,
                fixture_a,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.5),
            );
            registry.set_group(
                session,
                "front".into(),
                AttributeKey("pan".into()),
                AttributeValue::Spread(vec![0.1, 0.9]),
            );
        })
        .unwrap();

    assert_eq!(completed.event_sequence, None);
    assert_eq!(completed.values_event_sequence, Some(1));
    assert_eq!(projection_read_count(), 1);
    assert_eq!(registry.normal_values_revision(user_id), 1);
    let filter = EventFilter::for_desk(setup.context.desk_id)
        .with_object(EventObject::programming_values(user_id.0));
    let EventReplay::Events(events) = setup.events.replay(0, &filter) else {
        panic!("normal Programmer values should be replayable")
    };
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.desk_id, None);
    assert_eq!(event.class, EventClass::Projection);
    assert_eq!(event.delivery, DeliveryPolicy::Replaceable);
    assert_eq!(
        event.source,
        EventSource::Action(ActionSource::UserInterface)
    );
    let ApplicationEvent::Programming(ProgrammingEvent::ValuesChanged(change)) = &event.payload
    else {
        panic!("expected a typed Programmer values change")
    };
    let projection = &change.projection;
    assert_eq!(projection.user_id, user_id);
    assert_eq!(projection.revision, 1);
    assert_eq!(projection.fixture_values.len(), 2);
    assert!(
        projection.fixture_values[0].programmer_order
            < projection.fixture_values[1].programmer_order
    );
    assert_eq!(projection.fixture_values[0].fixture_id, fixture_b);
    assert!(projection.fixture_values[0].fade);
    assert_eq!(projection.fixture_values[0].fade_millis, Some(1_000));
    assert_eq!(projection.fixture_values[0].delay_millis, Some(250));
    assert_eq!(projection.group_values[0].group_id, "front");
}

#[test]
fn no_op_does_not_advance_revision_or_publish_values() {
    let setup = LiveSetup::new(8);
    let user_id = UserId(setup.context.user_id.unwrap());
    reset_projection_read_count();

    let completed = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || ())
        .unwrap();

    assert_eq!(completed.values_event_sequence, None);
    assert_eq!(setup.events.latest_sequence(), 0);
    assert_eq!(setup.service.programmers.normal_values_revision(user_id), 0);
    assert_eq!(projection_read_count(), 0);
}

#[test]
fn same_user_peer_waits_before_its_desk_gate_during_nested_refresh() {
    let setup = SharedUserSetup::new();
    let (actor_entered_tx, actor_entered_rx) = mpsc::channel();
    let (continue_actor_tx, continue_actor_rx) = mpsc::channel();
    let (peer_attempted_tx, peer_attempted_rx) = mpsc::channel();
    let (actor_done_tx, actor_done_rx) = mpsc::channel();
    let (peer_done_tx, peer_done_rx) = mpsc::channel();

    let actor = setup.spawn_actor(actor_entered_tx, continue_actor_rx, actor_done_tx);
    actor_entered_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let peer = setup.spawn_peer(peer_attempted_tx, peer_done_tx);
    peer_attempted_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    for _ in 0..100 {
        thread::yield_now();
    }
    continue_actor_tx.send(()).unwrap();

    actor_done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    peer_done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    actor.join().unwrap();
    peer.join().unwrap();
    assert_eq!(setup.registry.normal_values_revision(setup.user), 1);
    let object = EventObject::programming_values(setup.user.0);
    for desk in [setup.actor_context.desk_id, setup.peer_context.desk_id] {
        let filter = EventFilter::for_desk(desk).with_object(object.clone());
        assert!(matches!(
            setup.events.replay(0, &filter),
            EventReplay::Events(events) if events.len() == 1
        ));
    }
}

struct SharedUserSetup {
    registry: ProgrammerRegistry,
    service: ProgrammingService,
    events: EventBus,
    user: UserId,
    actor_session: SessionId,
    peer_session: SessionId,
    actor_context: ActionContext,
    peer_context: ActionContext,
}

impl SharedUserSetup {
    fn new() -> Self {
        let registry = ProgrammerRegistry::default();
        let user = UserId::new();
        let actor_session = SessionId::new();
        let peer_session = SessionId::new();
        let actor_desk = Uuid::new_v4();
        let peer_desk = Uuid::new_v4();
        registry.start(actor_session, user);
        registry.start(peer_session, user);
        registry.attach_command_context(actor_session, SessionId(actor_desk));
        registry.attach_command_context(peer_session, SessionId(peer_desk));
        let events = EventBus::new(16);
        let service = ProgrammingService::new(
            registry.clone(),
            events.clone(),
            Arc::new(HighlightRegistry::default()),
        );
        Self {
            registry,
            service,
            events,
            user,
            actor_session,
            peer_session,
            actor_context: ActionContext::operator(
                actor_desk,
                user.0,
                actor_session.0,
                ActionSource::Http,
            ),
            peer_context: ActionContext::operator(
                peer_desk,
                user.0,
                peer_session.0,
                ActionSource::Osc,
            ),
        }
    }

    fn spawn_actor(
        &self,
        entered: mpsc::Sender<()>,
        proceed: mpsc::Receiver<()>,
        done: mpsc::Sender<()>,
    ) -> thread::JoinHandle<()> {
        let service = self.service.clone();
        let registry = self.registry.clone();
        let context = self.actor_context.clone();
        let owner = ProgrammingSelectionTarget {
            desk_id: self.actor_context.desk_id,
            interaction_id: self.actor_session,
        };
        let peer = ProgrammingSelectionTarget {
            desk_id: self.peer_context.desk_id,
            interaction_id: self.peer_session,
        };
        thread::spawn(move || {
            service
                .run_external_interaction(&context, &LivePorts::default(), || {
                    entered.send(()).unwrap();
                    proceed.recv().unwrap();
                    service.run_selection_refresh_with_owned_target(
                        &context,
                        owner,
                        [owner, peer],
                        || registry.select(peer.interaction_id, [FixtureId::new()]),
                    );
                })
                .unwrap();
            done.send(()).unwrap();
        })
    }

    fn spawn_peer(
        &self,
        attempted: mpsc::Sender<()>,
        done: mpsc::Sender<()>,
    ) -> thread::JoinHandle<()> {
        let service = self.service.clone();
        let registry = self.registry.clone();
        let context = self.peer_context.clone();
        let session = self.peer_session;
        thread::spawn(move || {
            attempted.send(()).unwrap();
            service
                .run_external_interaction(&context, &LivePorts::default(), || {
                    registry.set(
                        session,
                        FixtureId::new(),
                        AttributeKey::intensity(),
                        AttributeValue::Normalized(0.75),
                    );
                })
                .unwrap();
            done.send(()).unwrap();
        })
    }
}
