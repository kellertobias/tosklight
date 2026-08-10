#![forbid(unsafe_code)]
//! Bounded CITP/MSEX 1.2 client primitives for media thumbnails and output previews.

mod cache;
mod client;
mod model;
mod protocol;

pub use cache::MediaCache;
pub use client::CitpClient;
pub use model::{
    CachedImage, ImageFormat, LibraryId, MediaError, MediaImage, MediaLayerStatus,
    MediaLibraryElement, MediaLibraryFolder, MediaPreviewSource, MediaServerInformation,
    MediaServerSnapshot, PreviewKey, ThumbnailKey,
};

pub const DEFAULT_CITP_PORT: u16 = 4809;

#[cfg(test)]
mod tests;
