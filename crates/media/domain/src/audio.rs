//! Audio analysis.
//!
//! The legacy analyser used Apple Accelerate and was therefore platform-partial. This is plain
//! arithmetic: the same band definitions, smoothing, thresholds, and numbers on macOS, Windows,
//! and Linux, because there is nothing platform-specific left to differ.
//!
//! Nothing here allocates in a real-time path or reads a clock. Capture publishes samples into a
//! bounded ring buffer and a worker calls this; every call is stamped by the caller.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

/// Samples one analysis window covers. A power of two, because the transform is radix-2.
pub const WINDOW: usize = 1_024;

/// Samples the published waveform carries, as the legacy analyser did.
pub const WAVEFORM_POINTS: usize = 512;

/// Logarithmically distributed spectrum bands.
pub const BANDS: usize = 64;

/// What one analysis pass found.
///
/// The default is silence, which is what a visualizer sees before any audio device is open — a
/// real state, not a placeholder, so nothing has to branch on "no analysis yet".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    /// The window, downsampled for display. Not for measurement.
    pub waveform: Vec<f32>,
    /// Band magnitudes, `0.0..`, low frequency first.
    pub spectrum: Vec<f32>,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    /// Root-mean-square of the window.
    pub energy: f32,
    /// The largest absolute sample in the window.
    pub peak: f32,
}

impl Analysis {
    /// This analysis as a source turned up or down by `gain` hears it.
    ///
    /// Every level scales; the waveform stays inside the `-1..=1` a trace is drawn in, so a
    /// turned-up oscilloscope clips at its edges rather than leaving the picture.
    pub fn scaled(&self, gain: f32) -> Self {
        Self {
            waveform: self
                .waveform
                .iter()
                .map(|sample| (sample * gain).clamp(-1.0, 1.0))
                .collect(),
            spectrum: self.spectrum.iter().map(|band| band * gain).collect(),
            bass: self.bass * gain,
            mid: self.mid * gain,
            treble: self.treble * gain,
            energy: self.energy * gain,
            peak: self.peak * gain,
        }
    }
}

/// One instrument the beat detector follows, as a visual reads it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    /// The instrument's band, auto-ranged to `0.0..=1.0` between its own recent floor and peak.
    pub level: f32,
    /// `1.0` when the instrument struck, falling toward zero afterwards.
    pub hit: f32,
}

/// The kick, snare, and hi-hat, so an effect can flash on the one it is about rather than on
/// whatever was loudest. The default is silence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instruments {
    pub kick: Instrument,
    pub snare: Instrument,
    pub hihat: Instrument,
}

impl Instruments {
    /// These instruments with their levels turned up or down by `gain`.
    ///
    /// A hit is the detector's decision that an instrument struck, not a level, so it is kept: a
    /// turned-down source still sees every kick, only smaller.
    pub fn scaled(self, gain: f32) -> Self {
        let level = |instrument: Instrument| Instrument {
            level: instrument.level * gain,
            ..instrument
        };
        Self {
            kick: level(self.kick),
            snare: level(self.snare),
            hihat: level(self.hihat),
        }
    }
}

/// How an operator has tuned the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tuning {
    /// Input gain, applied through a curve so low settings stay precise.
    pub input_gain: f32,
    pub eq_bass: f32,
    pub eq_mid: f32,
    pub eq_treble: f32,
    /// Scales the beat detector's thresholds. Higher means easier to trigger.
    pub beat_sensitivity: f32,
    /// Level the analysis to the program automatically; the input gain then trims on top of it.
    #[serde(default = "automatic")]
    pub auto_gain: bool,
}

const fn automatic() -> bool {
    true
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            input_gain: 1.0,
            eq_bass: 1.0,
            eq_mid: 1.0,
            eq_treble: 1.0,
            beat_sensitivity: 1.0,
            auto_gain: automatic(),
        }
    }
}

impl Tuning {
    /// The gain actually applied.
    ///
    /// Squared rather than linear, so the lower half of an operator's travel spreads across a
    /// quarter of the range and quiet inputs stay adjustable.
    pub fn effective_gain(&self) -> f32 {
        let gain = self.input_gain.max(0.0);
        gain * gain
    }
}

/// Analyses one window of mono samples.
///
/// `sample_rate` sets where the band edges fall, so the same audio at a different rate still
/// reports bass as bass.
pub fn analyse(samples: &[f32], sample_rate: f32, tuning: &Tuning) -> Analysis {
    let gain = tuning.effective_gain();
    let mut window = [0f32; WINDOW];
    for (slot, sample) in window.iter_mut().zip(samples.iter()) {
        *slot = sample * gain;
    }

    let peak = window
        .iter()
        .fold(0f32, |peak, sample| peak.max(sample.abs()));
    let energy = (window.iter().map(|sample| sample * sample).sum::<f32>() / WINDOW as f32).sqrt();

    let magnitudes = spectrum_magnitudes(&window);
    let spectrum = to_bands(&magnitudes, sample_rate);

    // Band edges at 250 Hz and 4 kHz, the split the legacy analyser used.
    let bin = sample_rate / WINDOW as f32;
    let bass_top = (250.0 / bin) as usize;
    let mid_top = (4_000.0 / bin) as usize;
    let half = magnitudes.len();

    let mean = |from: usize, to: usize| {
        let from = from.min(half);
        let to = to.min(half).max(from + 1);
        magnitudes[from..to].iter().sum::<f32>() / (to - from) as f32
    };

    Analysis {
        waveform: downsample(&window),
        spectrum,
        bass: mean(1, bass_top) * tuning.eq_bass,
        mid: mean(bass_top, mid_top) * tuning.eq_mid,
        treble: mean(mid_top, half) * tuning.eq_treble,
        energy,
        peak,
    }
}

