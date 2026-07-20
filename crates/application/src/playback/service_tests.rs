use super::*;
use crate::{ActionEnvelope, ActionError, ActionErrorKind, ActionSource};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

#[path = "service_tests/event_effects.rs"]
mod event_effects;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedAction {
    source: ActionSource,
    address: ResolvedPlaybackAddress,
    action: PlaybackAction,
    surface: PlaybackSurface,
}

#[derive(Default)]
struct FakePorts {
    current_page: Mutex<u8>,
    actions: Mutex<Vec<ObservedAction>>,
    projection_reads: Mutex<Vec<PlaybackRuntimeIdentity>>,
}

impl FakePorts {
    fn set_current_page(&self, page: u8) {
        *self.current_page.lock() = page;
    }

    fn actions(&self) -> Vec<ObservedAction> {
        self.actions.lock().clone()
    }

    fn projection_reads(&self) -> Vec<PlaybackRuntimeIdentity> {
        self.projection_reads.lock().clone()
    }
}

impl PlaybackPorts for FakePorts {
    fn current_page(&self, _context: &crate::ActionContext) -> Result<u8, ActionError> {
        Ok(*self.current_page.lock())
    }

    fn playback_at(&self, page: u8, slot: u8) -> Result<Option<u16>, ActionError> {
        Ok(Some(u16::from(page) * 100 + u16::from(slot)))
    }

    fn execute(
        &self,
        context: &crate::ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        self.actions.lock().push(ObservedAction {
            source: context.source,
            address,
            action,
            surface,
        });
        Ok(PlaybackExecution::Pool {
            changed: true,
            pending: None,
        })
    }

    fn projection(
        &self,
        _context: &crate::ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<PlaybackRuntimeProjection, ActionError> {
        self.projection_reads.lock().push(identity);
        Ok(missing_projection(identity))
    }

    fn desk_projection(
        &self,
        context: &crate::ActionContext,
    ) -> Result<Option<PlaybackDeskProjection>, ActionError> {
        Ok(Some(PlaybackDeskProjection {
            scope: test_scope(),
            desk_id: context.desk_id,
            active_page: *self.current_page.lock(),
            selected_playback: None,
        }))
    }
}

#[test]
fn current_page_is_resolved_inside_the_ordered_operation() {
    let service = PlaybackService::default();
    let ports = Arc::new(FakePorts::default());
    ports.set_current_page(1);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let operation_service = service.clone();
    let operation = thread::spawn(move || {
        operation_service.run_unit_of_work(BlockingOperation {
            entered: entered_tx,
            release: release_rx,
        });
    });
    entered_rx.recv().unwrap();
    let worker = spawn_action(
        service.clone(),
        Arc::clone(&ports),
        PlaybackAddress::CurrentPage { slot: 7 },
    );
    ports.set_current_page(3);
    release_tx.send(()).unwrap();
    operation.join().unwrap();

    let result = worker.join().unwrap().unwrap();
    assert_eq!(
        result.resolved,
        ResolvedPlaybackAddress::Pool {
            number: 307,
            page: Some(3),
            slot: Some(7),
        }
    );
}

struct BlockingOperation {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

impl PlaybackUnitOfWork for BlockingOperation {
    type Output = ();

    fn execute(self) -> PlaybackOperation<Self::Output> {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        PlaybackOperation::new(())
    }
}

#[test]
fn named_unit_of_work_publishes_returned_events_in_service_order() {
    let events = crate::EventBus::new(8);
    let service = PlaybackService::new(events.clone());
    let context = crate::ActionContext::system(Uuid::from_u128(9), ActionSource::System);

    let completed = service.run_unit_of_work(PublishingOperation { context });

    assert_eq!(completed.output, "committed");
    assert_eq!(completed.event_sequences, vec![1]);
    assert_eq!(events.latest_sequence(), 1);
}

struct PublishingOperation {
    context: crate::ActionContext,
}

impl PlaybackUnitOfWork for PublishingOperation {
    type Output = &'static str;

