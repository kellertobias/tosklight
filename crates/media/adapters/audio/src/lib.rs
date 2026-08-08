#![forbid(unsafe_code)]

//! Audio capture.
//!
//! The analysis is platform-independent and lives in the domain. This is the part that genuinely
//! is a platform adapter: opening an input device and getting samples off its real-time callback.
//!
//! The one rule that shapes everything here is what the callback may do. It may not run an FFT,
//! allocate, log, switch devices, or take a lock that could block — the operating system will drop
//! audio if it is late, and a glitch during a show is not recoverable. So the callback does one
//! thing: it pushes samples into a bounded lock-free queue. A worker thread drains that queue,
//! runs the analysis, and publishes a snapshot.
//!
//! A machine with no input device says so and the rest of the server runs. Silence is a real
//! analysis, not a failure.

mod service;
mod snapshot;

pub use service::{AudioError, AudioService};
pub use snapshot::{AnalysisSnapshot, SharedAnalysis, Worker};

/// How many samples the callback may run ahead of the worker.
///
/// Two analysis windows: enough to absorb a scheduling hiccup, small enough that a worker which
/// falls behind drops old audio instead of analysing something an operator heard a second ago.
pub const QUEUE_CAPACITY: usize = media_domain::audio::WINDOW * 2;
