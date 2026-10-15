use super::*;

fn decoded(samples: &[f32]) -> DecodedWav {
    DecodedWav {
        samples: samples.to_vec(),
        sample_rate: 1,
        channels: 1,
    }
}

fn fixture(value: u128) -> FixtureId {
    FixtureId(uuid::Uuid::from_u128(value))
}

#[test]
fn stereo_routes_to_one_output_pair_and_mono_retains_both_sides() {
    for (outputs, expected) in [
        (4, vec![0.75, -0.25, 0.0, 0.0]),
        (2, vec![0.75, -0.25]),
        (1, vec![0.25]),
    ] {
        let mut voice = Voice::new(
            DecodedWav {
                samples: vec![0.75, -0.25],
                sample_rate: 48_000,
                channels: 2,
            },
            TimecodeFrameRate::whole_frames(25).unwrap(),
            48_000,
            outputs,
        );
        voice.playing = true;
        let mut output = vec![0.0; outputs];
        voice.mix_frame(&mut output, 0);
        assert_eq!(output, expected);
    }
    let mut voice = Voice::new(
        decoded(&[0.5]),
        TimecodeFrameRate::whole_frames(1).unwrap(),
        1,
        4,
    );
    voice.playing = true;
    let mut output = [0.0; 4];
    voice.mix_frame(&mut output, 0);
    assert_eq!(output, [0.5, 0.5, 0.0, 0.0]);
}

#[test]
fn volume_set_before_play_never_leaks_a_full_volume_start() {
    let mut voice = Voice::new(
        decoded(&[1.0]),
        TimecodeFrameRate::whole_frames(1).unwrap(),
        48_000,
        2,
    );
    voice.set_volume(0.0);
    voice.playing = true;
    let mut output = [0.0; 2];
    voice.mix_frame(&mut output, 0);
    assert_eq!(output, [0.0, 0.0]);
}

#[test]
fn queued_controls_preserve_audio_cursor_and_apply_linear_gain_without_gaps() {
    let queue = AudioCommandQueue::default();
    let mut voices = DeviceVoices::default();
    let id = TimecodeId(uuid::Uuid::from_u128(999));
    let apply = |command, voices: &mut DeviceVoices| {
        assert!(queue.pending.push(command).is_ok());
        queue.apply_pending(voices, 1_000, 2);
        queue.completed.pop().unwrap().unwrap();
    };
    apply(
        NativeCommand::Prepare {
            timecode_id: id,
            decoded: DecodedWav {
                samples: vec![1.0; 20],
                sample_rate: 1_000,
                channels: 1,
            },
            timeline_rate: TimecodeFrameRate::whole_frames(1_000).unwrap(),
        },
        &mut voices,
    );
    apply(
        NativeCommand::Transport(TimecodeAudioCommand::Play {
            timecode_id: id,
            source_frame: TimecodeFrame::ZERO,
            audible_at_micros: 0,
        }),
        &mut voices,
    );
    for _ in 0..3 {
        let mut output = [0.0; 2];
        voices
            .timecodes
            .get_mut(&id)
            .unwrap()
            .mix_frame(&mut output, 0);
        assert_eq!(output, [1.0, 1.0]);
    }
    apply(
        NativeCommand::Transport(TimecodeAudioCommand::SetVolume {
            timecode_id: id,
            linear: 0.5,
        }),
        &mut voices,
    );
    assert_eq!(voices.timecodes[&id].position, 3.0);
    for expected in [0.9, 0.8, 0.7, 0.6, 0.5] {
        let mut output = [0.0; 2];
        voices
            .timecodes
            .get_mut(&id)
            .unwrap()
            .mix_frame(&mut output, 0);
        assert!((output[0] - expected).abs() < 1e-6);
        assert_eq!(output[0], output[1]);
    }
    assert_eq!(voices.timecodes[&id].position, 8.0);
    apply(
        NativeCommand::Transport(TimecodeAudioCommand::Seek {
            timecode_id: id,
            source_frame: TimecodeFrame(12),
            audible_at_micros: 0,
        }),
        &mut voices,
    );
    assert_eq!(voices.timecodes[&id].position, 12.0);
    apply(
        NativeCommand::Transport(TimecodeAudioCommand::Stop { timecode_id: id }),
        &mut voices,
    );
    assert!(!voices.timecodes[&id].playing);
    assert_eq!(voices.timecodes[&id].position, 0.0);
}

