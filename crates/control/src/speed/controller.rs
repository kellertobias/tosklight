use super::model::*;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct SpeedGroupController {
    manual_bpm: f64,
    sound: SoundToLightConfig,
    speed_master_scale: f64,
    paused: bool,
    last_observation: Option<SoundObservation>,
    last_accepted_at_millis: Option<u64>,
    smoothed_sound_bpm: Option<f64>,
    loss_reason: SoundLossReason,
    last_learn_tap_millis: Option<u64>,
    learn_intervals: VecDeque<u64>,
    synchronized_with: Option<u8>,
    phase_origin_millis: u64,
    phase_frozen_at_millis: Option<u64>,
}

impl SpeedGroupController {
    pub fn new(manual_bpm: f64, sound: SoundToLightConfig) -> Result<Self, SpeedError> {
        if !valid_bpm(manual_bpm) {
            return Err(SpeedError::InvalidManualBpm);
        }
        sound.validate()?;
        Ok(Self {
            manual_bpm,
            sound,
            speed_master_scale: 1.0,
            paused: false,
            last_observation: None,
            last_accepted_at_millis: None,
            smoothed_sound_bpm: None,
            loss_reason: SoundLossReason::WaitingForAnalysis,
            last_learn_tap_millis: None,
            learn_intervals: VecDeque::new(),
            synchronized_with: None,
            phase_origin_millis: 0,
            phase_frozen_at_millis: None,
        })
    }

    pub fn manual_bpm(&self) -> f64 {
        self.manual_bpm
    }

    pub fn sound_config(&self) -> &SoundToLightConfig {
        &self.sound
    }

    pub fn set_sound_config(&mut self, config: SoundToLightConfig) -> Result<(), SpeedError> {
        config.validate()?;
        let analysis_changed = self.sound.analysis_mode != config.analysis_mode
            || self.sound.frequency != config.frequency
            || self.sound.minimum_bpm != config.minimum_bpm
            || self.sound.maximum_bpm != config.maximum_bpm;
        let enabled_now = config.enabled && !self.sound.enabled;
        self.sound = config;
        if analysis_changed || enabled_now || !self.sound.enabled {
            self.clear_sound_runtime();
        }
        Ok(())
    }

    /// Direct/manual entry takes ownership predictably and leaves the last Sound setting intact
    /// except for its enabled flag, so the operator can explicitly re-enable it later.
    pub fn set_manual_bpm(&mut self, bpm: f64) -> Result<(), SpeedError> {
        if !valid_bpm(bpm) {
            return Err(SpeedError::InvalidManualBpm);
        }
        self.manual_bpm = bpm;
        self.sound.enabled = false;
        self.clear_sound_runtime();
        self.last_learn_tap_millis = None;
        self.learn_intervals.clear();
        Ok(())
    }

    /// Updates the stored fallback during configuration hydration without changing the currently
    /// selected source. Operator direct entry should use `set_manual_bpm` instead.
    pub fn set_manual_fallback_bpm(&mut self, bpm: f64) -> Result<(), SpeedError> {
        if !valid_bpm(bpm) {
            return Err(SpeedError::InvalidManualBpm);
        }
        self.manual_bpm = bpm;
        Ok(())
    }

    pub fn set_speed_master_scale(&mut self, scale: f64) -> Result<(), SpeedError> {
        if !scale.is_finite() || !(0.0..=MAX_SPEED_MASTER_SCALE).contains(&scale) {
            return Err(SpeedError::InvalidSpeedMasterScale);
        }
        self.speed_master_scale = scale;
        Ok(())
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if !paused {
            self.phase_frozen_at_millis = None;
        }
    }

    pub fn toggle_paused(&mut self) -> bool {
        self.paused = !self.paused;
        if !self.paused {
            self.phase_frozen_at_millis = None;
        }
        self.paused
    }

    pub fn set_paused_at(&mut self, paused: bool, now_millis: u64) {
        if paused == self.paused {
            return;
        }
        if paused {
            self.phase_frozen_at_millis = Some(now_millis);
        } else if let Some(frozen_at) = self.phase_frozen_at_millis.take() {
            self.phase_origin_millis = self
                .phase_origin_millis
                .saturating_add(now_millis.saturating_sub(frozen_at));
        }
        self.paused = paused;
    }