/// Every sample the display shows, taken evenly across the window.
fn downsample(window: &[f32; WINDOW]) -> Vec<f32> {
    let stride = WINDOW / WAVEFORM_POINTS;
    (0..WAVEFORM_POINTS)
        .map(|index| window[index * stride])
        .collect()
}

/// Magnitudes of the first half of the transform. The second half mirrors the first for real
/// input, so it carries nothing.
fn spectrum_magnitudes(window: &[f32; WINDOW]) -> Vec<f32> {
    let mut real = [0f32; WINDOW];
    let mut imaginary = [0f32; WINDOW];

    // A Hann window, so a tone that does not land exactly on a bin does not smear across all of
    // them.
    for index in 0..WINDOW {
        let taper = 0.5 - 0.5 * (2.0 * PI * index as f32 / WINDOW as f32).cos();
        real[index] = window[index] * taper;
    }

    transform(&mut real, &mut imaginary);
    (0..WINDOW / 2)
        .map(|index| (real[index] * real[index] + imaginary[index] * imaginary[index]).sqrt())
        .collect()
}

/// An in-place radix-2 Cooley-Tukey transform.
///
/// Written out rather than taken from a library so the numbers are identical on every platform —
/// which is the whole point, given the analyser this replaces was macOS-only.
fn transform(real: &mut [f32; WINDOW], imaginary: &mut [f32; WINDOW]) {
    // Bit-reversal permutation.
    let mut target = 0usize;
    for source in 1..WINDOW {
        let mut bit = WINDOW >> 1;
        while target & bit != 0 {
            target ^= bit;
            bit >>= 1;
        }
        target |= bit;
        if source < target {
            real.swap(source, target);
            imaginary.swap(source, target);
        }
    }

    let mut length = 2;
    while length <= WINDOW {
        let angle = -2.0 * PI / length as f32;
        for start in (0..WINDOW).step_by(length) {
            for offset in 0..length / 2 {
                let (sin, cos) = (angle * offset as f32).sin_cos();
                let (a, b) = (start + offset, start + offset + length / 2);
                let real_part = cos * real[b] - sin * imaginary[b];
                let imaginary_part = sin * real[b] + cos * imaginary[b];
                real[b] = real[a] - real_part;
                imaginary[b] = imaginary[a] - imaginary_part;
                real[a] += real_part;
                imaginary[a] += imaginary_part;
            }
        }
        length <<= 1;
    }
}

/// Folds the linear bins into logarithmically spaced bands.
///
/// Logarithmic because hearing is: a linear split would give almost every band to frequencies
/// above 10 kHz, where a visualizer has nothing interesting to show.
fn to_bands(magnitudes: &[f32], sample_rate: f32) -> Vec<f32> {
    let lowest = 20.0f32;
    let highest = (sample_rate / 2.0).min(20_000.0);
    let bin = sample_rate / WINDOW as f32;
    let ratio = (highest / lowest).powf(1.0 / BANDS as f32);

    let mut bands = Vec::with_capacity(BANDS);
    let mut edge = lowest;
    for _ in 0..BANDS {
        let next = edge * ratio;
        let from = ((edge / bin) as usize).min(magnitudes.len().saturating_sub(1));
        let to = ((next / bin) as usize).clamp(from + 1, magnitudes.len());
        bands.push(magnitudes[from..to].iter().sum::<f32>() / (to - from) as f32);
        edge = next;
    }
    bands
}

