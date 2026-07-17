//! Portable, self-contained ToskLight fixture packages.

use crate::FixtureProfile;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Write},
    path::{Component, Path},
};
use thiserror::Error;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const FIXTURE_PACKAGE_EXTENSION: &str = "toskfixture";
pub const FIXTURE_PACKAGE_MIME_TYPE: &str = "application/vnd.tosklight.fixture+zip";
pub const FIXTURE_PACKAGE_FORMAT: &str = "tosklight.fixture";
pub const FIXTURE_PACKAGE_FORMAT_VERSION: u16 = 1;
pub const FIXTURE_PACKAGE_MANIFEST_PATH: &str = "fixture.json";
pub const MAX_FIXTURE_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_FIXTURE_PACKAGE_EXPANDED_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_FIXTURE_PACKAGE_ENTRIES: usize = 32;
pub const MAX_FIXTURE_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_FIXTURE_PHOTOGRAPH_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_FIXTURE_ICON_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_FIXTURE_MODEL_BYTES: usize = 64 * 1024 * 1024;

const MAX_PHOTOGRAPH_DIMENSION: u32 = 8_192;
const MAX_ICON_DIMENSION: u32 = 2_048;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixturePackageManifest {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub format: String,
    pub format_version: u16,
    pub profile: FixtureProfile,
}

impl FixturePackageManifest {
    pub fn new(profile: FixtureProfile) -> Self {
        Self {
            schema: Some("https://tosklight.app/schemas/fixture-package-v1.json".into()),
            format: FIXTURE_PACKAGE_FORMAT.into(),
            format_version: FIXTURE_PACKAGE_FORMAT_VERSION,
            profile,
        }
    }
}

#[derive(Debug, Error)]
pub enum FixturePackageError {
    #[error("invalid fixture package: {0}")]
    Invalid(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Copy)]
enum AssetKind {
    Photograph,
    Icon,
    Model,
}

impl AssetKind {
    fn label(self) -> &'static str {
        match self {
            Self::Photograph => "photograph",
            Self::Icon => "stage icon",
            Self::Model => "3D model",
        }
    }

    fn max_bytes(self) -> usize {
        match self {
            Self::Photograph => MAX_FIXTURE_PHOTOGRAPH_BYTES,
            Self::Icon => MAX_FIXTURE_ICON_BYTES,
            Self::Model => MAX_FIXTURE_MODEL_BYTES,
        }
    }
}

struct PackageAsset {
    path: String,
    bytes: Vec<u8>,
}

/// Reads a `.toskfixture` archive and returns the normalized schema-v2 profile.
/// Relative asset paths are replaced by self-contained data URLs used by the current runtime.
pub fn read_fixture_package(bytes: &[u8]) -> Result<FixtureProfile, FixturePackageError> {
    if bytes.len() > MAX_FIXTURE_PACKAGE_BYTES {
        return Err(invalid("archive exceeds 64 MiB"));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    if zip.len() > MAX_FIXTURE_PACKAGE_ENTRIES {
        return Err(invalid("archive contains more than 32 entries"));
    }
    let mut files = HashMap::<String, Vec<u8>>::new();
    let mut names = HashSet::<String>::new();
    let mut expanded = 0_u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        validate_zip_entry(&entry)?;
        if entry.is_dir() {
            continue;
        }
        let path = entry.name().to_owned();
        let folded = path.to_ascii_lowercase();
        if !names.insert(folded) {
            return Err(invalid(format!("archive contains duplicate path {path}")));
        }
        expanded = expanded.saturating_add(entry.size());
        if expanded > MAX_FIXTURE_PACKAGE_EXPANDED_BYTES {
            return Err(invalid("expanded archive exceeds 128 MiB"));
        }
        if path == FIXTURE_PACKAGE_MANIFEST_PATH && entry.size() > MAX_FIXTURE_MANIFEST_BYTES as u64
        {
            return Err(invalid("fixture.json exceeds 64 MiB"));
        }
        if entry.size() > MAX_FIXTURE_MODEL_BYTES as u64 {
            return Err(invalid(format!("archive entry {path} exceeds 64 MiB")));
        }
        let mut data = Vec::with_capacity(entry.size().min(usize::MAX as u64) as usize);
        entry.read_to_end(&mut data)?;
        files.insert(path, data);
    }

    let manifest_bytes = files
        .remove(FIXTURE_PACKAGE_MANIFEST_PATH)
        .ok_or_else(|| invalid("fixture.json is missing"))?;
    let mut manifest: FixturePackageManifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.format != FIXTURE_PACKAGE_FORMAT {
        return Err(invalid(format!(
            "unsupported format {}; expected {FIXTURE_PACKAGE_FORMAT}",
            manifest.format
        )));
    }
    if manifest.format_version != FIXTURE_PACKAGE_FORMAT_VERSION {
        return Err(invalid(format!(
            "unsupported package format version {}",
            manifest.format_version
        )));
    }
    if manifest.profile.reserved_source.is_some() {
        return Err(invalid(
            "transferred fixture profiles cannot claim a reserved source",
        ));
    }

    resolve_asset_field(
        &mut manifest.profile.photograph_asset,
        AssetKind::Photograph,
        &mut files,
    )?;
    resolve_asset_field(
        &mut manifest.profile.stage_icon_asset,
        AssetKind::Icon,
        &mut files,
    )?;
    resolve_asset_field(
        &mut manifest.profile.model_asset,
        AssetKind::Model,
        &mut files,
    )?;
    if let Some(path) = files.keys().next() {
        return Err(invalid(format!("unreferenced archive entry {path}")));
    }
    validate_profile(&manifest.profile)?;
    Ok(manifest.profile)
}

