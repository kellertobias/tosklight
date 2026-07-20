use super::*;
use crate::{ActionErrorKind, EventObject};
use light_programmer::{PreloadPlaybackQueueAction, PreloadPlaybackQueueSurface};

struct QueuePorts {
    registry: ProgrammerRegistry,
}

impl ProgrammingPorts for QueuePorts {
    fn execute(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        if command == "QUEUE" {
            programmers.queue_preload_playback_action(
                SessionId(context.session_id.unwrap()),
                7,
                None,
                PreloadPlaybackQueueAction::Go,
                PreloadPlaybackQueueSurface::Virtual,
            );
        }
        ProgrammingExecution::Accepted {
            applied: 1,
            warning: None,
        }
    }

    fn persist(&self, _context: &ActionContext, _operation: &'static str) -> Option<String> {
        None
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

    fn commit_preload(&self, context: &ActionContext) -> Result<Option<String>, String> {
        let session = SessionId(context.session_id.ok_or("missing session")?);
        self.registry.activate_preload(session);
        self.registry.take_preload_playback_actions(session);
        Ok(None)
    }
}

struct QueueSetup {
    registry: ProgrammerRegistry,
    service: ProgrammingService,
    events: EventBus,
    user: UserId,
    first: SessionId,
    second: SessionId,
    first_context: ActionContext,
    second_context: ActionContext,
    ports: QueuePorts,
}

impl QueueSetup {
    fn new() -> Self {
        let registry = ProgrammerRegistry::default();
        let user = UserId::new();
        let first = SessionId::new();
        let second = SessionId::new();
        let first_desk = Uuid::new_v4();
        let second_desk = Uuid::new_v4();
        registry.start(first, user);
        registry.start(second, user);
        registry.attach_command_context(first, SessionId(first_desk));
        registry.attach_command_context(second, SessionId(second_desk));
        let events = EventBus::new(64);
        let service = ProgrammingService::new(
            registry.clone(),
            events.clone(),
            Arc::new(HighlightRegistry::default()),
        );
        Self {
            registry: registry.clone(),
            service,
            events,
            user,
            first,
            second,
            first_context: ActionContext::operator(first_desk, user.0, first.0, ActionSource::Http),
            second_context: ActionContext::operator(
                second_desk,
                user.0,
                second.0,
                ActionSource::Osc,
            ),
            ports: QueuePorts { registry },
        }
    }

    fn queue(&self, context: &ActionContext, number: u16) -> ProgrammingInteractionResult<bool> {
        self.queue_on_page(context, number, None)
    }

    fn queue_on_page(
        &self,
        context: &ActionContext,
        number: u16,
        page: Option<u8>,
    ) -> ProgrammingInteractionResult<bool> {
        self.service
            .run_external_interaction(context, &self.ports, || {
                self.registry.queue_preload_playback_action(
                    SessionId(context.session_id.unwrap()),
                    number,
                    page,
                    PreloadPlaybackQueueAction::Toggle,
                    PreloadPlaybackQueueSurface::Physical,
                )
            })
            .unwrap()
    }

    fn queue_events(&self) -> Vec<Arc<crate::EventEnvelope>> {
        let filter = EventFilter::default()
            .with_object(EventObject::programming_preload_playback_queue(self.user.0));
        let EventReplay::Events(events) = self.events.replay(0, &filter) else {
            panic!("queue events should remain replayable")
        };
        events
    }
}

#[test]
fn snapshot_is_authenticated_exact_user_and_preserves_ordered_duplicates() {
    let setup = QueueSetup::new();
    setup.queue_on_page(&setup.first_context, 3, Some(4));
    setup.queue(&setup.second_context, 3);

    let snapshot = setup
        .service
        .preload_playback_queue_snapshot(&setup.first_context, &setup.ports)
        .unwrap();
    assert_eq!(snapshot.projection.user_id, setup.user);
    assert_eq!(snapshot.projection.revision, 2);
    assert_eq!(
        snapshot
            .projection
            .actions
            .iter()
            .map(|action| action.playback_number)
            .collect::<Vec<_>>(),
        [3, 3]
    );
    assert_eq!(
        snapshot
            .projection
            .actions
            .iter()
            .map(|action| action.page)
            .collect::<Vec<_>>(),
        [Some(4), None]
    );

    let foreign = ActionContext::operator(
        setup.first_context.desk_id,
        UserId::new().0,
        setup.first.0,
        ActionSource::Http,
    );
    let error = setup
        .service
        .preload_playback_queue_snapshot(&foreign, &setup.ports)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Forbidden);
    let error = setup
        .service
        .preload_playback_queue_snapshot(
            &ActionContext::system(Uuid::new_v4(), ActionSource::System),
            &setup.ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Unauthorized);
}

