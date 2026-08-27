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
                            cue_starts: Vec::new(),
                            in_fade_frames: 0,
                            out_fade_frames: 0,
                        },
                        TimecodeCueListClip {
                            id: TimecodeClipId(id(7)),
                            start_frame: TimecodeFrame(300),
                            end_frame: TimecodeFrame(400),
                            start_cue_id: id(8),
                            end_cue_id: id(9),
                            start_behavior: TimecodeClipStart::Cue,
                            end_behavior: TimecodeClipEnd::Hold,
                            cue_starts: Vec::new(),
                            in_fade_frames: 0,
                            out_fade_frames: 0,
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

#[test]
fn validation_accepts_non_overlapping_cuelist_clips_in_any_persisted_order() {
    let mut timecode = definition();
    let TimecodeLaneContent::CueList { clips, .. } = &mut timecode.lanes[0].content else {
        panic!("expected Cuelist lane");
    };
    clips.reverse();

    timecode.validate().unwrap();
}

fn execution_cue_list() -> crate::CueList {
    let mut cues = (1..=4)
        .map(|number| {
            crate::Cue::new(crate::CueNumber::try_from_legacy_f64(f64::from(number)).unwrap())
        })
        .collect::<Vec<_>>();
    cues[0].fade_millis = 100;
    cues[0].trigger = crate::CueTrigger::Manual;
    cues[1].fade_millis = 100;
    cues[1].trigger = crate::CueTrigger::Follow { delay_millis: 100 };
    cues[2].fade_millis = 100;
    cues[2].trigger = crate::CueTrigger::Wait { delay_millis: 200 };
    cues[3].trigger = crate::CueTrigger::Follow { delay_millis: 0 };
    crate::CueList {
        id: CueListId(id(30)),
        name: "Timeline".into(),
        priority: 0,
        mode: crate::CueListMode::Sequence,
        looped: false,
        intensity_priority_mode: crate::IntensityPriorityMode::Htp,
        wrap_mode: Some(crate::WrapMode::Off),
        restart_mode: crate::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_step_millis: 1_000,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_group: None,
        speed_multiplier: 1.0,
        cues,
    }
}

fn execution_definition(cue_list: &crate::CueList) -> TimecodeDefinition {
    TimecodeDefinition {
        id: TimecodeId(id(31)),
        number: 31,
        name: "Timeline".into(),
        duration: Some(TimecodeFrame(300)),
        transport_offset: TimecodeFrame::ZERO,
        auto_start: false,
        audio: None,
        markers: vec![],
        lanes: vec![TimecodeLane {
            id: TimecodeLaneId(id(32)),
            name: "Cues".into(),
            content: TimecodeLaneContent::CueList {
                cue_list_id: cue_list.id,
                clips: vec![TimecodeCueListClip {
                    id: TimecodeClipId(id(33)),
                    start_frame: TimecodeFrame(100),
                    end_frame: TimecodeFrame(200),
                    start_cue_id: cue_list.cues[0].id,
                    end_cue_id: cue_list.cues[3].id,
                    start_behavior: TimecodeClipStart::State,
                    end_behavior: TimecodeClipEnd::Release,
                    cue_starts: Vec::new(),
                    in_fade_frames: 0,
                    out_fade_frames: 0,
                }],
            },
        }],
    }
}

fn execution_at(
    definition: &TimecodeDefinition,
    cue_list: &crate::CueList,
    frame: u64,
) -> TimecodeCueListClipExecution {
    definition
        .cue_list_clip_execution(
            std::slice::from_ref(cue_list),
            TimecodeFrame(frame),
            TimecodeTransportState::Playing,
            TimecodeCueTimingDefaults {
                frame_rate: TimecodeFrameRate::whole_frames(44).unwrap(),
                sequence_fade_millis: 0,
                release_fade_millis: 0,
            },
        )
        .remove(0)
}

#[test]
fn cuelist_clip_seek_matches_follow_and_wait_schedule() {
    let cue_list = execution_cue_list();
    let definition = execution_definition(&cue_list);
    assert_eq!(
        execution_at(&definition, &cue_list, 107).cue_id,
        Some(cue_list.cues[0].id)
    );
    assert_eq!(
        execution_at(&definition, &cue_list, 108).cue_id,
        Some(cue_list.cues[1].id)
    );
    assert_eq!(
        execution_at(&definition, &cue_list, 117).cue_id,
        Some(cue_list.cues[2].id)
    );
    assert_eq!(
        execution_at(&definition, &cue_list, 121).cue_id,
        Some(cue_list.cues[3].id)
    );
}

#[test]
fn a_clip_that_ends_early_simply_stops_playing_its_later_cues() {
    // Shortening a clip is an ordinary edit. The Cues past the new end stop being reached, and
    // the clip cuts where the operator put it — it used to refuse to run at all and report
    // "Cue N starts outside the clip".
    let cue_list = execution_cue_list();
    let mut definition = execution_definition(&cue_list);
    if let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content {
        clips[0].end_frame = TimecodeFrame(118);
    }

    // The Cues inside the shortened clip still play, in their own places.
    assert_eq!(
        execution_at(&definition, &cue_list, 107).cue_id,
        Some(cue_list.cues[0].id)
    );
    assert_eq!(
        execution_at(&definition, &cue_list, 117).cue_id,
        Some(cue_list.cues[2].id)
    );

    // The Cue that would have started at 121 is simply never reached, and nothing about the clip
    // is reported as unable.
    let cut = execution_at(&definition, &cue_list, 119);
    assert_ne!(cut.cue_id, Some(cue_list.cues[3].id));
    assert_ne!(cut.kind, TimecodeCueListClipExecutionKind::Unable);
}

#[test]
fn cuelist_clip_seek_follows_a_stable_link_destination() {
    let mut cue_list = execution_cue_list();
    let destination = cue_list.cues[3].id;
    cue_list.cues[0].trigger = crate::CueTrigger::Link {
        cue_id: destination,
        delay_millis: 100,
    };
    let definition = execution_definition(&cue_list);

    assert_eq!(
        execution_at(&definition, &cue_list, 107).cue_id,
        Some(cue_list.cues[0].id)
    );
    assert_eq!(
        execution_at(&definition, &cue_list, 108).cue_id,
        Some(destination)
    );
}

#[test]
fn manual_go_cue_follows_its_predecessor_until_the_lane_places_a_transition() {
    let mut cue_list = execution_cue_list();
    cue_list.cues[1].trigger = crate::CueTrigger::Manual;
    let mut definition = execution_definition(&cue_list);
    // Cue 1 completes its 100 ms fade four frames after the clip start, so without a
    // placed transition the manual Cue 2 takes over there.
    let followed = execution_at(&definition, &cue_list, 104);
    assert_eq!(followed.kind, TimecodeCueListClipExecutionKind::Active);
    assert_eq!(followed.cue_id, Some(cue_list.cues[1].id));
    assert_eq!(
        execution_at(&definition, &cue_list, 103).cue_id,
        Some(cue_list.cues[0].id)
    );

    let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content else {
        unreachable!()
    };
    clips[0].cue_starts = vec![TimecodeCueStart {
        cue_id: cue_list.cues[1].id,
        offset_frame: TimecodeFrame(30),
    }];
    assert_eq!(
        execution_at(&definition, &cue_list, 129).cue_id,
        Some(cue_list.cues[0].id)
    );
    assert_eq!(
        execution_at(&definition, &cue_list, 130).cue_id,
        Some(cue_list.cues[1].id)
    );
}

#[test]
fn cuelist_clip_reports_a_missing_cue_as_unable() {
    let cue_list = execution_cue_list();
    let mut definition = execution_definition(&cue_list);
    let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content else {
        unreachable!()
    };
    clips[0].start_cue_id = Uuid::new_v4();
    let missing = execution_at(&definition, &cue_list, 100);
    assert_eq!(missing.kind, TimecodeCueListClipExecutionKind::Unable);
    assert_eq!(missing.message.as_deref(), Some("start Cue does not exist"));
}

#[test]
fn valid_unsorted_clips_reconstruct_in_frame_order_and_zero_length_is_rejected() {
    let mut timecode = definition();
    let first_start = {
        let TimecodeLaneContent::CueList { clips, .. } = &mut timecode.lanes[0].content else {
            unreachable!()
        };
        clips.reverse();
        clips[0].start_frame
    };
    assert_eq!(
        timecode.state_at(TimecodeFrame(150)).cue_lists[0].clip_id,
        TimecodeClipId(id(4))
    );
    let TimecodeLaneContent::CueList { clips, .. } = &mut timecode.lanes[0].content else {
        unreachable!()
    };
    clips[0].end_frame = first_start;
    assert_eq!(
        timecode.validate().unwrap_err().message,
        "Cuelist clip end must follow its start"
    );
}

fn faded_clip(in_fade: u64, out_fade: u64) -> TimecodeCueListClip {
    TimecodeCueListClip {
        id: TimecodeClipId(id(4)),
        start_frame: TimecodeFrame(100),
        end_frame: TimecodeFrame(200),
        start_cue_id: id(5),
        end_cue_id: id(6),
        start_behavior: TimecodeClipStart::State,
        end_behavior: TimecodeClipEnd::Release,
        cue_starts: Vec::new(),
        in_fade_frames: in_fade,
        out_fade_frames: out_fade,
    }
}

#[test]
fn a_clip_without_fades_contributes_fully_from_its_first_frame() {
    let clip = faded_clip(0, 0);
    assert_eq!(clip.level_at(TimecodeFrame(99)), 0.0);
    assert_eq!(clip.level_at(TimecodeFrame(100)), 1.0);
    assert_eq!(clip.level_at(TimecodeFrame(200)), 1.0);
    assert_eq!(clip.level_at(TimecodeFrame(201)), 0.0);
}

#[test]
fn clip_fades_ramp_from_the_start_and_back_from_the_end() {
    let clip = faded_clip(20, 40);
    assert_eq!(clip.level_at(TimecodeFrame(100)), 0.0);
    assert_eq!(clip.level_at(TimecodeFrame(110)), 0.5);
    assert_eq!(clip.level_at(TimecodeFrame(120)), 1.0);
    assert_eq!(clip.level_at(TimecodeFrame(150)), 1.0);
    assert_eq!(clip.level_at(TimecodeFrame(180)), 0.5);
    assert_eq!(clip.level_at(TimecodeFrame(200)), 0.0);
}

#[test]
fn overlapping_clip_fades_take_the_lower_of_the_two() {
    // Both fades cover the whole clip, so they cross in the middle rather than fighting.
    let clip = faded_clip(100, 100);
    assert_eq!(clip.level_at(TimecodeFrame(125)), 0.25);
    assert_eq!(clip.level_at(TimecodeFrame(150)), 0.5);
    assert_eq!(clip.level_at(TimecodeFrame(175)), 0.25);
}

#[test]
fn a_clip_reports_its_faded_level_while_it_is_executing() {
    let cue_list = execution_cue_list();
    let mut definition = execution_definition(&cue_list);
    let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content else {
        panic!("the lane drives a Cuelist");
    };
    clips[0].in_fade_frames = 20;
    clips[0].out_fade_frames = 20;
    assert_eq!(execution_at(&definition, &cue_list, 90).level, 0.0);
    assert_eq!(execution_at(&definition, &cue_list, 110).level, 0.5);
    assert_eq!(execution_at(&definition, &cue_list, 150).level, 1.0);
    assert_eq!(execution_at(&definition, &cue_list, 190).level, 0.5);
}

#[test]
fn a_clip_fade_longer_than_the_clip_is_refused() {
    let mut definition = definition();
    let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content else {
        panic!("the first lane drives a Cuelist");
    };
    clips[0].out_fade_frames = 101;
    assert_eq!(
        definition.validate().unwrap_err().message,
        "Cuelist clip fades must fit within the clip"
    );
}