/// Writes a complete fixture profile as a `.toskfixture` archive.
/// Data URL assets are extracted into canonical files referenced by `fixture.json`.
pub fn write_fixture_package(profile: &FixtureProfile) -> Result<Vec<u8>, FixturePackageError> {
    validate_profile(profile)?;
    let mut portable = profile.clone();
    // A transferable package never carries ownership of an application catalog.
    portable.reserved_source = None;
    let mut assets = Vec::new();
    extract_asset_field(
        &mut portable.photograph_asset,
        AssetKind::Photograph,
        "assets/photograph",
        &mut assets,
    )?;
    extract_asset_field(
        &mut portable.stage_icon_asset,
        AssetKind::Icon,
        "assets/icon",
        &mut assets,
    )?;
    extract_asset_field(
        &mut portable.model_asset,
        AssetKind::Model,
        "assets/model",
        &mut assets,
    )?;
    let manifest = serde_json::to_vec_pretty(&FixturePackageManifest::new(portable))?;
    if manifest.len() > MAX_FIXTURE_MANIFEST_BYTES {
        return Err(invalid("fixture.json exceeds 64 MiB"));
    }

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(FIXTURE_PACKAGE_MANIFEST_PATH, options)?;
    zip.write_all(&manifest)?;
    for asset in assets {
        zip.start_file(asset.path, options)?;
        zip.write_all(&asset.bytes)?;
    }
    let bytes = zip.finish()?.into_inner();
    if bytes.len() > MAX_FIXTURE_PACKAGE_BYTES {
        return Err(invalid("archive exceeds 64 MiB"));
    }
    Ok(bytes)
}

pub fn read_package(bytes: &[u8]) -> Result<FixtureProfile, FixturePackageError> {
    read_fixture_package(bytes)
}

pub fn write_package(profile: &FixtureProfile) -> Result<Vec<u8>, FixturePackageError> {
    write_fixture_package(profile)
}

fn validate_zip_entry(
    entry: &zip::read::ZipFile<'_, Cursor<&[u8]>>,
) -> Result<(), FixturePackageError> {
    let name = entry.name();
    if name.is_empty()
        || name.contains('\0')
        || name.contains('\\')
        || name.contains("//")
        || name.starts_with('/')
    {
        return Err(invalid(format!("archive contains unsafe path {name}")));
    }
    let Some(enclosed) = entry.enclosed_name() else {
        return Err(invalid(format!("archive contains unsafe path {name}")));
    };
    if enclosed
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(format!("archive contains unsafe path {name}")));
    }
    if let Some(mode) = entry.unix_mode()
        && mode & 0o170000 == 0o120000
    {
        return Err(invalid(format!("archive entry {name} is a symbolic link")));
    }
    if entry.encrypted() {
        return Err(invalid(format!("archive entry {name} is encrypted")));
    }
    if !matches!(
        entry.compression(),
        CompressionMethod::Stored | CompressionMethod::Deflated
    ) {
        return Err(invalid(format!(
            "archive entry {name} uses unsupported compression"
        )));
    }
    Ok(())
}