// Beats, instruments, and tempo are found by `light-beat`, which the Light desk shares: the two
// products hear the same music the same way.

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn tone(frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..WINDOW)
            .map(|index| amplitude * (2.0 * PI * frequency * index as f32 / RATE).sin())
            .collect()
    }

    #[test]
    fn a_scaled_analysis_moves_every_level_and_keeps_the_trace_in_range() {
        let heard = Analysis {
            waveform: vec![0.4, -0.8],
            spectrum: vec![0.5, 2.0],
            bass: 0.5,
            mid: 0.25,
            treble: 0.1,
            energy: 0.3,
            peak: 0.8,
        };
        let louder = heard.scaled(2.0);
        assert_eq!(louder.waveform, vec![0.8, -1.0]);
        assert_eq!(louder.spectrum, vec![1.0, 4.0]);
        assert_eq!((louder.bass, louder.mid, louder.energy), (1.0, 0.5, 0.6));
        assert_eq!(heard.scaled(0.0).peak, 0.0);

        let struck = Instruments {
            kick: Instrument {
                level: 0.5,
                hit: 1.0,
            },
            ..Default::default()
        }
        .scaled(0.5);
        assert_eq!(
            struck.kick,
            Instrument {
                level: 0.25,
                hit: 1.0
            }
        );
    }

    fn loudest_band(analysis: &Analysis) -> usize {
        analysis
            .spectrum
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(index, _)| index)
            .unwrap()
    }

    #[test]
    fn silence_reports_nothing() {
        let analysis = analyse(&[0.0; WINDOW], RATE, &Tuning::default());
        assert_eq!(analysis.peak, 0.0);
        assert_eq!(analysis.energy, 0.0);
        assert!(analysis.spectrum.iter().all(|band| *band == 0.0));
    }

    #[test]
    fn the_published_shapes_are_the_documented_sizes() {
        let analysis = analyse(&tone(1_000.0, 0.5), RATE, &Tuning::default());
        assert_eq!(analysis.waveform.len(), WAVEFORM_POINTS);
        assert_eq!(analysis.spectrum.len(), BANDS);
    }

    #[test]
    fn peak_and_energy_describe_the_window() {
        let analysis = analyse(&tone(1_000.0, 0.8), RATE, &Tuning::default());
        assert!(
            (analysis.peak - 0.8).abs() < 0.05,
            "peak was {}",
            analysis.peak
        );
        // A sine's RMS is its amplitude over root two.
        assert!((analysis.energy - 0.8 / 2f32.sqrt()).abs() < 0.05);
    }

    #[test]
    fn a_tone_lands_in_the_band_it_belongs_to() {
        let low = loudest_band(&analyse(&tone(60.0, 0.8), RATE, &Tuning::default()));
        let middle = loudest_band(&analyse(&tone(1_000.0, 0.8), RATE, &Tuning::default()));
        let high = loudest_band(&analyse(&tone(10_000.0, 0.8), RATE, &Tuning::default()));

        assert!(low < middle, "60 Hz landed at {low}, 1 kHz at {middle}");
        assert!(middle < high, "1 kHz landed at {middle}, 10 kHz at {high}");
    }

    #[test]
    fn the_three_ranges_follow_the_content() {
        let default = Tuning::default();
        let low = analyse(&tone(80.0, 0.8), RATE, &default);
        assert!(low.bass > low.mid && low.bass > low.treble, "{low:?}");

        let high = analyse(&tone(9_000.0, 0.8), RATE, &default);
        assert!(high.treble > high.bass, "{high:?}");
    }

    #[test]
    fn the_equaliser_scales_its_range_and_leaves_the_others() {
        let samples = tone(80.0, 0.8);
        let plain = analyse(&samples, RATE, &Tuning::default());
        let boosted = analyse(
            &samples,
            RATE,
            &Tuning {
                eq_bass: 2.0,
                ..Default::default()
            },
        );

        assert!((boosted.bass - plain.bass * 2.0).abs() < 1e-3);
        assert!(
            (boosted.treble - plain.treble).abs() < 1e-6,
            "treble was untouched"
        );
    }

    #[test]
    fn input_gain_uses_a_curve_so_low_settings_stay_adjustable() {
        assert_eq!(
            Tuning {
                input_gain: 1.0,
                ..Default::default()
            }
            .effective_gain(),
            1.0
        );
        assert_eq!(
            Tuning {
                input_gain: 0.5,
                ..Default::default()
            }
            .effective_gain(),
            0.25
        );
        assert_eq!(
            Tuning {
                input_gain: -1.0,
                ..Default::default()
            }
            .effective_gain(),
            0.0,
            "a negative gain does not invert the signal"
        );
    }

    #[test]
    fn gain_scales_what_is_measured() {
        let samples = tone(1_000.0, 0.4);
        let quiet = analyse(
            &samples,
            RATE,
            &Tuning {
                input_gain: 0.5,
                ..Default::default()
            },
        );
        let loud = analyse(&samples, RATE, &Tuning::default());
        assert!(
            loud.peak > quiet.peak * 3.0,
            "quarter gain is a quarter as loud"
        );
    }

    #[test]
    fn a_short_window_is_analysed_rather_than_refused() {
        // Capture can hand over less than a full window at the start of a stream.
        let analysis = analyse(&[0.5; 100], RATE, &Tuning::default());
        assert_eq!(analysis.waveform.len(), WAVEFORM_POINTS);
        assert!(analysis.peak > 0.0);
    }

    #[test]
    fn a_tuning_written_before_automatic_gain_keeps_it_on() {
        let tuning: Tuning = serde_json::from_str(
            r#"{"inputGain":1.0,"eqBass":1.0,"eqMid":1.0,"eqTreble":1.0,"beatSensitivity":1.0}"#,
        )
        .expect("a tuning");
        assert!(tuning.auto_gain);
    }

    #[test]
    fn the_transform_is_deterministic() {
        // The point of writing it out rather than linking a library: identical numbers everywhere.
        let samples = tone(440.0, 0.7);
        let first = analyse(&samples, RATE, &Tuning::default());
        let second = analyse(&samples, RATE, &Tuning::default());
        assert_eq!(first, second);
    }
}
