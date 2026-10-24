//! Scope and content checks precede Blender. No asset may escape this job's private package.
use super::*;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

pub(in crate::ultimate) fn validate(
    directory: &Path,
    expected: u64,
    revision: u64,
) -> Result<(), UltimateError> {
    let path = checked_asset(directory, IMAGE_MANIFEST_FILE)?;
    if path.metadata()?.len() > 16 * 1024 * 1024 {
        return Err(failed("Image manifest exceeds 16 MiB"));
    }
    let bytes = std::fs::read(path)?;
    if hash(&bytes) != expected {
        return Err(failed("Frozen image manifest changed after quality choice"));
    }
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|error| failed(&error.to_string()))?;
    if document["format"] != 1 || document["scene_revision"] != revision {
        return Err(failed(
            "Frozen image manifest identity does not match the chosen scene",
        ));
    }
    let geometry = checked_asset(directory, text(&document["geometry_file"])?)?;
    if hash_file(&geometry)?
        != document["geometry_hash"]
            .as_u64()
            .ok_or_else(|| failed("Missing geometry fingerprint"))?
    {
        return Err(failed("Frozen image geometry changed after quality choice"));
    }
    for asset in document["media"]["assets"]
        .as_array()
        .ok_or_else(|| failed("Invalid frozen media asset list"))?
    {
        validate_rgba(directory, asset)?;
    }
    for light in document["optics"]["lights"]
        .as_array()
        .ok_or_else(|| failed("Invalid frozen optical light list"))?
    {
        let _ = checked_asset(directory, text(&light["mask_file"])?)?;
    }
    Ok(())
}

pub(super) fn checked_asset(directory: &Path, name: &str) -> Result<PathBuf, UltimateError> {
    if name.is_empty()
        || name.starts_with('.')
        || name.contains(['/', '\\'])
        || Path::new(name).components().count() != 1
    {
        return Err(failed(
            "Frozen image assets require package-local filenames",
        ));
    }
    let root = directory.canonicalize()?;
    let path = root.join(name).canonicalize()?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(failed("Frozen image asset escaped its package"));
    }
    Ok(path)
}

fn validate_rgba(directory: &Path, asset: &Value) -> Result<(), UltimateError> {
    let path = checked_asset(directory, text(&asset["file"])?)?;
    if path.metadata()?.len() > 16 * 1024 * 1024 {
        return Err(failed("Frozen media PNG exceeds its size budget"));
    }
    let mut decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    decoder.set_limits(png::Limits {
        bytes: 16 * 1024 * 1024,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|error| failed(&error.to_string()))?;
    let info = reader.info();
    if asset["width"].as_u64() != Some(u64::from(info.width))
        || asset["height"].as_u64() != Some(u64::from(info.height))
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
        || info.animation_control.is_some()
    {
        return Err(failed(
            "Frozen media PNG format/dimensions do not match the manifest",
        ));
    }
    let size = reader
        .output_buffer_size()
        .filter(|size| *size <= 16 * 1024 * 1024)
        .ok_or_else(|| failed("Frozen media PNG decoded size exceeds budget"))?;
    let mut pixels = vec![0; size];
    reader
        .next_frame(&mut pixels)
        .map_err(|error| failed(&error.to_string()))?;
    reader
        .finish()
        .map_err(|error| failed(&error.to_string()))?;
    if asset["rgba_hash"].as_u64() != Some(hash(&pixels)) {
        return Err(failed("Frozen media pixels changed after quality choice"));
    }
    Ok(())
}
fn text(value: &Value) -> Result<&str, UltimateError> {
    value
        .as_str()
        .ok_or_else(|| failed("Invalid image asset filename"))
}
fn failed(detail: &str) -> UltimateError {
    UltimateError::Failed(detail.into())
}
fn hash_file(path: &Path) -> Result<u64, UltimateError> {
    let mut file = File::open(path)?;
    let mut digest = DefaultHasher::new();
    let mut bytes = [0; 65536];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        digest.write(&bytes[..count]);
    }
    Ok(digest.finish())
}
