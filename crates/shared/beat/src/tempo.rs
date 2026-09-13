//! Tempo and beat phase.
//!
//! The tempo comes from the periodicity of the onset curve — its autocorrelation over the last
//! few bars, with the double and quadruple lags added so a bar-long pattern reinforces its beat —
//! rather than from the gaps between individual detected hits. A missed or extra hit therefore
//! moves the estimate very little. A mild preference for tempi near 120 BPM settles the choice
//! between a tempo and its half or double, which the curve alone cannot tell apart.
//!
//! The phase is a flywheel: it advances at the tempo every hop and is nudged toward kick onsets
//! that land near a predicted beat. It keeps counting through a breakdown with no kick.

use crate::level::decay;

const WINDOW_SECONDS: f32 = 8.0;
const EVALUATE_SECONDS: f32 = 0.5;
const MINIMUM_SECONDS: f32 = 4.0;
const FASTEST_BPM: f32 = 200.0;
const SLOWEST_BPM: f32 = 60.0;
const PREFERRED_BPM: f32 = 120.0;
/// How widely the preference spreads, in octaves. Wide enough that 170 BPM drum and bass still
/// wins over its own half-time when the curve supports it.
const PREFERENCE_OCTAVES: f32 = 0.8;
/// Correlation below this is not a tempo.
const MINIMUM_CONFIDENCE: f32 = 0.05;
/// Relative difference within which a new estimate refines the current tempo instead of replacing
/// it.
const SAME_TEMPO: f32 = 0.04;
/// Evaluations a different tempo must win in a row before it replaces the current one: three
/// seconds, so a breakdown's stray periodicity does not jerk every chase onto a new grid.
const ADOPT_AFTER: u8 = 6;
/// A challenger must be at least this sure, relative to the settled tempo, to count a win.
const CHALLENGE: f32 = 0.8;
const FOLLOW: f32 = 0.25;
/// Within this relative difference the kick-locked flywheel knows the tempo more finely than the
/// periodicity estimate, which moves in whole-hop steps, so the estimate only drifts it gently.
const LOCKED_TEMPO: f32 = 0.015;
const LOCKED_FOLLOW: f32 = 0.02;
/// How far from a predicted beat, as a fraction of a beat, a kick still counts as that beat.
const CAPTURE: f32 = 0.3;
/// How much of a kick's phase error the flywheel takes up at once.
const PULL: f32 = 0.25;
/// How much of a kick's phase error becomes a tempo correction. Without it, a flywheel running a
/// fraction slow settles a constant few milliseconds behind every kick instead of on it.
const NUDGE: f32 = 0.05;
/// Kicks away from every predicted beat, in a row, before the phase restarts on one.
const RESYNC_AFTER: u8 = 4;
/// How quickly confidence fades once the music stops.
const CONFIDENCE_HALF_LIFE_SECONDS: f32 = 4.0;
/// How quickly the settled tempo's authority fades while nothing confirms it. Long, so a vague
/// stretch cannot talk its way onto the grid, but finite, so a new song eventually does.
const SETTLED_HALF_LIFE_SECONDS: f32 = 30.0;

#[derive(Debug, Clone)]
pub(crate) struct Tempo {
    hops_per_second: f32,
    envelope: Vec<f32>,
    write: usize,
    filled: usize,
    ordered: Vec<f32>,
    correlation: Vec<f32>,
    evaluate_every: usize,
    since_evaluation: usize,
    minimum_filled: usize,
    shortest_lag: usize,
    longest_lag: usize,
    bpm: f32,
    confidence: f32,
    /// How sure the evidence for the current tempo was. Fades while nothing confirms it, so a new
    /// song can eventually take over.
    settled: f32,
    settled_decay: f32,
    confidence_decay: f32,
    pending: Option<(f32, u8)>,
    phase: f32,
    misses: u8,
}

impl Tempo {
    pub(crate) fn new(hops_per_second: f32) -> Self {
        let capacity = (WINDOW_SECONDS * hops_per_second).round() as usize;
        let shortest_lag = ((60.0 * hops_per_second / FASTEST_BPM).floor() as usize).max(2);
        let longest_lag = (60.0 * hops_per_second / SLOWEST_BPM).ceil() as usize;
        let correlation_len = (4 * longest_lag + 2).min(capacity);
        Self {
            hops_per_second,
            envelope: vec![0.0; capacity],
            write: 0,
            filled: 0,
            ordered: vec![0.0; capacity],
            correlation: vec![0.0; correlation_len],
            evaluate_every: ((EVALUATE_SECONDS * hops_per_second).round() as usize).max(1),
            since_evaluation: 0,
            minimum_filled: (MINIMUM_SECONDS * hops_per_second).round() as usize,
            shortest_lag,
            longest_lag,
            bpm: 0.0,
            confidence: 0.0,
            settled: 0.0,
            settled_decay: decay(EVALUATE_SECONDS, SETTLED_HALF_LIFE_SECONDS),
            confidence_decay: decay(EVALUATE_SECONDS, CONFIDENCE_HALF_LIFE_SECONDS),
            pending: None,
            phase: 0.0,
            misses: 0,
        }
    }

