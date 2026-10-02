//! Installed Color calibration is passive observational metadata of one physical instance.
//!
//! Unlike a Direct recipe it never adds a profile dependency and never blocks an import. A
//! calibration is rebased only when profile Duplicate copied the exact original immutable profile
//! and every path was current against that original. Stale, foreign, absent-source and Keep
//! observations are retained byte-for-byte, so the destination never receives an invented proof.
use super::locations::array_at;
use super::native_color::{PinnedProfileMap, merge_native_identity, rebase_pinned_native_identity};
use crate::selective_import::ImportObjectDescriptor;
use crate::selective_import::model::{ImportInstalledColorReference, ImportProfileReference};
use light_core::NativeColorIdentity;
use light_fixture::{FixtureProfile, InstalledColorCalibration, InstalledColorCalibrationStatus};
use serde_json::Value;
use uuid::Uuid;

/// Record root and independent multipatch calibrations that might share the fixture's selected
/// profile. Unreadable data stays untouched here; structural validation belongs to the patch
/// compiler, which already accepts stale observations.
pub(super) fn add_installed_color_references(
    body: &Value,
    profile: &ImportProfileReference,
    descriptor: &mut ImportObjectDescriptor,
) {
    let Some(mode_id) = ["/mode_id", "/definition/mode_id"]
        .into_iter()
        .find_map(|pointer| body.pointer(pointer).and_then(Value::as_str))
        .and_then(|value| Uuid::parse_str(value).ok())
    else {
        return;
    };
    let mut pointers = vec!["/color_calibration".to_owned()];
    for index in 0..array_at(body, "/multipatch").map_or(0, Vec::len) {
        pointers.push(format!("/multipatch/{index}/color_calibration"));
    }
    for pointer in pointers {
        let Some(sources) = body
            .pointer(&pointer)
            .filter(|value| !value.is_null())
            .and_then(source_identities)
        else {
            continue;
        };
        descriptor
            .installed_color_references
            .push(ImportInstalledColorReference {
                key: profile.key,
                mode_id,
                pointer,
                sources,
            });
    }
}

fn source_identities(calibration: &Value) -> Option<Vec<NativeColorIdentity>> {
    calibration
        .get("paths")?
        .as_array()?
        .iter()
        .map(|path| serde_json::from_value(path.get("source_identity")?.clone()).ok())
        .collect()
}

/// Rebase complete identities only for an actual profile Duplicate. Copy and SkipIdentical keep
/// the same immutable profile; Keep never certifies observations against a different profile.
pub(super) fn rebase_installed_color_calibrations(
    body: &mut Value,
    references: &[ImportInstalledColorReference],
    profiles: &PinnedProfileMap,
) -> Result<(), String> {
    for reference in references {
        let Some((original, destination)) = profiles.get(&reference.key) else {
            continue;
        };
        if destination.id().profile_id() == reference.key.profile_id {
            continue;
        }
        let current = body.pointer(&reference.pointer).ok_or_else(|| {
            format!(
                "installed Color calibration {} no longer exists",
                reference.pointer
            )
        })?;
        if source_identities(current).as_ref() != Some(&reference.sources) {
            return Err(format!(
                "installed Color calibration {} changed before rewrite",
                reference.pointer
            ));
        }
        let Ok(calibration) = serde_json::from_value::<InstalledColorCalibration>(current.clone())
        else {
            continue;
        };
        let original: FixtureProfile = serde_json::from_value(original.profile().clone())
            .map_err(|error| format!("invalid pinned installed Color source profile: {error}"))?;
        // A stale, foreign or absent-source proof was never certified against this profile.
        if calibration.status(&original, reference.mode_id)
            != InstalledColorCalibrationStatus::Current
        {
            continue;
        }
        let destination: FixtureProfile = serde_json::from_value(destination.profile().clone())
            .map_err(|error| format!("invalid destination installed Color profile: {error}"))?;
        let Ok(rebased) = calibration
            .paths
            .iter()
            .map(|path| {
                rebase_pinned_native_identity(&original, &destination, &path.source_identity)
            })
            .collect::<Result<Vec<_>, _>>()
        else {
            // Not a proven copy of the original model: keep the original stale metadata.
            continue;
        };
        let mut candidate = calibration.clone();
        for (path, identity) in candidate.paths.iter_mut().zip(&rebased) {
            path.source_identity = identity.clone();
        }
        candidate
            .validate_for_profile(&destination, reference.mode_id)
            .map_err(|error| {
                format!("rebased installed Color calibration is not current: {error}")
            })?;
        // Only identity fields change. Emitters, measurements, provenance and future fields stay.
        let paths = body
            .pointer_mut(&format!("{}/paths", reference.pointer))
            .and_then(Value::as_array_mut)
            .ok_or("installed Color calibration paths are not an array")?;
        for (path, identity) in paths.iter_mut().zip(&rebased) {
            let source = path
                .get_mut("source_identity")
                .ok_or("installed Color calibration path has no source identity")?;
            merge_native_identity(source, identity)?;
        }
    }
    Ok(())
}
