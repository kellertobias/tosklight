use super::*;
use crate::{ActionEnvelope, ActionError, ActionErrorKind, ActionSource};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

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
}

impl FakePorts {
    fn set_current_page(&self, page: u8) {
        *self.current_page.lock() = page;
    }

    fn actions(&self) -> Vec<ObservedAction> {
        self.actions.lock().clone()
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
}

#[test]
fn current_page_is_resolved_inside_the_ordered_operation() {
    let service = PlaybackService::default();
    let ports = Arc::new(FakePorts::default());
    ports.set_current_page(1);
    let ordered = service.operation_lock();
    let worker = spawn_action(
        service.clone(),
        Arc::clone(&ports),
        PlaybackAddress::CurrentPage { slot: 7 },
    );
    ports.set_current_page(3);
    drop(ordered);

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
