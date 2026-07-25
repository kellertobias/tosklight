use serde::{Deserialize, Serialize};

pub const MIN_BPM: f64 = 0.1;
pub const MAX_BPM: f64 = 999.0;
pub const MIN_SOUND_MULTIPLIER: f64 = 0.125;
pub const MAX_SOUND_MULTIPLIER: f64 = 8.0;
pub const MAX_SPEED_MASTER_SCALE: f64 = 4.0;
pub(super) const MAX_LEARN_INTERVALS: usize = 4;

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

pub(super) fn valid_bpm(value: f64) -> bool {
    value.is_finite() && (MIN_BPM..=MAX_BPM).contains(&value)
}
