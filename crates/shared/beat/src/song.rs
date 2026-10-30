//! Offline song analysis: a beat grid, bars and suggested song sections for a whole recording.
//!
//! The live [`Detector`] hears beats as they happen. A timecode track is different: the whole
//! song is on disk, so the grid can be fitted to the entire recording and the song can be split
//! where it changes. Sections are found the way a listener finds them — where the music stops
//! resembling what came before. Each bar becomes a small description of its harmony (a chroma
//! vector) and its weight (energy and brightness); a self-similarity matrix of those bars, read
//! through a checkerboard kernel along its diagonal (Foote's novelty), peaks where one passage
//! ends and another begins. Boundaries therefore always land on bars.
//!
//! Labels are suggestions for an operator to rename, not a claim about the music: a passage that
//! returns and is the loudest is a Chorus, one that returns quieter is a Verse, a quiet opening is
//! an Intro, a quiet ending an Outro, and a quiet passage that never returns is a Breakdown.

use crate::{Detector, HOP, Instrument, spectrum::Spectrum};

const FRAME: usize = 4096;
const BEATS_PER_BAR: usize = 4;
/// Novelty is read through a kernel this many bars either side of a candidate boundary.
const KERNEL_BARS: usize = 4;
/// No section is shorter than this many bars.
const MIN_SECTION_BARS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionLabel {
    Intro,
    Verse,
    Chorus,
    Breakdown,
    Bridge,
    Outro,
}

impl SectionLabel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Intro => "Intro",
            Self::Verse => "Verse",
            Self::Chorus => "Chorus",
            Self::Breakdown => "Breakdown",
            Self::Bridge => "Bridge",
            Self::Outro => "Outro",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SongSection {
    pub start_seconds: f64,
    pub label: SectionLabel,
    /// The section's loudness relative to the song, `0.0..=1.0`.
    pub energy: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SongAnalysis {
    pub duration_seconds: f64,
    /// Zero when the recording has no steady pulse; the grid and bars are then empty.
    pub bpm: f32,
    pub beats_seconds: Vec<f64>,
    /// The first beat of every bar.
    pub downbeats_seconds: Vec<f64>,
    pub sections: Vec<SongSection>,
}

/// Analyses a mono recording. `progress` is told how far the work is, `0.0..=1.0`, and returns
/// `false` to cancel, in which case nothing is returned.
pub fn analyse_song(
    samples: &[f32],
    sample_rate: f32,
    mut progress: impl FnMut(f32) -> bool,
) -> Option<SongAnalysis> {
    let duration_seconds = samples.len() as f64 / f64::from(sample_rate);
    let mut detector = Detector::new(sample_rate);
    let mut kicks: Vec<(f64, f32)> = Vec::new();
    let mut tempi: Vec<f32> = Vec::new();
    let mut frames: Vec<FrameFeature> = Vec::new();
    let mut spectrum = Spectrum::new(FRAME);
    let mut power = vec![0.0; FRAME / 2];
    let pitch_class = pitch_classes(sample_rate);
    let chunk = HOP * 64;
    let mut position = 0;
    while position < samples.len() {
        let end = (position + chunk).min(samples.len());
        detector.push(&samples[position..end], |onset| {
            if onset.instrument == Instrument::Kick {
                kicks.push((onset.sample as f64 / f64::from(sample_rate), onset.strength));
            }
        });
        let reading = detector.reading();
        if reading.bpm > 0.0 && reading.tempo_confidence > 0.2 {
            tempi.push(reading.bpm);
        }
        // Back-to-back spectral frames are plenty for describing bars.
        let mut frame_start = position;
        while frame_start < end && frame_start + FRAME <= samples.len() {
            frames.push(frame_feature(
                &mut spectrum,
                &mut power,
                &pitch_class,
                &samples[frame_start..frame_start + FRAME],
                frame_start as f64 / f64::from(sample_rate),
            ));
            frame_start += FRAME;
        }
        position = end;
        if !progress(0.8 * position as f32 / samples.len().max(1) as f32) {
            return None;
        }
    }
    let bpm = settle_tempo(&tempi);
    let (beats, downbeats) = if bpm > 0.0 {
        beat_grid(bpm, &kicks, duration_seconds)
    } else {
        (Vec::new(), Vec::new())
    };
    if !progress(0.9) {
        return None;
    }
    let bars = if downbeats.len() >= 2 {
        downbeats.clone()
    } else {
        // Without a pulse, fall back to eight-second blocks so sections can still be found.
        (0..)
            .map(|index| f64::from(index) * 8.0)
            .take_while(|time| *time < duration_seconds)
            .collect()
    };
    let sections = sections(&bars, &frames, duration_seconds);
    progress(1.0);
    Some(SongAnalysis {
        duration_seconds,
        bpm,
        beats_seconds: beats,
        downbeats_seconds: if bpm > 0.0 { downbeats } else { Vec::new() },
        sections,
    })
}

#[derive(Debug, Clone)]
struct FrameFeature {
    time: f64,
    chroma: [f32; 12],
    energy: f32,
    brightness: f32,
}

fn pitch_classes(sample_rate: f32) -> Vec<Option<usize>> {
    (0..FRAME / 2)
        .map(|bin| {
            let frequency = bin as f32 * sample_rate / FRAME as f32;
            (55.0..5000.0).contains(&frequency).then(|| {
                let note = 69.0 + 12.0 * (frequency / 440.0).log2();
                (note.round() as i64).rem_euclid(12) as usize
            })
        })
        .collect()
}

fn frame_feature(
    spectrum: &mut Spectrum,
    power: &mut [f32],
    pitch_class: &[Option<usize>],
    samples: &[f32],
    time: f64,
) -> FrameFeature {
    spectrum.power(|index| samples[index], power);
    let mut chroma = [0.0f32; 12];
    let mut total = 0.0f32;
    let mut high = 0.0f32;
    for (bin, value) in power.iter().enumerate() {
        total += value;
        if bin > power.len() / 8 {
            high += value;
        }
        if let Some(class) = pitch_class[bin] {
            chroma[class] += value;
        }
    }
    let norm = chroma.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut chroma {
            *value /= norm;
        }
    }
    let rms =
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt();
    FrameFeature {
        time,
        chroma,
        energy: (rms + 1e-6).log10(),
        brightness: if total > 0.0 { high / total } else { 0.0 },
    }
}

