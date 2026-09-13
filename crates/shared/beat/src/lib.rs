#![forbid(unsafe_code)]

//! Hearing the music the way a lighting operator does.
//!
//! A visualizer or a playback that reacts to "the beat" wants more than a loudness threshold: it
//! wants to flash on the kick drum, shimmer on the hi-hat, and keep counting beats through a
//! breakdown where the kick drops out. This crate turns a mono sample stream into exactly that:
//!
//! - **Onsets** for the kick, the snare or clap, and the hi-hat, each with a strength.
//! - **Levels** per instrument, auto-ranged to `0.0..=1.0` so a quiet room and a loud club both
//!   use the whole range without an operator riding a fader.
//! - **A tempo and a beat phase** that keep running between hits, so a chase stays on the grid
//!   when the kick pauses.
//! - **An automatic input gain** for level displays, and a clipping flag, because no gain applied
//!   after the converter can undo an input that is already clipped.
//!
//! Detection compares each band's *logarithmic* power with its own recent past, so it does not
//! depend on the input level at all: the same song at -40 dBFS and at -6 dBFS produces the same
//! onsets. The gain only scales what is *displayed*.
//!
//! Everything is streaming and deterministic. Samples go in, events come out stamped with the
//! sample count that produced them; nothing reads a clock, allocates after construction, or asks
//! the platform for anything, so a recording replayed offline reproduces a live show exactly.

mod detector;
mod filter;
mod level;
mod onset;
mod spectrum;
mod tempo;

pub use detector::{Detector, HOP, Instrument, Onset, Reading, Tuning, Voice};
