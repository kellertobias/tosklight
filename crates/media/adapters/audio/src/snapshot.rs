//! The analysis worker and what it publishes.
//!
//! Kept away from the device so it can be driven by a test that feeds it samples directly: beat
//! detection, tempo estimation, and the decay a visualizer reacts to are behaviour, not I/O.

use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use light_beat::{Detector, Reading, Voice};
use media_domain::audio::{Analysis, Instrument, Instruments, Tuning, WINDOW, analyse};

/// One instant of analysis, as a visualizer reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisSnapshot {
    pub analysis: Analysis,
    /// `1.0` on the frame a beat lands, falling toward zero afterwards, so an effect can flash
    /// rather than strobe on every analysis pass. Once a tempo is known this follows the tempo's
    /// flywheel, so it keeps time through a breakdown with no kick.
    pub beat: f32,
    /// Zero until the music has shown a tempo.
    pub bpm: f32,
    /// Where this instant sits between beats, `0.0..1.0`. Zero until a tempo is known.
    pub beat_phase: f32,
    /// How periodic the music is right now, `0.0..=1.0`.
    pub tempo_confidence: f32,
    /// The kick, snare, and hi-hat the detector follows.
    pub instruments: Instruments,
    /// The gain the levels were measured with: automatic times the operator's trim, or manual.
    pub gain: f32,
    /// The input reached full scale recently. Only the source or the interface can fix that.
    pub clipping: bool,
}

impl Default for AnalysisSnapshot {
    /// Silence. A real state, not a placeholder: it is what a machine with no input device has.
    fn default() -> Self {
        Self {
            analysis: Analysis::default(),
            beat: 0.0,
            bpm: 0.0,
            beat_phase: 0.0,
            tempo_confidence: 0.0,
            instruments: Instruments::default(),
            gain: 1.0,
            clipping: false,
        }
    }
}

/// The published snapshot, swapped whole so a reader never sees half an analysis.
pub type SharedAnalysis = Arc<arc_swap::ArcSwap<AnalysisSnapshot>>;

/// Turns captured samples into published analysis.
pub struct Worker {
    tuning: Tuning,
    detector: Detector,
    window: Vec<f32>,
    sample_rate: f32,
    published: SharedAnalysis,
}

impl Worker {
    pub fn new(tuning: Tuning, sample_rate: f32, published: SharedAnalysis) -> Self {
        let mut detector = Detector::new(sample_rate);
        detector.retune(detection(&tuning));
        Self {
            tuning,
            detector,
            window: Vec::with_capacity(WINDOW),
            sample_rate,
            published,
        }
    }

    /// Retunes without losing the tempo and thresholds the music has already built up.
    pub fn retune(&mut self, tuning: Tuning) {
        self.tuning = tuning;
        self.detector.retune(detection(&tuning));
    }

    /// Takes everything waiting and publishes an analysis for each whole window it completes.
    ///
    /// Returns how many windows were analysed, which is what a test asserts and what a diagnostic
    /// uses to tell "no audio arriving" from "audio arriving and silent".
    pub fn drain(&mut self, queue: &ArrayQueue<f32>) -> usize {
        let mut analysed = 0;
        while let Some(sample) = queue.pop() {
            self.window.push(sample);
            if self.window.len() == WINDOW {
                self.analyse_window();
                analysed += 1;
                self.window.clear();
            }
        }
        analysed
    }

    fn analyse_window(&mut self) {
        // The detector hears the input exactly as it arrived. Its onsets do not depend on level,
        // and a gain that moves with the program would blur the very rises it listens for.
        self.detector.push(&self.window, |_| {});
        let reading = self.detector.reading();

        let automatic = if self.tuning.auto_gain {
            reading.gain
        } else {
            1.0
        };
        if automatic != 1.0 {
            self.window
                .iter_mut()
                .for_each(|sample| *sample *= automatic);
        }
        let analysis = analyse(&self.window, self.sample_rate, &self.tuning);
        let gain = automatic * self.tuning.effective_gain();
        self.published
            .store(Arc::new(snapshot(analysis, &reading, gain)));
    }
}

/// What the operator's tuning means to the detector. The input gain is a trim on top of the
/// automatic gain when that is on, and the whole gain when it is off.
fn detection(tuning: &Tuning) -> light_beat::Tuning {
    light_beat::Tuning {
        sensitivity: tuning.beat_sensitivity,
        auto_gain: tuning.auto_gain,
        manual_gain: tuning.effective_gain(),
    }
}

fn instrument(voice: Voice) -> Instrument {
    Instrument {
        level: voice.level,
        hit: voice.hit,
    }
}

