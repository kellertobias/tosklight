//! Thumbnails.
//!
//! One 128-pixel-wide JPEG per item, matching the legacy layout so an existing library's
//! thumbnails are still found. Automatic thumbnails come from the playable `.toskclip`, which
//! makes retry independent of whether the original upload still exists.

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageReader};
use media_codec::ClipReader;
use media_codec::hap::{decode_blocks, expand_to_rgba};
use media_domain::{CatalogLocation, MediaAddress};

use crate::storage::LibraryStorage;

pub const THUMBNAIL_WIDTH: u32 = 128;
/// The browser library inspector is deliberately modest: it is a management preview, not a
/// second output surface.
pub const PREVIEW_WIDTH: u32 = 640;
const MAX_SAMPLED_FRAMES: usize = 12;
const MAX_CUSTOM_DIMENSION: u32 = 8_192;
const MAX_CUSTOM_DECODE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_THUMBNAIL_HEIGHT: u32 = 512;

#[derive(Debug, thiserror::Error)]
pub enum ThumbnailError {
    #[error("the playable clip at {} could not be read: {detail}", path.display())]
    Unreadable {
        path: std::path::PathBuf,
        detail: String,
    },
    #[error("the custom thumbnail is not a supported, readable image: {0}")]
    InvalidImage(String),
    #[error("cannot write the thumbnail to {}: {source}", path.display())]
    Unwritable {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Chooses the most useful of a bounded set of frames from the playable clip.
pub fn generate(
    storage: &LibraryStorage,
    address: CatalogLocation,
    clip: &Path,
) -> Result<std::path::PathBuf, ThumbnailError> {
    generate_cancellable(storage, address, clip, std::sync::Arc::new(|| false))
}

pub fn generate_cancellable(
    storage: &LibraryStorage,
    address: CatalogLocation,
    clip: &Path,
    cancelled: media_codec::import::Cancellation,
) -> Result<std::path::PathBuf, ThumbnailError> {
    let file = std::fs::File::open(clip).map_err(|error| unreadable(clip, error))?;
    let mut reader = ClipReader::open(file).map_err(|error| unreadable(clip, error))?;
    let width = reader.header().width;
    let height = reader.header().height;
    let indices = sample_indices(reader.index().len());
    let mut best: Option<(u64, Vec<u8>)> = None;

    for index in indices {
        if cancelled() {
            return Err(ThumbnailError::Unreadable {
                path: clip.to_path_buf(),
                detail: "thumbnail generation was cancelled".to_owned(),
            });
        }
        let Some(payload) = reader
            .frame(index)
            .map_err(|error| unreadable(clip, error))?
        else {
            continue;
        };
        let blocks =
            decode_blocks(width, height, &payload).map_err(|error| unreadable(clip, error))?;
        let rgba =
            expand_to_rgba(width, height, &blocks).map_err(|error| unreadable(clip, error))?;
        let score = usefulness_score(&rgba);
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, rgba));
        }
    }

    let (_, pixels) = best.ok_or_else(|| ThumbnailError::Unreadable {
        path: clip.to_path_buf(),
        detail: "the clip contains no readable frames".to_owned(),
    })?;
    write_rgba_thumbnail(storage, address, width, height, pixels)
}

/// Decodes one native clip frame into a browser-safe JPEG.
///
/// Pixel stores HAP frames in `.toskclip`, which browsers do not generally understand. Keeping
/// this conversion beside thumbnail generation means the web library can still show motion
/// without ever exposing a filesystem path or pretending that the original is browser-playable.
pub fn preview_frame(clip: &Path, requested_frame: usize) -> Result<Vec<u8>, ThumbnailError> {
    let file = std::fs::File::open(clip).map_err(|error| unreadable(clip, error))?;
    let mut reader = ClipReader::open(file).map_err(|error| unreadable(clip, error))?;
    let width = reader.header().width;
    let height = reader.header().height;
    let frame_count = reader.index().len();
    if frame_count == 0 {
        return Err(ThumbnailError::Unreadable {
            path: clip.to_path_buf(),
            detail: "the clip contains no readable frames".to_owned(),
        });
    }
    let index = requested_frame % frame_count;
    let payload = reader
        .frame(index)
        .map_err(|error| unreadable(clip, error))?
        .ok_or_else(|| ThumbnailError::Unreadable {
            path: clip.to_path_buf(),
            detail: "the requested frame is absent".to_owned(),
        })?;
    let blocks = decode_blocks(width, height, &payload).map_err(|error| unreadable(clip, error))?;
    let rgba = expand_to_rgba(width, height, &blocks).map_err(|error| unreadable(clip, error))?;
    let image = image::RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
        ThumbnailError::InvalidImage(
            "the decoded frame dimensions do not match its pixels".to_owned(),
        )
    })?;
    let preview_height = ((u64::from(image.height()) * u64::from(PREVIEW_WIDTH)
        / u64::from(image.width().max(1))) as u32)
        .clamp(1, MAX_THUMBNAIL_HEIGHT);
    let scaled = DynamicImage::ImageRgba8(image).resize_exact(
        PREVIEW_WIDTH,
        preview_height,
        FilterType::Triangle,
    );
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, 78)
        .encode_image(&scaled)
        .map_err(|error| ThumbnailError::InvalidImage(error.to_string()))?;
    Ok(encoded)
}

