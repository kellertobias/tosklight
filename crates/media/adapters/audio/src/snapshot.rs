//! The analysis worker and what it publishes.
//!
//! Kept away from the device so it can be driven by a test that feeds it samples directly: beat
//! detection, tempo estimation, and the decay a visualizer reacts to are behaviour, not I/O.

use std::sync::Arc;

use crossbeam_queue::ArrayQueue;
use media_domain::audio::{Analysis, BeatDetector, Tuning, WINDOW, analyse};

/// One instant of analysis, as a visualizer reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisSnapshot {
    pub analysis: Analysis,
    /// `1.0` on the frame a beat lands, falling toward zero afterwards, so an effect can flash
    /// rather than strobe on every analysis pass.
    pub beat: f32,
    /// Estimated from recent beat intervals. Zero until there have been enough to mean anything.
    pub bpm: f32,
    /// Where this instant sits between beats, `0.0..1.0`. Zero until a tempo is known.
    pub beat_phase: f32,
}

impl Default for AnalysisSnapshot {
    /// Silence. A real state, not a placeholder: it is what a machine with no input device has.
    fn default() -> Self {
        Self {
            analysis: Analysis::default(),
            beat: 0.0,
            bpm: 0.0,
            beat_phase: 0.0,
        }
    }
}

/// The published snapshot, swapped whole so a reader never sees half an analysis.
pub type SharedAnalysis = Arc<arc_swap::ArcSwap<AnalysisSnapshot>>;

/// How much of a beat's flash survives one analysis pass.
///
/// The window advances about every 20 ms at 48 kHz, so a beat fades over roughly a fifth of a
/// second — long enough for a 60 fps output to see it, short enough not to smear into the next.
const BEAT_DECAY: f32 = 0.82;

/// Turns captured samples into published analysis.
pub struct Worker {
    tuning: Tuning,
    detector: BeatDetector,
    window: Vec<f32>,
    sample_rate: f32,
    beat: f32,
    last_beat_millis: Option<u64>,
    published: SharedAnalysis,
}

impl Worker {
    pub fn new(tuning: Tuning, sample_rate: f32, published: SharedAnalysis) -> Self {
        Self {
            tuning,
            detector: BeatDetector::new(),
            window: Vec::with_capacity(WINDOW),
            sample_rate,
            beat: 0.0,
            last_beat_millis: None,
            published,
        }
    }

    /// Retunes without losing the beat history an operator's playing has already built up.
    pub fn retune(&mut self, tuning: Tuning) {
        self.tuning = tuning;
    }

    /// Takes everything waiting and publishes an analysis for each whole window it completes.
    ///
    /// Returns how many windows were analysed, which is what a test asserts and what a diagnostic
    /// uses to tell "no audio arriving" from "audio arriving and silent".
    pub fn drain(&mut self, queue: &ArrayQueue<f32>, now_millis: u64) -> usize {
        let mut analysed = 0;
        while let Some(sample) = queue.pop() {
            self.window.push(sample);
            if self.window.len() == WINDOW {
                self.analyse_window(now_millis);
                analysed += 1;
                self.window.clear();
            }
        }
        analysed
    }

    fn analyse_window(&mut self, now_millis: u64) {
        let analysis = analyse(&self.window, self.sample_rate, &self.tuning);
        // The flash decays first, so a beat this pass lands on a fresh one rather than adding to
        // whatever was left of the last.
        self.beat *= BEAT_DECAY;
        if self
            .detector
            .observe(analysis.energy, &self.tuning, now_millis)
        {
            self.beat = 1.0;
            self.last_beat_millis = Some(now_millis);
        }

        let bpm = self.detector.estimated_bpm().unwrap_or(0.0);
        self.published.store(Arc::new(AnalysisSnapshot {
            analysis,
            beat: self.beat,
            bpm,
            beat_phase: phase(bpm, self.last_beat_millis, now_millis),
        }));
    }
}

