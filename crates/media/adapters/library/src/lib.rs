#![forbid(unsafe_code)]

//! The media library on disk.
//!
//! Discovery reads the filesystem into an immutable catalog snapshot; edits are applied so that a
//! failure leaves the library as it was. Every consumer reads the snapshot rather than scanning
//! for itself, so nothing can disagree about what the library contains.

pub mod discovery;
pub mod naming;

pub use discovery::{DiscoveryError, discover};