#[test]
fn queue_transitions_publish_once_and_request_replay_is_sparse() {
    let setup = QueueSetup::new();
    let action = ActionEnvelope {
        context: setup.first_context.clone().with_request_id("queue-once"),
        command: ProgrammingCommand::Execute {
            command: Some("QUEUE".into()),
            policy: ExecutionPolicy::AtomicProgrammer,
        },
    };
    let first = setup.service.handle(action.clone(), &setup.ports).unwrap();
    assert!(first.preload_playback_queue_event_sequence.is_some());
    super::super::preload_playback_queue_projection::reset_projection_read_count();
    let replay = setup.service.handle(action, &setup.ports).unwrap();
    assert!(replay.replayed);
    assert_eq!(
        super::super::preload_playback_queue_projection::projection_read_count(),
        0
    );
    assert_eq!(setup.queue_events().len(), 1);

    let peer = setup.queue(&setup.second_context, 7);
    assert!(peer.preload_playback_queue_event_sequence.is_some());
    assert_eq!(setup.queue_events().len(), 2);

    let drain = setup
        .service
        .run_external_interaction(&setup.first_context, &setup.ports, || {
            setup.registry.take_preload_playback_actions(setup.first)
        })
        .unwrap();
    assert!(drain.preload_playback_queue_event_sequence.is_some());
    super::super::preload_playback_queue_projection::reset_projection_read_count();
    let no_op = setup
        .service
        .run_external_interaction(&setup.first_context, &setup.ports, || {
            setup.registry.take_preload_playback_actions(setup.first)
        })
        .unwrap();
    assert_eq!(no_op.preload_playback_queue_event_sequence, None);
    assert_eq!(
        super::super::preload_playback_queue_projection::projection_read_count(),
        0
    );
    assert_eq!(setup.queue_events().len(), 3);
}

#[test]
fn same_user_session_lifecycle_does_not_publish_queue_events() {
    let setup = QueueSetup::new();
    setup.queue(&setup.first_context, 5);
    let third = SessionId::new();
    let third_context =
        ActionContext::operator(Uuid::new_v4(), setup.user.0, third.0, ActionSource::Http);

    setup
        .service
        .run_lifecycle_transition(&third_context, setup.user, || {
            setup.registry.start(third, setup.user);
            setup
                .registry
                .attach_command_context(third, SessionId(third_context.desk_id));
        });
    setup
        .service
        .run_lifecycle_transition(&third_context, setup.user, || {
            setup.registry.disconnect(third);
        });

    assert_eq!(setup.queue_events().len(), 1);
    assert_eq!(
        setup
            .service
            .preload_playback_queue_snapshot(&setup.first_context, &setup.ports)
            .unwrap()
            .projection
            .revision,
        1
    );
}

#[test]
fn clear_release_undo_redo_and_replacement_each_publish_one_final_projection() {
    let setup = QueueSetup::new();
    setup.registry.arm_preload(setup.first, true);
    setup.queue(&setup.first_context, 1);
    let clear = setup
        .service
        .handle(
            ActionEnvelope {
                context: setup.first_context.clone().with_request_id("clear-queue"),
                command: ProgrammingCommand::ClearStep,
            },
            &setup.ports,
        )
        .unwrap();
    assert!(clear.preload_playback_queue_event_sequence.is_some());

    setup.registry.arm_preload(setup.first, true);
    setup.queue(&setup.first_context, 2);
    let release = setup
        .service
        .run_external_interaction(&setup.first_context, &setup.ports, || {
            setup.registry.release_preload(setup.first)
        })
        .unwrap();
    assert!(release.preload_playback_queue_event_sequence.is_some());

    setup.queue(&setup.first_context, 3);
    let undo = setup
        .service
        .run_external_interaction(&setup.first_context, &setup.ports, || {
            setup.registry.undo(setup.first)
        })
        .unwrap();
    assert!(undo.preload_playback_queue_event_sequence.is_some());
    let redo = setup
        .service
        .run_external_interaction(&setup.first_context, &setup.ports, || {
            setup.registry.redo(setup.first)
        })
        .unwrap();
    assert!(redo.preload_playback_queue_event_sequence.is_some());

    let replacement = setup
        .service
        .replace_user_programmer(
            &setup.first_context,
            &setup.ports,
            ProgrammingLifecycleTarget::new(
                setup.user,
                setup.first,
                vec![setup.first_context.desk_id, setup.second_context.desk_id],
            ),
            || {
                setup.registry.clear(setup.first);
                setup.registry.start(setup.first, setup.user);
                setup.registry.start(setup.second, setup.user);
                ProgrammingLifecycleCompletion::new((), Some(setup.first))
            },
        )
        .unwrap();
    assert!(replacement.preload_playback_queue_event_sequence.is_some());
    assert_eq!(replacement.preload_playback_queue_revision, 8);
    assert_eq!(setup.queue_events().len(), 8);
}
