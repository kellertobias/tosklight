use super::*;

fn enabled_config() -> SoundToLightConfig {
    SoundToLightConfig {
        enabled: true,
        smoothing: 0.0,
        ..SoundToLightConfig::default()
    }
}

#[test]
fn sound_rate_is_authoritative_without_overwriting_manual_fallback() {
    let mut speed = SpeedGroupController::new(96.0, enabled_config()).unwrap();
    speed.observe_sound(SoundObservation::tempo(1_000, 120.0, 0.9));
    let snapshot = speed.snapshot(1_000);
    assert_eq!(snapshot.source, EffectiveSpeedSource::Sound);
    assert_eq!(snapshot.effective_bpm, 120.0);
    assert_eq!(snapshot.manual_bpm, 96.0);

    let mut disabled = speed.sound_config().clone();
    disabled.enabled = false;
    speed.set_sound_config(disabled).unwrap();
    let snapshot = speed.snapshot(1_001);
    assert_eq!(snapshot.source, EffectiveSpeedSource::Manual);
    assert_eq!(snapshot.effective_bpm, 96.0);
    assert_eq!(snapshot.sound_status, SoundStatus::Disabled);
}

#[test]
fn missing_source_holds_then_falls_back_deterministically() {
    let mut config = enabled_config();
    config.signal_hold_millis = 2_000;
    let mut speed = SpeedGroupController::new(90.0, config).unwrap();
    speed.observe_sound(SoundObservation::tempo(1_000, 128.0, 0.95));
    speed.observe_sound(SoundObservation {
        captured_at_millis: 1_500,
        source_available: false,
        usable_signal: false,
        level: 0.0,
        selected_band_level: 0.0,
        detected_bpm: None,
        confidence: 0.0,
    });
    let holding = speed.snapshot(2_000);
    assert_eq!(holding.source, EffectiveSpeedSource::HeldSound);
    assert_eq!(holding.effective_bpm, 128.0);
    assert_eq!(
        holding.sound_status,
        SoundStatus::Holding {
            reason: SoundLossReason::SourceUnavailable,
            remaining_millis: 1_000,
        }
    );

    let fallback = speed.snapshot(3_001);
    assert_eq!(fallback.source, EffectiveSpeedSource::ManualFallback);
    assert_eq!(fallback.effective_bpm, 90.0);
    assert_eq!(
        fallback.sound_status,
        SoundStatus::ManualFallback {
            reason: SoundLossReason::SourceUnavailable
        }
    );
}

#[test]
fn low_confidence_and_out_of_range_tempos_never_take_ownership() {
    let mut speed = SpeedGroupController::new(100.0, enabled_config()).unwrap();
    speed.observe_sound(SoundObservation::tempo(1_000, 120.0, 0.2));
    assert_eq!(
        speed.snapshot(1_000).sound_status,
        SoundStatus::ManualFallback {
            reason: SoundLossReason::LowConfidence
        }
    );
    speed.observe_sound(SoundObservation::tempo(1_100, 300.0, 0.9));
    assert_eq!(
        speed.snapshot(1_100).sound_status,
        SoundStatus::ManualFallback {
            reason: SoundLossReason::TempoOutsideRange
        }
    );
}

#[test]
fn smoothing_dampens_abrupt_tempo_changes() {
    let mut config = enabled_config();
    config.smoothing = 0.75;
    let mut speed = SpeedGroupController::new(100.0, config).unwrap();
    speed.observe_sound(SoundObservation::tempo(1_000, 100.0, 1.0));
    speed.observe_sound(SoundObservation::tempo(1_100, 140.0, 1.0));
    assert_eq!(speed.snapshot(1_100).effective_bpm, 110.0);
}

#[test]
fn sound_double_half_adjust_ratio_but_manual_actions_adjust_manual_bpm() {
    let mut speed = SpeedGroupController::new(80.0, enabled_config()).unwrap();
    speed.observe_sound(SoundObservation::tempo(1_000, 100.0, 1.0));
    speed.double();
    assert_eq!(speed.snapshot(1_000).effective_bpm, 200.0);
    assert_eq!(speed.manual_bpm(), 80.0);
    speed.half();
    assert_eq!(speed.snapshot(1_000).effective_bpm, 100.0);

    speed.set_manual_bpm(80.0).unwrap();
    speed.double();
    assert_eq!(speed.snapshot(1_001).effective_bpm, 160.0);
    speed.half();
    assert_eq!(speed.snapshot(1_001).effective_bpm, 80.0);
}