    fn execute(self) -> PlaybackOperation<Self::Output> {
        let projection = PlaybackDeskProjection {
            scope: test_scope(),
            desk_id: self.context.desk_id,
            active_page: 2,
            selected_playback: Some(7),
        };
        PlaybackOperation::with_events(
            "committed",
            vec![crate::EventDraft::playback_view_changed(
                &self.context,
                projection,
            )],
        )
    }
}

#[test]
fn explicit_page_does_not_follow_the_current_page() {
    let service = PlaybackService::default();
    let ports = FakePorts::default();
    ports.set_current_page(9);

    let result = service
        .handle(
            envelope(
                ActionSource::Http,
                PlaybackAddress::ExplicitPage { page: 2, slot: 4 },
                None,
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(
        result.resolved,
        ResolvedPlaybackAddress::Pool {
            number: 204,
            page: Some(2),
            slot: Some(4),
        }
    );
}

#[test]
fn all_operator_sources_reach_the_same_semantic_action() {
    let service = PlaybackService::default();
    let ports = FakePorts::default();
    let sources = [
        ActionSource::UserInterface,
        ActionSource::Keyboard,
        ActionSource::Osc,
        ActionSource::Http,
    ];

    for source in sources {
        service
            .handle(envelope(source, PlaybackAddress::Pool(12), None), &ports)
            .unwrap();
    }

    let actions = ports.actions();
    assert_eq!(actions.len(), sources.len());
    assert!(actions.iter().all(|observed| {
        observed.address
            == ResolvedPlaybackAddress::Pool {
                number: 12,
                page: None,
                slot: None,
            }
            && observed.action == PlaybackAction::Go { pressed: true }
            && observed.surface == PlaybackSurface::Physical
    }));
    assert_eq!(
        actions
            .iter()
            .map(|action| action.source)
            .collect::<Vec<_>>(),
        sources
    );
}

#[test]
fn identical_request_replays_without_executing_again() {
    let service = PlaybackService::default();
    let ports = FakePorts::default();
    let first = envelope(
        ActionSource::UserInterface,
        PlaybackAddress::Pool(8),
        Some("request-8"),
    );

    assert!(!service.handle(first.clone(), &ports).unwrap().replayed);
    assert!(service.handle(first, &ports).unwrap().replayed);
    assert_eq!(ports.actions().len(), 1);
    assert_eq!(
        ports.projection_reads(),
        vec![
            PlaybackRuntimeIdentity::Playback(8),
            PlaybackRuntimeIdentity::Playback(8),
        ]
    );
}

#[test]
fn changed_runtime_publishes_one_authoritative_event_and_replay_reuses_it() {
    let events = crate::EventBus::new(8);
    let service = PlaybackService::new(events.clone());
    let ports = StatefulPorts::changing(8);
    let request = envelope(
        ActionSource::UserInterface,
        PlaybackAddress::Pool(8),
        Some("runtime-8"),
    );

    let first = service.handle(request.clone(), &ports).unwrap();
    let replay = service.handle(request, &ports).unwrap();

    assert_eq!(first.event_sequence, Some(1));
    assert_eq!(first.durability, PlaybackDurability::Durable);
    assert_eq!(replay.event_sequence, Some(1));
    assert!(replay.replayed);
    assert_eq!(ports.executions.load(Ordering::Relaxed), 1);
    assert_eq!(ports.reads.load(Ordering::Relaxed), 2);
    assert_eq!(events.latest_sequence(), 1);
    let crate::EventReplay::Events(published) = events.replay(0, &crate::EventFilter::default())
    else {
        panic!("event should remain replayable");
    };
    assert_eq!(published.len(), 1);
    let event = &published[0];
    assert_eq!(event.object, Some(crate::EventObject::playback(8)));
    assert!(
        event
            .related_objects
            .contains(&crate::EventObject::cue_list(Uuid::from_u128(80)))
    );
    assert_eq!(
        event.source,
        crate::EventSource::Action(ActionSource::UserInterface)
    );
    assert_eq!(event.correlation_id, Some(first.context.correlation_id));
    let crate::ApplicationEvent::Playback(crate::PlaybackEvent::RuntimeChanged(change)) =
        &event.payload
    else {
        panic!("expected runtime projection event");
    };
    assert_eq!(change.projection.current_cue().unwrap().number, 2.0);
    let transition = change.transition.as_ref().unwrap();
    assert_eq!(transition.previous.as_ref().unwrap().number, 1.0);
    assert_eq!(transition.current.as_ref().unwrap().number, 2.0);
    assert_eq!(transition.cause, PlaybackTransitionCause::Go);
}

#[test]
fn related_runtime_changes_publish_changed_peers_before_the_primary_once() {
    let events = crate::EventBus::new(8);
    let service = PlaybackService::new(events.clone());
    let ports = RelatedRuntimePorts::changing();
    let request = envelope(
        ActionSource::UserInterface,
        PlaybackAddress::Pool(8),
        Some("runtime-with-peer"),
    );

    let first = service.handle(request.clone(), &ports).unwrap();
    let replay = service.handle(request, &ports).unwrap();

    assert_eq!(first.event_sequence, Some(2));
    assert_eq!(first.related.len(), 1);
    assert_eq!(
        first.related[0].projection.requested,
        PlaybackRuntimeIdentity::Playback(6)
    );
    assert_eq!(first.related[0].event_sequence, 1);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(replay.related[0].event_sequence, 1);
    assert!(replay.replayed);
    assert_eq!(ports.executions.load(Ordering::Relaxed), 1);
    assert_eq!(ports.related_reads.load(Ordering::Relaxed), 1);
    assert_eq!(
        ports.related_projection_reads.lock().as_slice(),
        &[
            vec![
                PlaybackRuntimeIdentity::Playback(6),
                PlaybackRuntimeIdentity::Playback(7),
            ],
            vec![
                PlaybackRuntimeIdentity::Playback(6),
                PlaybackRuntimeIdentity::Playback(7),
            ],
        ],
        "the primary and duplicate peer identities must be removed before projection reads"
    );
    assert_eq!(events.latest_sequence(), 2, "replay must not republish");

    let crate::EventReplay::Events(published) = events.replay(0, &crate::EventFilter::default())
    else {
        panic!("peer and primary events should remain replayable");
    };
    assert_eq!(
        published
            .iter()
            .map(|event| event.object.clone())
            .collect::<Vec<_>>(),
        vec![
            Some(crate::EventObject::playback(6)),
            Some(crate::EventObject::playback(8)),
        ],
        "only the changed peer is emitted, in stable peer-before-primary order"
    );
    assert!(published.iter().all(|event| {
        event.correlation_id == Some(first.context.correlation_id)
            && event.source == crate::EventSource::Action(ActionSource::UserInterface)
    }));
    assert!(
        !runtime_projection(&published[0])
            .cue_list_runtime()
            .unwrap()
            .enabled
    );
    assert_eq!(
        runtime_projection(&published[1])
            .current_cue()
            .unwrap()
            .number,
        2.0
    );
}

#[test]
fn no_change_and_capture_do_not_publish_related_runtime_events() {
    for (execution, expected) in [
        (
            PlaybackExecution::Pool {
                changed: false,
                pending: None,
            },
            PlaybackOutcome::NoChange,
        ),
        (
            PlaybackExecution::Pool {
                changed: false,
                pending: Some(PendingPlaybackAction::Go),
            },
            PlaybackOutcome::Captured(PendingPlaybackAction::Go),
        ),
    ] {
        let events = crate::EventBus::new(4);
        let service = PlaybackService::new(events.clone());
        let ports = RelatedRuntimePorts::without_mutation(execution);

        let result = service
            .handle(
                envelope(ActionSource::Http, PlaybackAddress::Pool(8), None),
                &ports,
            )
            .unwrap();

        assert_eq!(result.outcome, expected);
        assert_eq!(result.event_sequence, None);
        assert!(result.related.is_empty());
        assert_eq!(events.latest_sequence(), 0);
        assert_eq!(ports.related_reads.load(Ordering::Relaxed), 1);
        assert_eq!(ports.related_projection_reads.lock().len(), 1);
    }
}

#[test]
fn repeated_direct_cuelist_pause_is_an_accepted_no_change() {
    let execution = PlaybackExecution::ActiveList {
        active: Vec::new(),
        changed: false,
    };

    assert_eq!(
        super::service::outcome(&execution),
        PlaybackOutcome::NoChange
    );
}

#[test]
fn accepted_pending_durability_is_retained_by_idempotent_replay() {
    let service = PlaybackService::default();
    let mut ports = StatefulPorts::changing(8);
    ports.durability = PlaybackDurability::PersistencePending;
    let request = envelope(
        ActionSource::Http,
        PlaybackAddress::Pool(8),
        Some("pending-persistence"),
    );

    let first = service.handle(request.clone(), &ports).unwrap();
    let replay = service.handle(request, &ports).unwrap();

    assert_eq!(first.durability, PlaybackDurability::PersistencePending);
    assert_eq!(replay.durability, PlaybackDurability::PersistencePending);
    assert!(replay.replayed);
    assert_eq!(ports.executions.load(Ordering::Relaxed), 1);
}

#[test]
fn desk_selection_emits_a_separate_desk_local_view_event() {
    let events = crate::EventBus::new(4);
    let service = PlaybackService::new(events.clone());
    let ports = StatefulPorts::with_execution(
        8,
        PlaybackExecution::Pool {
            changed: true,
            pending: None,
        },
    );
    let mut request = envelope(ActionSource::Http, PlaybackAddress::Pool(8), None);
    request.command.action = PlaybackAction::Select { pressed: true };

    let result = service.handle(request, &ports).unwrap();

    assert_eq!(result.event_sequence, None);
    assert_eq!(result.desk_event_sequence, Some(1));
    let crate::EventReplay::Events(events) = events.replay(0, &crate::EventFilter::default())
    else {
        panic!("desk view event should remain replayable");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].desk_id, Some(result.context.desk_id));
    assert_eq!(
        events[0].object,
        Some(crate::EventObject::playback_view(result.context.desk_id))
    );
    let crate::ApplicationEvent::Desk(crate::DeskEvent::PlaybackViewChanged(projection)) =
        &events[0].payload
    else {
        panic!("expected desk-local Playback view change");
    };
    assert_eq!(projection.selected_playback, Some(8));
}

#[test]
fn configured_button_uses_the_authoritative_resolved_navigation_cause() {
    let events = crate::EventBus::new(4);
    let service = PlaybackService::new(events.clone());
    let mut ports = StatefulPorts::changing(8);
    ports.configured_cause = Some(PlaybackTransitionCause::Go);
    let mut request = envelope(ActionSource::Http, PlaybackAddress::Pool(8), None);
    request.command.action = PlaybackAction::ConfiguredButton {
        number: 2,
        pressed: true,
    };

    service.handle(request, &ports).unwrap();

    let crate::EventReplay::Events(events) = events.replay(0, &crate::EventFilter::default())
    else {
        panic!("configured navigation should publish an event");
    };
    let crate::ApplicationEvent::Playback(crate::PlaybackEvent::RuntimeChanged(change)) =
        &events[0].payload
    else {
        panic!("expected playback runtime change");
    };
    assert_eq!(
        change
            .transition
            .as_ref()
            .map(|transition| transition.cause),
        Some(PlaybackTransitionCause::Go)
    );
}

#[test]
fn no_change_and_captured_preload_do_not_publish_runtime_events() {
    for (execution, expected) in [
        (
            PlaybackExecution::Pool {
                changed: false,
                pending: None,
            },
            PlaybackOutcome::NoChange,
        ),
        (
            PlaybackExecution::Pool {
                changed: false,
                pending: Some(PendingPlaybackAction::Go),
            },
            PlaybackOutcome::Captured(PendingPlaybackAction::Go),
        ),
    ] {
        let events = crate::EventBus::new(4);
        let service = PlaybackService::new(events.clone());
        let ports = StatefulPorts::with_execution(8, execution);

        let result = service
            .handle(
                envelope(ActionSource::Http, PlaybackAddress::Pool(8), None),
                &ports,
            )
            .unwrap();

        assert_eq!(result.outcome, expected);
        assert_eq!(result.event_sequence, None);
        assert_eq!(events.latest_sequence(), 0);
    }
}

#[test]
fn narrow_snapshot_captures_cursor_before_reads_and_replays_a_racing_change() {
    let events = crate::EventBus::new(8);
    let service = PlaybackService::new(events.clone());
    let ports = SnapshotPorts::new(events.clone());
    let identities = [
        PlaybackRuntimeIdentity::Playback(4),
        PlaybackRuntimeIdentity::CueList(light_core::CueListId(Uuid::from_u128(5))),
    ];
    let context = envelope(ActionSource::Http, PlaybackAddress::Pool(4), None).context;

    let snapshot = service.snapshot(&context, &identities, &ports).unwrap();

    assert_eq!(snapshot.event_sequence, 0);
    assert_eq!(
        ports.requests.lock().as_slice(),
        &[identities.to_vec()],
        "the projection port receives only current-view identities"
    );
    assert_eq!(
        snapshot
            .projections
            .iter()
            .map(|projection| projection.requested)
            .collect::<Vec<_>>(),
        identities
    );
    let crate::EventReplay::Events(replayed) =
        events.replay(snapshot.event_sequence, &crate::EventFilter::default())
    else {
        panic!("racing event must be replayable after the snapshot cursor");
    };
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].sequence, 1);
}

#[test]
fn desk_only_snapshot_accepts_an_empty_runtime_identity_list() {
    let events = crate::EventBus::new(8);
    let service = PlaybackService::new(events.clone());
    let ports = SnapshotPorts::new(events);
    let context = envelope(ActionSource::Http, PlaybackAddress::Pool(4), None).context;

    let snapshot = service.snapshot(&context, &[], &ports).unwrap();

    assert_eq!(ports.requests.lock().as_slice(), &[Vec::new()]);
    assert!(snapshot.projections.is_empty());
    assert_eq!(snapshot.desk.desk_id, context.desk_id);
}

#[test]
fn request_id_cannot_be_reused_for_a_different_action() {
    let service = PlaybackService::default();
    let ports = FakePorts::default();
    let first = envelope(
        ActionSource::UserInterface,
        PlaybackAddress::Pool(8),
        Some("request-8"),
    );
    service.handle(first.clone(), &ports).unwrap();
    let mut conflicting = first;
    conflicting.command.action = PlaybackAction::Back { pressed: true };

    let error = service.handle(conflicting, &ports).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(ports.actions().len(), 1);
}

#[test]
fn concurrent_requests_complete_in_the_order_the_service_admits_them() {
    let service = PlaybackService::default();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let ports = Arc::new(OrderedPorts {
        entered: entered_tx,
        release_first: std::sync::Mutex::new(release_rx),
    });
    let first = spawn_ordered_action(service.clone(), Arc::clone(&ports), 1);
    assert_eq!(entered_rx.recv().unwrap(), 1);
    let second = spawn_ordered_action(service, Arc::clone(&ports), 2);
    assert!(matches!(
        entered_rx.recv_timeout(Duration::from_millis(25)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));

    release_tx.send(()).unwrap();
    first.join().unwrap().unwrap();
    assert_eq!(entered_rx.recv().unwrap(), 2);
    second.join().unwrap().unwrap();
}

struct OrderedPorts {
    entered: mpsc::Sender<u16>,
    release_first: std::sync::Mutex<mpsc::Receiver<()>>,
}

impl PlaybackPorts for OrderedPorts {
    fn current_page(&self, _context: &crate::ActionContext) -> Result<u8, ActionError> {
        unreachable!("the ordering test uses pool addresses")
    }

    fn playback_at(&self, _page: u8, _slot: u8) -> Result<Option<u16>, ActionError> {
        unreachable!("the ordering test uses pool addresses")
    }

    fn execute(
        &self,
        _context: &crate::ActionContext,
        address: ResolvedPlaybackAddress,
        _action: PlaybackAction,
        _surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        let number = address.playback_number().unwrap();
        self.entered.send(number).unwrap();
        if number == 1 {
            self.release_first.lock().unwrap().recv().unwrap();
        }
        Ok(PlaybackExecution::Pool {
            changed: true,
            pending: None,
        })
    }

    fn projection(
        &self,
        _context: &crate::ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<PlaybackRuntimeProjection, ActionError> {
        Ok(missing_projection(identity))
    }

    fn desk_projection(
        &self,
        context: &crate::ActionContext,
    ) -> Result<Option<PlaybackDeskProjection>, ActionError> {
        Ok(Some(PlaybackDeskProjection {
            scope: test_scope(),
            desk_id: context.desk_id,
            active_page: 1,
            selected_playback: None,
        }))
    }
}

fn spawn_ordered_action(
    service: PlaybackService,
    ports: Arc<OrderedPorts>,
    number: u16,
) -> thread::JoinHandle<Result<PlaybackResult, ActionError>> {
    thread::spawn(move || {
        service.handle(
            envelope(ActionSource::Osc, PlaybackAddress::Pool(number), None),
            ports.as_ref(),
        )
    })
}

fn spawn_action(
    service: PlaybackService,
    ports: Arc<FakePorts>,
    address: PlaybackAddress,
) -> thread::JoinHandle<Result<PlaybackResult, ActionError>> {
    thread::spawn(move || {
        service.handle(envelope(ActionSource::Osc, address, None), ports.as_ref())
    })
}

fn envelope(
    source: ActionSource,
    address: PlaybackAddress,
    request_id: Option<&str>,
) -> ActionEnvelope<PlaybackCommand> {
    let mut context = crate::ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        source,
    );
    context.request_id = request_id.map(str::to_owned);
    ActionEnvelope {
        context,
        command: PlaybackCommand {
            address,
            action: PlaybackAction::Go { pressed: true },
            surface: PlaybackSurface::Physical,
        },
    }
}

fn missing_projection(identity: PlaybackRuntimeIdentity) -> PlaybackRuntimeProjection {
    PlaybackRuntimeProjection {
        scope: test_scope(),
        requested: identity,
        playback_number: match identity {
            PlaybackRuntimeIdentity::Playback(number) => Some(number),
            PlaybackRuntimeIdentity::CueList(_) => None,
        },
        target: PlaybackTargetProjection::Missing,
    }
}

struct StatefulPorts {
    projection: Mutex<PlaybackRuntimeProjection>,
    execution: PlaybackExecution,
    mutate: bool,
    executions: AtomicUsize,
    reads: AtomicUsize,
    configured_cause: Option<PlaybackTransitionCause>,
    durability: PlaybackDurability,
    addressed_event_required: bool,
    selected_playback: Mutex<Option<u16>>,
}

struct RelatedRuntimePorts {
    projections: Mutex<Vec<PlaybackRuntimeProjection>>,
    related_projection_reads: Mutex<Vec<Vec<PlaybackRuntimeIdentity>>>,
    execution: PlaybackExecution,
    mutate_primary: bool,
    mutate_peer: bool,
    executions: AtomicUsize,
    related_reads: AtomicUsize,
}

impl RelatedRuntimePorts {
    fn changing() -> Self {
        Self::new(
            PlaybackExecution::Pool {
                changed: true,
                pending: None,
            },
            true,
            true,
        )
    }

    fn peer_only() -> Self {
        Self::new(
            PlaybackExecution::Pool {
                changed: true,
                pending: None,
            },
            false,
            true,
        )
    }

    fn without_mutation(execution: PlaybackExecution) -> Self {
        Self::new(execution, false, false)
    }

    fn new(execution: PlaybackExecution, mutate_primary: bool, mutate_peer: bool) -> Self {
        let mut inactive_peer = cue_projection(7, 1.0);
        set_enabled(&mut inactive_peer, false);
        Self {
            projections: Mutex::new(vec![
                cue_projection(8, 1.0),
                cue_projection(6, 1.0),
                inactive_peer,
            ]),
            related_projection_reads: Mutex::default(),
            execution,
            mutate_primary,
            mutate_peer,
            executions: AtomicUsize::new(0),
            related_reads: AtomicUsize::new(0),
        }
    }

    fn mutate_related_runtime(&self) {
        let mut projections = self.projections.lock();
        if self.mutate_primary {
            *find_projection_mut(&mut projections, 8) = cue_projection(8, 2.0);
        }
        if self.mutate_peer {
            set_enabled(find_projection_mut(&mut projections, 6), false);
        }
    }

    fn projections_for(
        &self,
        identities: &[PlaybackRuntimeIdentity],
    ) -> Vec<PlaybackRuntimeProjection> {
        let projections = self.projections.lock();
        identities
            .iter()
            .map(|identity| {
                projections
                    .iter()
                    .find(|projection| projection.requested == *identity)
                    .cloned()
                    .unwrap_or_else(|| missing_projection(*identity))
            })
            .collect()
    }
}

impl PlaybackPorts for RelatedRuntimePorts {
    fn current_page(&self, _context: &crate::ActionContext) -> Result<u8, ActionError> {
        Ok(1)
    }

    fn playback_at(&self, _page: u8, _slot: u8) -> Result<Option<u16>, ActionError> {
        Ok(None)
    }

    fn execute(
        &self,
        _context: &crate::ActionContext,
        _address: ResolvedPlaybackAddress,
        _action: PlaybackAction,
        _surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        self.executions.fetch_add(1, Ordering::Relaxed);
        if self.mutate_primary || self.mutate_peer {
            self.mutate_related_runtime();
        }
        Ok(self.execution.clone())
    }

    fn related_runtime_identities(
        &self,
        _context: &crate::ActionContext,
        _address: ResolvedPlaybackAddress,
        _action: PlaybackAction,
        _surface: PlaybackSurface,
    ) -> Result<Vec<PlaybackRuntimeIdentity>, ActionError> {
        self.related_reads.fetch_add(1, Ordering::Relaxed);
        Ok(vec![
            PlaybackRuntimeIdentity::Playback(6),
            PlaybackRuntimeIdentity::Playback(8),
            PlaybackRuntimeIdentity::Playback(7),
            PlaybackRuntimeIdentity::Playback(6),
            PlaybackRuntimeIdentity::Playback(8),
        ])
    }

    fn projection(
        &self,
        _context: &crate::ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<PlaybackRuntimeProjection, ActionError> {
        Ok(self.projections_for(&[identity]).remove(0))
    }

    fn projections(
        &self,
        _context: &crate::ActionContext,
        identities: &[PlaybackRuntimeIdentity],
    ) -> Result<Vec<PlaybackRuntimeProjection>, ActionError> {
        self.related_projection_reads
            .lock()
            .push(identities.to_vec());
        Ok(self.projections_for(identities))
    }

    fn desk_projection(
        &self,
        context: &crate::ActionContext,
    ) -> Result<Option<PlaybackDeskProjection>, ActionError> {
        Ok(Some(test_desk(context.desk_id)))
    }
}

impl StatefulPorts {
    fn changing(number: u16) -> Self {
        Self {
            projection: Mutex::new(cue_projection(number, 1.0)),
            execution: PlaybackExecution::Pool {
                changed: true,
                pending: None,
            },
            mutate: true,
            executions: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            configured_cause: None,
            durability: PlaybackDurability::Durable,
            addressed_event_required: false,
            selected_playback: Mutex::new(None),
        }
    }

    fn with_execution(number: u16, execution: PlaybackExecution) -> Self {
        Self {
            projection: Mutex::new(cue_projection(number, 1.0)),
            execution,
            mutate: false,
            executions: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            configured_cause: None,
            durability: PlaybackDurability::Durable,
            addressed_event_required: false,
            selected_playback: Mutex::new(None),
        }
    }
}

impl PlaybackPorts for StatefulPorts {
    fn current_page(&self, _context: &crate::ActionContext) -> Result<u8, ActionError> {
        Ok(1)
    }

    fn playback_at(&self, _page: u8, _slot: u8) -> Result<Option<u16>, ActionError> {
        Ok(None)
    }

    fn execute(
        &self,
        _context: &crate::ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        _surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        self.executions.fetch_add(1, Ordering::Relaxed);
        if action == (PlaybackAction::Select { pressed: true }) {
            *self.selected_playback.lock() = address.playback_number();
        }
        if self.mutate {
            *self.projection.lock() = cue_projection(8, 2.0);
        }
        Ok(self.execution.clone())
    }

    fn durability(&self) -> PlaybackDurability {
        self.durability
    }

    fn addressed_runtime_event_required(&self) -> bool {
        self.addressed_event_required
    }

    fn transition_cause(
        &self,
        _context: &crate::ActionContext,
        _address: ResolvedPlaybackAddress,
        _action: PlaybackAction,
    ) -> Result<Option<PlaybackTransitionCause>, ActionError> {
        Ok(self.configured_cause)
    }

    fn projection(
        &self,
        _context: &crate::ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<PlaybackRuntimeProjection, ActionError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let mut projection = self.projection.lock().clone();
        projection.requested = identity;
        Ok(projection)
    }

    fn desk_projection(
        &self,
        context: &crate::ActionContext,
    ) -> Result<Option<PlaybackDeskProjection>, ActionError> {
        let mut projection = test_desk(context.desk_id);
        projection.selected_playback = *self.selected_playback.lock();
        Ok(Some(projection))
    }
}

struct SnapshotPorts {
    events: crate::EventBus,
    requests: Mutex<Vec<Vec<PlaybackRuntimeIdentity>>>,
}

impl SnapshotPorts {
    fn new(events: crate::EventBus) -> Self {
        Self {
            events,
            requests: Mutex::default(),
        }
    }
}

impl PlaybackPorts for SnapshotPorts {
    fn current_page(&self, _context: &crate::ActionContext) -> Result<u8, ActionError> {
        unreachable!("snapshot does not resolve current-page actions")
    }

    fn playback_at(&self, _page: u8, _slot: u8) -> Result<Option<u16>, ActionError> {
        unreachable!("snapshot does not resolve page assignments")
    }

    fn execute(
        &self,
        _context: &crate::ActionContext,
        _address: ResolvedPlaybackAddress,
        _action: PlaybackAction,
        _surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        unreachable!("snapshot does not execute actions")
    }

    fn projection(
        &self,
        _context: &crate::ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<PlaybackRuntimeProjection, ActionError> {
        Ok(missing_projection(identity))
    }

    fn projections(
        &self,
        _context: &crate::ActionContext,
        identities: &[PlaybackRuntimeIdentity],
    ) -> Result<Vec<PlaybackRuntimeProjection>, ActionError> {
        self.requests.lock().push(identities.to_vec());
        self.events.publish(runtime_event(99));
        Ok(identities.iter().copied().map(missing_projection).collect())
    }

    fn desk_projection(
        &self,
        context: &crate::ActionContext,
    ) -> Result<Option<PlaybackDeskProjection>, ActionError> {
        Ok(Some(test_desk(context.desk_id)))
    }
}

fn cue_projection(number: u16, cue_number: f64) -> PlaybackRuntimeProjection {
    PlaybackRuntimeProjection {
        scope: test_scope(),
        requested: PlaybackRuntimeIdentity::Playback(number),
        playback_number: Some(number),
        target: PlaybackTargetProjection::CueList {
            cue_list_id: light_core::CueListId(Uuid::from_u128(80)),
            runtime: Some(Box::new(CueListRuntimeProjection {
                cue_index: cue_number as usize - 1,
                previous_index: None,
                current: Some(PlaybackCueReference {
                    id: Uuid::from_u128(100 + cue_number as u128),
                    number: cue_number,
                }),
                loaded: None,
                normal_next: None,
                effective_next: None,
                effective_next_is_loaded: false,
                paused: false,
                activated_at: chrono::Utc::now(),
                master: 1.0,
                fader_position: 1.0,
                fader_pickup_required: false,
                flash: false,
                temporary: false,
                temporary_active: false,
                temporary_master: 0.0,
                swap_active: false,
                enabled: true,
                transition_timing_bypassed: false,
                manual_xfade_position: 0.0,
                manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
                manual_xfade_progress: 0.0,
            })),
        },
    }
}

fn find_projection_mut(
    projections: &mut [PlaybackRuntimeProjection],
    number: u16,
) -> &mut PlaybackRuntimeProjection {
    projections
        .iter_mut()
        .find(|projection| projection.requested == PlaybackRuntimeIdentity::Playback(number))
        .expect("test projection should exist")
}

fn set_enabled(projection: &mut PlaybackRuntimeProjection, enabled: bool) {
    let PlaybackTargetProjection::CueList {
        runtime: Some(runtime),
        ..
    } = &mut projection.target
    else {
        panic!("test projection should target a Cuelist runtime");
    };
    runtime.enabled = enabled;
}

fn runtime_projection(event: &crate::EventEnvelope) -> &PlaybackRuntimeProjection {
    let crate::ApplicationEvent::Playback(crate::PlaybackEvent::RuntimeChanged(change)) =
        &event.payload
    else {
        panic!("expected Playback runtime event");
    };
    &change.projection
}

fn test_desk(desk_id: Uuid) -> PlaybackDeskProjection {
    PlaybackDeskProjection {
        scope: test_scope(),
        desk_id,
        active_page: 1,
        selected_playback: None,
    }
}

fn test_scope() -> PlaybackShowScope {
    PlaybackShowScope {
        show_id: Uuid::from_u128(70),
        show_revision: 3,
    }
}

fn runtime_event(number: u16) -> crate::EventDraft {
    crate::EventDraft::playback_runtime_changed(
        None,
        PlaybackRuntimeChange {
            projection: missing_projection(PlaybackRuntimeIdentity::Playback(number)),
            transition: None,
        },
        crate::EventSource::Runtime,
        None,
    )
}
