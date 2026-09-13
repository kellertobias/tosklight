//! A low-latency band filter for the kick.
//!
//! The spectrum only sees a kick once the hit has travelled into the middle of its tapered frame,
//! tens of milliseconds after it sounded. A short chain of recursive filters sees it within a few
//! milliseconds, which is the difference between a light flashing with the kick and after it.

use std::f32::consts::{FRAC_1_SQRT_2, PI};

/// Below this a filter state is inaudible, and left in place it decays into subnormal numbers that
/// some processors handle hundreds of times more slowly.
const NEGLIGIBLE: f32 = 1.0e-20;

#[derive(Debug, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn low_pass(frequency: f32, q: f32, sample_rate: f32) -> Self {
        let (cos, alpha) = Self::prewarp(frequency, q, sample_rate);
        Self::normalised(
            (1.0 - cos) / 2.0,
            1.0 - cos,
            (1.0 - cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    fn high_pass(frequency: f32, q: f32, sample_rate: f32) -> Self {
        let (cos, alpha) = Self::prewarp(frequency, q, sample_rate);
        Self::normalised(
            (1.0 + cos) / 2.0,
            -(1.0 + cos),
            (1.0 + cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    fn prewarp(frequency: f32, q: f32, sample_rate: f32) -> (f32, f32) {
        let angle = 2.0 * PI * frequency / sample_rate;
        (angle.cos(), angle.sin() / (2.0 * q))
    }

    fn normalised(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn next(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * output + self.z2;
        self.z2 = self.b2 * input - self.a2 * output;
        if self.z1.abs() < NEGLIGIBLE {
            self.z1 = 0.0;
        }
        if self.z2.abs() < NEGLIGIBLE {
            self.z2 = 0.0;
        }
        output
    }
}

/// 40–120 Hz: a second-order high-pass under a fourth-order Butterworth low-pass.
#[derive(Debug, Clone, Copy)]
pub(crate) struct KickFilter {
    stages: [Biquad; 3],
}

impl KickFilter {
    pub(crate) fn new(sample_rate: f32) -> Self {
        Self {
            stages: [
                Biquad::high_pass(40.0, FRAC_1_SQRT_2, sample_rate),
                // The two quality factors of a fourth-order Butterworth response.
                Biquad::low_pass(120.0, 0.541_196_1, sample_rate),
                Biquad::low_pass(120.0, 1.306_563, sample_rate),
            ],
        }
    }

    pub(crate) fn next(&mut self, sample: f32) -> f32 {
        self.stages
            .iter_mut()
            .fold(sample, |signal, stage| stage.next(signal))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// The steady-state RMS gain for a tone, measured after the filter has settled.
    fn gain(frequency: f32) -> f32 {
        let mut filter = KickFilter::new(RATE);
        let count = RATE as usize;
        let mut energy = 0.0;
        let mut input = 0.0;
        for index in 0..count {
            let sample = (2.0 * PI * frequency * index as f32 / RATE).sin();
            let output = filter.next(sample);
            if index >= count / 2 {
                energy += output * output;
                input += sample * sample;
            }
        }
        (energy / input).sqrt()
    }

    #[test]
    fn the_kick_band_passes_and_everything_else_is_rejected() {
        assert!(gain(70.0) > 0.7, "70 Hz: {}", gain(70.0));
        assert!(gain(1_000.0) < 0.01, "1 kHz: {}", gain(1_000.0));
        assert!(gain(8_000.0) < 0.001, "8 kHz: {}", gain(8_000.0));
        assert!(gain(15.0) < 0.2, "15 Hz: {}", gain(15.0));
    }

    #[test]
    fn a_kick_is_heard_within_milliseconds_of_sounding() {
        let mut filter = KickFilter::new(RATE);
        let onset = (0.1 * RATE) as usize;
        let mut first_loud = None;
        for index in 0..(RATE as usize / 5) {
            let sample = if index >= onset {
                (2.0 * PI * 70.0 * (index - onset) as f32 / RATE).sin()
            } else {
                0.0
            };
            if filter.next(sample).abs() > 0.5 && first_loud.is_none() {
                first_loud = Some(index);
            }
        }
        let delay = (first_loud.expect("the tone comes through") - onset) as f32 / RATE;
        assert!(
            delay < 0.012,
            "first loud sample after {:.1} ms",
            delay * 1_000.0
        );
    }

    #[test]
    fn silence_stays_silent() {
        let mut filter = KickFilter::new(RATE);
        assert!((0..10_000).all(|_| filter.next(0.0) == 0.0));
    }
}
