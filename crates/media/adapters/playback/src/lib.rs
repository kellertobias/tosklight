#![forbid(unsafe_code)]

//! Media playback.
//!
//! One session per layer's selected asset, each owning its own transport, and a loader that makes
//! clips resident so playback never waits on storage.

pub mod loader;
pub mod session;
pub mod sessions;

pub use loader::{ClipLoader, LoadError, LoadProgress};
pub use session::{Delivery, PlaybackSession};
pub use sessions::{LayerSessions, LayerSource};
