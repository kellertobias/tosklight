//! Automatic ranging: an input gain for displays, and a per-band span for instrument levels.

/// The per-hop smoothing coefficient that gives `time_constant` seconds of response.
pub(crate) fn coefficient(hop_seconds: f32, time_constant: f32) -> f32 {
    1.0 - (-hop_seconds / time_constant.max(1e-6)).exp()
}

/// The per-hop factor that halves a value every `half_life` seconds.
pub(crate) fn decay(hop_seconds: f32, half_life: f32) -> f32 {
    0.5f32.powf(hop_seconds / half_life.max(1e-6))
}

/// Below this root-mean-square the input is treated as silence: -80 dBFS, under any real room.
pub(crate) const SILENCE: f32 = 1.0e-4;

/// The level the automatic gain brings a program to: about -14 dBFS RMS.
const TARGET_RMS: f32 = 0.2;
/// Enough to lift a quiet microphone by 30 dB; beyond that the gain amplifies only the room.
const MAXIMUM_GAIN: f32 = 30.0;
const MINIMUM_GAIN: f32 = 0.1;
/// Fast enough to catch a drop landing, so the first hit does not pin every meter.
const ATTACK_SECONDS: f32 = 0.05;
/// Slow enough that a breakdown reads as quieter rather than being pumped back up at once.
const RELEASE_SECONDS: f32 = 4.0;

/// Follows the program level and proposes the gain that brings it to a working level.
#[derive(Debug, Clone)]
pub(crate) struct AutoGain {
    envelope: f32,
    primed: bool,
    attack: f32,
    release: f32,
}

impl AutoGain {
    pub(crate) fn new(hop_seconds: f32) -> Self {
        Self {
            envelope: TARGET_RMS,
            primed: false,
            attack: coefficient(hop_seconds, ATTACK_SECONDS),
            release: coefficient(hop_seconds, RELEASE_SECONDS),
        }
    }

    /// Offers one hop's RMS and returns the followed program level.
    ///
    /// The first sound sets the level outright, so a quiet source is not ignored for the seconds a
    /// release would take to come down to it. Silence holds the level rather than releasing it, so
    /// a pause between songs does not wind the gain up to maximum and slam the next first bar.
    pub(crate) fn observe(&mut self, rms: f32) -> f32 {
        if rms > SILENCE && !self.primed {
            self.envelope = rms;
            self.primed = true;
        } else if rms > SILENCE {
            let rate = if rms > self.envelope {
                self.attack
            } else {
                self.release
            };
            self.envelope += (rms - self.envelope) * rate;
        }
        self.envelope
    }

    pub(crate) fn gain(&self) -> f32 {
        (TARGET_RMS / self.envelope.max(SILENCE)).clamp(MINIMUM_GAIN, MAXIMUM_GAIN)
    }
}

/// The narrowest span, in natural-log power, that is shown as the full range: about 13 dB.
///
/// Without it, a steady band with no dynamics would have its noise stretched to fill the meter.
const MINIMUM_SPAN: f32 = 3.0;
const FLOOR_SECONDS: f32 = 6.0;
const PEAK_SECONDS: f32 = 3.0;

/// Maps a logarithmic band power onto `0.0..=1.0` between its recent floor and peak.
#[derive(Debug, Clone)]
pub(crate) struct Span {
    floor: f32,
    peak: f32,
    primed: bool,
    floor_rise: f32,
    peak_fall: f32,
}

impl Span {
    pub(crate) fn new(hop_seconds: f32) -> Self {
        Self {
            floor: 0.0,
            peak: 0.0,
            primed: false,
            floor_rise: coefficient(hop_seconds, FLOOR_SECONDS),
            peak_fall: coefficient(hop_seconds, PEAK_SECONDS),
        }
    }

    pub(crate) fn observe(&mut self, value: f32) -> f32 {
        if !self.primed {
            self.floor = value;
            self.peak = value + MINIMUM_SPAN;
            self.primed = true;
        }
        // The floor falls quickly and rises slowly, so it follows the quiet moments between hits
        // rather than the average.
        if value < self.floor {
            self.floor += (value - self.floor) * 0.5;
        } else {
            self.floor += (value - self.floor) * self.floor_rise;
        }
        if value > self.peak {
            self.peak = value;
        } else {
            self.peak += (value - self.peak) * self.peak_fall;
        }
        ((value - self.floor) / (self.peak - self.floor).max(MINIMUM_SPAN)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOP: f32 = 512.0 / 48_000.0;

    #[test]
    fn a_quiet_input_is_lifted_and_a_loud_one_lowered() {
        let mut quiet = AutoGain::new(HOP);
        let mut loud = AutoGain::new(HOP);
        quiet.observe(0.2);
        for _ in 0..5_000 {
            quiet.observe(0.01);
            loud.observe(0.7);
        }
        assert!(
            (quiet.gain() - 20.0).abs() < 1.0,
            "quiet gain {}",
            quiet.gain()
        );
        assert!(loud.gain() < 0.4, "loud gain {}", loud.gain());
    }

    #[test]
    fn silence_holds_the_gain_instead_of_winding_it_up() {
        let mut gain = AutoGain::new(HOP);
        for _ in 0..500 {
            gain.observe(0.2);
        }
        let before = gain.gain();
        for _ in 0..5_000 {
            gain.observe(0.0);
        }
        assert_eq!(gain.gain(), before);
    }

    #[test]
    fn a_hit_attacks_fast_and_releases_slowly() {
        let mut gain = AutoGain::new(HOP);
        for _ in 0..1_000 {
            gain.observe(0.05);
        }
        for _ in 0..10 {
            gain.observe(0.8);
        }
        let after_hit = gain.gain();
        assert!(
            after_hit < 1.0,
            "a tenth of a second of a loud hit: {after_hit}"
        );
        for _ in 0..10 {
            gain.observe(0.05);
        }
        assert!(
            gain.gain() < after_hit * 1.2,
            "the release does not pump straight back"
        );
    }

    #[test]
    fn a_span_uses_the_whole_range_at_any_absolute_level() {
        for offset in [-20.0, 0.0, 20.0] {
            let mut span = Span::new(HOP);
            let mut top: f32 = 0.0;
            let mut bottom: f32 = 1.0;
            for hop in 0..3_000 {
                let value = offset + if hop % 40 < 4 { 6.0 } else { 0.0 };
                let level = span.observe(value);
                if hop > 1_000 {
                    top = top.max(level);
                    bottom = bottom.min(level);
                }
            }
            assert!(top > 0.9, "hits reach the top at offset {offset}: {top}");
            assert!(
                bottom < 0.1,
                "gaps reach the bottom at offset {offset}: {bottom}"
            );
        }
    }
}
