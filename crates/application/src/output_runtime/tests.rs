use parking_lot::Mutex;
use uuid::Uuid;

use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ApplicationEvent,
    EventFilter, EventObject, EventReplay, EventSource, OutputEvent,
};

use super::*;

#[test]
fn output_level_rejects_non_finite_and_out_of_range_values() {
    for invalid in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
        assert!(OutputLevel::new(invalid).is_none());
    }
    assert_eq!(OutputLevel::new(0.0).unwrap().value(), 0.0);
    assert_eq!(OutputLevel::new(1.0).unwrap().value(), 1.0);
}

#[test]
fn combined_global_output_change_publishes_one_installation_event() {
    let events = crate::EventBus::new(8);
    let service = OutputRuntimeService::new(events.clone());
    let ports = FakePorts::default();
    let context = context(ActionSource::Midi);
    let result = service
        .handle(
            envelope(
                context.clone(),
                OutputRuntimeCommand::new(level(0.4), Some(true)),
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(result.outcome, OutputRuntimeOutcome::Applied);
    assert_eq!(result.event_sequence, Some(1));
    assert_eq!(ports.applies(), 1);
    let EventReplay::Events(retained) = events.replay(
        0,
        &EventFilter::default().with_object(EventObject::global_output()),
    ) else {
        panic!("output event should be retained");
    };
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].desk_id, None);
    assert_eq!(retained[0].source, EventSource::Action(ActionSource::Midi));
    assert_eq!(retained[0].correlation_id, Some(context.correlation_id));
    let ApplicationEvent::Output(OutputEvent::RuntimeChanged(change)) = &retained[0].payload else {
        panic!("expected global-output projection event");
    };
    assert_eq!(change.projection.grand_master, 0.4);
    assert!(change.projection.blackout);
}

#[test]
fn no_change_skips_the_adapter_and_event_bus() {
    let events = crate::EventBus::new(4);
    let service = OutputRuntimeService::new(events.clone());
    let ports = FakePorts::default();
    let result = service
        .handle(
            envelope(
                context(ActionSource::Http),
                OutputRuntimeCommand::new(level(1.0), Some(false)),
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(result.outcome, OutputRuntimeOutcome::NoChange);
    assert_eq!(ports.applies(), 0);
    assert_eq!(events.latest_sequence(), 0);
}

#[test]
fn request_replay_is_idempotent_and_scoped_to_the_active_show() {
    let events = crate::EventBus::new(4);
    let service = OutputRuntimeService::new(events.clone());
    let ports = FakePorts::default();
    let request_context = context(ActionSource::UserInterface).with_request_id("master-1");
    let command = OutputRuntimeCommand::new(level(0.5), None);

    let first = service
        .handle(envelope(request_context.clone(), command), &ports)
        .unwrap();
    let replay = service
        .handle(envelope(request_context.clone(), command), &ports)
        .unwrap();
    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(ports.applies(), 1);
    assert_eq!(events.latest_sequence(), 1);

    let error = service
        .handle(
            envelope(request_context, OutputRuntimeCommand::new(level(0.7), None)),
            &ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);

    ports.set_scope(OutputRuntimeScope {
        show_id: Uuid::from_u128(99),
        show_revision: 0,
    });
    let next_show = service
        .handle(
            envelope(
                context(ActionSource::UserInterface).with_request_id("master-1"),
                OutputRuntimeCommand::new(level(0.7), None),
            ),
            &ports,
        )
        .unwrap();
    assert!(!next_show.replayed);
    assert_eq!(ports.applies(), 2);
}

#[test]
fn snapshot_returns_the_exact_identity_and_pre_read_cursor() {
    let events = crate::EventBus::new(4);
    let service = OutputRuntimeService::new(events);
    let ports = FakePorts::default();
    service
        .handle(
            envelope(
                context(ActionSource::Http),
                OutputRuntimeCommand::new(None, Some(true)),
            ),
            &ports,
        )
        .unwrap();

    let snapshot = service
        .snapshot(
            &context(ActionSource::Http),
            OutputRuntimeIdentity::GlobalMaster,
            &ports,
        )
        .unwrap();
    assert_eq!(snapshot.event_sequence, 1);
    assert_eq!(
        snapshot.projection.identity,
        OutputRuntimeIdentity::GlobalMaster
    );
    assert!(snapshot.projection.blackout);
}

fn context(source: ActionSource) -> ActionContext {
    ActionContext::system(Uuid::from_u128(1), source)
}

fn envelope(
    context: ActionContext,
    command: OutputRuntimeCommand,
) -> ActionEnvelope<OutputRuntimeCommand> {
    ActionEnvelope { context, command }
}

fn level(value: f32) -> Option<OutputLevel> {
    Some(OutputLevel::new(value).unwrap())
}

struct FakePorts {
    projection: Mutex<OutputRuntimeProjection>,
    applies: Mutex<usize>,
}

impl Default for FakePorts {
    fn default() -> Self {
        Self {
            projection: Mutex::new(OutputRuntimeProjection {
                scope: OutputRuntimeScope {
                    show_id: Uuid::from_u128(10),
                    show_revision: 4,
                },
                identity: OutputRuntimeIdentity::GlobalMaster,
                grand_master: 1.0,
                blackout: false,
            }),
            applies: Mutex::new(0),
        }
    }
}

impl FakePorts {
    fn applies(&self) -> usize {
        *self.applies.lock()
    }

    fn set_scope(&self, scope: OutputRuntimeScope) {
        self.projection.lock().scope = scope;
    }
}

impl OutputRuntimePorts for FakePorts {
    fn projection(
        &self,
        _context: &ActionContext,
        identity: OutputRuntimeIdentity,
    ) -> Result<OutputRuntimeProjection, ActionError> {
        let mut projection = *self.projection.lock();
        projection.identity = identity;
        Ok(projection)
    }

    fn apply(
        &self,
        _context: &ActionContext,
        command: OutputRuntimeCommand,
    ) -> Result<OutputRuntimeDurability, ActionError> {
        *self.applies.lock() += 1;
        let current = *self.projection.lock();
        *self.projection.lock() = command.desired(current);
        Ok(OutputRuntimeDurability::Durable)
    }
}
