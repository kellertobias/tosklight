use crate::selective_import::{ImportNativeColorReference, ImportProfileKey};
use light_core::{FixtureId, NativeColorIdentity};
use light_fixture::FixtureProfile;
use light_show::FixtureProfileRevision;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) type PinnedProfileMap =
    BTreeMap<ImportProfileKey, (FixtureProfileRevision, FixtureProfileRevision)>;

/// Verify the original pinned source before rebinding a duplicate. Same layout alone is not
/// sufficient: different calibration/emitter data would change the retained portable meaning.
pub(super) fn rewrite_native_color_sources(
    body: &mut Value,
    references: &[ImportNativeColorReference],
    profiles: &PinnedProfileMap,
) -> Result<(), String> {
    for reference in references {
        let expected = &reference.source;
        let key = ImportProfileKey {
            profile_id: FixtureId(expected.profile_id),
            revision: u64::from(expected.profile_revision),
        };
        let (original, destination) = profiles
            .get(&key)
            .ok_or_else(|| "pinned native Color profile is unavailable".to_string())?;
        let original: FixtureProfile = serde_json::from_value(original.profile().clone())
            .map_err(|error| format!("invalid pinned native Color source profile: {error}"))?;
        let destination: FixtureProfile = serde_json::from_value(destination.profile().clone())
            .map_err(|error| format!("invalid destination native Color profile: {error}"))?;
        let identity = rebase_pinned_native_identity(&original, &destination, expected)?;
        let current = body
            .pointer_mut(&reference.pointer)
            .ok_or_else(|| format!("native source {} no longer exists", reference.pointer))?;
        let actual = serde_json::from_value::<light_core::NativeColorIdentity>(current.clone())
            .map_err(|error| error.to_string())?;
        if actual != *expected {
            return Err(format!(
                "native source {} changed before rewrite",
                reference.pointer
            ));
        }
        merge_native_identity(current, &identity)?;
    }
    Ok(())
}

/// Exact original-source proof plus a destination that only renames the immutable profile.
/// Shared by strict Direct recipes and passive installed Color calibration.
pub(super) fn rebase_pinned_native_identity(
    original: &FixtureProfile,
    destination: &FixtureProfile,
    expected: &NativeColorIdentity,
) -> Result<NativeColorIdentity, String> {
    let source_identity = original
        .native_color_identity(expected.mode_id, expected.head_id)
        .map_err(|error| error.to_string())?;
    if source_identity != *expected {
        return Err(
            "pinned native Color source does not match its original immutable profile".into(),
        );
    }
    let source_body = serde_json::to_value(original).map_err(|error| error.to_string())?;
    let mut destination_body =
        serde_json::to_value(destination).map_err(|error| error.to_string())?;
    destination_body["id"] = source_body["id"].clone();
    if destination_body != source_body {
        return Err("destination profile changes the pinned native Color model; duplicate the source profile to preserve its recipe".into());
    }
    destination
        .native_color_identity(expected.mode_id, expected.head_id)
        .map_err(|error| error.to_string())
}

/// Preserve any future raw extension fields just as ordinary import references do.
pub(super) fn merge_native_identity(
    current: &mut Value,
    identity: &NativeColorIdentity,
) -> Result<(), String> {
    let fields = serde_json::to_value(identity).map_err(|error| error.to_string())?;
    current
        .as_object_mut()
        .ok_or("native source is not an object")?
        .extend(
            fields
                .as_object()
                .ok_or("native identity is not an object")?
                .clone(),
        );
    Ok(())
}