/// The tempo the song settles on: the median of the confident later readings, folded into
/// 70–180 BPM so a half- or double-time reading names the same pulse.
fn settle_tempo(tempi: &[f32]) -> f32 {
    if tempi.len() < 4 {
        return 0.0;
    }
    let mut later: Vec<f32> = tempi[tempi.len() / 3..]
        .iter()
        .map(|bpm| {
            let mut value = *bpm;
            while value < 70.0 {
                value *= 2.0;
            }
            while value > 180.0 {
                value /= 2.0;
            }
            value
        })
        .collect();
    later.sort_by(f32::total_cmp);
    later[later.len() / 2]
}

/// A regular grid at `bpm` whose phase lands most kicks on beats, and whose bar starts land on
/// the beats with the strongest kicks.
fn beat_grid(bpm: f32, kicks: &[(f64, f32)], duration: f64) -> (Vec<f64>, Vec<f64>) {
    // Refine the period around the settled tempo against the kicks themselves.
    let mut best = (f64::from(60.0 / bpm), 0.0, f64::MIN);
    for step in -20..=20 {
        let period = f64::from(60.0 / bpm) * (1.0 + f64::from(step) * 0.0005);
        for phase_step in 0..96 {
            let phase = period * f64::from(phase_step) / 96.0;
            let score: f64 = kicks
                .iter()
                .map(|(time, strength)| {
                    let offset = ((time - phase) / period).rem_euclid(1.0);
                    let distance = offset.min(1.0 - offset);
                    f64::from(*strength) * (-(distance * distance) / 0.004).exp()
                })
                .sum();
            if score > best.2 {
                best = (period, phase, score);
            }
        }
    }
    let (period, phase, _) = best;
    let beats: Vec<f64> = (0..)
        .map(|index| phase + f64::from(index) * period)
        .take_while(|time| *time < duration)
        .collect();
    let weight = |offset: usize| -> f64 {
        beats
            .iter()
            .skip(offset)
            .step_by(BEATS_PER_BAR)
            .map(|beat| {
                kicks
                    .iter()
                    .filter(|(time, _)| (time - beat).abs() < period * 0.15)
                    .map(|(_, strength)| f64::from(*strength))
                    .sum::<f64>()
            })
            .sum()
    };
    // The bar starts where the kicks are clearly strongest; when every beat hits alike, the
    // first beat of the recording is the first beat of a bar.
    let mut bar_offset = 0;
    for offset in 1..BEATS_PER_BAR {
        if weight(offset) > weight(bar_offset) * 1.05 {
            bar_offset = offset;
        }
    }
    let downbeats = beats
        .iter()
        .skip(bar_offset)
        .step_by(BEATS_PER_BAR)
        .copied()
        .collect();
    (beats, downbeats)
}

