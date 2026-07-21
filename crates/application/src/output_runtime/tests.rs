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
    assert_eq!(result.projection.revision, 1);
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
    assert_eq!(error.current_revision, Some(1));

    ports.set_scope(OutputRuntimeScope {
        show_id: Uuid::from_u128(99),
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
fn exact_expectation_rejects_stale_or_replaced_authority_without_applying() {
    let service = OutputRuntimeService::new(crate::EventBus::new(4));
    let ports = FakePorts::default();
    let show_id = ports.projection().scope.show_id;

    let stale = service
        .handle(
            envelope(
                context(ActionSource::Http),
                OutputRuntimeCommand::exact(show_id, 3, level(0.5), None),
            ),
            &ports,
        )
        .unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_revision, Some(0));

    let replaced_show = Uuid::from_u128(99);
    let replaced = service
        .handle(
            envelope(
                context(ActionSource::Http),
                OutputRuntimeCommand::exact(replaced_show, 0, level(0.5), None),
            ),
            &ports,
        )
        .unwrap_err();
    assert_eq!(replaced.kind, ActionErrorKind::Conflict);
    assert_eq!(replaced.current_revision, Some(0));
    assert_eq!(ports.applies(), 0);
    assert_eq!(service.events().latest_sequence(), 0);
}

#[test]
fn exact_request_replay_precedes_newer_projection_validation() {
    let events = crate::EventBus::new(4);
    let service = OutputRuntimeService::new(events.clone());
    let ports = FakePorts::default();
    let show_id = ports.projection().scope.show_id;
    let request_context = context(ActionSource::Http).with_request_id("exact-master");
    let command = OutputRuntimeCommand::exact(show_id, 0, level(0.5), None);

    let first = service
        .handle(envelope(request_context.clone(), command), &ports)
        .unwrap();
    service
        .handle(
            envelope(
                context(ActionSource::Midi),
                OutputRuntimeCommand::new(None, Some(true)),
            ),
            &ports,
        )
        .unwrap();
    let replay = service
        .handle(envelope(request_context, command), &ports)
        .unwrap();

    assert!(replay.replayed);
    assert_eq!(replay.projection, first.projection);
    assert_eq!(replay.event_sequence, first.event_sequence);
    assert_eq!(ports.applies(), 2);
    assert_eq!(events.latest_sequence(), 2);
}

#[test]
fn persistence_pending_outcome_and_warning_are_replayed_exactly() {
    let service = OutputRuntimeService::new(crate::EventBus::new(4));
    let ports = FakePorts::default();
    ports.set_application(OutputRuntimeApplication {
        durability: OutputRuntimeDurability::PersistencePending,
        warning: Some("write deferred".into()),
    });
    let request_context = context(ActionSource::Matter).with_request_id("pending-master");
    let command = OutputRuntimeCommand::new(level(0.7), Some(true));

    let first = service
        .handle(envelope(request_context.clone(), command), &ports)
        .unwrap();
    let replay = service
        .handle(envelope(request_context, command), &ports)
        .unwrap();

    assert_eq!(
        first.durability,
        OutputRuntimeDurability::PersistencePending
    );
    assert_eq!(first.warning.as_deref(), Some("write deferred"));
    assert!(replay.replayed);
    assert_eq!(replay.durability, first.durability);
    assert_eq!(replay.warning, first.warning);
}

#[test]
fn malformed_adapter_projection_never_publishes_authority() {
    for malformation in [
        AppliedMalformation::UnchangedRevision,
        AppliedMalformation::WrongGrandMaster,
        AppliedMalformation::WrongBlackout,
        AppliedMalformation::ReplacedScope,
    ] {
        let events = crate::EventBus::new(4);
        let service = OutputRuntimeService::new(events.clone());
        let ports = FakePorts::default();
        ports.malform_next(malformation);

        let error = service
            .handle(
                envelope(
                    context(ActionSource::Http),
                    OutputRuntimeCommand::new(level(0.4), Some(true)),
                ),
                &ports,
            )
            .unwrap_err();

        assert!(matches!(
            error.kind,
            ActionErrorKind::Internal | ActionErrorKind::Busy
        ));
        assert_eq!(events.latest_sequence(), 0);
    }
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
    application: Mutex<OutputRuntimeApplication>,
    malformation: Mutex<Option<AppliedMalformation>>,
}

impl Default for FakePorts {
    fn default() -> Self {
        Self {
            projection: Mutex::new(OutputRuntimeProjection {
                scope: OutputRuntimeScope {
                    show_id: Uuid::from_u128(10),
                },
                identity: OutputRuntimeIdentity::GlobalMaster,
                revision: 0,
                grand_master: 1.0,
                blackout: false,
            }),
            applies: Mutex::new(0),
            application: Mutex::new(OutputRuntimeApplication::durable()),
            malformation: Mutex::new(None),
        }
    }
}

impl FakePorts {
    fn applies(&self) -> usize {
        *self.applies.lock()
    }

    fn projection(&self) -> OutputRuntimeProjection {
        *self.projection.lock()
    }

    fn set_scope(&self, scope: OutputRuntimeScope) {
        self.projection.lock().scope = scope;
    }

    fn set_application(&self, application: OutputRuntimeApplication) {
        *self.application.lock() = application;
    }

    fn malform_next(&self, malformation: AppliedMalformation) {
        *self.malformation.lock() = Some(malformation);
    }
}

#[derive(Clone, Copy)]
enum AppliedMalformation {
    UnchangedRevision,
    WrongGrandMaster,
    WrongBlackout,
    ReplacedScope,
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
    ) -> Result<OutputRuntimeApplication, ActionError> {
        *self.applies.lock() += 1;
        let mut current = *self.projection.lock();
        current = command.desired(current);
        current.revision += 1;
        match self.malformation.lock().take() {
            Some(AppliedMalformation::UnchangedRevision) => current.revision -= 1,
            Some(AppliedMalformation::WrongGrandMaster) => current.grand_master = 0.9,
            Some(AppliedMalformation::WrongBlackout) => current.blackout = false,
            Some(AppliedMalformation::ReplacedScope) => {
                current.scope.show_id = Uuid::from_u128(999)
            }
            None => {}
        }
        *self.projection.lock() = current;
        Ok(self.application.lock().clone())
    }
}