#[test]
fn timecode_loops_at_timeline_end_with_silence_after_a_short_asset() {
    let id = TimecodeId(uuid::Uuid::from_u128(1));
    let mut voices = DeviceVoices::default();
    apply_native(
        &mut voices,
        NativeCommand::Prepare {
            timecode_id: id,
            decoded: decoded(&[1.0, 0.5]),
            timeline_rate: TimecodeFrameRate::whole_frames(1).unwrap(),
        },
        1,
        1,
    )
    .unwrap();
    apply_native(
        &mut voices,
        NativeCommand::Transport(TimecodeAudioCommand::SetLoop {
            timecode_id: id,
            enabled: true,
            end_exclusive: TimecodeFrame(4),
        }),
        1,
        1,
    )
    .unwrap();
    apply_native(
        &mut voices,
        NativeCommand::Transport(TimecodeAudioCommand::Play {
            timecode_id: id,
            source_frame: TimecodeFrame::ZERO,
            audible_at_micros: 0,
        }),
        1,
        1,
    )
    .unwrap();
    let voice = voices.timecodes.get_mut(&id).unwrap();
    let mut mixed = Vec::new();
    for _ in 0..5 {
        let mut output = [0.0];
        voice.mix_frame(&mut output, 0);
        mixed.push(output[0]);
    }
    assert_eq!(mixed, vec![1.0, 0.5, 0.0, 0.0, 1.0]);
    voice.seek(TimecodeFrame(3));
    assert_eq!(voice.position, 3.0);
    let mut output = [0.0];
    voice.mix_frame(&mut output, 0);
    assert_eq!(output, [0.0]);
    // Shortening a timeline trims the asset and wraps at the new frame boundary.
    voice.loop_end = 1;
    voice.position = 0.0;
    for _ in 0..3 {
        let mut output = [0.0];
        voice.mix_frame(&mut output, 0);
        assert_eq!(output, [1.0]);
    }
}

#[test]
fn the_volume_control_closes_to_silence_and_opens_to_unity_gain() {
    assert_eq!(audio_gain(0.0), 0.0);
    assert_eq!(audio_gain(1.0), 1.0);
    assert_eq!(audio_gain(-0.5), 0.0);
    assert_eq!(audio_gain(2.0), 1.0);
    assert_eq!(audio_gain(f32::NAN), 0.0);
}

#[test]
fn the_control_rises_without_reversing() {
    let mut previous = audio_gain(0.0);
    for step in 1..=100 {
        let gain = audio_gain(step as f32 / 100.0);
        assert!(
            gain > previous,
            "gain must increase with the control at step {step}: {gain} <= {previous}"
        );
        previous = gain;
    }
}

#[test]
fn latency_trim_is_signed_and_saturating() {
    assert_eq!(add_signed(10_000, 2_500), 12_500);
    assert_eq!(add_signed(10_000, -2_500), 7_500);
    assert_eq!(add_signed(1_000, -2_500), 0);
}

#[test]
fn startup_wait_reports_timeout_without_blocking_server_startup() {
    let (_sender, receiver) = std::sync::mpsc::channel::<()>();

    assert_eq!(
        receive_startup(
            &receiver,
            Duration::ZERO,
            "startup timed out",
            "startup disconnected",
        ),
        Err("startup timed out".to_owned())
    );
}

#[test]
fn startup_wait_distinguishes_a_stopped_worker() {
    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    drop(sender);

    assert_eq!(
        receive_startup(
            &receiver,
            Duration::from_secs(1),
            "startup timed out",
            "startup disconnected",
        ),
        Err("startup disconnected".to_owned())
    );
}

#[test]
fn a_voice_resampling_to_the_device_interpolates_between_recorded_frames() {
    // A 44.1 kHz track on a 48 kHz device lands the play head between two recorded frames on
    // most output frames. Repeating the nearer frame instead of reading through the pair
    // rasps over the whole track, so the ramp below has to come out as a ramp.
    let mut voices = DeviceVoices::default();
    apply_native(
        &mut voices,
        NativeCommand::PrepareInternal {
            fixture_id: fixture(1),
            decoded: decoded(&[0.0, 1.0]),
        },
        2,
        1,
    )
    .unwrap();
    apply_native(
        &mut voices,
        NativeCommand::InternalTransport {
            fixture_id: fixture(1),
            action: NativeInternalTransport::Play,
        },
        2,
        1,
    )
    .unwrap();

    let voice = voices
        .internal
        .get_mut(&fixture(1))
        .expect("prepared voice");
    let mut mixed = Vec::new();
    for _ in 0..3 {
        let mut output = [0.0];
        voice.mix_frame(&mut output, 0);
        mixed.push(output[0]);
    }

    assert_eq!(mixed, vec![0.0, 0.5, 1.0]);
}

