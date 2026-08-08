#![forbid(unsafe_code)]

//! The Media playback codec.
//!
//! Import normalises every source to HAP Alpha, so this is the only format playback ever reads.
//! See `docs/engineering/media-playback-codec-decision.md` for why.

pub mod hap;

pub use hap::{BC3_BLOCK_BYTES, FrameError, TEXTURE_FORMAT, block_bytes, decode_blocks, encode};
