use crate::SpectrumSample;
use std::sync::OnceLock;

pub(super) const SAMPLES: usize = 471;
pub(super) type Spectrum = [f64; SAMPLES];
static OBSERVER: OnceLock<[[f64; 3]; SAMPLES]> = OnceLock::new();

pub(super) fn observer() -> &'static [[f64; 3]; SAMPLES] {
    OBSERVER.get_or_init(|| {
        let mut lines = include_str!("data/CIE_xyz_1931_2deg.csv").lines();
        std::array::from_fn(|index| {
            let mut values = lines.next().expect("vendored CIE row").split(',');
            assert_eq!(
                values.next().unwrap().parse::<usize>().unwrap(),
                360 + index
            );
            std::array::from_fn(|_| values.next().unwrap().parse::<f64>().unwrap())
        })
    })
}

/// Missing tails are unknown. Only authored coverage can establish complete visible output.
pub(super) fn resample(samples: &[SpectrumSample]) -> Option<Box<Spectrum>> {
    if samples.first()?.wavelength_nm > 360.0 || samples.last()?.wavelength_nm < 830.0 {
        return None;
    }
    // The compiled evaluator uses a 1 nm grid. Do not erase sub-nanometre peaks or
    // notches by sampling past them; retain uncertainty until a finer model is available.
    if samples
        .iter()
        .any(|s| (360.0..830.0).contains(&s.wavelength_nm) && s.wavelength_nm.fract() != 0.0)
    {
        return None;
    }
    Some(Box::new(std::array::from_fn(|index| {
        let nm = (360 + index) as f64;
        let upper = samples
            .partition_point(|s| f64::from(s.wavelength_nm) < nm)
            .clamp(1, samples.len() - 1);
        let a = &samples[upper - 1];
        let b = &samples[upper];
        let t = (nm - f64::from(a.wavelength_nm))
            / (f64::from(b.wavelength_nm) - f64::from(a.wavelength_nm));
        f64::from(a.value) + t * (f64::from(b.value) - f64::from(a.value))
    })))
}

/// Trapezoidal 1 nm integration, common path-relative density scale. Never normalize emitters.
pub(super) fn integrate(spectrum: &Spectrum, transmission: Option<&Spectrum>) -> [f64; 3] {
    let mut xyz = [0.0; 3];
    for (index, cmf) in observer().iter().enumerate() {
        let endpoint = if index == 0 || index == SAMPLES - 1 {
            0.5
        } else {
            1.0
        };
        let value = spectrum[index] * endpoint * transmission.map_or(1.0, |t| t[index]);
        for component in 0..3 {
            xyz[component] += value * cmf[component];
        }
    }
    xyz
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn official_observer_bytes_and_reference_wavelength_are_pinned() {
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(include_bytes!("data/CIE_xyz_1931_2deg.csv"))
            ),
            "fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1"
        );
        assert_eq!(observer()[555 - 360][1], 1.0);
        let white = integrate(&[1.0; SAMPLES], None);
        assert!((white[1] - 106.856915).abs() < 0.00001);
        let half = integrate(&[0.5; SAMPLES], None);
        assert_eq!(half.map(|v| v * 2.0), white);
    }
    #[test]
    fn sub_nanometre_feature_is_not_misreported_as_complete_black() {
        let samples = [
            (360.0, 0.0),
            (500.2, 0.0),
            (500.3, 1.0),
            (500.4, 0.0),
            (830.0, 0.0),
        ]
        .map(|(wavelength_nm, value)| SpectrumSample {
            wavelength_nm,
            value,
        });
        assert!(resample(&samples).is_none());
    }
    #[test]
    fn missing_fixture_coverage_never_becomes_zero_tails() {
        assert!(
            resample(&[
                SpectrumSample {
                    wavelength_nm: 380.0,
                    value: 1.0
                },
                SpectrumSample {
                    wavelength_nm: 780.0,
                    value: 1.0
                }
            ])
            .is_none()
        );
    }
}
