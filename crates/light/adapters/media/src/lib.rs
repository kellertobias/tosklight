#![forbid(unsafe_code)]
//! Bounded CITP/MSEX 1.2 client primitives for media thumbnails and output previews.

mod cache;
mod client;
mod model;
mod protocol;

pub use cache::MediaCache;
pub use client::CitpClient;
pub use model::{
    CachedImage, ImageFormat, LibraryId, MediaControlCapability, MediaError, MediaImage,
    MediaLayerCapabilities, MediaLayerStatus, MediaLibraryElement, MediaLibraryFolder,
    MediaPreviewSource, MediaProviderCapabilities, MediaServerInformation, MediaServerSnapshot,
    PreviewKey, ThumbnailKey,
};

pub const DEFAULT_CITP_PORT: u16 = 4809;

/// Announces the Light Desk's active show as a CITP lighting-console identity.
pub fn console_announcement(show_name: &str) -> Vec<u8> {
    fn ucs1(target: &mut Vec<u8>, value: &str) {
        target.extend(
            value
                .bytes()
                .map(|byte| if byte.is_ascii() { byte } else { b'?' }),
        );
        target.push(0);
    }

    let mut body = Vec::new();
    body.extend_from_slice(&0_u16.to_le_bytes());
    ucs1(&mut body, "LightingConsole");
    ucs1(&mut body, show_name);
    ucs1(&mut body, "Running");
    let size = 24 + body.len();
    let mut message = Vec::with_capacity(size);
    message.extend_from_slice(b"CITP");
    message.extend_from_slice(&[1, 0]);
    message.extend_from_slice(&0_u16.to_le_bytes());
    message.extend_from_slice(&(size as u32).to_le_bytes());
    message.extend_from_slice(&1_u16.to_le_bytes());
    message.extend_from_slice(&0_u16.to_le_bytes());
    message.extend_from_slice(b"PINFPLoc");
    message.extend_from_slice(&body);
    message
}

#[cfg(test)]
mod tests;