fn snapshot(analysis: Analysis, reading: &Reading, gain: f32) -> AnalysisSnapshot {
    AnalysisSnapshot {
        analysis,
        beat: reading.beat,
        bpm: reading.bpm,
        beat_phase: reading.beat_phase,
        tempo_confidence: reading.tempo_confidence,
        instruments: Instruments {
            kick: instrument(reading.kick),
            snare: instrument(reading.snare),
            hihat: instrument(reading.hihat),
        },
        gain,
        clipping: reading.clipping,
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;

    const RATE: f32 = 48_000.0;

    fn published() -> SharedAnalysis {
        Arc::new(arc_swap::ArcSwap::from_pointee(AnalysisSnapshot::default()))
    }

    fn queue_of(samples: &[f32]) -> ArrayQueue<f32> {
        let queue = ArrayQueue::new(samples.len().max(1));
        for sample in samples {
            let _ = queue.push(*sample);
        }
        queue
    }

    /// A kick drum on every beat over a quiet pad.
    fn kicks(bpm: f32, seconds: f32, amplitude: f32) -> Vec<f32> {
        let beat = 60.0 / bpm;
        (0..(seconds * RATE) as usize)
            .map(|index| {
                let t = index as f32 / RATE;
                let since = t % beat;
                let sweep = 45.0 * since + 65.0 * 0.03 * (1.0 - (-since / 0.03).exp());
                let kick = 0.8 * (-since / 0.18).exp() * (2.0 * PI * sweep).sin();
                amplitude * (kick + 0.03 * (2.0 * PI * 220.0 * t).sin())
            })
            .collect()
    }

    /// Feeds whole windows the way the device thread does, and reports the strongest kick flash
    /// any published snapshot carried.
    fn feed(worker: &mut Worker, published: &SharedAnalysis, samples: &[f32]) -> f32 {
        let mut strongest: f32 = 0.0;
        for window in samples.chunks(WINDOW) {
            worker.drain(&queue_of(window));
            strongest = strongest.max(published.load().instruments.kick.hit);
        }
        strongest
    }

    #[test]
    fn a_whole_window_publishes_an_analysis_and_a_partial_one_waits() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), RATE, Arc::clone(&published));

        let partial = queue_of(&[0.5; WINDOW - 1]);
        assert_eq!(worker.drain(&partial), 0, "an incomplete window waits");
        assert_eq!(published.load().analysis, Analysis::default());

        // One more sample completes it.
        assert_eq!(worker.drain(&queue_of(&[0.5])), 1);
        assert!(published.load().analysis.energy > 0.0);
    }

    #[test]
    fn silence_analyses_as_silence_rather_than_as_nothing() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), RATE, Arc::clone(&published));

        assert_eq!(worker.drain(&queue_of(&[0.0; WINDOW])), 1);
        let snapshot = published.load();
        assert_eq!(snapshot.analysis.energy, 0.0);
        assert_eq!(snapshot.beat, 0.0);
        assert_eq!(snapshot.instruments, Instruments::default());
        assert_eq!(
            snapshot.analysis.waveform.len(),
            media_domain::audio::WAVEFORM_POINTS,
            "a visualizer still gets a whole waveform to draw"
        );
    }

    #[test]
    fn kicks_flash_and_a_tempo_follows_them() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), RATE, Arc::clone(&published));

        let strongest = feed(&mut worker, &published, &kicks(128.0, 12.0, 1.0));
        assert!(strongest > 0.9, "a kick flashes: {strongest}");
        let snapshot = published.load();
        assert!(
            (snapshot.bpm - 128.0).abs() < 2.5,
            "the tempo follows the kicks: {}",
            snapshot.bpm
        );
        assert!(
            snapshot.instruments.kick.hit < 1.0,
            "and the flash falls between them"
        );
    }

    #[test]
    fn a_tempo_is_only_published_once_it_means_something() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), RATE, Arc::clone(&published));

        feed(&mut worker, &published, &kicks(128.0, 1.0, 1.0));
        assert_eq!(
            published.load().bpm,
            0.0,
            "a bar is not a tempo, and guessing one would move every synchronized layer"
        );
        assert_eq!(published.load().beat_phase, 0.0);
    }

    #[test]
    fn automatic_gain_levels_a_quiet_input_for_the_meters() {
        let quiet = kicks(128.0, 3.0, 0.01);

        let automatic = published();
        let mut worker = Worker::new(Tuning::default(), RATE, Arc::clone(&automatic));
        feed(&mut worker, &automatic, &quiet);

        let manual = published();
        let mut worker = Worker::new(
            Tuning {
                auto_gain: false,
                ..Tuning::default()
            },
            RATE,
            Arc::clone(&manual),
        );
        feed(&mut worker, &manual, &quiet);

        let (lifted, plain) = (automatic.load(), manual.load());
        assert!(lifted.gain > 10.0, "gain {}", lifted.gain);
        assert_eq!(plain.gain, 1.0);
        assert!(
            lifted.analysis.peak > plain.analysis.peak * 10.0,
            "{} against {}",
            lifted.analysis.peak,
            plain.analysis.peak
        );
    }

    #[test]
    fn retuning_keeps_the_tempo_the_music_has_already_shown() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), RATE, Arc::clone(&published));
        feed(&mut worker, &published, &kicks(128.0, 12.0, 1.0));

        worker.retune(Tuning {
            input_gain: 2.0,
            beat_sensitivity: 1.5,
            ..Tuning::default()
        });
        feed(&mut worker, &published, &kicks(128.0, 0.1, 1.0));

        assert!(
            (published.load().bpm - 128.0).abs() < 2.5,
            "a gain change must not make the detector relearn the music: {}",
            published.load().bpm
        );
    }
}
