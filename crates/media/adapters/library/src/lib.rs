#![forbid(unsafe_code)]

//! The media library on disk.
//!
//! Discovery reads the filesystem into an immutable catalog snapshot; edits are applied so that a
//! failure leaves the library as it was. Every consumer reads the snapshot rather than scanning
//! for itself, so nothing can disagree about what the library contains.

pub mod discovery;
pub mod importer;
pub mod jobs;
pub mod naming;
pub mod storage;
pub mod thumbnails;

pub use discovery::{DiscoveryError, Pending, discover, pending_imports};
pub use importer::{Importer, Published};
pub use jobs::{ImportQueue, Job, JobId, JobState};
pub use storage::{LibraryStorage, StorageError};
pub use thumbnails::ThumbnailError;