#[test]
fn internal_players_mix_without_voice_stealing() {
    let mut voices = DeviceVoices::default();
    for (fixture_id, samples) in [(fixture(1), &[0.25][..]), (fixture(2), &[0.5][..])] {
        apply_native(
            &mut voices,
            NativeCommand::PrepareInternal {
                fixture_id,
                decoded: decoded(samples),
            },
            1,
            1,
        )
        .unwrap();
        apply_native(
            &mut voices,
            NativeCommand::InternalTransport {
                fixture_id,
                action: NativeInternalTransport::Play,
            },
            1,
            1,
        )
        .unwrap();
    }

    let mut output = [0.0];
    for voice in voices.internal.values_mut() {
        voice.mix_frame(&mut output, 0);
    }

    assert_eq!(voices.internal.len(), 2);
    assert_eq!(output, [0.75]);
}

#[test]
fn internal_transport_resets_on_stop_and_non_repeating_end() {
    let fixture_id = fixture(3);
    let mut voices = DeviceVoices::default();
    apply_native(
        &mut voices,
        NativeCommand::PrepareInternal {
            fixture_id,
            decoded: decoded(&[0.5]),
        },
        1,
        1,
    )
    .unwrap();
    apply_native(
        &mut voices,
        NativeCommand::InternalTransport {
            fixture_id,
            action: NativeInternalTransport::Play,
        },
        1,
        1,
    )
    .unwrap();

    let voice = voices.internal.get_mut(&fixture_id).unwrap();
    voice.mix_frame(&mut [0.0], 0);
    voice.mix_frame(&mut [0.0], 1);
    assert!(!voice.playing);
    assert_eq!(voice.position, 0.0);

    voice.position = 1.0;
    voice.playing = true;
    apply_native(
        &mut voices,
        NativeCommand::InternalTransport {
            fixture_id,
            action: NativeInternalTransport::Stop,
        },
        1,
        1,
    )
    .unwrap();
    let voice = &voices.internal[&fixture_id];
    assert!(!voice.playing);
    assert_eq!(voice.position, 0.0);
}

#[test]
fn restart_play_is_an_edge_action_and_repeat_wraps() {
    let fixture_id = fixture(4);
    let mut voices = DeviceVoices::default();
    apply_native(
        &mut voices,
        NativeCommand::PrepareInternal {
            fixture_id,
            decoded: decoded(&[0.25, 0.75]),
        },
        1,
        1,
    )
    .unwrap();
    apply_native(
        &mut voices,
        NativeCommand::InternalRepeat {
            fixture_id,
            enabled: true,
        },
        1,
        1,
    )
    .unwrap();
    apply_native(
        &mut voices,
        NativeCommand::InternalTransport {
            fixture_id,
            action: NativeInternalTransport::RestartPlay,
        },
        1,
        1,
    )
    .unwrap();
    let voice = voices.internal.get_mut(&fixture_id).unwrap();
    let mut first = [0.0];
    voice.mix_frame(&mut first, 0);
    voice.mix_frame(&mut [0.0], 1);
    let mut wrapped = [0.0];
    voice.mix_frame(&mut wrapped, 2);
    assert_eq!(first, [0.25]);
    assert_eq!(wrapped, [0.25]);

    apply_native(
        &mut voices,
        NativeCommand::InternalTransport {
            fixture_id,
            action: NativeInternalTransport::RestartPlay,
        },
        1,
        1,
    )
    .unwrap();
    assert_eq!(voices.internal[&fixture_id].position, 0.0);
}

#[cfg(unix)]
#[test]
fn failed_device_probe_is_reported_without_terminating_the_server_process() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "kill -SEGV $$"]);

    let error = output_devices_from_command(&mut command).unwrap_err();

    assert!(error.contains("stopped unexpectedly"), "{error}");
}

#[cfg(unix)]
#[test]
fn malformed_device_probe_output_is_actionable() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf not-json"]);

    let error = output_devices_from_command(&mut command).unwrap_err();

    assert!(error.contains("returned invalid data"), "{error}");
}