    /// Offers one hop of onset strength.
    pub(crate) fn observe(&mut self, strength: f32) {
        self.envelope[self.write] = strength;
        self.write = (self.write + 1) % self.envelope.len();
        self.filled = (self.filled + 1).min(self.envelope.len());
        self.since_evaluation += 1;
        if self.since_evaluation >= self.evaluate_every && self.filled >= self.minimum_filled {
            self.since_evaluation = 0;
            self.evaluate();
        }
    }

    /// Advances the flywheel one hop. Returns whether a beat was crossed.
    pub(crate) fn advance(&mut self) -> bool {
        if self.bpm <= 0.0 {
            return false;
        }
        self.phase += self.bpm / 60.0 / self.hops_per_second;
        if self.phase >= 1.0 {
            self.phase = self.phase.fract();
            return true;
        }
        false
    }

    /// Pulls the flywheel toward a kick that was confirmed `latency_seconds` after it sounded.
    ///
    /// The error is measured where the kick *sounded*, so the beat lands on the kick rather than
    /// on the moment the detector was sure of it.
    pub(crate) fn align(&mut self, strength: f32, latency_seconds: f32) {
        if self.bpm <= 0.0 {
            return;
        }
        let elapsed = latency_seconds * self.bpm / 60.0;
        let struck = (self.phase - elapsed).rem_euclid(1.0);
        let error = if struck >= 0.5 { struck - 1.0 } else { struck };
        if error.abs() < CAPTURE {
            self.misses = 0;
            let weight = strength.clamp(0.3, 1.0);
            self.phase = (self.phase - error * PULL * weight).max(0.0);
            // A kick landing early says the flywheel runs slow; landing late, fast.
            self.bpm = (self.bpm * (1.0 - error * NUDGE * weight)).clamp(SLOWEST_BPM, FASTEST_BPM);
        } else {
            self.misses += 1;
            if self.misses >= RESYNC_AFTER {
                // Consistently off the grid: the flywheel was started on the wrong hit.
                self.misses = 0;
                self.phase = elapsed.fract();
            }
        }
    }

    pub(crate) fn bpm(&self) -> f32 {
        self.bpm
    }

    pub(crate) fn confidence(&self) -> f32 {
        self.confidence
    }

    pub(crate) fn phase(&self) -> f32 {
        self.phase
    }

    fn evaluate(&mut self) {
        let count = self.filled;
        let capacity = self.envelope.len();
        let start = (self.write + capacity - count) % capacity;
        for index in 0..count {
            self.ordered[index] = self.envelope[(start + index) % capacity];
        }
        let values = &mut self.ordered[..count];
        let mean = values.iter().sum::<f32>() / count as f32;
        values.iter_mut().for_each(|value| *value -= mean);
        let energy = values.iter().map(|value| value * value).sum::<f32>();
        if energy <= 1e-6 {
            self.confidence *= self.confidence_decay;
            return;
        }

        let longest = self.correlation.len().min(count);
        for lag in 0..longest {
            let sum: f32 = values[..count - lag]
                .iter()
                .zip(&values[lag..])
                .map(|(a, b)| a * b)
                .sum();
            self.correlation[lag] = sum / energy;
        }
        let correlation = |lag: usize| {
            if lag < longest {
                self.correlation[lag]
            } else {
                0.0
            }
        };
        let hops_per_second = self.hops_per_second;
        let score = |lag: usize| {
            let bpm = 60.0 * hops_per_second / lag as f32;
            let octaves = (bpm / PREFERRED_BPM).log2() / PREFERENCE_OCTAVES;
            let preference = (-0.5 * octaves * octaves).exp();
            (correlation(lag) + 0.5 * correlation(2 * lag) + 0.25 * correlation(4 * lag))
                * preference
        };

        let longest_lag = self.longest_lag.min(longest.saturating_sub(2));
        if longest_lag <= self.shortest_lag {
            return;
        }
        let best = (self.shortest_lag..=longest_lag)
            .max_by(|a, b| score(*a).total_cmp(&score(*b)))
            .unwrap_or(self.shortest_lag);

        // A parabola through the neighbours places the peak between whole hops, which is the
        // difference between 143 and 144.2 BPM at this resolution.
        let mut offset = 0.0;
        if best > self.shortest_lag && best < longest_lag {
            let (before, at, after) = (score(best - 1), score(best), score(best + 1));
            let curvature = before - 2.0 * at + after;
            if curvature.abs() > 1e-9 {
                offset = (0.5 * (before - after) / curvature).clamp(-0.5, 0.5);
            }
        }

        let candidate = 60.0 * hops_per_second / (best as f32 + offset);
        let confidence = correlation(best).clamp(0.0, 1.0);
        self.confidence = confidence;
        if confidence < MINIMUM_CONFIDENCE {
            self.settled *= self.settled_decay;
            return;
        }
        self.adopt(candidate, confidence);
    }

