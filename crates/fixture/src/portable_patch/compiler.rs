use super::{
    PatchedFixturePatch, PatchedFixtureProfileReference, PortablePatchError,
    PortablePatchedFixtureRecord, fixture_profile_content_digest,
};
use crate::{FixtureDefinition, FixtureProfile, PatchedFixture};
use light_core::{FixtureId, Revision};
use serde_json::Value;
use std::collections::{HashMap, hash_map::Entry};

/// Raw immutable profile revision supplied by the portable-show boundary.
///
/// An application adapter can construct this directly from
/// `PortableShowDocument::fixture_profile_revision` and `FixtureProfileRevision` getters. The
/// compiler verifies all metadata again before using the content.
#[derive(Clone, Debug)]
pub struct ResolvedFixtureProfileRevision {
    profile_id: FixtureId,
    profile_revision: Revision,
    content_digest: String,
    profile: Value,
}

impl ResolvedFixtureProfileRevision {
    pub fn new(
        profile_id: FixtureId,
        profile_revision: Revision,
        content_digest: impl Into<String>,
        profile: Value,
    ) -> Self {
        Self {
            profile_id,
            profile_revision,
            content_digest: content_digest.into(),
            profile,
        }
    }
}

/// Boundary implemented by an in-memory portable-show document adapter.
pub trait FixtureProfileRevisionResolver {
    fn resolve(
        &mut self,
        reference: PatchedFixtureProfileReference,
    ) -> Option<ResolvedFixtureProfileRevision>;
}

impl<F> FixtureProfileRevisionResolver for F
where
    F: FnMut(PatchedFixtureProfileReference) -> Option<ResolvedFixtureProfileRevision>,
{
    fn resolve(
        &mut self,
        reference: PatchedFixtureProfileReference,
    ) -> Option<ResolvedFixtureProfileRevision> {
        self(reference)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ProfileRevisionKey {
    profile_id: FixtureId,
    profile_revision: Revision,
}

impl From<PatchedFixtureProfileReference> for ProfileRevisionKey {
    fn from(reference: PatchedFixtureProfileReference) -> Self {
        Self {
            profile_id: reference.profile_id,
            profile_revision: reference.profile_revision,
        }
    }
}

/// Compiles portable records into the expanded runtime fixture expected by the current engine.
///
/// Profile JSON is resolved, digest-checked, deserialized, and validated once per immutable
/// revision. Mode projections are likewise cached across every fixture in one show compile.
pub struct PatchedFixtureCompiler<R> {
    resolver: R,
    profiles: HashMap<ProfileRevisionKey, CachedProfile>,
    definitions: HashMap<PatchedFixtureProfileReference, FixtureDefinition>,
}

struct CachedProfile {
    definition: FixtureProfile,
    content_digest: String,
}

impl<R: FixtureProfileRevisionResolver> PatchedFixtureCompiler<R> {
    pub fn new(resolver: R) -> Self {
        Self {
            resolver,
            profiles: HashMap::new(),
            definitions: HashMap::new(),
        }
    }

    pub fn compile(
        &mut self,
        record: &PortablePatchedFixtureRecord,
    ) -> Result<PatchedFixture, PortablePatchError> {
        let result = self.compile_record(record);
        if result.is_err() {
            self.clear_candidate_cache();
        }
        result
    }

    fn compile_record(
        &mut self,
        record: &PortablePatchedFixtureRecord,
    ) -> Result<PatchedFixture, PortablePatchError> {
        if record.is_legacy_inline() {
            let fixture = record.legacy_fixture()?;
            fixture
                .definition
                .validate()
                .map_err(|error| PortablePatchError::InvalidRecord(error.to_string()))?;
            if let Some(reference) = record.selected_profile_reference()? {
                self.verify_legacy_profile(record, reference)?;
            }
            return Ok(fixture);
        }
        let reference = record.profile_reference()?.ok_or_else(|| {
            PortablePatchError::InvalidRecord("profile reference is missing".into())
        })?;
        let definition = self.definition(reference)?;
        Ok(into_runtime_fixture(record.patch()?, definition))
    }

    fn clear_candidate_cache(&mut self) {
        self.profiles.clear();
        self.definitions.clear();
    }

    pub fn compile_all<'a>(
        &mut self,
        records: impl IntoIterator<Item = &'a PortablePatchedFixtureRecord>,
    ) -> Result<Vec<PatchedFixture>, PortablePatchError> {
        records
            .into_iter()
            .map(|record| self.compile(record))
            .collect()
    }

    pub fn cached_profile_count(&self) -> usize {
        self.profiles.len()
    }

    pub fn into_resolver(self) -> R {
        self.resolver
    }

    fn definition(
        &mut self,
        reference: PatchedFixtureProfileReference,
    ) -> Result<FixtureDefinition, PortablePatchError> {
        if let Some(definition) = self.definitions.get(&reference) {
            return Ok(definition.clone());
        }
        let profile = &self.profile(reference)?.definition;
        require_mode(profile, reference)?;
        let definition = profile
            .compact_resolved_definition_from_validated_profile(reference.mode_id)
            .map_err(|error| invalid_profile(reference, error))?;
        self.definitions.insert(reference, definition.clone());
        Ok(definition)
    }

    fn profile(
        &mut self,
        reference: PatchedFixtureProfileReference,
    ) -> Result<&CachedProfile, PortablePatchError> {
        let key = ProfileRevisionKey::from(reference);
        match self.profiles.entry(key) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let resolved = self.resolver.resolve(reference).ok_or(
                    PortablePatchError::MissingProfileRevision {
                        profile_id: reference.profile_id,
                        profile_revision: reference.profile_revision,
                    },
                )?;
                let profile = validate_resolved_profile(reference, resolved)?;
                Ok(entry.insert(profile))
            }
        }
    }

    fn verify_legacy_profile(
        &mut self,
        record: &PortablePatchedFixtureRecord,
        reference: PatchedFixtureProfileReference,
    ) -> Result<(), PortablePatchError> {
        let inline_digest = fixture_profile_content_digest(record.legacy_profile_snapshot()?)?;
        let canonical_digest = &self.profile(reference)?.content_digest;
        if inline_digest == *canonical_digest {
            return Ok(());
        }
        Err(PortablePatchError::ProfileDigestMismatch {
            profile_id: reference.profile_id,
            profile_revision: reference.profile_revision,
            expected_digest: canonical_digest.clone(),
            actual_digest: inline_digest,
        })
    }
}

