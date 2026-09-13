//! The streaming detector: samples in, onsets and a reading out.

use std::ops::Range;

use crate::filter::KickFilter;
use crate::level::{AutoGain, SILENCE, Span, decay};
use crate::onset::{OnsetPicker, Settings};
use crate::spectrum::Spectrum;
use crate::tempo::Tempo;

/// Samples per analysis step: about 11 ms at 48 kHz, well inside one 60 fps frame.
pub const HOP: usize = 512;
/// The long frame resolves the kick: 23 Hz bins at 48 kHz, where a short frame would give the
/// whole kick one bin.
const FRAME: usize = 2_048;
/// The short frame times the hi-hat and snare. It does not overlap its neighbours, so a click
/// shorter than a millisecond lives in exactly one of them — which is how it is told apart from a
/// cymbal that rings.
const SHORT: usize = HOP;

/// A hop more than 34 dB under the recent program level is a pause, not a quiet hit.
const RELATIVE_GATE: f32 = 0.02;
/// The share of the whole spectrum the kick band must hold for a rise in it to be a kick.
///
/// Detection is level-independent, which also means an empty bass band would report every
/// microscopic rise — a hi-hat's faint low-frequency leak, say. A real kick carries a large share
/// of a mix's power; one percent (-20 dB) leaves a wide margin for thin playback systems.
const KICK_SHARE: f32 = 0.01;
const CLIP_LEVEL: f32 = 0.999;
const CLIPPED_SAMPLES: u32 = 3;
const CLIP_HOLD_SECONDS: f32 = 0.5;
const POWER_FLOOR: f32 = 1.0e-12;
/// How much of an onset's rise must still be there one hop later. A hi-hat or snare rings and
/// keeps most of it; an electrical click or a converter glitch keeps none. Measured on a room
/// microphone full of such clicks, this rejects them without losing the hats.
const PERSISTENCE: f32 = 0.35;
/// A hit fires when an instrument's auto-ranged level rises through this mark, at sensitivity one,
/// and re-arms once the level has fallen below `RELEASE` of it.
///
/// Measured against the beat grid of a room-microphone recording, this found twice the kicks and
/// hi-hats the onset curve did, at better precision: it follows the pumping an operator sees in
/// the level, including soft-attack bass that never rises sharply enough to count as an onset.
const TRIGGER: f32 = 0.7;
const RELEASE: f32 = 0.5;
/// The shortest time between two hits, as a multiple of the voice's onset refractory time.
const HIT_GAP: f32 = 1.5;
/// The kick's level comes from a fast filter that hears a hit within milliseconds but also hears
/// every bass note swell. Measured on a room microphone, three rules keep it to kicks: at most one
/// hit per 300 ms (a 200 BPM quarter note), and a rise of at least a fifth of the range within the
/// last four hops, so a slow swell through the trigger does not count. That kept most of the
/// accuracy of the slow spectral level while reporting kicks about 50 ms sooner.
const KICK_HIT_GAP_SECONDS: f32 = 0.3;
const KICK_ATTACK: f32 = 0.2;
const ATTACK_HOPS: usize = 4;
const TEMPO_NOVELTY_CAP: f32 = 4.0;
/// How long after a kick sounds the detector confirms it: the long frame fills with the hit, the
/// three-hop smoothing rises, the peak hop ends, and one more hop confirms it was the peak.
/// Measured on synthetic kicks at 48 kHz as 38–48 ms, median 42 ms. The beat flywheel subtracts
/// it, so a light flashes with the kick rather than a frame or two behind it.
const KICK_LATENCY_SECONDS: f32 = 0.042;
const BEAT_HALF_LIFE_SECONDS: f32 = 0.1;

const KICK: Settings = Settings {
    deviations: 2.5,
    floor: 0.3,
    refractory_seconds: 0.1,
};
const SNARE: Settings = Settings {
    deviations: 3.5,
    floor: 0.3,
    refractory_seconds: 0.08,
};
const HIHAT: Settings = Settings {
    deviations: 2.5,
    floor: 0.6,
    refractory_seconds: 0.05,
};

/// What struck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Instrument {
    /// 40–120 Hz: the kick drum, or a bass note hard enough to act as one.
    Kick,
    /// 150–400 Hz body with 1.5–4 kHz crack: snare, clap, rim.
    Snare,
    /// 6–12 kHz and ringing: closed and open hi-hat, shaker, ride.
    HiHat,
}

