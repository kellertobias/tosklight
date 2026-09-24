use media_audio::AnalysisSnapshot;
use media_domain::BeatSource;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BeatEvents {
    counts: [u64; 5],
    legacy_pulse: f32,
}

impl From<&AnalysisSnapshot> for BeatEvents {
    fn from(heard: &AnalysisSnapshot) -> Self {
        Self {
            counts: [
                heard.beats,
                heard.kick_hits,
                heard.hihat_hits,
                heard.snare_hits,
                heard.detected_beats,
            ],
            legacy_pulse: 0.0,
        }
    }
}

// Existing coordinator tests use a pulse directly; production uses monotonically counted events.
impl From<f32> for BeatEvents {
    fn from(pulse: f32) -> Self {
        Self {
            legacy_pulse: pulse,
            ..Self::default()
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct BeatTracker {
    previous: [u64; 5],
    legacy_high: bool,
}

impl BeatTracker {
    pub(crate) fn landed(&mut self, events: BeatEvents) -> [bool; 5] {
        let high = events.legacy_pulse >= 0.95;
        let pulse_landed = high && !self.legacy_high;
        self.legacy_high = high;
        let mut landed = [false; 5];
        for (index, count) in events.counts.iter().copied().enumerate() {
            landed[index] = count > self.previous[index] || pulse_landed;
        }
        self.previous = events.counts;
        landed
    }
}

pub(crate) const fn source_index(source: BeatSource) -> usize {
    match source {
        BeatSource::LiveBeat => 0,
        BeatSource::Kick => 1,
        BeatSource::HiHat => 2,
        BeatSource::Snare => 3,
        BeatSource::DetectedBeat => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_survive_a_pulse_that_fades_between_rendered_frames() {
        let mut tracker = BeatTracker::default();
        assert!(!tracker.landed(BeatEvents::default())[0]);
        let mut events = BeatEvents::default();
        events.counts[0] = 1;
        assert!(tracker.landed(events)[0]);
        assert!(!tracker.landed(events)[0]);
        events.counts[0] = 2;
        assert!(tracker.landed(events)[0]);
    }
}
