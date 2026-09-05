//! Thumbnails.
//!
//! One 128-pixel-wide JPEG per item, matching the legacy layout so an existing library's
//! thumbnails are still found. FFmpeg makes them, out of process, for the same reason it decodes
//! media out of process: it keeps its licensing entirely separate and works for every source
//! format without this crate knowing any of them.

use std::path::Path;
use std::process::Command;

use media_domain::MediaAddress;

use crate::storage::LibraryStorage;

/// The width the legacy application used. Height follows the source's aspect ratio.
pub const THUMBNAIL_WIDTH: u32 = 128;

/// Why a thumbnail could not be made.
#[derive(Debug, thiserror::Error)]
pub enum ThumbnailError {
    #[error("FFmpeg is not installed or not on PATH; thumbnails need it")]
    FfmpegMissing,
    #[error("FFmpeg could not make a thumbnail from {}: {detail}", path.display())]
    Failed {
        path: std::path::PathBuf,
        detail: String,
    },
    #[error("cannot write the thumbnail to {}: {source}", path.display())]
    Unwritable {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Makes an item's thumbnail from a source file.
///
/// A video is sampled half a second in rather than at zero, because the first frame of a clip is
/// very often black and a library of black squares is useless.
pub fn generate(
    storage: &LibraryStorage,
    address: MediaAddress,
    source: &Path,
    is_video: bool,
) -> Result<std::path::PathBuf, ThumbnailError> {
    generate_cancellable(
        storage,
        address,
        source,
        is_video,
        std::sync::Arc::new(|| false),
    )
}

pub fn generate_cancellable(
    storage: &LibraryStorage,
    address: MediaAddress,
    source: &Path,
    is_video: bool,
    cancelled: media_codec::import::Cancellation,
) -> Result<std::path::PathBuf, ThumbnailError> {
    let destination = storage.thumbnail_path(address);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ThumbnailError::Unwritable {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut command = Command::new("ffmpeg");
    command.args(["-v", "error", "-y"]);
    if is_video {
        command.args(["-ss", "00:00:00.500"]);
    }
    command.arg("-i").arg(source);
    if is_video {
        command.args(["-frames:v", "1"]);
    }
    command
        .args(["-vf", &format!("scale={THUMBNAIL_WIDTH}:-1")])
        .args(["-q:v", "5"])
        .arg(&destination);

    let output = media_codec::import::command_output_cancellable(&mut command, cancelled).map_err(
        |error| {
            let _ = std::fs::remove_file(&destination);
            ThumbnailError::Failed {
                path: source.to_path_buf(),
                detail: error.to_string(),
            }
        },
    )?;
    if !output.status.success() || !destination.exists() {
        let _ = std::fs::remove_file(&destination);
        return Err(ThumbnailError::Failed {
            path: source.to_path_buf(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(destination)
}

/// Whether an item already has a thumbnail.
pub fn exists(storage: &LibraryStorage, address: MediaAddress) -> bool {
    storage.thumbnail_path(address).exists()
}

/// Removes an item's thumbnail. A missing one is not an error.
pub fn remove(storage: &LibraryStorage, address: MediaAddress) {
    let _ = std::fs::remove_file(storage.thumbnail_path(address));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage(name: &str) -> LibraryStorage {
        let root = std::env::temp_dir().join("media-thumbnails").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        LibraryStorage::new(root)
    }

    #[test]
    fn thumbnails_live_where_the_legacy_layout_puts_them() {
        let storage = storage("layout");
        let path = storage.thumbnail_path(MediaAddress::new(3, 7));
        assert!(path.ends_with(format!(
            "{}/007-thumb.jpg",
            crate::naming::THUMBNAIL_DIRECTORY
        )));
        assert!(
            path.components()
                .any(|component| component.as_os_str() == "003")
        );
        let _ = std::fs::remove_dir_all(storage.root());
    }

    #[test]
    fn presence_and_removal_are_reported_without_ceremony() {
        let storage = storage("presence");
        let address = MediaAddress::new(1, 2);
        assert!(!exists(&storage, address));

        let path = storage.thumbnail_path(address);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"thumb").unwrap();
        assert!(exists(&storage, address));

        remove(&storage, address);
        assert!(!exists(&storage, address));
        remove(&storage, address); // a missing one is not an error
        let _ = std::fs::remove_dir_all(storage.root());
    }

    #[test]
    fn a_source_that_is_not_media_reports_rather_than_writing_rubbish() {
        let storage = storage("bad-source");
        let source = storage.root().join("not-media.txt");
        std::fs::write(&source, b"this is not a video").unwrap();

        match generate(&storage, MediaAddress::new(1, 1), &source, true) {
            Err(ThumbnailError::Failed { .. } | ThumbnailError::FfmpegMissing) => {}
            other => panic!("expected a readable failure, got {other:?}"),
        }
        assert!(
            !exists(&storage, MediaAddress::new(1, 1)),
            "nothing was left behind"
        );
        let _ = std::fs::remove_dir_all(storage.root());
    }
}
