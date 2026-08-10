use std::sync::{Arc, Mutex};

use light_core::CueListId;
use light_playback::{
    TimecodeClipEnd, TimecodeClipId, TimecodeClipStart, TimecodeCueListClip, TimecodeFrame,
    TimecodeFrameRate, TimecodeId, TimecodeLane, TimecodeLaneContent, TimecodeLaneId,
    TimecodeTransportAction, TimecodeTransportState,
};
use uuid::Uuid;

use super::*;

#[derive(Default)]
struct ManualTimecodeClock(Mutex<u64>);

impl ManualTimecodeClock {
    fn advance(&self, micros: u64) {
        *self.0.lock().unwrap() += micros;
    }
}

impl TimecodeClock for ManualTimecodeClock {
    fn now_micros(&self) -> u64 {
        *self.0.lock().unwrap()
    }
}

#[derive(Default)]
struct RecordingPublisher(Mutex<Vec<TimecodeRuntimeChange>>);

impl RecordingPublisher {
    fn changes(&self) -> Vec<TimecodeRuntimeChange> {
        self.0.lock().unwrap().clone()
    }
}

impl TimecodeChangePublisher for RecordingPublisher {
    fn publish(&self, change: &TimecodeRuntimeChange) {
        self.0.lock().unwrap().push(change.clone());
    }
}

fn definition() -> light_playback::TimecodeDefinition {
    light_playback::TimecodeDefinition {
        id: TimecodeId(Uuid::from_u128(1)),
        number: 1,
        name: "Opener".into(),
        duration: Some(TimecodeFrame(100)),
        transport_offset: TimecodeFrame::ZERO,
        auto_start: false,
        audio: None,
        markers: Vec::new(),
        lanes: vec![TimecodeLane {
            id: TimecodeLaneId(Uuid::from_u128(2)),
            name: "Opening".into(),
            content: TimecodeLaneContent::CueList {
                cue_list_id: CueListId(Uuid::from_u128(3)),
                clips: vec![TimecodeCueListClip {
                    id: TimecodeClipId(Uuid::from_u128(4)),
                    start_frame: TimecodeFrame(10),
                    end_frame: TimecodeFrame(20),
                    start_cue_id: Uuid::from_u128(5),
                    end_cue_id: Uuid::from_u128(6),
                    start_behavior: TimecodeClipStart::State,
                    end_behavior: TimecodeClipEnd::Release,
                }],
            },
        }],
    }
}

fn service() -> (
    TimecodeRuntimeService,
    Arc<ManualTimecodeClock>,
    Arc<RecordingPublisher>,
) {
    let clock = Arc::new(ManualTimecodeClock::default());
    let publisher = Arc::new(RecordingPublisher::default());
    (
        TimecodeRuntimeService::new(
            clock.clone(),
            publisher.clone(),
            TimecodeFrameRate::whole_frames(25).unwrap(),
        ),
        clock,
        publisher,
    )
}

#[test]
fn go_tick_pause_resume_seek_stop_and_rewind_are_authoritative() {
    let (service, clock, publisher) = service();
    let id = definition().id;
    service.install(definition(), None).unwrap();
    service.handle(id, TimecodeTransportAction::Go).unwrap();

    clock.advance(400_000);
    let changes = service.tick();
    assert_eq!(changes[0].snapshot.frame, TimecodeFrame(10));
    assert_eq!(changes[0].snapshot.reconstructed.cue_lists.len(), 1);

    let paused = service.handle(id, TimecodeTransportAction::Pause).unwrap();
    assert_eq!(paused.snapshot.transport, TimecodeTransportState::Paused);
    clock.advance(800_000);
    assert!(service.tick().is_empty());
    assert_eq!(service.snapshot(id).unwrap().frame, TimecodeFrame(10));

    service.handle(id, TimecodeTransportAction::Pause).unwrap();
    clock.advance(400_000);
    assert_eq!(service.tick()[0].snapshot.frame, TimecodeFrame(20));
    let seek = service
        .handle(
            id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(80),
            },
        )
        .unwrap();
    assert_eq!(seek.snapshot.frame, TimecodeFrame(80));
    assert_eq!(
        service
            .handle(id, TimecodeTransportAction::Stop)
            .unwrap()
            .snapshot
            .frame,
        TimecodeFrame::ZERO
    );
    assert_eq!(
        service
            .handle(id, TimecodeTransportAction::Rewind)
            .unwrap()
            .snapshot
            .transport,
        TimecodeTransportState::Playing
    );
    assert!(publisher.changes().len() >= 8);
}

#[test]
fn tick_loops_without_accumulating_clock_drift() {
    let (service, clock, _) = service();
    let id = definition().id;
    service.install(definition(), None).unwrap();
    service.handle(id, TimecodeTransportAction::Go).unwrap();

    clock.advance(4_400_000);
    let change = service.tick().pop().unwrap();
    assert_eq!(change.snapshot.frame, TimecodeFrame(10));
    assert_eq!(
        change.cause,
        TimecodeRuntimeChangeCause::Tick { completed_loops: 1 }
    );
    clock.advance(400_000);
    assert_eq!(service.tick()[0].snapshot.frame, TimecodeFrame(20));
}

#[test]
fn unchanged_actions_and_clock_subframes_do_not_publish_changes() {
    let (service, clock, publisher) = service();
    let id = definition().id;
    service.install(definition(), None).unwrap();
    let initial_events = publisher.changes().len();
    let pause = service.handle(id, TimecodeTransportAction::Pause).unwrap();
    assert!(!pause.changed);
    assert_eq!(publisher.changes().len(), initial_events);

    service.handle(id, TimecodeTransportAction::Go).unwrap();
    clock.advance(20_000);
    assert!(service.tick().is_empty());

    let unchanged_seek = service
        .handle(
            id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame::ZERO,
            },
        )
        .unwrap();
    assert!(!unchanged_seek.changed);
}

#[test]
fn go_at_zero_restarts_the_clock_and_is_an_observable_action() {
    let (service, clock, publisher) = service();
    let id = definition().id;
    service.install(definition(), None).unwrap();
    service.handle(id, TimecodeTransportAction::Go).unwrap();
    clock.advance(20_000);

    let restarted = service.handle(id, TimecodeTransportAction::Go).unwrap();
    assert!(restarted.changed);
    assert_eq!(restarted.snapshot.frame, TimecodeFrame::ZERO);
    clock.advance(20_000);
    assert!(service.tick().is_empty());
    assert_eq!(
        publisher.changes().last().unwrap().cause,
        TimecodeRuntimeChangeCause::Action(TimecodeTransportAction::Go)
    );
}

#[test]
fn installed_runtimes_tick_in_stable_timecode_id_order() {
    let (service, clock, _) = service();
    let mut later = definition();
    later.id = TimecodeId(Uuid::from_u128(2));
    let earlier = definition();
    service.install(later.clone(), None).unwrap();
    service.install(earlier.clone(), None).unwrap();
    service
        .handle(later.id, TimecodeTransportAction::Go)
        .unwrap();
    service
        .handle(earlier.id, TimecodeTransportAction::Go)
        .unwrap();
    clock.advance(400_000);

    let changes = service.tick();
    assert_eq!(changes[0].snapshot.timecode_id, earlier.id);
    assert_eq!(changes[1].snapshot.timecode_id, later.id);
}
