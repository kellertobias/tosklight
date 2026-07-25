use crate::{
    DirectControlEndpoint, FixtureLocation, FixtureVector, MultiPatchInstance, PatchedFixture,
    PatchedHead, SplitPatch, default_patch_layer,
};
use light_core::{DmxAddress, FixtureId, Revision, Universe};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

pub const PORTABLE_PATCH_RECORD_SCHEMA_VERSION: u16 = 1;
pub const RETAINED_LEGACY_DEFINITION_FIELDS: &str = "_light_legacy_definition_fields";

/// Stable reference to one immutable profile revision and its selected mode.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PatchedFixtureProfileReference {
    pub profile_id: FixtureId,
    pub profile_revision: Revision,
    pub mode_id: Uuid,
}

/// Fields owned by the show patch rather than the immutable fixture profile.
///
/// Keeping these fields separate from [`crate::FixtureDefinition`] lets portable shows store one
/// profile revision once, regardless of how many fixtures use it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PatchedFixturePatch {
    pub fixture_id: FixtureId,
    /// Operator-facing fixture number. This is distinct from the stable internal UUID.
    #[serde(default)]
    pub fixture_number: Option<u32>,
    /// Operator-facing number in the reserved visual-only `0.x` namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtual_fixture_number: Option<u32>,
    /// Show-local operator name. Profile names remain immutable library metadata.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub universe: Option<Universe>,
    #[serde(default)]
    pub address: Option<DmxAddress>,
    #[serde(default)]
    pub split_patches: Vec<SplitPatch>,
    #[serde(default = "default_patch_layer")]
    pub layer_id: String,
    #[serde(default)]
    pub direct_control: Option<DirectControlEndpoint>,
    #[serde(default)]
    pub location: FixtureLocation,
    #[serde(default)]
    pub rotation: FixtureVector,
    #[serde(default)]
    pub logical_heads: Vec<PatchedHead>,
    #[serde(default)]
    pub multipatch: Vec<MultiPatchInstance>,
    #[serde(default = "default_true")]
    pub move_in_black_enabled: bool,
    #[serde(default)]
    pub move_in_black_delay_millis: u64,
    #[serde(default)]
    pub highlight_overrides: BTreeMap<Uuid, u32>,
}

impl PatchedFixturePatch {
    pub(crate) fn from_fixture(fixture: &PatchedFixture) -> Self {
        Self {
            fixture_id: fixture.fixture_id,
            fixture_number: fixture.fixture_number,
            virtual_fixture_number: fixture.virtual_fixture_number,
            name: fixture.name.clone(),
            universe: fixture.universe,
            address: fixture.address,
            split_patches: fixture.split_patches.clone(),
            layer_id: fixture.layer_id.clone(),
            direct_control: fixture.direct_control.clone(),
            location: fixture.location,
            rotation: fixture.rotation,
            logical_heads: fixture.logical_heads.clone(),
            multipatch: fixture.multipatch.clone(),
            move_in_black_enabled: fixture.move_in_black_enabled,
            move_in_black_delay_millis: fixture.move_in_black_delay_millis,
            highlight_overrides: fixture.highlight_overrides.clone(),
        }
    }
}

const fn default_true() -> bool {
    true
}

/// Lossless raw portable record with a typed patch editing surface.
///
/// The record remembers whether it was read in legacy inline form. Updating patch-owned fields on
/// a legacy record deliberately retains its inline definition until the show migration chooses to
/// replace it. New records always use the lean reference-only representation.
#[derive(Clone, Debug)]
pub struct PortablePatchedFixtureRecord {
    pub(crate) body: Value,
    pub(crate) representation: RecordRepresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordRepresentation {
    LegacyInline,
    ProfileReference,
}

#[derive(Debug, Error)]
pub enum PortablePatchError {
    #[error("invalid portable patched fixture: {0}")]
    InvalidRecord(String),
    #[error("portable patched fixture identity cannot change from {expected:?} to {actual:?}")]
    FixtureIdentityChanged {
        expected: FixtureId,
        actual: FixtureId,
    },
    #[error("portable patched fixture mixes legacy inline and profile-reference representations")]
    AmbiguousRepresentation,
    #[error("portable patched fixture has unsupported record schema {0}")]
    UnsupportedRecordSchema(u64),
    #[error("portable patched fixture contains a duplicate {collection} identity")]
    DuplicateNestedIdentity { collection: &'static str },
    #[error("portable patched fixture {collection} identities require an explicit topology change")]
    NestedIdentityChanged { collection: &'static str },
    #[error(
        "fixture profile {profile_id:?} revision {profile_revision} is missing from the portable show"
    )]
    MissingProfileRevision {
        profile_id: FixtureId,
        profile_revision: Revision,
    },
    #[error(
        "fixture profile reference {expected_profile_id:?} revision {expected_revision} resolved to {actual_profile_id:?} revision {actual_revision}"
    )]
    ProfileIdentityMismatch {
        expected_profile_id: FixtureId,
        expected_revision: Revision,
        actual_profile_id: FixtureId,
        actual_revision: Revision,
    },
    #[error(
        "fixture profile {profile_id:?} revision {profile_revision} has digest {actual_digest}, expected {expected_digest}"
    )]
    ProfileDigestMismatch {
        profile_id: FixtureId,
        profile_revision: Revision,
        expected_digest: String,
        actual_digest: String,
    },
    #[error(
        "fixture profile {profile_id:?} revision {profile_revision} does not contain mode {mode_id}"
    )]
    MissingMode {
        profile_id: FixtureId,
        profile_revision: Revision,
        mode_id: Uuid,
    },
    #[error("fixture profile {profile_id:?} revision {profile_revision} is invalid: {message}")]
    InvalidProfile {
        profile_id: FixtureId,
        profile_revision: Revision,
        message: String,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
