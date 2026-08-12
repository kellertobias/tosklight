use super::*;
use light_core::{CueListId, FixtureId};
use uuid::Uuid;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn definition() -> TimecodeDefinition {
    TimecodeDefinition {
        id: TimecodeId(id(1)),
        number: 1,
        name: "Opener".into(),
        duration: Some(TimecodeFrame(1_000)),
        transport_offset: TimecodeFrame::ZERO,
        auto_start: false,
        audio: None,
        markers: Vec::new(),
        lanes: vec![
            TimecodeLane {
                id: TimecodeLaneId(id(2)),
                name: "Opening".into(),
                content: TimecodeLaneContent::CueList {
                    cue_list_id: CueListId(id(3)),
                    clips: vec![
                        TimecodeCueListClip {
                            id: TimecodeClipId(id(4)),
                            start_frame: TimecodeFrame(100),
                            end_frame: TimecodeFrame(200),
                            start_cue_id: id(5),
                            end_cue_id: id(6),
                            start_behavior: TimecodeClipStart::State,
                            end_behavior: TimecodeClipEnd::Release,
                        },
                        TimecodeCueListClip {
                            id: TimecodeClipId(id(7)),
                            start_frame: TimecodeFrame(300),
                            end_frame: TimecodeFrame(400),
                            start_cue_id: id(8),
                            end_cue_id: id(9),
                            start_behavior: TimecodeClipStart::Cue,
                            end_behavior: TimecodeClipEnd::Hold,
                        },
                    ],
                },
            },
            TimecodeLane {
                id: TimecodeLaneId(id(10)),
                name: "Speed A".into(),
                content: TimecodeLaneContent::SpeedGroup {
                    group: "A".into(),
                    keyframes: vec![
                        TimecodeSpeedKeyframe {
                            id: TimecodeKeyframeId(id(11)),
                            frame: TimecodeFrame(100),
                            bpm: 120.0,
                            phase: 0.0,
                        },
                        TimecodeSpeedKeyframe {
                            id: TimecodeKeyframeId(id(12)),
                            frame: TimecodeFrame(100),
                            bpm: 90.0,
                            phase: 0.25,
                        },
                    ],
                },
            },
            TimecodeLane {
                id: TimecodeLaneId(id(13)),
                name: "Main audio".into(),
                content: TimecodeLaneContent::AudioVolume {
                    keyframes: vec![TimecodeVolumeKeyframe {
                        id: TimecodeKeyframeId(id(14)),
                        frame: TimecodeFrame(100),
                        value: 0.0,
                        fade_frames: 100,
                        curve: TimecodeCurve::Linear,
                    }],
                },
            },
        ],
    }
}

#[test]
fn rational_frame_rate_has_no_float_drift() {
    let rate = TimecodeFrameRate::new(30_000, 1_001, true).unwrap();
    assert_eq!(rate.frame_at_micros(1_001_000), TimecodeFrame(30));
    assert_eq!(
        TimecodeFrameRate::whole_frames(25).unwrap().convert(
            TimecodeFrame(125),
            TimecodeFrameRate::whole_frames(50).unwrap()
        ),
        TimecodeFrame(250)
    );
    assert!(TimecodeFrameRate::new(25, 1, true).is_err());
    assert!(TimecodeFrameRate::whole_frames(0).is_err());
}

#[test]
fn transport_offset_maps_to_the_zero_based_editor_timeline() {
    let mut timecode = definition();
    timecode.transport_offset = TimecodeFrame(90_000);
    assert_eq!(
        timecode.local_frame_at_transport(TimecodeFrame(90_044)),
        Some(TimecodeFrame(44))
    );
    assert_eq!(
        timecode.local_frame_at_transport(TimecodeFrame(89_999)),
        None
    );
}

#[test]
fn state_at_is_deterministic_for_seek_and_same_frame_values() {
    let timecode = definition();
    let first = timecode.state_at(TimecodeFrame(150));
    let seek = timecode.state_at(TimecodeFrame(150));

    assert_eq!(first, seek);
    assert_eq!(first.cue_lists.len(), 1);
    assert_eq!(first.cue_lists[0].kind, TimecodeCueListStateKind::Active);
    assert_eq!(first.speed_groups["A"].bpm, 90.0);
    assert_eq!(first.speed_groups["A"].phase, 0.25);
    assert_eq!(first.audio_volume, 0.5);
}

#[test]
fn audio_player_clip_reconstructs_source_cursor_repeat_and_volume() {
    let mut timecode = definition();
    let fixture_id = FixtureId(id(20));
    timecode.lanes.push(TimecodeLane {
        id: TimecodeLaneId(id(21)),
        name: "Atmosphere player".into(),
        content: TimecodeLaneContent::AudioPlayer {
            fixture_id,
            clips: vec![TimecodeAudioPlayerClip {
                id: TimecodeClipId(id(22)),
                start_frame: TimecodeFrame(100),
                end_frame: TimecodeFrame(300),
                folder: 1,
                file: 2,
                repeat: true,
                volume_keyframes: vec![TimecodeVolumeKeyframe {
                    id: TimecodeKeyframeId(id(23)),
                    frame: TimecodeFrame(100),
                    value: 0.0,
                    fade_frames: 100,
                    curve: TimecodeCurve::Linear,
                }],
            }],
        },
    });

    let state = timecode.state_at(TimecodeFrame(150));
    assert_eq!(state.audio_players.len(), 1);
    assert_eq!(state.audio_players[0].fixture_id, fixture_id);
    assert_eq!(state.audio_players[0].folder, 1);
    assert_eq!(state.audio_players[0].file, 2);
    assert!(state.audio_players[0].repeat);
    assert_eq!(state.audio_players[0].cursor_frame, TimecodeFrame(50));
    assert_eq!(state.audio_players[0].volume, 0.5);

    assert!(
        timecode
            .state_at(TimecodeFrame(300))
            .audio_players
            .is_empty()
    );
}