/// Bars described, compared with every other bar, and split where the comparison changes.
fn sections(bars: &[f64], frames: &[FrameFeature], duration: f64) -> Vec<SongSection> {
    if bars.is_empty() || frames.is_empty() {
        return Vec::new();
    }
    let features: Vec<Vec<f32>> = bars
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = bars.get(index + 1).copied().unwrap_or(duration);
            bar_feature(frames, *start, end)
        })
        .collect();
    let normalised = standardise(&features);
    let count = normalised.len();
    let similarity = |left: usize, right: usize| cosine(&normalised[left], &normalised[right]);
    // Foote novelty: high where the bars before a point resemble each other and the bars after
    // do too, but the two sides do not resemble each other.
    let novelty: Vec<f32> = (0..count)
        .map(|centre| {
            let mut score = 0.0;
            for offset_a in 1..=KERNEL_BARS {
                for offset_b in 1..=KERNEL_BARS {
                    let (Some(before_a), Some(before_b)) =
                        (centre.checked_sub(offset_a), centre.checked_sub(offset_b))
                    else {
                        continue;
                    };
                    let (after_a, after_b) = (centre + offset_a - 1, centre + offset_b - 1);
                    if after_a >= count || after_b >= count {
                        continue;
                    }
                    score += similarity(before_a, before_b) + similarity(after_a, after_b)
                        - similarity(before_a, after_b)
                        - similarity(after_a, before_b);
                }
            }
            score
        })
        .collect();
    let mean = novelty.iter().sum::<f32>() / count as f32;
    let deviation = (novelty
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f32>()
        / count as f32)
        .sqrt();
    let mut boundaries = vec![0usize];
    let mut candidates: Vec<usize> = (1..count)
        .filter(|&index| {
            let window = index.saturating_sub(2)..(index + 3).min(count);
            novelty[index] > mean + 0.5 * deviation
                && window.clone().all(|other| novelty[other] <= novelty[index])
        })
        .collect();
    candidates.sort_by(|left, right| novelty[*right].total_cmp(&novelty[*left]));
    for candidate in candidates {
        if boundaries
            .iter()
            .all(|boundary| boundary.abs_diff(candidate) >= MIN_SECTION_BARS)
            && count - candidate >= MIN_SECTION_BARS / 2
        {
            boundaries.push(candidate);
        }
    }
    boundaries.sort_unstable();
    label_sections(bars, &features, &normalised, &boundaries)
}

fn bar_feature(frames: &[FrameFeature], start: f64, end: f64) -> Vec<f32> {
    let inside: Vec<&FrameFeature> = frames
        .iter()
        .filter(|frame| frame.time >= start && frame.time < end)
        .collect();
    let chosen: Vec<&FrameFeature> = if inside.is_empty() {
        frames
            .iter()
            .min_by(|left, right| {
                (left.time - start)
                    .abs()
                    .total_cmp(&(right.time - start).abs())
            })
            .into_iter()
            .collect()
    } else {
        inside
    };
    let count = chosen.len() as f32;
    let mut feature = vec![0.0; 14];
    for frame in &chosen {
        for (slot, value) in feature.iter_mut().zip(frame.chroma) {
            *slot += value / count;
        }
        feature[12] += frame.energy / count;
        feature[13] += frame.brightness / count;
    }
    feature
}

