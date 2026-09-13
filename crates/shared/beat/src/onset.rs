//! Deciding when a novelty curve holds an onset.
//!
//! The threshold follows the curve's own recent statistics — median plus a multiple of the mean
//! absolute deviation — so a dense passage and a sparse one both yield their real hits rather than
//! the dense one triggering on everything. An onset is confirmed one hop late, at a local peak, so
//! a rising transient is reported once at its top instead of on its way up.

/// How a band's onsets are picked.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Settings {
    /// Deviations above the median that count as a hit, at sensitivity one.
    pub(crate) deviations: f32,
    /// An absolute novelty floor in natural-log power, so a perfectly steady band cannot trigger
    /// on a vanishing deviation.
    pub(crate) floor: f32,
    /// The shortest time between two onsets in this band.
    pub(crate) refractory_seconds: f32,
}

/// How much history the threshold describes.
const HISTORY_SECONDS: f32 = 1.5;
/// Hops of history before "loud" means anything.
const WARM_UP: usize = 20;

#[derive(Debug, Clone)]
pub(crate) struct OnsetPicker {
    settings: Settings,
    history: Vec<f32>,
    scratch: Vec<f32>,
    write: usize,
    filled: usize,
    before: f32,
    candidate: f32,
    refractory_hops: u32,
    since_onset: u32,
    threshold: f32,
}

impl OnsetPicker {
    pub(crate) fn new(settings: Settings, hops_per_second: f32) -> Self {
        let capacity = ((HISTORY_SECONDS * hops_per_second).round() as usize).max(WARM_UP + 1);
        Self {
            settings,
            history: vec![0.0; capacity],
            scratch: vec![0.0; capacity],
            write: 0,
            filled: 0,
            before: 0.0,
            candidate: 0.0,
            refractory_hops: (settings.refractory_seconds * hops_per_second).round() as u32,
            since_onset: u32::MAX,
            threshold: 0.0,
        }
    }

    /// Offers this hop's novelty. Returns the strength, `0.0..=1.0`, of an onset confirmed at the
    /// previous hop.
    ///
    /// `armed` is false while the input is too quiet to mean anything; the history still grows so
    /// the threshold is ready when the music starts.
    pub(crate) fn offer(&mut self, novelty: f32, sensitivity: f32, armed: bool) -> Option<f32> {
        self.since_onset = self.since_onset.saturating_add(1);
        let candidate = self.candidate;
        let mut found = None;

        if self.filled >= WARM_UP {
            self.threshold = self.threshold_for(sensitivity);
            let is_peak =
                candidate > self.threshold && candidate >= self.before && candidate > novelty;
            if armed && is_peak && self.since_onset > self.refractory_hops {
                self.since_onset = 0;
                found = Some((1.0 - self.threshold / candidate).clamp(0.0, 1.0));
            }
        }

        self.history[self.write] = candidate;
        self.write = (self.write + 1) % self.history.len();
        self.filled = (self.filled + 1).min(self.history.len());
        self.before = candidate;
        self.candidate = novelty;
        found
    }

    /// The threshold the last offer was measured against. Zero until warmed up.
    pub(crate) fn threshold(&self) -> f32 {
        self.threshold
    }

    fn threshold_for(&mut self, sensitivity: f32) -> f32 {
        let values = &self.history[..self.filled];
        let count = values.len() as f32;
        let mean = values.iter().sum::<f32>() / count;
        let deviation = values.iter().map(|value| (value - mean).abs()).sum::<f32>() / count;

        let scratch = &mut self.scratch[..self.filled];
        scratch.copy_from_slice(values);
        let middle = scratch.len() / 2;
        let (_, median, _) = scratch.select_nth_unstable_by(middle, f32::total_cmp);

        let sensitivity = sensitivity.max(0.05);
        *median + (self.settings.deviations * deviation + self.settings.floor) / sensitivity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOPS: f32 = 48_000.0 / 512.0;

    fn picker() -> OnsetPicker {
        OnsetPicker::new(
            Settings {
                deviations: 2.0,
                floor: 0.2,
                refractory_seconds: 0.1,
            },
            HOPS,
        )
    }

    #[test]
    fn a_spike_over_a_steady_curve_is_found_once_at_its_peak() {
        let mut picker = picker();
        let curve: Vec<f32> = (0..200)
            .map(|hop| match hop {
                100 => 1.5,
                101 => 3.0,
                102 => 1.0,
                _ => 0.05,
            })
            .collect();
        let found: Vec<usize> = curve
            .iter()
            .enumerate()
            .filter_map(|(hop, value)| picker.offer(*value, 1.0, true).map(|_| hop - 1))
            .collect();
        assert_eq!(found, vec![101]);
    }

    #[test]
    fn a_steady_curve_has_no_onsets() {
        let mut picker = picker();
        assert!((0..500).all(|_| picker.offer(0.3, 4.0, true).is_none()));
    }

    #[test]
    fn two_peaks_inside_the_refractory_time_are_one_onset() {
        let mut picker = picker();
        let mut count = 0;
        for hop in 0..200 {
            let value = if hop == 100 || hop == 104 { 3.0 } else { 0.05 };
            count += usize::from(picker.offer(value, 1.0, true).is_some());
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn a_disarmed_picker_learns_but_does_not_report() {
        let mut picker = picker();
        let mut count = 0;
        for hop in 0..200 {
            let value = if hop % 50 == 25 { 3.0 } else { 0.05 };
            count += usize::from(picker.offer(value, 1.0, false).is_some());
        }
        assert_eq!(count, 0);
        assert!(picker.threshold() > 0.0);
    }
}