    pub fn toggle_paused_at(&mut self, now_millis: u64) -> bool {
        self.set_paused_at(!self.paused, now_millis);
        self.paused
    }

    pub fn synchronized_with(&self) -> Option<u8> {
        self.synchronized_with
    }

    pub fn phase_origin_millis(&self) -> u64 {
        self.phase_origin_millis
    }

    pub fn phase_reference_millis(&self, now_millis: u64) -> u64 {
        self.phase_frozen_at_millis.unwrap_or(now_millis)
    }

    pub fn synchronize_phase(
        &mut self,
        peer_group: u8,
        phase_origin_millis: u64,
        phase_reference_millis: u64,
    ) {
        self.synchronized_with = Some(peer_group);
        self.phase_origin_millis = phase_origin_millis;
        self.phase_frozen_at_millis = self.paused.then_some(phase_reference_millis);
    }

    pub fn break_synchronization(&mut self, phase_origin_millis: u64) {
        self.synchronized_with = None;
        self.phase_origin_millis = phase_origin_millis;
        self.phase_frozen_at_millis = self.paused.then_some(phase_origin_millis);
    }

    /// Removes only the peer relationship. This is used for the unaffected side when its peer
    /// takes an independent manual action: that group keeps its current beat origin and rate.
    pub fn clear_synchronization(&mut self) {
        self.synchronized_with = None;
    }

    /// Accepts analyzer feedback in monotonic order. Stale packets never replace newer desk state.
    pub fn observe_sound(&mut self, observation: SoundObservation) {
        if self
            .last_observation
            .is_some_and(|last| observation.captured_at_millis < last.captured_at_millis)
        {
            return;
        }
        self.last_observation = Some(observation);
        if !observation.source_available {
            self.loss_reason = SoundLossReason::SourceUnavailable;
            return;
        }
        if !observation.usable_signal
            || !observation.level.is_finite()
            || !observation.selected_band_level.is_finite()
        {
            self.loss_reason = SoundLossReason::NoUsableSignal;
            return;
        }
        if !observation.confidence.is_finite()
            || observation.confidence < self.sound.confidence_threshold
        {
            self.loss_reason = SoundLossReason::LowConfidence;
            return;
        }
        let Some(bpm) = observation.detected_bpm else {
            self.loss_reason = SoundLossReason::WaitingForAnalysis;
            return;
        };
        if !bpm.is_finite() || !(self.sound.minimum_bpm..=self.sound.maximum_bpm).contains(&bpm) {
            self.loss_reason = SoundLossReason::TempoOutsideRange;
            return;
        }
        let smoothed = self
            .smoothed_sound_bpm
            .map(|previous| {
                let update_weight = 1.0 - f64::from(self.sound.smoothing);
                previous + (bpm - previous) * update_weight
            })
            .unwrap_or(bpm);
        self.smoothed_sound_bpm = Some(smoothed);
        self.last_accepted_at_millis = Some(observation.captured_at_millis);
        self.loss_reason = SoundLossReason::WaitingForAnalysis;
    }

    /// In Sound mode Double/Half changes the configured ratio. Outside Sound mode it changes the
    /// manual learned rate. Pause is independent and therefore preserved by both operations.
    pub fn double(&mut self) {
        if self.sound.enabled {
            self.sound.multiplier = (self.sound.multiplier * 2.0).min(MAX_SOUND_MULTIPLIER);
        } else {
            self.manual_bpm = (self.manual_bpm * 2.0).min(MAX_BPM);
        }
    }

    pub fn half(&mut self) {
        if self.sound.enabled {
            self.sound.multiplier = (self.sound.multiplier / 2.0).max(MIN_SOUND_MULTIPLIER);
        } else {
            self.manual_bpm = (self.manual_bpm / 2.0).max(MIN_BPM);
        }
    }