/// One detected hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Onset {
    pub instrument: Instrument,
    /// The instrument's auto-ranged level when it struck, `0.0..=1.0`. For brightness, not for
    /// deciding.
    pub strength: f32,
    /// The input sample count at the end of the hop before the hit was reported.
    pub sample: u64,
}

/// One instrument's state, ready to drive a visual.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Voice {
    /// The band's loudness between its own recent floor and peak, `0.0..=1.0`.
    pub level: f32,
    /// `1.0` on the hop a hit landed, decaying afterwards, so a light can flash and fall.
    pub hit: f32,
    /// The strength of the most recent hit.
    pub strength: f32,
    /// Hits since the detector started.
    pub count: u64,
}

/// Everything the detector knows after the latest hop.
///
/// The default is silence before any audio, a real state rather than a placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Reading {
    pub kick: Voice,
    pub snare: Voice,
    pub hihat: Voice,
    /// Zero until there is evidence for a tempo.
    pub bpm: f32,
    /// How periodic the music is, `0.0..=1.0`. Low in a breakdown or with no clear pulse.
    pub tempo_confidence: f32,
    /// Where this instant sits between beats, `0.0..1.0`.
    pub beat_phase: f32,
    /// `1.0` when the beat flywheel crosses a beat, decaying afterwards. Before a tempo is known
    /// it follows the kick instead, so something still pulses from the first bar.
    pub beat: f32,
    /// Beats counted since the detector started.
    pub beats: u64,
    /// The gain a level display should apply: automatic, or the operator's manual gain.
    pub gain: f32,
    /// The root-mean-square of the latest hop before any gain.
    pub input_rms: f32,
    /// The input reached full scale recently. Gain applied here cannot repair that; the source
    /// or the interface input needs turning down.
    pub clipping: bool,
}

/// What an operator adjusts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    /// Scales every threshold. Higher finds quieter hits; one is the measured default.
    pub sensitivity: f32,
    /// Follow the program level automatically. Detection does not need it; displays do.
    pub auto_gain: bool,
    /// The gain reported when automatic gain is off.
    pub manual_gain: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            sensitivity: 1.0,
            auto_gain: true,
            manual_gain: 1.0,
        }
    }
}

/// Novelty: how far a value rose over the highest of the few before it.
///
/// Comparing with the highest rather than the last lets a hit that takes two hops to build count
/// once, fully, instead of twice at half height.
#[derive(Debug, Clone, Copy)]
struct Rise<const LAG: usize> {
    previous: [f32; LAG],
    primed: bool,
}

impl<const LAG: usize> Rise<LAG> {
    const fn new() -> Self {
        Self {
            previous: [0.0; LAG],
            primed: false,
        }
    }

    fn next(&mut self, value: f32) -> f32 {
        if !self.primed {
            self.previous = [value; LAG];
            self.primed = true;
            return 0.0;
        }
        let reference = self.previous.iter().copied().fold(f32::MIN, f32::max);
        self.previous.rotate_right(1);
        self.previous[0] = value;
        (value - reference).max(0.0)
    }
}

/// A three-hop mean, which steadies the kick band's few bins without delaying a hit by more than
/// a hop.
#[derive(Debug, Clone, Copy)]
struct Smooth {
    values: [f32; 3],
    primed: bool,
}

impl Smooth {
    fn next(&mut self, value: f32) -> f32 {
        if !self.primed {
            self.values = [value; 3];
            self.primed = true;
        }
        self.values.rotate_right(1);
        self.values[0] = value;
        self.values.iter().sum::<f32>() / 3.0
    }
}

/// What one hop of a voice produced.
#[derive(Debug, Clone, Copy, Default)]
struct Heard {
    /// A sharp rise on the onset curve: early and exact, which is what the beat flywheel wants.
    onset: Option<f32>,
    /// The level striking through its trigger: what a light flashes on.
    hit: Option<f32>,
}

#[derive(Debug, Clone)]
struct VoiceState {
    picker: OnsetPicker,
    span: Span,
    hit_decay: f32,
    rings: bool,
    /// The band level two hops ago and one hop ago.
    recent: [f32; 2],
    /// Whether the level has fallen far enough since the last hit to strike again.
    ready: bool,
    /// A ringing voice's crossing waits one hop to prove it was not a click.
    pending: Option<f32>,
    gap_hops: u32,
    since_hit: u32,
    /// How far the level must have risen within the last few hops for a crossing to count.
    attack: f32,
    levels: [f32; ATTACK_HOPS],
    voice: Voice,
}