fn standardise(features: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let dimensions = features[0].len();
    let count = features.len() as f32;
    let mut result = features.to_vec();
    for dimension in 0..dimensions {
        let mean = features
            .iter()
            .map(|feature| feature[dimension])
            .sum::<f32>()
            / count;
        let deviation = (features
            .iter()
            .map(|feature| (feature[dimension] - mean).powi(2))
            .sum::<f32>()
            / count)
            .sqrt()
            .max(1e-4);
        // Energy and brightness weigh as much as the twelve pitch classes together.
        let weight = if dimension >= 12 { 2.5 } else { 1.0 };
        for feature in &mut result {
            feature[dimension] = weight * (feature[dimension] - mean) / deviation;
        }
    }
    result
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let dot: f32 = left.iter().zip(right).map(|(a, b)| a * b).sum();
    let norms = left.iter().map(|a| a * a).sum::<f32>().sqrt()
        * right.iter().map(|b| b * b).sum::<f32>().sqrt();
    if norms > 0.0 { dot / norms } else { 0.0 }
}

fn label_sections(
    bars: &[f64],
    raw: &[Vec<f32>],
    normalised: &[Vec<f32>],
    boundaries: &[usize],
) -> Vec<SongSection> {
    let spans: Vec<(usize, usize)> = boundaries
        .iter()
        .enumerate()
        .map(|(index, start)| {
            (
                *start,
                boundaries.get(index + 1).copied().unwrap_or(raw.len()),
            )
        })
        .collect();
    let mean = |range: (usize, usize), data: &[Vec<f32>]| -> Vec<f32> {
        let width = data[0].len();
        let mut sum = vec![0.0; width];
        for bar in &data[range.0..range.1] {
            for (slot, value) in sum.iter_mut().zip(bar) {
                *slot += value;
            }
        }
        sum.iter()
            .map(|value| value / (range.1 - range.0) as f32)
            .collect()
    };
    let energies: Vec<f32> = spans.iter().map(|span| mean(*span, raw)[12]).collect();
    let (low, high) = energies
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), value| {
            (low.min(*value), high.max(*value))
        });
    let relative: Vec<f32> = energies
        .iter()
        .map(|energy| {
            if high > low {
                (energy - low) / (high - low)
            } else {
                0.5
            }
        })
        .collect();
    let profiles: Vec<Vec<f32>> = spans.iter().map(|span| mean(*span, normalised)).collect();
    // Passages that return: a section belongs with the first earlier one it closely resembles.
    let mut family = vec![usize::MAX; spans.len()];
    for index in 0..spans.len() {
        family[index] = (0..index)
            .find(|earlier| cosine(&profiles[index], &profiles[*earlier]) > 0.8)
            .map_or(index, |earlier| family[earlier]);
    }
    let repeats = |index: usize| {
        family
            .iter()
            .filter(|other| **other == family[index])
            .count()
            > 1
    };
    let loudest_repeating = (0..spans.len())
        .filter(|index| repeats(*index))
        .max_by(|left, right| relative[*left].total_cmp(&relative[*right]))
        .map(|index| family[index]);
    let last = spans.len() - 1;
    spans
        .iter()
        .enumerate()
        .map(|(index, span)| {
            let quiet = relative[index] < 0.5;
            let label = if Some(family[index]) == loudest_repeating {
                SectionLabel::Chorus
            } else if repeats(index) {
                SectionLabel::Verse
            } else if index == 0 && quiet && spans.len() > 1 {
                SectionLabel::Intro
            } else if index == last && quiet && spans.len() > 1 {
                SectionLabel::Outro
            } else if quiet {
                SectionLabel::Breakdown
            } else {
                SectionLabel::Bridge
            };
            SongSection {
                start_seconds: bars[span.0],
                label,
                energy: relative[index],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
