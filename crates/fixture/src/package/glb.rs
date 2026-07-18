use super::manifest::FixturePackageError;
use crate::FixtureProfile;

pub(super) fn validate_glb(bytes: &[u8]) -> Result<(), FixturePackageError> {
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

pub(super) fn validate_profile(profile: &FixtureProfile) -> Result<(), FixturePackageError> {
    profile
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    // FixtureProfile::validate covers every mode and channel. Calling resolved_definition for
    // every mode here would clone the complete profile snapshot into every temporary definition,
    // becoming quadratic for large imported or operator-authored profiles with many modes.
    Ok(())
}

pub(super) fn invalid(message: impl Into<String>) -> FixturePackageError {
    FixturePackageError::Invalid(message.into())
}