impl VoiceState {
    fn new(settings: Settings, rings: bool, half_life: f32, hop_seconds: f32) -> Self {
        Self {
            picker: OnsetPicker::new(settings, 1.0 / hop_seconds),
            span: Span::new(hop_seconds),
            hit_decay: decay(hop_seconds, half_life),
            rings,
            recent: [0.0; 2],
            ready: true,
            pending: None,
            gap_hops: (settings.refractory_seconds * HIT_GAP / hop_seconds).round() as u32,
            since_hit: u32::MAX,
            attack: 0.0,
            levels: [0.0; ATTACK_HOPS],
            voice: Voice::default(),
        }
    }

    /// Only counts a crossing that is part of a sharp attack, at most once per `gap_seconds`.
    fn struck_on_attack(mut self, gap_seconds: f32, attack: f32, hop_seconds: f32) -> Self {
        self.gap_hops = (gap_seconds / hop_seconds).round() as u32;
        self.attack = attack;
        self
    }

    fn offer(&mut self, novelty: f32, level: f32, sensitivity: f32, armed: bool) -> Heard {
        self.voice.hit *= self.hit_decay;
        self.voice.level = if armed {
            self.span.observe(level)
        } else {
            self.voice.level * self.hit_decay
        };
        let [before, peak] = self.recent;
        self.recent = [peak, level];

        let rise = peak - before;
        let clicked = self.rings && rise > 0.0 && level - before < PERSISTENCE * rise;
        let onset = self
            .picker
            .offer(novelty, sensitivity, armed)
            .filter(|_| !clicked);

        let hit = self.strike(sensitivity, armed);
        if let Some(strength) = hit {
            self.voice.hit = 1.0;
            self.voice.strength = strength;
            self.voice.count += 1;
        }
        Heard { onset, hit }
    }

    /// Fires when the level rises through its trigger, and re-arms once it has fallen back.
    fn strike(&mut self, sensitivity: f32, armed: bool) -> Option<f32> {
        self.since_hit = self.since_hit.saturating_add(1);
        let trigger = (1.0 - (1.0 - TRIGGER) * sensitivity.max(0.0)).clamp(0.2, 0.95);
        let release = trigger * RELEASE;
        let level = self.voice.level;

        let mut hit = None;
        if let Some(struck) = self.pending.take() {
            // Still sounding a hop later: a cymbal or a snare, not a click.
            if armed && level >= release {
                hit = Some(struck);
            }
        }
        if level <= release {
            self.ready = true;
        }
        let lowest = self.levels.iter().copied().fold(f32::MAX, f32::min);
        let attacking = level - lowest >= self.attack;
        self.levels.rotate_right(1);
        self.levels[0] = level;
        let crossed =
            armed && self.ready && attacking && level >= trigger && self.since_hit >= self.gap_hops;
        if crossed && hit.is_none() {
            self.ready = false;
            self.since_hit = 0;
            if self.rings {
                self.pending = Some(level);
            } else {
                hit = Some(level);
            }
        }
        hit
    }
}

/// Listens to one mono stream.
#[derive(Debug, Clone)]
pub struct Detector {
    tuning: Tuning,
    ring: Vec<f32>,
    write: usize,
    since_hop: usize,
    hop_energy: f32,
    hop_clipped: u32,
    samples: u64,
    long: Spectrum,
    short: Spectrum,
    long_power: Vec<f32>,
    short_power: Vec<f32>,
    kick_bins: Range<usize>,
    body_bins: Range<usize>,
    crack_bins: Range<usize>,
    hat_bins: Range<usize>,
    kick_smooth: Smooth,
    /// The kick's fast path: filtered energy of this hop and the one before.
    kick_filter: KickFilter,
    kick_energy: f32,
    kick_energy_before: f32,
    kick_rise: Rise<3>,
    body_rise: Rise<2>,
    crack_rise: Rise<2>,
    hat_rise: Rise<2>,
    kick: VoiceState,
    snare: VoiceState,
    hihat: VoiceState,
    tempo: Tempo,
    beat: f32,
    beats: u64,
    beat_decay: f32,
    gain: AutoGain,
    clip_hold: u32,
    clip_hold_hops: u32,
    reading: Reading,
}