    /// Learn is manual tap tempo. The first tap exits Sound mode immediately; two or more valid
    /// intervals update the learned BPM using a rolling mean of the last four intervals.
    pub fn tap_learn(&mut self, tapped_at_millis: u64) -> LearnResult {
        self.sound.enabled = false;
        self.clear_sound_runtime();
        let Some(previous) = self.last_learn_tap_millis.replace(tapped_at_millis) else {
            return LearnResult::Armed;
        };
        let interval = tapped_at_millis.saturating_sub(previous);
        if interval == 0 {
            self.learn_intervals.clear();
            return LearnResult::RejectedInterval;
        }
        let bpm = 60_000.0 / interval as f64;
        if !valid_bpm(bpm) {
            self.learn_intervals.clear();
            return LearnResult::RejectedInterval;
        }
        self.learn_intervals.push_back(interval);
        while self.learn_intervals.len() > MAX_LEARN_INTERVALS {
            self.learn_intervals.pop_front();
        }
        let mean_interval =
            self.learn_intervals.iter().sum::<u64>() as f64 / self.learn_intervals.len() as f64;
        self.manual_bpm = (60_000.0 / mean_interval).clamp(MIN_BPM, MAX_BPM);
        LearnResult::Learned {
            bpm: self.manual_bpm,
        }
    }

    pub fn snapshot(&self, now_millis: u64) -> SpeedSnapshot {
        let (base_bpm, source, sound_status) = self.select_source(now_millis);
        let effective_bpm = (base_bpm * self.speed_master_scale).clamp(0.0, MAX_BPM);
        let observation = self.last_observation;
        let phase_at = self.phase_frozen_at_millis.unwrap_or(now_millis);
        let beat_phase = if effective_bpm > 0.0 {
            ((phase_at.saturating_sub(self.phase_origin_millis) as f64 * effective_bpm / 60_000.0)
                % 1.0)
                .max(0.0)
        } else {
            0.0
        };
        SpeedSnapshot {
            manual_bpm: self.manual_bpm,
            sound_bpm: self.smoothed_sound_bpm,
            effective_bpm,
            source,
            sound_status,
            paused: self.paused,
            phase_advancing: !self.paused && effective_bpm > 0.0,
            speed_master_scale: self.speed_master_scale,
            sound_multiplier: self.sound.multiplier,
            source_available: observation.is_some_and(|value| value.source_available),
            usable_signal: observation.is_some_and(|value| value.usable_signal),
            input_level: observation.map_or(0.0, |value| value.level),
            selected_band_level: observation.map_or(0.0, |value| value.selected_band_level),
            synchronized_with: self.synchronized_with,
            phase_origin_millis: self.phase_origin_millis,
            beat_phase,
        }
    }

    fn select_source(&self, now_millis: u64) -> (f64, EffectiveSpeedSource, SoundStatus) {
        if !self.sound.enabled {
            return (
                self.manual_bpm,
                EffectiveSpeedSource::Manual,
                SoundStatus::Disabled,
            );
        }
        let Some(sound_bpm) = self.smoothed_sound_bpm else {
            return (
                self.manual_bpm,
                EffectiveSpeedSource::ManualFallback,
                SoundStatus::ManualFallback {
                    reason: self.loss_reason,
                },
            );
        };
        let accepted_at = self
            .last_accepted_at_millis
            .expect("a smoothed Sound BPM always has an acceptance time");
        let age = now_millis.saturating_sub(accepted_at);
        let rate = (sound_bpm * self.sound.multiplier).clamp(MIN_BPM, MAX_BPM);
        let current_observation_is_accepted = self.last_observation.is_some_and(|observation| {
            observation.captured_at_millis == accepted_at
                && observation.source_available
                && observation.usable_signal
        });
        if current_observation_is_accepted && age <= self.sound.signal_hold_millis {
            return (
                rate,
                EffectiveSpeedSource::Sound,
                SoundStatus::Active {
                    detected_bpm: sound_bpm,
                    confidence: self.last_observation.map_or(0.0, |value| value.confidence),
                },
            );
        }
        if age <= self.sound.signal_hold_millis {
            return (
                rate,
                EffectiveSpeedSource::HeldSound,
                SoundStatus::Holding {
                    reason: self.loss_reason,
                    remaining_millis: self.sound.signal_hold_millis.saturating_sub(age),
                },
            );
        }
        (
            self.manual_bpm,
            EffectiveSpeedSource::ManualFallback,
            SoundStatus::ManualFallback {
                reason: if current_observation_is_accepted {
                    SoundLossReason::WaitingForAnalysis
                } else {
                    self.loss_reason
                },
            },
        )
    }

    fn clear_sound_runtime(&mut self) {
        self.last_observation = None;
        self.last_accepted_at_millis = None;
        self.smoothed_sound_bpm = None;
        self.loss_reason = SoundLossReason::WaitingForAnalysis;
    }
}
