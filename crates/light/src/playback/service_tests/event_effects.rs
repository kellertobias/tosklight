use super::*;

#[test]
fn addressed_runtime_hidden_from_projection_still_publishes_once() {
    let events = crate::EventBus::new(4);
    let service = PlaybackService::new(events.clone());
    let mut ports = StatefulPorts::with_execution(
        8,
        PlaybackExecution::Playback {
            changed: true,
            pending: None,
        },
    );
    ports.addressed_event_required = true;
    let request = envelope(
        ActionSource::UserInterface,
        PlaybackAddress::Playback(test_address(1, 8)),
        Some("hidden-runtime-effect"),
    );

    let first = service.handle(request.clone(), &ports).unwrap();
    let replay = service.handle(request, &ports).unwrap();

    assert_eq!(first.event_sequence, Some(1));
    assert_eq!(replay.event_sequence, Some(2));
    assert_eq!(events.latest_sequence(), 2);
    assert_eq!(ports.executions.load(Ordering::Relaxed), 2);
}

#[test]
fn peer_only_runtime_change_does_not_force_an_equal_primary_event() {
    let events = crate::EventBus::new(4);
    let service = PlaybackService::new(events.clone());
    let ports = RelatedRuntimePorts::peer_only();

    let result = service
        .handle(
            envelope(
                ActionSource::Http,
                PlaybackAddress::Playback(test_address(1, 8)),
                None,
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(result.outcome, PlaybackOutcome::Applied);
    assert_eq!(result.event_sequence, Some(1));
    assert_eq!(result.related.len(), 1);
    let crate::EventReplay::Events(published) = events.replay(0, &crate::EventFilter::default())
    else {
        panic!("the peer transition should remain replayable");
    };
    assert_eq!(published.len(), 1);
    assert_eq!(
        published[0].object,
        Some(crate::EventObject::playback(test_address(1, 6)))
    );
}