#[test]
fn transport_batch_rejects_a_late_missing_voice_or_invalid_value_without_earlier_mutation() {
    let id = TimecodeId(uuid::Uuid::from_u128(796));
    let missing = TimecodeId(uuid::Uuid::from_u128(797));
    let mut voices = DeviceVoices::default();
    apply_native(
        &mut voices,
        NativeCommand::Prepare {
            timecode_id: id,
            decoded: decoded(&[1.0; 16]),
            timeline_rate: TimecodeFrameRate::whole_frames(1).unwrap(),
        },
        1,
        1,
    )
    .unwrap();
    for invalid in [
        TimecodeAudioCommand::Stop {
            timecode_id: missing,
        },
        TimecodeAudioCommand::SetVolume {
            timecode_id: id,
            linear: f64::NAN,
        },
    ] {
        let error = apply_native(
            &mut voices,
            prepared_transport_commands(vec![
                TimecodeAudioCommand::Play {
                    timecode_id: id,
                    source_frame: TimecodeFrame(3),
                    audible_at_micros: 123,
                },
                invalid,
            ]),
            1,
            1,
        )
        .unwrap_err();
        assert!(!error.is_empty());
        let voice = &voices.timecodes[&id];
        assert!(!voice.playing);
        assert_eq!(voice.position, 0.0);
        assert_eq!(voice.play_at_micros, None);
        assert_eq!(voice.target_volume, 1.0);
    }
    apply_native(
        &mut voices,
        prepared_transport_commands(vec![
            TimecodeAudioCommand::SetLoop {
                timecode_id: id,
                enabled: true,
                end_exclusive: TimecodeFrame(10),
            },
            TimecodeAudioCommand::SetVolume {
                timecode_id: id,
                linear: 0.4,
            },
            TimecodeAudioCommand::Play {
                timecode_id: id,
                source_frame: TimecodeFrame(3),
                audible_at_micros: 123,
            },
            TimecodeAudioCommand::Pause { timecode_id: id },
            TimecodeAudioCommand::Seek {
                timecode_id: id,
                source_frame: TimecodeFrame(5),
                audible_at_micros: 456,
            },
        ]),
        1,
        1,
    )
    .unwrap();
    let voice = &voices.timecodes[&id];
    assert!(!voice.playing);
    assert_eq!(voice.position, 5.0);
    assert_eq!(voice.play_at_micros, None);
    assert!(voice.looping);
    assert_eq!(voice.loop_end, 10);
    assert_eq!(voice.target_volume, 0.4);
    apply_native(
        &mut voices,
        prepared_transport_commands(vec![TimecodeAudioCommand::Stop { timecode_id: id }]),
        1,
        1,
    )
    .unwrap();
    assert_eq!(voices.timecodes[&id].position, 0.0);
}

#[test]
fn cancelled_batch_cannot_apply_late_or_acknowledge_the_next_request() {
    let queue = AudioCommandQueue::default();
    let id = TimecodeId(uuid::Uuid::from_u128(798));
    let mut voices = DeviceVoices::default();
    let error = queue
        .submit_batch(
            vec![
                NativeCommand::Prepare {
                    timecode_id: id,
                    decoded: decoded(&[1.0; 16]),
                    timeline_rate: TimecodeFrameRate::whole_frames(1).unwrap(),
                },
                NativeCommand::Transport(TimecodeAudioCommand::Play {
                    timecode_id: id,
                    source_frame: TimecodeFrame::ZERO,
                    audible_at_micros: 0,
                }),
            ],
            Duration::ZERO,
        )
        .unwrap_err();
    assert!(error.contains("stopped consuming"));
    // Deliberately resume the callback after refusal. It must discard the cancelled request.
    queue.apply_pending(&mut voices, 1, 1);
    assert!(voices.timecodes.is_empty());
    assert!(queue.completed.pop().is_none());
    let receipt = Arc::new(AudioBatchReceipt::new());
    queue
        .pending
        .push(NativeCommand::CorrelatedBatch {
            receipt: receipt.clone(),
            commands: vec![
                NativeCommand::Prepare {
                    timecode_id: id,
                    decoded: decoded(&[1.0; 16]),
                    timeline_rate: TimecodeFrameRate::whole_frames(1).unwrap(),
                },
                NativeCommand::Transport(TimecodeAudioCommand::Play {
                    timecode_id: id,
                    source_frame: TimecodeFrame(4),
                    audible_at_micros: 9,
                }),
            ],
        })
        .ok()
        .unwrap();
    assert!(receipt.completed.pop().is_none());
    queue.apply_pending(&mut voices, 1, 1);
    assert_eq!(receipt.completed.pop(), Some(Ok(())));
    assert!(voices.timecodes[&id].playing);
    assert_eq!(voices.timecodes[&id].position, 4.0);
    assert_eq!(voices.timecodes[&id].play_at_micros, Some(9));
    assert!(queue.completed.pop().is_none());
    assert!(
        !receipt.cancel_pending(),
        "an acknowledged batch can never become a refused request"
    );
}