/// Validates and normalizes an uploaded image into the same JPEG companion artifact.
pub fn replace_from_image(
    storage: &LibraryStorage,
    address: CatalogLocation,
    bytes: &[u8],
) -> Result<std::path::PathBuf, ThumbnailError> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader = reader
        .with_guessed_format()
        .map_err(|error| ThumbnailError::InvalidImage(error.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_CUSTOM_DIMENSION);
    limits.max_image_height = Some(MAX_CUSTOM_DIMENSION);
    limits.max_alloc = Some(MAX_CUSTOM_DECODE_BYTES);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| ThumbnailError::InvalidImage(error.to_string()))?;
    if image.width() == 0 || image.height() == 0 {
        return Err(ThumbnailError::InvalidImage(
            "the image has no pixels".to_owned(),
        ));
    }
    write_image_thumbnail(storage, address, image)
}

fn write_rgba_thumbnail(
    storage: &LibraryStorage,
    address: CatalogLocation,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
) -> Result<std::path::PathBuf, ThumbnailError> {
    let image = image::RgbaImage::from_raw(width, height, pixels).ok_or_else(|| {
        ThumbnailError::InvalidImage(
            "the decoded frame dimensions do not match its pixels".to_owned(),
        )
    })?;
    write_image_thumbnail(storage, address, DynamicImage::ImageRgba8(image))
}

fn write_image_thumbnail(
    storage: &LibraryStorage,
    address: CatalogLocation,
    image: DynamicImage,
) -> Result<std::path::PathBuf, ThumbnailError> {
    let destination = storage.thumbnail_path(address);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ThumbnailError::Unwritable {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let height = ((u64::from(image.height()) * u64::from(THUMBNAIL_WIDTH)
        / u64::from(image.width().max(1))) as u32)
        .clamp(1, MAX_THUMBNAIL_HEIGHT);
    let scaled = image.resize_exact(THUMBNAIL_WIDTH, height, FilterType::Triangle);
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, 78)
        .encode_image(&scaled)
        .map_err(|error| ThumbnailError::InvalidImage(error.to_string()))?;
    let staging = destination.with_extension("jpg.pending");
    let backup = destination.with_extension("jpg.replacing");
    if !destination.exists() && backup.exists() {
        std::fs::rename(&backup, &destination).map_err(|source| ThumbnailError::Unwritable {
            path: destination.clone(),
            source,
        })?;
    } else {
        let _ = std::fs::remove_file(&backup);
    }
    std::fs::write(&staging, encoded).map_err(|source| ThumbnailError::Unwritable {
        path: staging.clone(),
        source,
    })?;
    if destination.exists() {
        std::fs::rename(&destination, &backup).map_err(|source| ThumbnailError::Unwritable {
            path: destination.clone(),
            source,
        })?;
    }
    std::fs::rename(&staging, &destination).map_err(|source| {
        let _ = std::fs::remove_file(&staging);
        if backup.exists() {
            let _ = std::fs::rename(&backup, &destination);
        }
        ThumbnailError::Unwritable {
            path: destination.clone(),
            source,
        }
    })?;
    let _ = std::fs::remove_file(backup);
    Ok(destination)
}

fn sample_indices(frame_count: usize) -> Vec<usize> {
    let samples = frame_count.min(MAX_SAMPLED_FRAMES);
    match samples {
        0 => Vec::new(),
        1 => vec![0],
        _ => (0..samples)
            .map(|sample| sample * (frame_count - 1) / (samples - 1))
            .collect(),
    }
}

/// Favour visible pixels first, then detail and colour. A fully black frame therefore loses to
/// any meaningful frame, while an all-white flash does not automatically beat a detailed image.
fn usefulness_score(rgba: &[u8]) -> u64 {
    let mut visible = 0u64;
    let mut luminance_sum = 0u64;
    let mut luminance_squared = 0u64;
    let mut saturation = 0u64;
    let mut pixels = 0u64;
    for pixel in rgba.chunks_exact(4) {
        let [red, green, blue, alpha] = [pixel[0], pixel[1], pixel[2], pixel[3]];
        let luminance = (u64::from(red) * 54 + u64::from(green) * 183 + u64::from(blue) * 19) / 256;
        if alpha > 16 && luminance > 16 {
            visible += 1;
        }
        luminance_sum += luminance;
        luminance_squared += luminance * luminance;
        saturation += u64::from(red.max(green).max(blue) - red.min(green).min(blue));
        pixels += 1;
    }
    if pixels == 0 {
        return 0;
    }
    let variance = luminance_squared / pixels - (luminance_sum / pixels).pow(2);
    visible * 1_000_000 / pixels + variance * 32 + saturation / pixels
}

