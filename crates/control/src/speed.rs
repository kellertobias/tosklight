//! Authoritative Speed Group and Sound-to-Light source-selection semantics.
//!
//! Audio capture and frequency analysis intentionally live outside this module. A browser or
//! attached desk submits timestamped analysis observations; this state machine decides whether
//! they are trustworthy and exposes the single effective rate consumed by chasers and controls.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const MIN_BPM: f64 = 0.1;
pub const MAX_BPM: f64 = 999.0;
pub const MIN_SOUND_MULTIPLIER: f64 = 0.125;
pub const MAX_SOUND_MULTIPLIER: f64 = 8.0;
pub const MAX_SPEED_MASTER_SCALE: f64 = 4.0;
const MAX_LEARN_INTERVALS: usize = 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundAnalysisMode {
    /// The analyzer reports a stable tempo estimate. Individual beat and level-following modes
    /// can be added later without changing the source-selection contract.
    #[default]
    TempoBpm,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyPreset {
    Sub,
    #[default]
    Low,
    Mid,
    High,
    FullRange,
}

impl FrequencyPreset {
    pub fn range_hz(self) -> (u16, u16) {
        match self {
            Self::Sub => (30, 80),
            Self::Low => (60, 180),
            Self::Mid => (180, 2_000),
            Self::High => (2_000, 12_000),
            Self::FullRange => (30, 18_000),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrequencySelection {
    Preset { preset: FrequencyPreset },
    Custom { low_hz: u16, high_hz: u16 },
}

impl Default for FrequencySelection {
    fn default() -> Self {
        Self::Preset {
            preset: FrequencyPreset::Low,
        }
    }
}

impl FrequencySelection {
    pub fn range_hz(&self) -> (u16, u16) {
        match self {
            Self::Preset { preset } => preset.range_hz(),
            Self::Custom { low_hz, high_hz } => (*low_hz, *high_hz),
        }
    }

    fn validate(&self) -> Result<(), SpeedError> {
        let (low, high) = self.range_hz();
        if low < 20 || high > 20_000 || low >= high {
            return Err(SpeedError::InvalidFrequencyRange);
        }
        Ok(())
    }
}

/// Show-portable analysis and response settings. The selected device identifier is deliberately
/// absent: it belongs to the capturing desk/browser because device IDs are machine-specific.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundToLightConfig {
    pub enabled: bool,
    pub analysis_mode: SoundAnalysisMode,
    pub frequency: FrequencySelection,
    /// Analyzer-side input gain in decibels.
    pub input_gain_db: f32,
    /// Minimum analyzer confidence accepted as an authoritative tempo, from zero to one.
    pub confidence_threshold: f32,
    /// Zero applies updates directly; values approaching one increasingly smooth changes.
    pub smoothing: f32,
    pub minimum_bpm: f64,
    pub maximum_bpm: f64,
    pub signal_hold_millis: u64,
    pub multiplier: f64,
}

impl Default for SoundToLightConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            analysis_mode: SoundAnalysisMode::TempoBpm,
            frequency: FrequencySelection::default(),
            input_gain_db: 0.0,
            confidence_threshold: 0.65,
            smoothing: 0.35,
            minimum_bpm: 40.0,
            maximum_bpm: 240.0,
            signal_hold_millis: 2_000,
            multiplier: 1.0,
        }
    }
}

