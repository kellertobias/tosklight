//! Replays a recording through the detector and prints what it heard, second by second.
//!
//! Reads mono 32-bit float little-endian samples on standard input:
//!
//! ```sh
//! sox recording.wav -t f32 -c 1 -r 48000 - | cargo run --release -p light-beat --example analyse_raw -- 48000
//! ```
//!
//! Pass `--onsets` to print every hit as well.

use std::io::Read as _;

use light_beat::{Detector, Instrument};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let rate: f32 = arguments
        .iter()
        .find_map(|argument| argument.parse().ok())
        .unwrap_or(48_000.0);
    let verbose = arguments.iter().any(|argument| argument == "--onsets");
    let per_hop = arguments.iter().any(|argument| argument == "--hops");

    let mut bytes = Vec::new();
    std::io::stdin()
        .read_to_end(&mut bytes)
        .expect("samples on standard input");
    let (whole, _) = bytes.as_chunks::<4>();
    let samples: Vec<f32> = whole
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect();

    let mut detector = Detector::new(rate);
    if per_hop {
        // One CSV row per hop, for plotting: what a visual would read at that instant.
        println!(
            "seconds,kick_level,kick_hit,snare_level,snare_hit,hat_level,hat_hit,bpm,phase,beat,gain,clipping,onset"
        );
        for (hop, chunk) in samples.chunks(light_beat::HOP).enumerate() {
            let mut struck = String::new();
            detector.push(chunk, |onset| {
                struck.push(match onset.instrument {
                    Instrument::Kick => 'K',
                    Instrument::Snare => 'S',
                    Instrument::HiHat => 'H',
                });
            });
            let r = detector.reading();
            println!(
                "{:.4},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.2},{:.3},{:.3},{:.3},{},{}",
                (hop + 1) as f32 * light_beat::HOP as f32 / rate,
                r.kick.level,
                r.kick.hit,
                r.snare.level,
                r.snare.hit,
                r.hihat.level,
                r.hihat.hit,
                r.bpm,
                r.beat_phase,
                r.beat,
                r.gain,
                u8::from(r.clipping),
                struck
            );
        }
        return;
    }
    let mut totals = [0usize; 3];
    let started = std::time::Instant::now();
    for (second, chunk) in samples.chunks(rate as usize).enumerate() {
        let mut heard = [0usize; 3];
        detector.push(chunk, |onset| {
            let slot = match onset.instrument {
                Instrument::Kick => 0,
                Instrument::Snare => 1,
                Instrument::HiHat => 2,
            };
            heard[slot] += 1;
            if verbose {
                println!(
                    "{:9.3}  {:?}  {:.2}",
                    onset.sample as f32 / rate,
                    onset.instrument,
                    onset.strength
                );
            }
        });
        for (total, count) in totals.iter_mut().zip(heard) {
            *total += count;
        }
        let reading = detector.reading();
        println!(
            "{:4}s  bpm {:6.1}  confidence {:.2}  beats {:4}  kick/snare/hat {:2}/{:2}/{:2}  gain {:5.2}  rms {:.3}{}",
            second + 1,
            reading.bpm,
            reading.tempo_confidence,
            reading.beats,
            heard[0],
            heard[1],
            heard[2],
            reading.gain,
            reading.input_rms,
            if reading.clipping { "  CLIPPING" } else { "" },
        );
    }
    println!(
        "kick {}  snare {}  hi-hat {}  over {:.1} s of audio, analysed in {:.0} ms",
        totals[0],
        totals[1],
        totals[2],
        samples.len() as f32 / rate,
        started.elapsed().as_secs_f64() * 1_000.0
    );
}