#[test]
fn same_frame_actions_keep_persisted_lane_and_keyframe_order() {
    let actions = definition().actions_at(TimecodeFrame(100));
    assert!(matches!(
        actions[0],
        TimecodeScheduledAction::CueListStart { .. }
    ));
    assert!(matches!(
        actions[1],
        TimecodeScheduledAction::SpeedGroup { bpm: 120.0, .. }
    ));
    assert!(matches!(
        actions[2],
        TimecodeScheduledAction::SpeedGroup { bpm: 90.0, .. }
    ));
    assert!(matches!(
        actions[3],
        TimecodeScheduledAction::AudioVolume { value: 0.0, .. }
    ));
}

#[test]
fn reconstruction_releases_or_holds_clips_and_clamps_to_duration() {
    let timecode = definition();
    let state = timecode.state_at(TimecodeFrame(2_000));

    assert_eq!(state.frame, TimecodeFrame(1_000));
    assert_eq!(state.cue_lists.len(), 1);
    assert_eq!(state.cue_lists[0].clip_id, TimecodeClipId(id(7)));
    assert_eq!(state.cue_lists[0].kind, TimecodeCueListStateKind::Held);
}

#[test]
fn clip_end_is_effective_at_its_frame_and_a_later_clip_supersedes_hold() {
    let mut timecode = definition();
    let TimecodeLaneContent::CueList { clips, .. } = &mut timecode.lanes[0].content else {
        panic!("expected Cuelist lane");
    };
    clips[0].end_behavior = TimecodeClipEnd::Hold;

    let at_first_end = timecode.state_at(TimecodeFrame(200));
    assert_eq!(at_first_end.cue_lists.len(), 1);
    assert_eq!(
        at_first_end.cue_lists[0].kind,
        TimecodeCueListStateKind::Held
    );

    let during_second = timecode.state_at(TimecodeFrame(350));
    assert_eq!(during_second.cue_lists.len(), 1);
    assert_eq!(during_second.cue_lists[0].clip_id, TimecodeClipId(id(7)));
    assert_eq!(
        during_second.cue_lists[0].kind,
        TimecodeCueListStateKind::Active
    );
}

#[test]
fn transport_actions_match_go_pause_stop_rewind_and_seek_contract() {
    let duration = TimecodeFrame(500);
    let playing = TimecodeTransport {
        state: TimecodeTransportState::Paused,
        frame: TimecodeFrame(300),
    }
    .apply(TimecodeTransportAction::Go, duration);
    assert_eq!(
        playing,
        TimecodeTransport {
            state: TimecodeTransportState::Playing,
            frame: TimecodeFrame::ZERO
        }
    );
    let paused = playing.apply(TimecodeTransportAction::Pause, duration);
    assert_eq!(paused.state, TimecodeTransportState::Paused);
    assert_eq!(
        paused.apply(TimecodeTransportAction::Pause, duration).state,
        TimecodeTransportState::Playing
    );
    assert_eq!(
        paused.apply(
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(900)
            },
            duration
        ),
        TimecodeTransport {
            state: TimecodeTransportState::Paused,
            frame: duration
        }
    );
    assert_eq!(
        paused.apply(TimecodeTransportAction::Stop, duration),
        TimecodeTransport::default()
    );
    assert_eq!(
        paused
            .apply(TimecodeTransportAction::Rewind, duration)
            .state,
        TimecodeTransportState::Playing
    );
}

#[test]
fn portable_definition_round_trips_without_losing_order() {
    let expected = definition();
    expected.validate().unwrap();
    let encoded = serde_json::to_value(&expected).unwrap();
    let decoded: TimecodeDefinition = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, expected);
}

#[test]
fn validation_rejects_duration_only_and_unordered_invalid_data() {
    let mut timecode = definition();
    timecode.duration = None;
    assert_eq!(
        timecode.validate().unwrap_err().message,
        "a Timecode without audio requires a configured duration"
    );

    let mut timecode = definition();
    let TimecodeLaneContent::SpeedGroup { keyframes, .. } = &mut timecode.lanes[1].content else {
        panic!("expected Speed Group lane");
    };
    keyframes[1].frame = TimecodeFrame(99);
    assert_eq!(
        timecode.validate().unwrap_err().message,
        "Speed Group keyframes must remain in persisted frame order"
    );
}

#[test]
fn validation_rejects_overlapping_cuelist_clips() {
    let mut timecode = definition();
    let TimecodeLaneContent::CueList { clips, .. } = &mut timecode.lanes[0].content else {
        panic!("expected Cuelist lane");
    };
    clips[1].start_frame = TimecodeFrame(199);

    assert_eq!(
        timecode.validate().unwrap_err().message,
        "Cuelist clips on one lane must not overlap"
    );
}