impl SoundToLightConfig {
    pub fn validate(&self) -> Result<(), SpeedError> {
        self.frequency.validate()?;
        if !self.input_gain_db.is_finite() || !(-60.0..=60.0).contains(&self.input_gain_db) {
            return Err(SpeedError::InvalidInputGain);
        }
        if !self.confidence_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.confidence_threshold)
        {
            return Err(SpeedError::InvalidConfidenceThreshold);
        }
        if !self.smoothing.is_finite() || !(0.0..=0.99).contains(&self.smoothing) {
            return Err(SpeedError::InvalidSmoothing);
        }
        if !valid_bpm(self.minimum_bpm)
            || !valid_bpm(self.maximum_bpm)
            || self.minimum_bpm >= self.maximum_bpm
        {
            return Err(SpeedError::InvalidSoundBpmRange);
        }
        if self.signal_hold_millis > 60_000 {
            return Err(SpeedError::InvalidSignalHold);
        }
        if !self.multiplier.is_finite()
            || !(MIN_SOUND_MULTIPLIER..=MAX_SOUND_MULTIPLIER).contains(&self.multiplier)
        {
            return Err(SpeedError::InvalidMultiplier);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundObservation {
    /// Monotonic capture timestamp in milliseconds.
    pub captured_at_millis: u64,
    pub source_available: bool,
    pub usable_signal: bool,
    pub level: f32,
    pub selected_band_level: f32,
    pub detected_bpm: Option<f64>,
    pub confidence: f32,
}

impl SoundObservation {
    pub fn tempo(captured_at_millis: u64, detected_bpm: f64, confidence: f32) -> Self {
        Self {
            captured_at_millis,
            source_available: true,
            usable_signal: true,
            level: 1.0,
            selected_band_level: 1.0,
            detected_bpm: Some(detected_bpm),
            confidence,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundLossReason {
    SourceUnavailable,
    NoUsableSignal,
    LowConfidence,
    TempoOutsideRange,
    WaitingForAnalysis,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SoundStatus {
    Disabled,
    Active {
        detected_bpm: f64,
        confidence: f32,
    },
    Holding {
        reason: SoundLossReason,
        remaining_millis: u64,
    },
    ManualFallback {
        reason: SoundLossReason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveSpeedSource {
    Manual,
    Sound,
    HeldSound,
    ManualFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeedSnapshot {
    pub manual_bpm: f64,
    pub sound_bpm: Option<f64>,
    /// The selected source after Sound multiplier, then Speed Master scale. It stays available
    /// while paused so resuming never loses the learned rate.
    pub effective_bpm: f64,
    pub source: EffectiveSpeedSource,
    pub sound_status: SoundStatus,
    pub paused: bool,
    pub phase_advancing: bool,
    pub speed_master_scale: f64,
    pub sound_multiplier: f64,
    pub source_available: bool,
    pub usable_signal: bool,
    pub input_level: f32,
    pub selected_band_level: f32,
    /// One-based peer Speed Group number while two groups share an operator-established beat
    /// phase. This is authoritative feedback, not a second speed source.
    pub synchronized_with: Option<u8>,
    pub phase_origin_millis: u64,
    /// Normalized beat position in `[0, 1)`, frozen while paused.
    pub beat_phase: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LearnResult {
    Armed,
    Learned { bpm: f64 },
    RejectedInterval,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeedError {
    InvalidManualBpm,
    InvalidFrequencyRange,
    InvalidInputGain,
    InvalidConfidenceThreshold,
    InvalidSmoothing,
    InvalidSoundBpmRange,
    InvalidSignalHold,
    InvalidMultiplier,
    InvalidSpeedMasterScale,
}

impl std::fmt::Display for SpeedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidManualBpm => "manual BPM must be finite and within 0.1-999",
            Self::InvalidFrequencyRange => "frequency range must be ordered and within 20-20000 Hz",
            Self::InvalidInputGain => "input gain must be finite and within -60 to 60 dB",
            Self::InvalidConfidenceThreshold => "confidence threshold must be within 0-1",
            Self::InvalidSmoothing => "smoothing must be within 0-0.99",
            Self::InvalidSoundBpmRange => "sound BPM range must be ordered and within 0.1-999",
            Self::InvalidSignalHold => "signal hold must not exceed 60 seconds",
            Self::InvalidMultiplier => "sound multiplier must be within 0.125-8",
            Self::InvalidSpeedMasterScale => "Speed Master scale must be within 0-4",
        })
    }
}

impl std::error::Error for SpeedError {}

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

fn valid_bpm(value: f64) -> bool {
    value.is_finite() && (MIN_BPM..=MAX_BPM).contains(&value)
}

#[cfg(test)]
mod tests {
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
}
