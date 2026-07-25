use super::manifest::{
    AssetKind, FixturePackageError, MAX_ICON_DIMENSION, MAX_PHOTOGRAPH_DIMENSION, PackageAsset,
};
use super::{invalid, validate_glb};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Component, Path};

pub(super) fn resolve_asset_field(
    value: &mut Option<String>,
    kind: AssetKind,
    files: &mut HashMap<String, Vec<u8>>,
) -> Result<(), FixturePackageError> {
    let Some(path) = value.as_deref() else {
        return Ok(());
    };
    validate_asset_path(path)?;
    let bytes = files
        .remove(path)
        .ok_or_else(|| invalid(format!("{} asset {path} is missing", kind.label())))?;
    let mime = validate_asset(kind, path, &bytes)?;
    *value = Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)));
    Ok(())
}

pub(super) fn extract_asset_field(
    value: &mut Option<String>,
    kind: AssetKind,
    stem: &str,
    assets: &mut Vec<PackageAsset>,
) -> Result<(), FixturePackageError> {
    let Some(data_url) = value.as_deref() else {
        return Ok(());
    };
    let (declared_mime, bytes) = decode_data_url(data_url, kind)?;
    if bytes.len() > kind.max_bytes() {
        return Err(invalid(format!("{} exceeds its size limit", kind.label())));
    }
    let extension = match kind {
        AssetKind::Photograph | AssetKind::Icon => {
            let format = sniff_image(&bytes)?;
            let mime = image_mime(format);
            if declared_mime != mime {
                return Err(invalid(format!(
                    "{} declares {declared_mime} but contains {mime}",
                    kind.label()
                )));
            }
            validate_image_dimensions(kind, &bytes, format)?;
            image_extension(format)
        }
        AssetKind::Model => {
            if !matches!(
                declared_mime,
                "model/gltf-binary" | "application/octet-stream"
            ) {
                return Err(invalid(format!(
                    "3D model has unsupported media type {declared_mime}"
                )));
            }
            validate_glb(&bytes)?;
            "glb"
        }
    };
    let path = format!("{stem}.{extension}");
    *value = Some(path.clone());
    assets.push(PackageAsset { path, bytes });
    Ok(())
}

fn decode_data_url(value: &str, kind: AssetKind) -> Result<(&str, Vec<u8>), FixturePackageError> {
    let payload = value.strip_prefix("data:").ok_or_else(|| {
        invalid(format!(
            "{} must be a self-contained data URL",
            kind.label()
        ))
    })?;
    let (metadata, encoded) = payload
        .split_once(',')
        .ok_or_else(|| invalid(format!("{} data URL is malformed", kind.label())))?;
    let mime = metadata
        .strip_suffix(";base64")
        .ok_or_else(|| invalid(format!("{} data URL must use base64", kind.label())))?;
    if mime.is_empty() || mime.contains(';') {
        return Err(invalid(format!(
            "{} data URL media type is invalid",
            kind.label()
        )));
    }
    let maximum_encoded = kind.max_bytes().saturating_add(2) / 3 * 4;
    if encoded.len() > maximum_encoded {
        return Err(invalid(format!("{} exceeds its size limit", kind.label())));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| invalid(format!("{} data URL is invalid: {error}", kind.label())))?;
    Ok((mime, bytes))
}

fn validate_asset_path(path: &str) -> Result<(), FixturePackageError> {
    if path.contains('\0') || path.contains('\\') || path.starts_with('/') {
        return Err(invalid(format!("asset path {path} is unsafe")));
    }
    let path_value = Path::new(path);
    if !path.starts_with("assets/")
        || path_value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(format!("asset path {path} is unsafe")));
    }
    Ok(())
}

fn validate_asset(
    kind: AssetKind,
    path: &str,
    bytes: &[u8],
) -> Result<&'static str, FixturePackageError> {
    if bytes.is_empty() || bytes.len() > kind.max_bytes() {
        return Err(invalid(format!("{} exceeds its size limit", kind.label())));
    }
    match kind {
        AssetKind::Photograph | AssetKind::Icon => {
            let format = sniff_image(bytes)?;
            let expected = image_extension(format);
            let actual = Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let extension_matches =
                actual == expected || (format == ImageFormat::Jpeg && actual == "jpeg");
            if !extension_matches {
                return Err(invalid(format!(
                    "{} path extension does not match its content",
                    kind.label()
                )));
            }
            validate_image_dimensions(kind, bytes, format)?;
            Ok(image_mime(format))
        }
        AssetKind::Model => {
            if !path.to_ascii_lowercase().ends_with(".glb") {
                return Err(invalid("3D model must use the .glb extension"));
            }
            validate_glb(bytes)?;
            Ok("model/gltf-binary")
        }
    }
}

fn sniff_image(bytes: &[u8]) -> Result<ImageFormat, FixturePackageError> {
    match image::guess_format(bytes) {
        Ok(format @ (ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)) => Ok(format),
        Ok(_) | Err(_) => Err(invalid("fixture images must be PNG, JPEG, or WebP")),
    }
}

fn validate_image_dimensions(
    kind: AssetKind,
    bytes: &[u8],
    format: ImageFormat,
) -> Result<(), FixturePackageError> {
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|error| invalid(format!("{} cannot be decoded: {error}", kind.label())))?;
    let limit = match kind {
        AssetKind::Photograph => MAX_PHOTOGRAPH_DIMENSION,
        AssetKind::Icon => MAX_ICON_DIMENSION,
        AssetKind::Model => unreachable!(),
    };
    if width == 0 || height == 0 || width > limit || height > limit {
        return Err(invalid(format!(
            "{} dimensions must be 1-{limit} pixels",
            kind.label()
        )));
    }
    Ok(())
}

fn image_mime(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::WebP => "image/webp",
        _ => unreachable!(),
    }
}

fn image_extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::WebP => "webp",
        _ => unreachable!(),
    }
}
