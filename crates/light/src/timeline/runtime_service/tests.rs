use std::sync::{Arc, Mutex};

use light_core::CueListId;
use light_playback::{
    TimecodeClipEnd, TimecodeClipId, TimecodeClipStart, TimecodeCueListClip, TimecodeFrame,
    TimecodeFrameRate, TimecodeId, TimecodeLane, TimecodeLaneContent, TimecodeLaneId,
    TimecodeTransportAction, TimecodeTransportState,
};
use uuid::Uuid;

use super::*;
use crate::timeline::{
    TimecodeAudioCommand, TimecodeAudioOutput, TimecodeAudioService, WavEncoding, WavMetadata,
};

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

#[derive(Default)]
struct RecordingAudioOutput(Mutex<Vec<TimecodeAudioCommand>>);

impl TimecodeAudioOutput for RecordingAudioOutput {
    fn output_latency_micros(&self) -> u64 {
        5_000
    }

    fn apply(&self, command: TimecodeAudioCommand) -> Result<(), String> {
        self.0.lock().unwrap().push(command);
        Ok(())
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

#[test]
fn authoritative_transport_and_ticks_drive_prepared_audio() {
    let clock = Arc::new(ManualTimecodeClock::default());
    let output = Arc::new(RecordingAudioOutput::default());
    let audio = Arc::new(TimecodeAudioService::new(output.clone()));
    let rate = TimecodeFrameRate::whole_frames(25).unwrap();
    let id = definition().id;
    audio
        .prepare(
            id,
            crate::AssetReference {
                id: crate::AssetId(Uuid::from_u128(90)),
                revision: crate::AssetRevision(1),
            },
            WavMetadata {
                encoding: WavEncoding::PcmInteger,
                channels: 2,
                sample_rate: 48_000,
                bits_per_sample: 16,
                data_bytes: 192_000,
                sample_frames: 48_000,
            },
            rate,
            true,
        )
        .unwrap();
    let service =
        TimecodeRuntimeService::new(clock.clone(), Arc::new(RecordingPublisher::default()), rate)
            .with_audio(audio);
    service
        .install(definition(), Some(TimecodeFrame(25)))
        .unwrap();

    service.handle(id, TimecodeTransportAction::Go).unwrap();
    clock.advance(40_000);
    service.tick();
    service
        .handle(
            id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(12),
            },
        )
        .unwrap();

    let commands = output.0.lock().unwrap();
    assert!(commands.contains(&TimecodeAudioCommand::Play {
        timecode_id: id,
        source_frame: TimecodeFrame::ZERO,
        audible_at_micros: 5_000,
    }));
    assert!(commands.contains(&TimecodeAudioCommand::Seek {
        timecode_id: id,
        source_frame: TimecodeFrame(1),
        audible_at_micros: 45_000,
    }));
    assert!(commands.contains(&TimecodeAudioCommand::Seek {
        timecode_id: id,
        source_frame: TimecodeFrame(12),
        audible_at_micros: 45_000,
    }));
}

#[test]
fn duration_only_timecode_ignores_available_audio_output() {
    let clock = Arc::new(ManualTimecodeClock::default());
    let output = Arc::new(RecordingAudioOutput::default());
    let audio = Arc::new(TimecodeAudioService::new(output.clone()));
    let rate = TimecodeFrameRate::whole_frames(25).unwrap();
    let definition = definition();
    let id = definition.id;
    let service =
        TimecodeRuntimeService::new(clock.clone(), Arc::new(RecordingPublisher::default()), rate)
            .with_audio(audio);
    service.install(definition, None).unwrap();

    service.handle(id, TimecodeTransportAction::Go).unwrap();
    clock.advance(40_000);
    service.tick();

    assert!(output.0.lock().unwrap().is_empty());
    assert!(!service.snapshot(id).unwrap().audio_linked);
}

#[test]
fn external_sync_applies_offsets_and_only_starts_armed_or_running_timecodes() {
    let (service, _clock, publisher) = service();
    let mut armed = definition();
    armed.auto_start = true;
    armed.transport_offset = TimecodeFrame(50);
    let mut inactive = definition();
    inactive.id = TimecodeId(Uuid::from_u128(20));
    inactive.auto_start = false;
    inactive.transport_offset = TimecodeFrame(10);
    let mut running = definition();
    running.id = TimecodeId(Uuid::from_u128(30));
    running.auto_start = false;
    running.transport_offset = TimecodeFrame(10);
    service.install(armed.clone(), None).unwrap();
    service.install(inactive.clone(), None).unwrap();
    service.install(running.clone(), None).unwrap();
    service
        .handle(running.id, TimecodeTransportAction::Go)
        .unwrap();

    let before_offset = service.synchronize_external(TimecodeFrame(49));
    assert_eq!(before_offset.len(), 1);
    assert_eq!(
        service.snapshot(running.id).unwrap().frame,
        TimecodeFrame(39)
    );
    assert_eq!(
        service.snapshot(armed.id).unwrap().transport,
        TimecodeTransportState::Stopped
    );
    let changes = service.synchronize_external(TimecodeFrame(50));
    assert_eq!(changes.len(), 2);
    assert_eq!(
        service.snapshot(armed.id).unwrap().transport,
        TimecodeTransportState::Playing
    );
    assert_eq!(
        service.snapshot(armed.id).unwrap().frame,
        TimecodeFrame::ZERO
    );
    assert_eq!(
        service.snapshot(running.id).unwrap().frame,
        TimecodeFrame(40)
    );
    assert_eq!(
        service.snapshot(inactive.id).unwrap().transport,
        TimecodeTransportState::Stopped
    );
    assert!(publisher.changes().iter().any(|change| {
        change.cause
            == TimecodeRuntimeChangeCause::ExternalSync {
                transport_frame: TimecodeFrame(50),
            }
            && change.snapshot.timecode_id == armed.id
    }));
}
