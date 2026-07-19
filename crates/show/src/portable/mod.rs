mod candidate;
mod document;
mod migration;
mod profile_revision;
mod repository;
mod store;
mod transaction;

pub use document::{
    PortablePatchRevision, PortableShowDocument, PortableShowObject, PortableShowObjectKey,
    PortableShowRevision,
};
pub use profile_revision::{
    FixtureProfileDigest, FixtureProfileRevision, FixtureProfileRevisionId,
    FixtureProfileRevisionInsertResult, FixtureProfileRevisionInsertStatus,
    LegacyInlineProfileSnapshot, canonical_fixture_profile_json,
    canonicalize_legacy_inline_profile_snapshots, discover_legacy_inline_profile_snapshots,
};
pub use transaction::{PortableShowCommit, PortableShowTransaction};

pub(crate) use migration::{SHOW_SCHEMA_VERSION, migrate_show, validate_show_connection};
pub(crate) use repository::{
    delete_legacy_object, mutate_legacy_objects, put_legacy_object, undo_legacy_object,
};
pub(crate) use store::{bump_revision, initialise_revision};

#[cfg(test)]
mod candidate_tests;
#[cfg(test)]
mod tests;
pub use candidate::{
    PortableShowCandidate, PortableShowCandidateObject, PortableShowCandidateObjects,
    PortableShowCandidateProfiles,
};