fn resolve_asset_field(
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

fn extract_asset_field(
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

fn validate_glb(bytes: &[u8]) -> Result<(), FixturePackageError> {
    if bytes.len() < 20 || &bytes[..4] != b"glTF" {
        return Err(invalid("3D model is not a GLB file"));
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes"));
    let declared = u32::from_le_bytes(bytes[8..12].try_into().expect("four bytes")) as usize;
    if version != 2 || declared != bytes.len() {
        return Err(invalid("3D model must be a complete GLB 2.0 file"));
    }
    let mut cursor = 12_usize;
    let mut json = None;
    while cursor < bytes.len() {
        if bytes.len() - cursor < 8 {
            return Err(invalid("3D model contains a truncated GLB chunk"));
        }
        let length =
            u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().expect("four bytes")) as usize;
        let kind = u32::from_le_bytes(
            bytes[cursor + 4..cursor + 8]
                .try_into()
                .expect("four bytes"),
        );
        cursor += 8;
        let end = cursor
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| invalid("3D model contains an invalid GLB chunk length"))?;
        if kind == 0x4e4f_534a && json.is_none() {
            json = Some(&bytes[cursor..end]);
        }
        cursor = end;
    }
    let json = json.ok_or_else(|| invalid("3D model has no GLB JSON chunk"))?;
    let json = json.strip_suffix(&[0]).unwrap_or(json);
    let document: serde_json::Value = serde_json::from_slice(json)?;
    for collection in ["buffers", "images"] {
        if document
            .get(collection)
            .and_then(serde_json::Value::as_array)
            .is_some_and(|entries| entries.iter().any(|entry| entry.get("uri").is_some()))
        {
            return Err(invalid(format!(
                "3D model contains an external {collection} URI; GLB assets must be self-contained"
            )));
        }
    }
    Ok(())
}

fn validate_profile(profile: &FixtureProfile) -> Result<(), FixturePackageError> {
    profile
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    // FixtureProfile::validate covers every mode and channel. Calling resolved_definition for
    // every mode here would clone the complete profile snapshot into every temporary definition,
    // becoming quadratic for exhaustive Generic profiles with thousands of ordered modes.
    Ok(())
}

fn invalid(message: impl Into<String>) -> FixturePackageError {
    FixturePackageError::Invalid(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChannelResolution, FixtureSplit, ModelUnits, PatchPolicy};
    use std::fs;

    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

    fn profile() -> FixtureProfile {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "Portable fixture".into();
        profile.short_name = "Portable".into();
        profile
    }

    fn shipped_profile(filename: &str) -> FixtureProfile {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixture-library")
            .join(filename);
        read_fixture_package(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn requested_generic_and_venue_packages_have_exact_portable_contracts() {
        let blinder = shipped_profile("generic--blinder.toskfixture");
        assert_eq!(
            blinder
                .modes
                .iter()
                .map(|mode| mode.name.as_str())
                .collect::<Vec<_>>(),
            [
                "One channel, two blind",
                "Two channel, two blind",
                "One channel, four blind",
                "Two channel, four blind",
                "One channel, eight blind",
                "Two channel, eight blind",
                "Four channel, eight blind",
            ]
        );
        for mode in &blinder.modes {
            assert!(mode.heads.iter().all(|head| !head.master_shared));
            assert_eq!(mode.heads.len(), mode.channels.len());
            assert_eq!(mode.splits[0].footprint as usize, mode.heads.len());
            assert!(mode.channels.iter().all(|channel| {
                channel.attribute.is_intensity()
                    && channel.resolution == ChannelResolution::U8
                    && channel.highlight_raw == 255
            }));
        }

        let fogger = shipped_profile("generic--fogger.toskfixture");
        assert_eq!(
            fogger
                .modes
                .iter()
                .map(|mode| mode.name.as_str())
                .collect::<Vec<_>>(),
            ["Fan, Fog", "Fog, Fan", "Fog 8-bit"]
        );
        let hazer = shipped_profile("generic--hazer.toskfixture");
        assert_eq!(
            hazer
                .modes
                .iter()
                .map(|mode| mode.name.as_str())
                .collect::<Vec<_>>(),
            ["Fan, Fog", "Fog, Fan"]
        );

        let venue = [
            ("venue--stage-element-1-1-m.toskfixture", 10),
            ("venue--stage-element-2-1-m.toskfixture", 10),
            ("venue--stage-element-1-0-5-m.toskfixture", 10),
            ("venue--stage-stairs.toskfixture", 10),
            ("venue--four-point-truss.toskfixture", 5),
            ("venue--three-point-truss.toskfixture", 5),
            ("venue--two-point-truss.toskfixture", 5),
            ("venue--one-point-truss-pipe.toskfixture", 6),
            ("venue--curtain-1-m.toskfixture", 10),
            ("venue--curtain-2-m.toskfixture", 10),
            ("venue--curtain-3-m.toskfixture", 10),
            ("venue--curtain-5-m.toskfixture", 10),
            ("venue--curtain-6-m.toskfixture", 10),
        ];
        for (filename, mode_count) in venue {
            let profile = shipped_profile(filename);
            assert_eq!(profile.manufacturer, "Venue");
            assert_eq!(profile.patch_policy, PatchPolicy::VisualOnly);
            assert_eq!(profile.model_units, ModelUnits::Metres);
            assert_eq!(profile.modes.len(), mode_count);
            assert!(
                profile
                    .photograph_asset
                    .as_deref()
                    .is_some_and(|asset| asset.starts_with("data:image/png;base64,"))
            );
            assert!(
                profile
                    .stage_icon_asset
                    .as_deref()
                    .is_some_and(|asset| asset.starts_with("data:image/png;base64,"))
            );
            assert!(
                profile
                    .model_asset
                    .as_deref()
                    .is_some_and(|asset| asset.starts_with("data:model/gltf-binary;base64,"))
            );
            assert!(profile.modes.iter().all(|mode| mode.splits
                == [FixtureSplit {
                    number: 1,
                    footprint: 0
                }]
                && mode.channels.is_empty()));
        }
    }

    fn minimal_glb(external_uri: bool) -> Vec<u8> {
        let json = if external_uri {
            br#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":0,"uri":"outside.bin"}]}"#
                .to_vec()
        } else {
            br#"{"asset":{"version":"2.0"}}"#.to_vec()
        };
        let padded = (json.len() + 3) & !3;
        let total = 12 + 8 + padded;
        let mut result = Vec::with_capacity(total);
        result.extend_from_slice(b"glTF");
        result.extend_from_slice(&2_u32.to_le_bytes());
        result.extend_from_slice(&(total as u32).to_le_bytes());
        result.extend_from_slice(&(padded as u32).to_le_bytes());
        result.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
        result.extend_from_slice(&json);
        result.resize(total, b' ');
        result
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn round_trips_profile_and_embedded_assets() {
        let mut profile = profile();
        profile.photograph_asset = Some(format!("data:image/png;base64,{PNG_1X1}"));
        profile.stage_icon_asset = Some(format!("data:image/png;base64,{PNG_1X1}"));
        profile.model_asset = Some(format!(
            "data:model/gltf-binary;base64,{}",
            STANDARD.encode(minimal_glb(false))
        ));

        let bytes = write_fixture_package(&profile).unwrap();
        let restored = read_fixture_package(&bytes).unwrap();
        assert_eq!(restored.id, profile.id);
        assert_eq!(restored.modes[0].id, profile.modes[0].id);
        assert_eq!(restored.photograph_asset, profile.photograph_asset);
        assert_eq!(restored.stage_icon_asset, profile.stage_icon_asset);
        assert_eq!(restored.model_asset, profile.model_asset);
        assert_eq!(restored.reserved_source, None);

        let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names = (0..zip.len())
            .map(|index| zip.by_index(index).unwrap().name().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "fixture.json",
                "assets/photograph.png",
                "assets/icon.png",
                "assets/model.glb"
            ]
        );
    }

    #[test]
    fn rejects_unsafe_duplicate_and_unreferenced_paths() {
        let manifest = serde_json::to_vec(&FixturePackageManifest::new(profile())).unwrap();
        assert!(read_fixture_package(&archive(&[("../fixture.json", &manifest)])).is_err());
        assert!(
            read_fixture_package(&archive(&[
                ("fixture.json", &manifest),
                ("FIXTURE.JSON", &manifest),
            ]))
            .is_err()
        );
        assert!(
            read_fixture_package(&archive(&[
                ("fixture.json", &manifest),
                ("assets/unused.png", &[1, 2, 3]),
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_missing_mistyped_and_non_self_contained_assets() {
        let mut missing = profile();
        missing.stage_icon_asset = Some("assets/icon.png".into());
        let manifest = serde_json::to_vec(&FixturePackageManifest::new(missing)).unwrap();
        assert!(read_fixture_package(&archive(&[("fixture.json", &manifest)])).is_err());

        let mut mistyped = profile();
        mistyped.stage_icon_asset = Some("assets/icon.jpg".into());
        let manifest = serde_json::to_vec(&FixturePackageManifest::new(mistyped)).unwrap();
        let png = STANDARD.decode(PNG_1X1).unwrap();
        assert!(
            read_fixture_package(&archive(&[
                ("fixture.json", &manifest),
                ("assets/icon.jpg", &png),
            ]))
            .is_err()
        );

        let mut external = profile();
        external.model_asset = Some("assets/model.glb".into());
        let manifest = serde_json::to_vec(&FixturePackageManifest::new(external)).unwrap();
        let glb = minimal_glb(true);
        assert!(
            read_fixture_package(&archive(&[
                ("fixture.json", &manifest),
                ("assets/model.glb", &glb),
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_unknown_manifest_fields_and_reserved_sources() {
        let json = serde_json::json!({
            "format": FIXTURE_PACKAGE_FORMAT,
            "format_version": FIXTURE_PACKAGE_FORMAT_VERSION,
            "profile": profile(),
            "typo": true
        });
        let manifest = serde_json::to_vec(&json).unwrap();
        assert!(read_fixture_package(&archive(&[("fixture.json", &manifest)])).is_err());

        let mut reserved = profile();
        reserved.reserved_source = Some("builtin:anything".into());
        let manifest = serde_json::to_vec(&FixturePackageManifest::new(reserved)).unwrap();
        assert!(read_fixture_package(&archive(&[("fixture.json", &manifest)])).is_err());
    }
}
