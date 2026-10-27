//! Decode before atomic no-overwrite publication. Staging alone is disposable, never user images.

use std::{
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    Cancelled,
    Failed(String),
}
impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Image rendering cancelled"),
            Self::Failed(detail) => f.write_str(detail),
        }
    }
}
impl std::error::Error for ImageError {}
impl From<std::io::Error> for ImageError {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(error.to_string())
    }
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), ImageError> {
    if cancel.load(Ordering::Acquire) {
        Err(ImageError::Cancelled)
    } else {
        Ok(())
    }
}

const SIZE: (u32, u32) = (3840, 2160);

#[derive(Clone, Debug)]
pub struct RenderedImage {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

fn validate_destination(destination: &Path) -> Result<PathBuf, ImageError> {
    if !destination
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        || destination.file_stem().is_none()
    {
        return Err(ImageError::Failed("Choose a PNG image filename".into()));
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let parent = parent.canonicalize()?;
    let snapshots = crate::default_root();
    let snapshots = snapshots.canonicalize().unwrap_or(snapshots);
    if parent.starts_with(snapshots) {
        return Err(ImageError::Failed("Choose an image folder outside the rotating scene-snapshot store so saved PNGs cannot be pruned".into()));
    }
    Ok(parent)
}

/// Fully decode one still PNG, enforce 3840×2160, then publish an atomic new hard link beside it.
/// Existing destination names are preserved and a numerical suffix is selected. The staged file
/// is caller-owned and is never deleted here. Unsupported cross-filesystem/hard-link publication
/// fails explicitly rather than exposing a partially written destination or replacing an image.
pub fn publish_png(
    staged: &Path,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<RenderedImage, ImageError> {
    check_cancel(cancel)?;
    let parent = validate_destination(destination)?;
    decode(staged)?;
    File::open(staged)?.sync_all()?;
    check_cancel(cancel)?;
    let stem = destination
        .file_stem()
        .ok_or_else(|| ImageError::Failed("Choose an image filename".into()))?
        .to_string_lossy();
    for suffix in 0..10_000 {
        check_cancel(cancel)?;
        let name = if suffix == 0 {
            destination.file_name().unwrap().to_os_string()
        } else {
            format!("{stem}-{}.png", suffix + 1).into()
        };
        let path = parent.join(name);
        match fs::hard_link(staged, &path) {
            Ok(()) => {
                return Ok(RenderedImage {
                    path,
                    width: SIZE.0,
                    height: SIZE.1,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(ImageError::Failed(format!(
                    "Cannot publish PNG atomically without overwrite: {error}; choose a writable folder on the staging filesystem"
                )));
            }
        }
    }
    Err(ImageError::Failed(
        "Too many images share this filename; choose another name".into(),
    ))
}
fn decode(path: &Path) -> Result<(), ImageError> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 256 * 1024 * 1024 {
        return Err(ImageError::Failed(
            "Rendered output is not a bounded PNG file".into(),
        ));
    }
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_limits(png::Limits {
        bytes: 128 * 1024 * 1024,
    });
    let mut reader = decoder.read_info().map_err(png_error)?;
    let info = reader.info();
    if (info.width, info.height) != SIZE || info.animation_control.is_some() {
        return Err(ImageError::Failed(format!(
            "Expected one 3840×2160 PNG, received {}×{}{}",
            info.width,
            info.height,
            if info.animation_control.is_some() {
                " animated PNG"
            } else {
                ""
            }
        )));
    }
    let size = reader
        .output_buffer_size()
        .filter(|size| *size <= 128 * 1024 * 1024)
        .ok_or_else(|| {
            ImageError::Failed("PNG decoded image exceeds the capture memory limit".into())
        })?;
    let mut pixels = vec![0; size];
    reader.next_frame(&mut pixels).map_err(png_error)?;
    reader.finish().map_err(png_error)?;
    Ok(())
}
fn png_error(error: png::DecodingError) -> ImageError {
    ImageError::Failed(format!(
        "Rendered output is not a complete decodable PNG: {error}"
    ))
}

#[cfg(test)]
mod tests;
