use super::assets::{extract_asset_field, resolve_asset_field};
use super::manifest::AssetKind;
use super::manifest::*;
use super::{invalid, validate_profile, validate_zip_entry};
use crate::FixtureProfile;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

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