/// How far through the current beat this instant is.
fn phase(bpm: f32, last_beat_millis: Option<u64>, now_millis: u64) -> f32 {
    let (Some(last), true) = (last_beat_millis, bpm > 0.0) else {
        return 0.0;
    };
    let interval = 60_000.0 / bpm;
    let since = now_millis.saturating_sub(last) as f32;
    (since / interval).fract().clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn published() -> SharedAnalysis {
        Arc::new(arc_swap::ArcSwap::from_pointee(AnalysisSnapshot::default()))
    }

    /// A queue holding one window of a loud tone.
    fn loud_window() -> ArrayQueue<f32> {
        let queue = ArrayQueue::new(WINDOW);
        for index in 0..WINDOW {
            let _ = queue.push((index as f32 * 0.2).sin() * 0.9);
        }
        queue
    }

    fn silent_window() -> ArrayQueue<f32> {
        let queue = ArrayQueue::new(WINDOW);
        for _ in 0..WINDOW {
            let _ = queue.push(0.0);
        }
        queue
    }

    #[test]
    fn a_whole_window_publishes_an_analysis_and_a_partial_one_waits() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), 48_000.0, Arc::clone(&published));

        let partial = ArrayQueue::new(WINDOW);
        for _ in 0..WINDOW - 1 {
            let _ = partial.push(0.5);
        }
        assert_eq!(worker.drain(&partial, 0), 0, "an incomplete window waits");
        assert_eq!(published.load().analysis, Analysis::default());

        // One more sample completes it.
        let last = ArrayQueue::new(1);
        let _ = last.push(0.5);
        assert_eq!(worker.drain(&last, 20), 1);
        assert!(published.load().analysis.energy > 0.0);
    }

    #[test]
    fn silence_analyses_as_silence_rather_than_as_nothing() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), 48_000.0, Arc::clone(&published));

        assert_eq!(worker.drain(&silent_window(), 0), 1);
        let snapshot = published.load();
        assert_eq!(snapshot.analysis.energy, 0.0);
        assert_eq!(snapshot.beat, 0.0);
        assert_eq!(
            snapshot.analysis.waveform.len(),
            media_domain::audio::WAVEFORM_POINTS,
            "a visualizer still gets a whole waveform to draw"
        );
    }

    #[test]
    fn a_beat_flashes_and_then_fades_rather_than_staying_on() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), 48_000.0, Arc::clone(&published));

        // Quiet history first, so a loud window reads as a hit rather than as the norm.
        for pass in 0..12 {
            worker.drain(&silent_window(), pass * 20);
        }
        worker.drain(&loud_window(), 400);
        let struck = published.load().beat;
        assert!(
            struck > 0.5,
            "a hit against quiet history is a beat: {struck}"
        );

        for pass in 1..4 {
            worker.drain(&silent_window(), 400 + pass * 20);
        }
        let faded = published.load().beat;
        assert!(faded < struck, "and it fades: {faded} from {struck}");
        assert!(faded > 0.0, "but not instantly, or nothing could flash");
    }

    #[test]
    fn a_tempo_is_only_published_once_it_means_something() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), 48_000.0, Arc::clone(&published));

        worker.drain(&loud_window(), 0);
        assert_eq!(
            published.load().bpm,
            0.0,
            "one hit is not a tempo, and guessing one would move every synchronized layer"
        );
        assert_eq!(published.load().beat_phase, 0.0);
    }

    #[test]
    fn phase_runs_from_a_beat_and_wraps() {
        assert_eq!(phase(0.0, Some(0), 500), 0.0, "no tempo, no phase");
        assert_eq!(phase(120.0, None, 500), 0.0, "no beat yet, no phase");

        // 120 BPM is a beat every 500 ms.
        assert!((phase(120.0, Some(1_000), 1_250) - 0.5).abs() < 1e-3);
        assert!(
            phase(120.0, Some(1_000), 1_500) < 1e-3,
            "it wraps at the beat"
        );
        assert!((phase(120.0, Some(1_000), 1_750) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn retuning_keeps_the_history_an_operator_has_already_built_up() {
        let published = published();
        let mut worker = Worker::new(Tuning::default(), 48_000.0, Arc::clone(&published));
        for pass in 0..12 {
            worker.drain(&silent_window(), pass * 20);
        }

        worker.retune(Tuning {
            input_gain: 2.0,
            ..Default::default()
        });
        worker.drain(&loud_window(), 400);

        assert!(
            published.load().beat > 0.5,
            "a gain change must not make the detector relearn what loud means"
        );
    }
}