    fn adopt(&mut self, candidate: f32, confidence: f32) {
        if self.bpm <= 0.0 {
            self.bpm = candidate;
            self.settled = confidence;
            self.phase = 0.0;
            self.pending = None;
            return;
        }
        let difference = (candidate / self.bpm - 1.0).abs();
        if difference < SAME_TEMPO {
            let follow = if difference < LOCKED_TEMPO {
                LOCKED_FOLLOW
            } else {
                FOLLOW
            };
            self.bpm += (candidate - self.bpm) * follow;
            self.settled += (confidence - self.settled) * FOLLOW;
            self.pending = None;
            return;
        }
        self.settled *= self.settled_decay;
        if confidence < self.settled * CHALLENGE {
            self.pending = None;
            return;
        }
        self.pending = match self.pending {
            Some((pending, wins)) if (candidate / pending - 1.0).abs() < SAME_TEMPO => {
                if wins + 1 >= ADOPT_AFTER {
                    self.bpm = candidate;
                    self.settled = confidence;
                    None
                } else {
                    Some((candidate, wins + 1))
                }
            }
            _ => Some((candidate, 1)),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOPS: f32 = 48_000.0 / 512.0;

    fn run(tempo: &mut Tempo, bpm: f32, seconds: f32) {
        let hops = (seconds * HOPS) as usize;
        let period = 60.0 * HOPS / bpm;
        for hop in 0..hops {
            let position = hop as f32 % period;
            tempo.observe(if position < 1.0 { 1.0 } else { 0.0 });
        }
    }

    #[test]
    fn a_pulse_train_reports_its_tempo() {
        for bpm in [90.0, 128.0, 144.0, 160.0] {
            let mut tempo = Tempo::new(HOPS);
            run(&mut tempo, bpm, 12.0);
            assert!(
                (tempo.bpm() - bpm).abs() / bpm < 0.02,
                "{bpm} BPM read as {}",
                tempo.bpm()
            );
        }
    }

    #[test]
    fn drum_and_bass_reads_as_its_tempo_or_its_half_time() {
        // A bare pulse at 174 BPM carries no evidence for one reading over the other; both put a
        // chase on the same grid.
        let mut tempo = Tempo::new(HOPS);
        run(&mut tempo, 174.0, 12.0);
        let read = tempo.bpm();
        assert!(
            (read - 174.0).abs() < 3.5 || (read - 87.0).abs() < 1.8,
            "read {read}"
        );
    }

    #[test]
    fn no_tempo_before_there_is_evidence() {
        let mut tempo = Tempo::new(HOPS);
        run(&mut tempo, 120.0, 2.0);
        assert_eq!(tempo.bpm(), 0.0);
    }

    #[test]
    fn one_odd_estimate_does_not_replace_a_settled_tempo() {
        let mut tempo = Tempo::new(HOPS);
        run(&mut tempo, 128.0, 12.0);
        let sure = tempo.confidence();
        for _ in 1..ADOPT_AFTER {
            tempo.adopt(100.0, sure);
        }
        assert!((tempo.bpm() - 128.0).abs() < 2.0);
        tempo.adopt(100.0, sure);
        assert_eq!(
            tempo.bpm(),
            100.0,
            "but a persistent, equally sure one does"
        );
    }

    #[test]
    fn a_vague_challenger_never_replaces_a_settled_tempo() {
        let mut tempo = Tempo::new(HOPS);
        run(&mut tempo, 128.0, 12.0);
        let vague = tempo.confidence() * 0.3;
        for _ in 0..(ADOPT_AFTER * 3) {
            tempo.adopt(100.0, vague);
        }
        assert!((tempo.bpm() - 128.0).abs() < 2.0, "read {}", tempo.bpm());
    }

    #[test]
    fn the_flywheel_keeps_beating_and_a_kick_pulls_it_into_line() {
        let mut tempo = Tempo::new(HOPS);
        run(&mut tempo, 120.0, 12.0);
        let hops_per_beat = (60.0 * HOPS / 120.0) as usize;
        let beats = (0..hops_per_beat * 8).filter(|_| tempo.advance()).count();
        assert!((7..=9).contains(&beats), "{beats} beats in eight");

        // A kick a tenth of a beat late moves the phase toward it.
        while tempo.phase() > 0.05 {
            tempo.advance();
        }
        let before = tempo.phase();
        for _ in 0..(hops_per_beat / 10) {
            tempo.advance();
        }
        let late = tempo.phase();
        tempo.align(1.0, 0.0);
        assert!(tempo.phase() < late && tempo.phase() >= before.min(late) * 0.5);
    }
}