fn unreadable(path: &Path, error: impl std::fmt::Display) -> ThumbnailError {
    ThumbnailError::Unreadable {
        path: path.to_path_buf(),
        detail: error.to_string(),
    }
}

pub fn exists(storage: &LibraryStorage, address: MediaAddress) -> bool {
    storage.thumbnail_path(address).exists()
}

pub fn remove(storage: &LibraryStorage, address: MediaAddress) {
    let _ = std::fs::remove_file(storage.thumbnail_path(address));
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use media_codec::{ClipHeader, ClipWriter};

    use super::*;

    fn storage(name: &str) -> LibraryStorage {
        let root = std::env::temp_dir().join("media-thumbnails").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        LibraryStorage::new(root)
    }

    fn clip(path: &Path, colours: &[[u8; 4]]) {
        let mut writer = ClipWriter::new(
            Cursor::new(Vec::new()),
            ClipHeader {
                width: 8,
                height: 8,
                frame_count: 0,
                frame_rate: (25, 1),
                intrinsic_bpm: None,
            },
        )
        .unwrap();
        for (index, colour) in colours.iter().enumerate() {
            let rgba = colour.repeat(64);
            let encoded = media_codec::hap::encode(8, 8, &rgba).unwrap();
            writer.write_frame(&encoded, index as u64 * 40_000).unwrap();
        }
        std::fs::write(path, writer.finish().unwrap().into_inner()).unwrap();
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
    fn automatic_selection_prefers_a_visible_frame_over_black_frames() {
        let storage = storage("useful-frame");
        let source = storage.root().join("clip.toskclip");
        clip(
            &source,
            &[[0, 0, 0, 255], [0, 0, 0, 255], [30, 180, 80, 255]],
        );

        let output = generate(&storage, MediaAddress::new(1, 1).into(), &source).unwrap();
        let thumbnail = image::open(output).unwrap().to_rgb8();
        assert!(
            thumbnail
                .pixels()
                .any(|pixel| pixel.0.iter().any(|channel| *channel > 40)),
            "a useful later frame should beat the black opening"
        );
        let _ = std::fs::remove_dir_all(storage.root());
    }

    #[test]
    fn preview_frames_are_browser_safe_and_wrap_a_native_clip() {
        let storage = storage("preview-frame");
        let source = storage.root().join("clip.toskclip");
        clip(&source, &[[220, 10, 10, 255], [10, 10, 220, 255]]);

        let jpeg = preview_frame(&source, 3).unwrap();
        assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
        let image = image::load_from_memory(&jpeg).unwrap().to_rgb8();
        let pixel = image.get_pixel(0, 0).0;
        assert!(
            pixel[2] > pixel[0],
            "frame 3 wraps to the blue second frame"
        );
        assert_eq!(image.width(), PREVIEW_WIDTH);
        let _ = std::fs::remove_dir_all(storage.root());
    }

    #[test]
    fn custom_images_are_validated_normalized_and_replace_only_after_success() {
        let storage = storage("custom");
        let address = MediaAddress::new(1, 2);
        let destination = storage.thumbnail_path(address);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, b"existing").unwrap();
        assert!(matches!(
            replace_from_image(&storage, address.into(), b"not an image"),
            Err(ThumbnailError::InvalidImage(_))
        ));
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing");

        let mut png = Cursor::new(Vec::new());
        DynamicImage::new_rgb8(20, 10)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        replace_from_image(&storage, address.into(), png.get_ref()).unwrap();
        let normalized = image::open(&destination).unwrap();
        assert_eq!((normalized.width(), normalized.height()), (128, 64));
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
        remove(&storage, address);
        let _ = std::fs::remove_dir_all(storage.root());
    }

    #[test]
    fn a_source_that_is_not_a_clip_reports_without_destroying_the_old_thumbnail() {
        let storage = storage("bad-source");
        let source = storage.root().join("not-media.txt");
        std::fs::write(&source, b"this is not a video").unwrap();
        let destination = storage.thumbnail_path(MediaAddress::new(1, 1));
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, b"existing").unwrap();

        assert!(matches!(
            generate(&storage, MediaAddress::new(1, 1).into(), &source),
            Err(ThumbnailError::Unreadable { .. })
        ));
        assert_eq!(std::fs::read(destination).unwrap(), b"existing");
        let _ = std::fs::remove_dir_all(storage.root());
    }
}