#[test]
fn learn_exits_sound_mode_and_averages_recent_taps() {
    let mut speed = SpeedGroupController::new(100.0, enabled_config()).unwrap();
    speed.observe_sound(SoundObservation::tempo(500, 140.0, 1.0));
    assert_eq!(speed.tap_learn(1_000), LearnResult::Armed);
    assert!(!speed.sound_config().enabled);
    assert_eq!(speed.tap_learn(1_500), LearnResult::Learned { bpm: 120.0 });
    assert_eq!(
        speed.tap_learn(2_100),
        LearnResult::Learned {
            bpm: 60_000.0 / 550.0
        }
    );
    assert_eq!(speed.snapshot(2_100).source, EffectiveSpeedSource::Manual);
}

#[test]
fn pause_freezes_phase_without_discarding_live_sound_rate() {
    let mut speed = SpeedGroupController::new(90.0, enabled_config()).unwrap();
    speed.observe_sound(SoundObservation::tempo(1_000, 120.0, 1.0));
    speed.set_paused_at(true, 1_000);
    speed.observe_sound(SoundObservation::tempo(1_200, 130.0, 1.0));
    let paused = speed.snapshot(1_200);
    assert!(paused.paused);
    assert!(!paused.phase_advancing);
    assert_eq!(paused.effective_bpm, 130.0);

    speed.set_paused_at(false, 1_201);
    let resumed = speed.snapshot(1_201);
    assert!(resumed.phase_advancing);
    assert_eq!(resumed.effective_bpm, 130.0);
}

#[test]
fn synchronized_phase_feedback_freezes_and_resumes_from_the_same_beat() {
    let mut speed = SpeedGroupController::new(120.0, SoundToLightConfig::default()).unwrap();
    speed.synchronize_phase(3, 1_000, 1_000);
    let half_beat = speed.snapshot(1_250);
    assert_eq!(half_beat.synchronized_with, Some(3));
    assert_eq!(half_beat.phase_origin_millis, 1_000);
    assert!((half_beat.beat_phase - 0.5).abs() < 0.000_001);

    speed.set_paused_at(true, 1_250);
    assert!((speed.snapshot(5_000).beat_phase - 0.5).abs() < 0.000_001);
    speed.set_paused_at(false, 5_000);
    assert!((speed.snapshot(5_250).beat_phase - 0.0).abs() < 0.000_001);

    speed.break_synchronization(6_000);
    let independent = speed.snapshot(6_000);
    assert_eq!(independent.synchronized_with, None);
    assert_eq!(independent.beat_phase, 0.0);
}

#[test]
fn speed_master_scale_is_applied_after_source_selection() {
    let mut speed = SpeedGroupController::new(80.0, enabled_config()).unwrap();
    speed.observe_sound(SoundObservation::tempo(1_000, 120.0, 1.0));
    speed.set_speed_master_scale(0.5).unwrap();
    assert_eq!(speed.snapshot(1_000).effective_bpm, 60.0);
    speed.set_speed_master_scale(0.0).unwrap();
    let stopped = speed.snapshot(1_000);
    assert_eq!(stopped.effective_bpm, 0.0);
    assert!(!stopped.phase_advancing);
}

#[test]
fn stale_analyzer_packets_do_not_replace_new_feedback() {
    let mut speed = SpeedGroupController::new(80.0, enabled_config()).unwrap();
    speed.observe_sound(SoundObservation::tempo(2_000, 120.0, 1.0));
    speed.observe_sound(SoundObservation::tempo(1_000, 90.0, 1.0));
    assert_eq!(speed.snapshot(2_000).effective_bpm, 120.0);
}

#[test]
fn analyzer_silence_expires_without_an_explicit_disconnect_packet() {
    let mut config = enabled_config();
    config.signal_hold_millis = 500;
    let mut speed = SpeedGroupController::new(80.0, config).unwrap();
    speed.observe_sound(SoundObservation::tempo(2_000, 120.0, 1.0));
    assert_eq!(speed.snapshot(2_500).source, EffectiveSpeedSource::Sound);
    let expired = speed.snapshot(2_501);
    assert_eq!(expired.source, EffectiveSpeedSource::ManualFallback);
    assert_eq!(expired.effective_bpm, 80.0);
    assert_eq!(
        expired.sound_status,
        SoundStatus::ManualFallback {
            reason: SoundLossReason::WaitingForAnalysis
        }
    );
}

#[test]
fn persisted_settings_have_backward_compatible_defaults_and_validation() {
    let config: SoundToLightConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
    assert!(config.enabled);
    assert_eq!(config.frequency, FrequencySelection::default());
    assert_eq!(config.multiplier, 1.0);
    config.validate().unwrap();

    let invalid = SoundToLightConfig {
        frequency: FrequencySelection::Custom {
            low_hz: 500,
            high_hz: 100,
        },
        ..SoundToLightConfig::default()
    };
    assert_eq!(invalid.validate(), Err(SpeedError::InvalidFrequencyRange));
}
