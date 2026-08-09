#![forbid(unsafe_code)]

//! The Media playback codec.
//!
//! Import normalises every source to HAP Alpha, so this is the only format playback ever reads.
//! See `docs/engineering/media-playback-codec-decision.md` for why.

pub mod cache;
pub mod container;
pub mod hap;
pub mod import;

pub use cache::{AdmissionError, ClipCache, Residency, ResidentClip};
pub use container::{ClipHeader, ClipReader, ClipWriter, ContainerError, FrameEntry};
pub use hap::{BC3_BLOCK_BYTES, FrameError, TEXTURE_FORMAT, block_bytes, decode_blocks, encode};
pub use import::{ImportError, Progress, SourceInfo, import, probe};