impl Detector {
    /// A detector for a stream at `sample_rate`. Rates below 16 kHz are treated as 16 kHz, where
    /// the hi-hat band still exists.
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = if sample_rate.is_finite() {
            sample_rate.max(16_000.0)
        } else {
            48_000.0
        };
        let hop_seconds = HOP as f32 / sample_rate;
        let hops_per_second = 1.0 / hop_seconds;
        let nyquist = sample_rate / 2.0;
        Self {
            tuning: Tuning::default(),
            ring: vec![0.0; FRAME],
            write: 0,
            since_hop: 0,
            hop_energy: 0.0,
            hop_clipped: 0,
            samples: 0,
            long: Spectrum::new(FRAME),
            short: Spectrum::new(SHORT),
            long_power: vec![0.0; FRAME / 2],
            short_power: vec![0.0; SHORT / 2],
            kick_bins: bins(40.0, 120.0, sample_rate, FRAME),
            body_bins: bins(150.0, 400.0, sample_rate, FRAME),
            crack_bins: bins(1_500.0, 4_000.0, sample_rate, SHORT),
            hat_bins: bins(6_000.0, 12_000.0f32.min(nyquist * 0.95), sample_rate, SHORT),
            kick_smooth: Smooth {
                values: [0.0; 3],
                primed: false,
            },
            kick_filter: KickFilter::new(sample_rate),
            kick_energy: 0.0,
            kick_energy_before: 0.0,
            kick_rise: Rise::new(),
            body_rise: Rise::new(),
            crack_rise: Rise::new(),
            hat_rise: Rise::new(),
            kick: VoiceState::new(KICK, false, 0.12, hop_seconds).struck_on_attack(
                KICK_HIT_GAP_SECONDS,
                KICK_ATTACK,
                hop_seconds,
            ),
            snare: VoiceState::new(SNARE, true, 0.1, hop_seconds),
            hihat: VoiceState::new(HIHAT, true, 0.06, hop_seconds),
            tempo: Tempo::new(hops_per_second),
            beat: 0.0,
            beats: 0,
            beat_decay: decay(hop_seconds, BEAT_HALF_LIFE_SECONDS),
            gain: AutoGain::new(hop_seconds),
            clip_hold: 0,
            clip_hold_hops: (CLIP_HOLD_SECONDS * hops_per_second).round() as u32,
            reading: Reading {
                gain: 1.0,
                ..Reading::default()
            },
        }
    }

    /// Applies an operator's tuning without forgetting what the detector has learned.
    pub fn retune(&mut self, tuning: Tuning) {
        self.tuning = tuning;
    }

    pub fn tuning(&self) -> Tuning {
        self.tuning
    }

    /// The state after the latest complete hop.
    pub fn reading(&self) -> Reading {
        self.reading
    }

    /// Feeds mono samples, calling `on_onset` for every hit they complete. Returns how many hops
    /// were analysed.
    pub fn push(&mut self, samples: &[f32], mut on_onset: impl FnMut(Onset)) -> usize {
        let mut hops = 0;
        for &sample in samples {
            let sample = if sample.is_finite() { sample } else { 0.0 };
            self.ring[self.write] = sample;
            self.write = (self.write + 1) % FRAME;
            self.hop_energy += sample * sample;
            let low = self.kick_filter.next(sample);
            self.kick_energy += low * low;
            if sample.abs() >= CLIP_LEVEL {
                self.hop_clipped += 1;
            }
            self.samples += 1;
            self.since_hop += 1;
            if self.since_hop == HOP {
                self.hop(&mut on_onset);
                hops += 1;
            }
        }
        hops
    }

    fn hop(&mut self, on_onset: &mut impl FnMut(Onset)) {
        let rms = (self.hop_energy / HOP as f32).sqrt();
        let clipped = self.hop_clipped;
        self.since_hop = 0;
        self.hop_energy = 0.0;
        self.hop_clipped = 0;

        let program = self.gain.observe(rms);
        let armed = rms > SILENCE && rms > program * RELATIVE_GATE;
        self.clip_hold = if clipped >= CLIPPED_SAMPLES {
            self.clip_hold_hops
        } else {
            self.clip_hold.saturating_sub(1)
        };

        let ring = &self.ring;
        let write = self.write;
        self.long
            .power(|index| ring[(write + index) % FRAME], &mut self.long_power);
        self.short.power(
            |index| ring[(write + FRAME - SHORT + index) % FRAME],
            &mut self.short_power,
        );

        let kick_log = band_log(&self.long_power, &self.kick_bins);
        let total = self.long_power.iter().sum::<f32>();
        let kick_relevant = kick_log.exp() >= KICK_SHARE * total;
        // Two paths for the kick. The spectral one is steady and exact about *when* in the bar a
        // kick falls, which the tempo and flywheel want. The filtered one hears the hit within a
        // few milliseconds, which is what the kick's own level and flash want. Two hops of energy
        // span more than a cycle of the lowest kick frequencies, so the level does not ripple.
        let kick_novelty = self.kick_rise.next(self.kick_smooth.next(kick_log));
        let kick_level =
            ((self.kick_energy + self.kick_energy_before) / (2 * HOP) as f32 + POWER_FLOOR).ln();
        self.kick_energy_before = self.kick_energy;
        self.kick_energy = 0.0;
        let body = band_log(&self.long_power, &self.body_bins);
        let crack = band_log(&self.short_power, &self.crack_bins);
        let snare_novelty = 0.5 * self.body_rise.next(body) + self.crack_rise.next(crack);
        let hat_level = band_log(&self.short_power, &self.hat_bins);
        let hat_novelty = self.hat_rise.next(hat_level);

        let sensitivity = self.tuning.sensitivity;
        let at = self.samples.saturating_sub(HOP as u64);
        let kick = self.kick.offer(
            kick_novelty,
            kick_level,
            sensitivity,
            armed && kick_relevant,
        );
        let snare = self.snare.offer(snare_novelty, crack, sensitivity, armed);
        let hihat = self.hihat.offer(hat_novelty, hat_level, sensitivity, armed);
        for (instrument, heard) in [
            (Instrument::Kick, kick),
            (Instrument::Snare, snare),
            (Instrument::HiHat, hihat),
        ] {
            if let Some(strength) = heard.hit {
                on_onset(Onset {
                    instrument,
                    strength,
                    sample: at,
                });
            }
        }

        // The tempo listens mostly to the kick, with the hats as a weaker second opinion; each is
        // measured against its own threshold so neither band's loudness decides.
        let periodic = if armed {
            relative(kick_novelty, self.kick.picker.threshold())
                + 0.5 * relative(hat_novelty, self.hihat.picker.threshold())
        } else {
            0.0
        };
        self.tempo.observe(periodic);

        self.beat *= self.beat_decay;
        if self.tempo.advance() {
            self.beat = 1.0;
            self.beats += 1;
        }
        // The flywheel aligns to the onset curve, which is earlier and more exact than a level
        // crossing; before there is a tempo, the beat simply flashes with the kick.
        if let Some(strength) = kick.onset
            && self.tempo.bpm() > 0.0
        {
            self.tempo.align(strength, KICK_LATENCY_SECONDS);
        }
        if self.tempo.bpm() <= 0.0 && kick.hit.is_some() {
            self.beat = 1.0;
        }

        self.reading = Reading {
            kick: self.kick.voice,
            snare: self.snare.voice,
            hihat: self.hihat.voice,
            bpm: self.tempo.bpm(),
            tempo_confidence: self.tempo.confidence(),
            beat_phase: self.tempo.phase(),
            beat: self.beat,
            beats: self.beats,
            gain: if self.tuning.auto_gain {
                self.gain.gain()
            } else {
                self.tuning.manual_gain.max(0.0)
            },
            input_rms: rms,
            clipping: self.clip_hold > 0,
        };
    }
}