fn validate_resolved_profile(
    expected: PatchedFixtureProfileReference,
    resolved: ResolvedFixtureProfileRevision,
) -> Result<CachedProfile, PortablePatchError> {
    ensure_resolved_identity(expected, &resolved)?;
    ensure_resolved_digest(expected, &resolved)?;
    let content_digest = resolved.content_digest.clone();
    let profile = decode_profile(expected, resolved.profile)?;
    ensure_profile_identity(expected, &profile)?;
    profile
        .validate()
        .map_err(|error| invalid_profile(expected, error))?;
    Ok(CachedProfile {
        definition: profile,
        content_digest,
    })
}

fn require_mode(
    profile: &FixtureProfile,
    reference: PatchedFixtureProfileReference,
) -> Result<(), PortablePatchError> {
    profile
        .mode(reference.mode_id)
        .map(|_| ())
        .ok_or(PortablePatchError::MissingMode {
            profile_id: reference.profile_id,
            profile_revision: reference.profile_revision,
            mode_id: reference.mode_id,
        })
}

fn ensure_resolved_identity(
    expected: PatchedFixtureProfileReference,
    resolved: &ResolvedFixtureProfileRevision,
) -> Result<(), PortablePatchError> {
    ensure_identity(expected, resolved.profile_id, resolved.profile_revision)
}

fn ensure_resolved_digest(
    expected: PatchedFixtureProfileReference,
    resolved: &ResolvedFixtureProfileRevision,
) -> Result<(), PortablePatchError> {
    let actual_digest = fixture_profile_content_digest(&resolved.profile)?;
    if actual_digest == resolved.content_digest {
        return Ok(());
    }
    Err(PortablePatchError::ProfileDigestMismatch {
        profile_id: expected.profile_id,
        profile_revision: expected.profile_revision,
        expected_digest: resolved.content_digest.clone(),
        actual_digest,
    })
}

fn decode_profile(
    reference: PatchedFixtureProfileReference,
    profile: Value,
) -> Result<FixtureProfile, PortablePatchError> {
    serde_json::from_value(profile).map_err(|error| invalid_profile(reference, error))
}

fn ensure_profile_identity(
    expected: PatchedFixtureProfileReference,
    profile: &FixtureProfile,
) -> Result<(), PortablePatchError> {
    ensure_identity(expected, profile.id, Revision::from(profile.revision))
}

fn ensure_identity(
    expected: PatchedFixtureProfileReference,
    actual_profile_id: FixtureId,
    actual_revision: Revision,
) -> Result<(), PortablePatchError> {
    if actual_profile_id == expected.profile_id && actual_revision == expected.profile_revision {
        return Ok(());
    }
    Err(PortablePatchError::ProfileIdentityMismatch {
        expected_profile_id: expected.profile_id,
        expected_revision: expected.profile_revision,
        actual_profile_id,
        actual_revision,
    })
}

fn invalid_profile(
    reference: PatchedFixtureProfileReference,
    error: impl std::fmt::Display,
) -> PortablePatchError {
    PortablePatchError::InvalidProfile {
        profile_id: reference.profile_id,
        profile_revision: reference.profile_revision,
        message: error.to_string(),
    }
}

fn into_runtime_fixture(
    patch: PatchedFixturePatch,
    definition: FixtureDefinition,
) -> PatchedFixture {
    PatchedFixture {
        fixture_id: patch.fixture_id,
        fixture_number: patch.fixture_number,
        virtual_fixture_number: patch.virtual_fixture_number,
        name: patch.name,
        definition,
        universe: patch.universe,
        address: patch.address,
        split_patches: patch.split_patches,
        layer_id: patch.layer_id,
        direct_control: patch.direct_control,
        location: patch.location,
        rotation: patch.rotation,
        logical_heads: patch.logical_heads,
        multipatch: patch.multipatch,
        move_in_black_enabled: patch.move_in_black_enabled,
        move_in_black_delay_millis: patch.move_in_black_delay_millis,
        highlight_overrides: patch.highlight_overrides,
    }
}
