use super::*;
use crate::{DeliveryPolicy, EventClass, EventSource};
use light_programmer::ProgrammerCaptureMode;

use super::super::values_projection::{projection_read_count, reset_projection_read_count};

#[test]
fn external_mode_changes_publish_one_exact_user_projection() {
    let setup = LiveSetup::new(8);
    let registry = setup.ports.registry.as_ref().unwrap();
    let session = SessionId(setup.context.session_id.unwrap());
    let user = UserId(setup.context.user_id.unwrap());
    reset_projection_read_count();

    let result = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            registry.set_modes(session, Some(true), None, None, None);
            registry.set_modes(session, None, Some(true), None, None);
            registry.arm_preload(session, false);
        })
        .unwrap();

    assert_eq!(result.event_sequence, None);
    assert_eq!(result.capture_mode_event_sequence, Some(1));
    assert_eq!(result.values_event_sequence, None);
    assert_eq!(registry.capture_mode_revision(), 1);
    assert_eq!(projection_read_count(), 0);
    let filter = EventFilter::for_desk(setup.context.desk_id)
        .with_object(EventObject::programming_capture_mode(user.0));
    let EventReplay::Events(events) = setup.events.replay(0, &filter) else {
        panic!("capture-mode events should remain replayable")
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
    let ApplicationEvent::Programming(ProgrammingEvent::CaptureModeChanged(change)) =
        &event.payload
    else {
        panic!("expected a typed capture-mode event")
    };
    assert_eq!(change.projection.user_id, user);
    assert_eq!(change.projection.revision, 1);
    assert_eq!(
        change.projection.mode(),
        ProgrammerCaptureMode {
            blind: true,
            preview: true,
            preload_capture_programmer: false,
        }
    );
}

#[test]
fn no_op_active_context_and_round_trip_mode_actions_stay_quiet() {
    let setup = LiveSetup::new(8);
    let registry = setup.ports.registry.as_ref().unwrap();
    let session = SessionId(setup.context.session_id.unwrap());
    reset_projection_read_count();

    let active_context = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            registry.set_modes(
                session,
                None,
                None,
                None,
                Some(Some("fixture-sheet".into())),
            );
        })
        .unwrap();
    let round_trip = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            registry.set_modes(session, Some(true), None, None, None);
            registry.set_modes(session, Some(false), None, None, None);
        })
        .unwrap();

    assert_eq!(active_context.capture_mode_event_sequence, None);
    assert_eq!(round_trip.capture_mode_event_sequence, None);
    assert_eq!(registry.capture_mode_revision(), 0);
    assert_eq!(setup.events.latest_sequence(), 0);
    assert_eq!(projection_read_count(), 0);
}

#[test]
fn typed_preload_handle_publishes_capture_mode_and_reconciles_exact_tuple() {
    let setup = LiveSetup::new(8);
    reset_projection_read_count();

    let result = setup.handle(ProgrammingCommand::Preload {
        capture_programmer: false,
    });

    assert_eq!(result.capture_mode_event_sequence, Some(1));
    assert_eq!(result.values_event_sequence, None);
    assert_eq!(projection_read_count(), 0);
    assert_eq!(setup.service.programmers.capture_mode_revision(), 1);
    assert_eq!(
        *setup.ports.reconciliations.lock(),
        vec![ProgrammingReconciliation::CaptureModeChanged]
    );
}

#[test]
fn capture_mode_is_the_desks_and_every_surface_reads_it() {
    let setup = LiveSetup::new(8);
    let registry = setup.ports.registry.as_ref().unwrap();
    let user = UserId(setup.context.user_id.unwrap());
    let second_screen = SessionId::new();
    let screen_desk = Uuid::new_v4();
    registry.start(second_screen, user);
    registry.attach_command_context(second_screen, SessionId(screen_desk));
    let screen_context = ActionContext::operator(
        screen_desk,
        user.0,
        second_screen.0,
        ActionSource::UserInterface,
    );
    setup.handle(ProgrammingCommand::Preload {
        capture_programmer: false,
    });

    let snapshot = setup
        .service
        .capture_mode_snapshot(&screen_context, &setup.ports)
        .unwrap();
    assert_eq!(snapshot.event_sequence, 1);
    assert_eq!(snapshot.projection.user_id, user);
    assert_eq!(snapshot.projection.revision, 1);
    assert!(snapshot.projection.blind);
    assert!(!snapshot.projection.preload_capture_programmer);

    // An identity from before the desk had only one is not foreign: there is nothing left for it
    // to be foreign to, so it reads the desk's own capture mode.
    let legacy = UserId::new();
    let legacy_context =
        ActionContext::operator(screen_desk, legacy.0, second_screen.0, ActionSource::Http);
    let through_legacy_identity = setup
        .service
        .capture_mode_snapshot(&legacy_context, &setup.ports)
        .unwrap();
    assert_eq!(through_legacy_identity.projection.user_id, user);
    assert!(through_legacy_identity.projection.blind);

    // Authentication still gates the read. A system context operates no surface.
    let system = ActionContext::system(screen_desk, ActionSource::System);
    assert_eq!(
        setup
            .service
            .capture_mode_snapshot(&system, &setup.ports)
            .unwrap_err()
            .kind,
        crate::ActionErrorKind::Unauthorized
    );

    // A subscription filtered to a different Programmer still matches nothing.
    let denied = EventFilter {
        programmer_user_id: Some(legacy.0),
        ..EventFilter::for_desk(screen_desk)
            .with_object(EventObject::programming_capture_mode(user.0))
    };
    assert!(matches!(
        setup.events.replay(0, &denied),
        EventReplay::Events(events) if events.is_empty()
    ));
}
