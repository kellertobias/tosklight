use super::*;

const RATE: f32 = 48_000.0;

/// A synthetic song at `bpm`: a kick on every beat, plus a held chord per section whose loudness
/// and hi-hats mark verse and chorus apart.
fn song(bpm: f32, sections: &[(usize, &[f32], f32, bool)]) -> Vec<f32> {
    let beat = (60.0 / bpm * RATE) as usize;
    let mut samples = Vec::new();
    for (bars, chord, level, hats) in sections {
        let length = bars * 4 * beat;
        let start = samples.len();
        samples.resize(start + length, 0.0);
        for index in 0..length {
            let t = index as f32 / RATE;
            let in_beat = index % beat;
            let kick_t = in_beat as f32 / RATE;
            let kick =
                (2.0 * std::f32::consts::PI * 55.0 * kick_t).sin() * (-kick_t * 18.0).exp() * 0.8;
            let pad: f32 = chord
                .iter()
                .map(|frequency| (2.0 * std::f32::consts::PI * frequency * t).sin())
                .sum::<f32>()
                / chord.len() as f32;
            let hat = if *hats && in_beat % (beat / 2) < 400 {
                (((index * 1103515245 + 12345) >> 8) % 2000) as f32 / 1000.0 - 1.0
            } else {
                0.0
            } * 0.15;
            samples[start + index] = kick + pad * level + hat;
        }
    }
    samples
}

const C_MAJOR: &[f32] = &[261.63, 329.63, 392.0];
const G_MAJOR: &[f32] = &[196.0, 246.94, 293.66];
const D_MAJOR: &[f32] = &[293.66, 369.99, 440.0];

#[test]
fn finds_the_tempo_a_bar_aligned_grid_and_the_section_changes() {
    let samples = song(
        120.0,
        &[
            (8, D_MAJOR, 0.08, false),
            (8, C_MAJOR, 0.15, false),
            (8, G_MAJOR, 0.45, true),
            (8, C_MAJOR, 0.15, false),
            (8, G_MAJOR, 0.45, true),
        ],
    );
    let mut reported = Vec::new();
    let analysis = analyse_song(&samples, RATE, |fraction| {
        reported.push(fraction);
        true
    })
    .expect("not cancelled");
    assert!((analysis.bpm - 120.0).abs() < 1.5, "bpm {}", analysis.bpm);
    let gaps: Vec<f64> = analysis
        .beats_seconds
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert!(
        gaps.iter().all(|gap| (gap - 0.5).abs() < 0.01),
        "grid gaps {gaps:?}"
    );
    // Beats land on the kicks, which start every half second from zero.
    let phase = analysis.beats_seconds[0].rem_euclid(0.5);
    assert!(phase.min(0.5 - phase) < 0.03, "phase {phase}");
    let bars = analysis.downbeats_seconds.len();
    assert!(
        (analysis.beats_seconds.len() / 4).abs_diff(bars) <= 1,
        "{bars} bars for {} beats",
        analysis.beats_seconds.len()
    );
    let starts: Vec<f64> = analysis
        .sections
        .iter()
        .map(|section| section.start_seconds)
        .collect();
    for expected in [16.0, 32.0, 48.0, 64.0] {
        assert!(
            starts.iter().any(|start| (start - expected).abs() <= 0.1),
            "a boundary near {expected}s in {starts:?}"
        );
    }
    let label_at = |time: f64| {
        analysis
            .sections
            .iter()
            .rev()
            .find(|section| section.start_seconds <= time + 0.01)
            .map(|section| section.label)
    };
    assert_eq!(label_at(40.0), Some(SectionLabel::Chorus));
    assert_eq!(label_at(72.0), Some(SectionLabel::Chorus));
    assert_eq!(label_at(20.0), Some(SectionLabel::Verse));
    assert_eq!(label_at(0.5), Some(SectionLabel::Intro));
    assert!(reported.windows(2).all(|pair| pair[1] >= pair[0]));
    assert_eq!(reported.last(), Some(&1.0));
}

#[test]
fn stops_when_cancelled() {
    let samples = song(120.0, &[(8, C_MAJOR, 0.2, false)]);
    let mut calls = 0;
    assert!(
        analyse_song(&samples, RATE, |_| {
            calls += 1;
            calls < 3
        })
        .is_none()
    );
    assert_eq!(calls, 3);
}

#[test]
fn silence_has_no_grid_and_one_section_at_most() {
    let analysis = analyse_song(&vec![0.0; RATE as usize * 20], RATE, |_| true).unwrap();
    assert_eq!(analysis.bpm, 0.0);
    assert!(analysis.beats_seconds.is_empty());
    assert!(analysis.sections.len() <= 1);
}
