use light_playback::PlaybackTelemetrySample;

use super::PlaybackShowScope;

/// A sampled playback-telemetry tick taken on a render-frame divider nearest ~10 Hz.
///
/// Delta-oriented: `samples` holds only the playbacks whose sampled values changed since the
/// previous tick, and `released` the numbers that stopped reporting. No change → no tick.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackTelemetryTick {
    pub scope: PlaybackShowScope,
    /// Completed render frame this tick was sampled on.
    pub frame: u64,
    /// The telemetry sample rate implied by the configured output rate and its divider.
    pub sample_rate_hz: f32,
    pub samples: Vec<PlaybackTelemetrySample>,
    pub released: Vec<u16>,
}

/// The render-frame divider that brings `rate_hz` nearest ~10 Hz telemetry.
///
/// Derived from the frame counter — not a wall-clock timer — so samples stay frame-coherent,
/// never beat against the output clock, and add no timer wakeups. 44 Hz → 4 (≈11 Hz);
/// 40/100/120 Hz → exactly 10 Hz.
pub fn telemetry_frame_divider(rate_hz: u16) -> u64 {
    let rate = u64::from(rate_hz.max(1));
    let mut best = 1;
    let mut best_distance = f64::INFINITY;
    for divider in 1..=rate {
        if rate % divider != 0 {
            continue;
        }
        let distance = (rate as f64 / divider as f64 - 10.0).abs();
        if distance < best_distance {
            best_distance = distance;
            best = divider;
        }
    }
    best
}

/// Delta detection between consecutive telemetry sweeps.
///
/// Keeps the previous sweep keyed by playback number; `advance` returns only changed samples
/// and the numbers that disappeared.
#[derive(Debug, Default)]
pub struct PlaybackTelemetryDeltas {
    previous: std::collections::HashMap<u16, PlaybackTelemetrySample>,
}

impl PlaybackTelemetryDeltas {
    pub fn advance(
        &mut self,
        sweep: Vec<PlaybackTelemetrySample>,
    ) -> (Vec<PlaybackTelemetrySample>, Vec<u16>) {
        let mut next = std::collections::HashMap::with_capacity(sweep.len());
        let mut changed = Vec::new();
        for sample in sweep {
            if self.previous.get(&sample.playback_number) != Some(&sample) {
                changed.push(sample.clone());
            }
            next.insert(sample.playback_number, sample);
        }
        let mut released: Vec<u16> = self
            .previous
            .keys()
            .filter(|number| !next.contains_key(number))
            .copied()
            .collect();
        released.sort_unstable();
        self.previous = next;
        (changed, released)
    }

    pub fn reset(&mut self) {
        self.previous.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::telemetry_frame_divider;

    #[test]
    fn frame_divider_is_the_divisor_nearest_ten_hertz() {
        assert_eq!(telemetry_frame_divider(44), 4); // ≈11 Hz
        assert_eq!(telemetry_frame_divider(40), 4); // 10 Hz
        assert_eq!(telemetry_frame_divider(100), 10); // 10 Hz
        assert_eq!(telemetry_frame_divider(120), 12); // 10 Hz
        assert_eq!(telemetry_frame_divider(30), 3); // 10 Hz
        assert_eq!(telemetry_frame_divider(1), 1);
    }
}
