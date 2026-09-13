//! A windowed power spectrum with every table built once.
//!
//! The transform runs once per hop on the analysis thread, so the trigonometry, the taper, and the
//! bit-reversal permutation are computed at construction and the streaming path only indexes them.

use std::f32::consts::PI;

/// A radix-2 power spectrum of a fixed size.
#[derive(Debug, Clone)]
pub(crate) struct Spectrum {
    size: usize,
    taper: Vec<f32>,
    reversal: Vec<usize>,
    cosine: Vec<f32>,
    sine: Vec<f32>,
    real: Vec<f32>,
    imaginary: Vec<f32>,
    /// Scales power so a full-scale sine reads about one wherever its bin falls.
    normalisation: f32,
}

impl Spectrum {
    /// A spectrum of `size` samples. `size` must be a power of two.
    pub(crate) fn new(size: usize) -> Self {
        assert!(size.is_power_of_two() && size >= 4, "a radix-2 size");
        // A Hann window, so a tone between two bins does not smear across all of them.
        let taper = (0..size)
            .map(|index| 0.5 - 0.5 * (2.0 * PI * index as f32 / size as f32).cos())
            .collect();
        let bits = size.trailing_zeros();
        let reversal = (0..size)
            .map(|index| index.reverse_bits() >> (usize::BITS - bits))
            .collect();
        let cosine = (0..size / 2)
            .map(|index| (-2.0 * PI * index as f32 / size as f32).cos())
            .collect();
        let sine = (0..size / 2)
            .map(|index| (-2.0 * PI * index as f32 / size as f32).sin())
            .collect();
        // A Hann-windowed full-scale sine peaks at size / 4 in magnitude.
        let peak = size as f32 / 4.0;
        Self {
            size,
            taper,
            reversal,
            cosine,
            sine,
            real: vec![0.0; size],
            imaginary: vec![0.0; size],
            normalisation: 1.0 / (peak * peak),
        }
    }

    /// Transforms the samples `sample(0..size)`, oldest first, into `power[0..size / 2]`.
    pub(crate) fn power(&mut self, sample: impl Fn(usize) -> f32, power: &mut [f32]) {
        let size = self.size;
        for index in 0..size {
            self.real[self.reversal[index]] = sample(index) * self.taper[index];
            self.imaginary[index] = 0.0;
        }

        let mut length = 2;
        while length <= size {
            let half = length / 2;
            let stride = size / length;
            for start in (0..size).step_by(length) {
                for offset in 0..half {
                    let (cos, sin) = (self.cosine[offset * stride], self.sine[offset * stride]);
                    let (a, b) = (start + offset, start + offset + half);
                    let real_part = cos * self.real[b] - sin * self.imaginary[b];
                    let imaginary_part = sin * self.real[b] + cos * self.imaginary[b];
                    self.real[b] = self.real[a] - real_part;
                    self.imaginary[b] = self.imaginary[a] - imaginary_part;
                    self.real[a] += real_part;
                    self.imaginary[a] += imaginary_part;
                }
            }
            length <<= 1;
        }

        for (index, slot) in power.iter_mut().take(size / 2).enumerate() {
            *slot = (self.real[index] * self.real[index]
                + self.imaginary[index] * self.imaginary[index])
                * self.normalisation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_is_loudest_in_its_own_bin_and_reads_about_one_at_full_scale() {
        let size = 1_024;
        let rate = 48_000.0;
        let bin = 40;
        let frequency = bin as f32 * rate / size as f32;
        let mut spectrum = Spectrum::new(size);
        let mut power = vec![0.0; size / 2];
        spectrum.power(
            |index| (2.0 * PI * frequency * index as f32 / rate).sin(),
            &mut power,
        );

        let loudest = power
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(index, _)| index);
        assert_eq!(loudest, Some(bin));
        assert!((power[bin] - 1.0).abs() < 0.05, "read {}", power[bin]);
        assert!(power[bin + 10] < 1e-4, "a Hann window keeps the tone local");
    }

    #[test]
    fn silence_has_no_power() {
        let mut spectrum = Spectrum::new(64);
        let mut power = vec![1.0; 32];
        spectrum.power(|_| 0.0, &mut power);
        assert!(power.iter().all(|value| *value == 0.0));
    }
}
