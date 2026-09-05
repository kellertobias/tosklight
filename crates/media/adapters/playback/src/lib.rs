#![forbid(unsafe_code)]

//! Media playback.
//!
//! One session per layer's selected asset, each owning its own transport, and a loader that makes
//! clips resident so playback never waits on storage.

pub mod async_loader;
pub mod loader;
pub use async_loader::AsyncClipLoader;
pub mod session;
pub mod sessions;
pub mod source;
pub use source::MediaLoader;

pub use loader::{ClipLoader, LoadError, LoadProgress};
pub use session::{Delivery, PlaybackSession};
pub use sessions::{LayerSessions, LayerSource};