#[test]
fn a_late_invalid_cold_voice_batch_cannot_install_the_earlier_prepared_voice() {
    let id = TimecodeId(uuid::Uuid::from_u128(799));
    let mut voices = DeviceVoices::default();
    let result = apply_native(
        &mut voices,
        NativeCommand::PreparedBatch(vec![
            NativeCommand::PreparedVoice {
                timecode_id: id,
                voice: Voice::new(
                    decoded(&[1.0; 16]),
                    TimecodeFrameRate::whole_frames(1).unwrap(),
                    1,
                    1,
                ),
            },
            NativeCommand::Transport(TimecodeAudioCommand::Play {
                timecode_id: TimecodeId(uuid::Uuid::from_u128(800)),
                source_frame: TimecodeFrame::ZERO,
                audible_at_micros: 0,
            }),
        ]),
        1,
        1,
    );
    assert!(result.is_err());
    assert!(voices.timecodes.is_empty());
}

fn prepared_transport_commands(commands: Vec<TimecodeAudioCommand>) -> NativeCommand {
    NativeCommand::PreparedBatch(commands.into_iter().map(NativeCommand::Transport).collect())
}

#[test]
fn decoded_token_rejects_changed_or_unused_resources_before_any_voice_install() {
    let id = TimecodeId(uuid::Uuid::from_u128(801));
    let prepare = TimecodeAudioCommand::Prepare {
        timecode_id: id,
        asset: AssetReference {
            id: light_application::AssetId(uuid::Uuid::from_u128(802)),
            revision: light_application::AssetRevision(1),
        },
        sample_rate: 1,
        channels: 1,
        timeline_rate: TimecodeFrameRate::whole_frames(1).unwrap(),
    };
    let resource = || {
        vec![(
            prepare,
            NativeCommand::PreparedVoice {
                timecode_id: id,
                voice: Voice::new(
                    decoded(&[1.0; 16]),
                    TimecodeFrameRate::whole_frames(1).unwrap(),
                    1,
                    1,
                ),
            },
        )]
    };
    let mut changed = prepare;
    if let TimecodeAudioCommand::Prepare { asset, .. } = &mut changed {
        asset.revision = light_application::AssetRevision(2);
    }
    let mut voices = DeviceVoices::default();
    for commands in [vec![changed], Vec::new()] {
        let result = resolve_prepared_audio(resource(), &commands);
        assert!(result.is_err());
        assert!(voices.timecodes.is_empty());
    }
    let commands = resolve_prepared_audio(
        resource(),
        &[
            prepare,
            TimecodeAudioCommand::Play {
                timecode_id: id,
                source_frame: TimecodeFrame(2),
                audible_at_micros: 123,
            },
        ],
    )
    .unwrap();
    apply_native(&mut voices, NativeCommand::PreparedBatch(commands), 1, 1).unwrap();
    assert!(voices.timecodes[&id].playing);
    assert_eq!(voices.timecodes[&id].position, 2.0);
    assert_eq!(voices.timecodes[&id].play_at_micros, Some(123));
}

#[test]
fn prepared_voice_validation_rejects_forward_dependencies_and_nested_batches_atomically() {
    let id = TimecodeId(uuid::Uuid::from_u128(803));
    let voice = || NativeCommand::PreparedVoice {
        timecode_id: id,
        voice: Voice::new(
            decoded(&[1.0; 16]),
            TimecodeFrameRate::whole_frames(1).unwrap(),
            1,
            1,
        ),
    };
    for commands in [
        vec![
            NativeCommand::Transport(TimecodeAudioCommand::Stop { timecode_id: id }),
            voice(),
        ],
        vec![voice(), NativeCommand::PreparedBatch(Vec::new())],
    ] {
        let mut voices = DeviceVoices::default();
        assert!(apply_native(&mut voices, NativeCommand::PreparedBatch(commands), 1, 1).is_err());
        assert!(voices.timecodes.is_empty());
    }
    let mut voices = DeviceVoices::default();
    let mut invalid = Voice::new(
        decoded(&[1.0; 16]),
        TimecodeFrameRate::whole_frames(1).unwrap(),
        1,
        1,
    );
    invalid.source_rate = 0;
    assert!(
        apply_native(
            &mut voices,
            NativeCommand::PreparedBatch(vec![
                voice(),
                NativeCommand::PreparedVoice {
                    timecode_id: id,
                    voice: invalid
                }
            ]),
            1,
            1
        )
        .is_err()
    );
    assert!(voices.timecodes.is_empty());
}
