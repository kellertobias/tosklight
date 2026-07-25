use crate::StoreError;
use light_core::{FixtureId, Revision};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use uuid::Uuid;

/// Stable identity of an immutable fixture-profile revision in a portable show.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FixtureProfileRevisionId {
    profile_id: FixtureId,
    revision: Revision,
}

impl FixtureProfileRevisionId {
    pub fn new(profile_id: FixtureId, revision: Revision) -> Result<Self, StoreError> {
        validate_revision(revision)?;
        Ok(Self {
            profile_id,
            revision,
        })
    }

    pub const fn profile_id(&self) -> FixtureId {
        self.profile_id
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }
}

impl Ord for FixtureProfileRevisionId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.profile_id
            .0
            .as_bytes()
            .cmp(other.profile_id.0.as_bytes())
            .then_with(|| self.revision.cmp(&other.revision))
    }
}

impl PartialOrd for FixtureProfileRevisionId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Canonical SHA-256 digest of the complete raw fixture-profile JSON.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FixtureProfileDigest(String);

impl FixtureProfileDigest {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Immutable raw fixture profile with unknown fields and inline assets retained.
#[derive(Clone, Debug, PartialEq)]
pub struct FixtureProfileRevision {
    id: FixtureProfileRevisionId,
    digest: FixtureProfileDigest,
    profile: Value,
}

impl FixtureProfileRevision {
    pub fn from_profile(profile: Value) -> Result<Self, StoreError> {
        let (profile_id, revision) = profile_identity(&profile)?;
        Self::new(profile_id, revision, profile)
    }

    pub fn new(
        profile_id: FixtureId,
        revision: Revision,
        profile: Value,
    ) -> Result<Self, StoreError> {
        let id = FixtureProfileRevisionId::new(profile_id, revision)?;
        ensure_profile_identity(&profile, &id)?;
        let digest = digest_profile(&profile)?;
        Ok(Self {
            id,
            digest,
            profile,
        })
    }

    pub fn id(&self) -> &FixtureProfileRevisionId {
        &self.id
    }

    pub fn digest(&self) -> &FixtureProfileDigest {
        &self.digest
    }

    pub fn profile(&self) -> &Value {
        &self.profile
    }

    pub(crate) fn from_stored(
        profile_id: String,
        revision: Revision,
        stored_digest: String,
        profile: Value,
    ) -> Result<Self, StoreError> {
        let profile_id = FixtureId(Uuid::parse_str(&profile_id)?);
        let candidate = Self::new(profile_id, revision, profile)?;
        if candidate.digest.as_str() == stored_digest {
            Ok(candidate)
        } else {
            Err(StoreError::Invalid(format!(
                "fixture profile {} revision {} has an invalid content digest",
                candidate.id.profile_id.0, candidate.id.revision
            )))
        }
    }
}

/// Serializes a profile with recursively sorted object keys and no insignificant whitespace.
pub fn canonical_fixture_profile_json(profile: &Value) -> Result<String, StoreError> {
    String::from_utf8(canonical_profile_bytes(profile)?)
        .map_err(|_| StoreError::Invalid("canonical fixture profile is not UTF-8".into()))
}

pub(crate) fn profile_conflict(
    existing: &FixtureProfileRevision,
    candidate: &FixtureProfileRevision,
) -> StoreError {
    StoreError::FixtureProfileRevisionConflict {
        profile_id: candidate.id.profile_id.0.to_string(),
        revision: candidate.id.revision,
        existing_digest: existing.digest.0.clone(),
        candidate_digest: candidate.digest.0.clone(),
    }
}

fn digest_profile(profile: &Value) -> Result<FixtureProfileDigest, StoreError> {
    let canonical = canonical_profile_bytes(profile)?;
    Ok(FixtureProfileDigest(format!(
        "sha256:{:x}",
        Sha256::digest(canonical)
    )))
}

fn profile_identity(profile: &Value) -> Result<(FixtureId, Revision), StoreError> {
    let object = profile
        .as_object()
        .ok_or_else(|| invalid_profile("content must be a JSON object"))?;
    let profile_id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_profile("id must be a string"))?;
    let revision = object
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_profile("revision must be a non-negative integer"))?;
    let profile_id = Uuid::parse_str(profile_id)
        .map(FixtureId)
        .map_err(|_| invalid_profile("id must be a UUID"))?;
    Ok((profile_id, revision))
}

fn ensure_profile_identity(
    profile: &Value,
    expected: &FixtureProfileRevisionId,
) -> Result<(), StoreError> {
    let (profile_id, revision) = profile_identity(profile)?;
    if profile_id == expected.profile_id && revision == expected.revision {
        Ok(())
    } else {
        Err(invalid_profile("content identity does not match its key"))
    }
}

fn validate_revision(revision: Revision) -> Result<(), StoreError> {
    if revision > i64::MAX as u64 {
        return Err(invalid_profile("revision exceeds the SQLite integer range"));
    }
    Ok(())
}

fn canonical_profile_bytes(profile: &Value) -> Result<Vec<u8>, StoreError> {
    let mut output = Vec::new();
    write_canonical(profile, &mut output)?;
    Ok(output)
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) -> Result<(), StoreError> {
    match value {
        Value::Array(values) => write_array(values, output),
        Value::Object(values) => write_object(values, output),
        _ => serde_json::to_writer(output, value).map_err(Into::into),
    }
}

fn write_array(values: &[Value], output: &mut Vec<u8>) -> Result<(), StoreError> {
    output.push(b'[');
    for (index, value) in values.iter().enumerate() {
        push_separator(output, index);
        write_canonical(value, output)?;
    }
    output.push(b']');
    Ok(())
}

fn write_object(values: &Map<String, Value>, output: &mut Vec<u8>) -> Result<(), StoreError> {
    let mut entries = values.iter().collect::<Vec<_>>();
    entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
    output.push(b'{');
    for (index, (key, value)) in entries.into_iter().enumerate() {
        push_separator(output, index);
        serde_json::to_writer(&mut *output, key)?;
        output.push(b':');
        write_canonical(value, output)?;
    }
    output.push(b'}');
    Ok(())
}

fn push_separator(output: &mut Vec<u8>, index: usize) {
    if index > 0 {
        output.push(b',');
    }
}

fn invalid_profile(message: &str) -> StoreError {
    StoreError::Invalid(format!("invalid fixture profile revision: {message}"))
}