fn relative(novelty: f32, threshold: f32) -> f32 {
    if threshold <= 0.0 {
        return 0.0;
    }
    (novelty / threshold).min(TEMPO_NOVELTY_CAP)
}

fn band_log(power: &[f32], bins: &Range<usize>) -> f32 {
    (power[bins.clone()].iter().sum::<f32>() + POWER_FLOOR).ln()
}

/// The transform bins covering `low..high` Hz.
fn bins(low: f32, high: f32, sample_rate: f32, size: usize) -> Range<usize> {
    let width = sample_rate / size as f32;
    let half = size / 2;
    let from = ((low / width).ceil() as usize).clamp(1, half - 1);
    let to = ((high / width).floor() as usize + 1).clamp(from + 1, half);
    from..to
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;

    const RATE: f32 = 48_000.0;

    /// A kick on every beat and a hi-hat on every off-beat, over a quiet pad.
    fn song(bpm: f32, seconds: f32, amplitude: f32, kicks: bool, hats: bool) -> Vec<f32> {
        let beat = 60.0 / bpm;
        let mut noise = 0x1234_5678u32;
        let mut previous = 0.0;
        (0..(seconds * RATE) as usize)
            .map(|index| {
                let t = index as f32 / RATE;
                noise ^= noise << 13;
                noise ^= noise >> 17;
                noise ^= noise << 5;
                let white = noise as f32 / u32::MAX as f32 * 2.0 - 1.0;
                let bright = white - previous;
                previous = white;

                let mut sample = 0.03 * (2.0 * PI * 220.0 * t).sin() + 0.005 * white;
                if kicks {
                    let since = t % beat;
                    let sweep = 45.0 * since + 65.0 * 0.03 * (1.0 - (-since / 0.03).exp());
                    sample += 0.8 * (-since / 0.18).exp() * (2.0 * PI * sweep).sin();
                }
                if hats {
                    let since = (t + beat / 2.0) % beat;
                    sample += 0.25 * (-since / 0.04).exp() * bright;
                }
                sample * amplitude
            })
            .collect()
    }

    fn listen(samples: &[f32]) -> (Detector, Vec<Onset>) {
        let mut detector = Detector::new(RATE);
        let mut onsets = Vec::new();
        // Uneven chunks, the way a device delivers them.
        for chunk in samples.chunks(441) {
            detector.push(chunk, |onset| onsets.push(onset));
        }
        (detector, onsets)
    }

    fn count(onsets: &[Onset], instrument: Instrument, from_seconds: f32) -> usize {
        onsets
            .iter()
            .filter(|onset| {
                onset.instrument == instrument && onset.sample as f32 / RATE >= from_seconds
            })
            .count()
    }

    #[test]
    fn kicks_and_hats_are_found_and_kept_apart() {
        let (_, onsets) = listen(&song(128.0, 12.0, 1.0, true, true));
        // Eleven seconds after warm-up at 128 BPM is about 23 of each.
        let expected = 11.0 * 128.0 / 60.0;
        let kicks = count(&onsets, Instrument::Kick, 1.0) as f32;
        let hats = count(&onsets, Instrument::HiHat, 1.0) as f32;
        assert!(
            (kicks / expected - 1.0).abs() < 0.12,
            "{kicks} kicks, expected {expected}"
        );
        assert!(
            (hats / expected - 1.0).abs() < 0.12,
            "{hats} hats, expected {expected}"
        );
    }

    #[test]
    fn a_kick_alone_is_not_a_hi_hat() {
        let (_, onsets) = listen(&song(128.0, 10.0, 1.0, true, false));
        let kicks = count(&onsets, Instrument::Kick, 1.0);
        let hats = count(&onsets, Instrument::HiHat, 1.0);
        assert!(kicks > 15, "{kicks} kicks");
        assert!(hats * 10 <= kicks, "{hats} hats from {kicks} kicks");
    }

    #[test]
    fn a_hi_hat_alone_is_not_a_kick() {
        let (_, onsets) = listen(&song(128.0, 10.0, 1.0, false, true));
        let hats = count(&onsets, Instrument::HiHat, 1.0);
        let kicks = count(&onsets, Instrument::Kick, 1.0);
        assert!(hats > 15, "{hats} hats");
        assert!(kicks * 10 <= hats, "{kicks} kicks from {hats} hats");
    }

    #[test]
    fn clicks_are_not_hi_hats() {
        let mut samples = song(128.0, 10.0, 1.0, false, false);
        for index in (0..samples.len()).step_by((0.37 * RATE) as usize).skip(1) {
            samples[index] = 0.9;
        }
        let (_, onsets) = listen(&samples);
        let hats = count(&onsets, Instrument::HiHat, 1.0);
        let snares = count(&onsets, Instrument::Snare, 1.0);
        assert!(hats <= 2, "{hats} of 24 clicks read as hats");
        assert!(snares <= 2, "{snares} of 24 clicks read as snares");
    }

    #[test]
    fn the_tempo_is_found_and_the_beat_follows_the_kick() {
        let bpm = 128.0;
        let (detector, onsets) = listen(&song(bpm, 16.0, 1.0, true, true));
        let reading = detector.reading();
        assert!(
            (reading.bpm - bpm).abs() / bpm < 0.02,
            "read {}",
            reading.bpm
        );
        assert!(
            reading.tempo_confidence > 0.2,
            "{}",
            reading.tempo_confidence
        );

        // Replay and look at the phase whenever a late kick lands: it should be near a beat.
        let mut replay = Detector::new(RATE);
        let mut errors = Vec::new();
        let samples = song(bpm, 16.0, 1.0, true, true);
        for chunk in samples.chunks(HOP) {
            let mut kicked = None;
            replay.push(chunk, |onset| {
                if onset.instrument == Instrument::Kick && onset.sample as f32 / RATE > 12.0 {
                    kicked = Some(());
                }
            });
            if kicked.is_some() {
                let phase = replay.reading().beat_phase;
                errors.push(if phase > 0.5 { 1.0 - phase } else { phase });
            }
        }
        assert!(!errors.is_empty());
        let worst = errors.iter().copied().fold(0.0, f32::max);
        assert!(
            // A hit is reported once the level has risen through its trigger, a few hops in.
            worst < 0.25,
            "a kick landed {worst} of a beat off the flywheel"
        );
        assert!(count(&onsets, Instrument::Kick, 12.0) > 0);
    }

    #[test]
    fn the_beat_lands_on_the_kick_rather_than_after_detecting_it() {
        let bpm = 128.0;
        let period = 60.0 / bpm;
        let samples = song(bpm, 24.0, 1.0, true, true);
        let mut detector = Detector::new(RATE);
        let mut offsets = Vec::new();
        let mut beats = 0;
        for (hop, chunk) in samples.chunks(HOP).enumerate() {
            detector.push(chunk, |_| {});
            let reading = detector.reading();
            if reading.beats > beats {
                beats = reading.beats;
                let seconds = (hop + 1) as f32 * HOP as f32 / RATE;
                if seconds > 16.0 {
                    offsets.push((seconds + period / 2.0) % period - period / 2.0);
                }
            }
        }
        assert!(offsets.len() > 10, "{} beats", offsets.len());
        let mean = offsets.iter().sum::<f32>() / offsets.len() as f32;
        let worst = offsets
            .iter()
            .fold(0.0f32, |worst, offset| worst.max(offset.abs()));
        // A beat is only observed at the end of its hop, so up to 11 ms late by construction.
        assert!(
            mean.abs() < 0.012,
            "beats land {:.1} ms from the kicks on average",
            mean * 1_000.0
        );
        assert!(worst < 0.025, "and never {:.1} ms away", worst * 1_000.0);
    }

    #[test]
    fn detection_does_not_depend_on_the_input_level() {
        let (_, loud) = listen(&song(128.0, 10.0, 1.0, true, true));
        let (quiet_detector, quiet) = listen(&song(128.0, 10.0, 0.01, true, true));
        for instrument in [Instrument::Kick, Instrument::HiHat] {
            let (a, b) = (
                count(&loud, instrument, 1.0),
                count(&quiet, instrument, 1.0),
            );
            assert!(
                a.abs_diff(b) <= 1,
                "{instrument:?}: {a} loud, {b} at -40 dB"
            );
        }
        assert!(
            quiet_detector.reading().gain > 10.0,
            "and the gain lifts the quiet input: {}",
            quiet_detector.reading().gain
        );
    }

    #[test]
    fn silence_hears_nothing() {
        let (detector, onsets) = listen(&vec![0.0; (10.0 * RATE) as usize]);
        assert!(onsets.is_empty());
        let reading = detector.reading();
        assert_eq!(reading.bpm, 0.0);
        assert_eq!(reading.beats, 0);
        assert!(!reading.clipping);
    }

    #[test]
    fn a_clipped_input_says_so() {
        let square: Vec<f32> = (0..(RATE as usize))
            .map(|index| if (index / 40) % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let (detector, _) = listen(&square);
        assert!(detector.reading().clipping);
    }

    #[test]
    fn manual_gain_is_reported_when_automatic_gain_is_off() {
        let mut detector = Detector::new(RATE);
        detector.retune(Tuning {
            auto_gain: false,
            manual_gain: 2.5,
            ..Tuning::default()
        });
        detector.push(&song(128.0, 1.0, 0.01, true, true), |_| {});
        assert_eq!(detector.reading().gain, 2.5);
    }

    #[test]
    fn the_same_samples_give_the_same_onsets() {
        let samples = song(128.0, 6.0, 1.0, true, true);
        assert_eq!(listen(&samples).1, listen(&samples).1);
    }
}
