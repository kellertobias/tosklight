//! Pure, frame-addressed Timecode show data and deterministic reconstruction.
//!
//! This module deliberately owns no clock, storage, audio device, or application service. A
//! runtime can therefore reconstruct the same state for continuous playback, seek, and loop.

mod model;
mod reconstruction;
mod transport;
mod validation;

pub use model::*;
pub use transport::*;
pub use validation::*;

#[cfg(test)]
mod tests;
