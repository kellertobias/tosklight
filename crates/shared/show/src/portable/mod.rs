mod candidate;
mod document;
mod lossless_body;
mod lossless_json;
mod migration;
mod profile_revision;
mod repository;
mod schedule_occurrence;
mod store;
mod transaction;

pub use document::{
    PortablePatchRevision, PortableShowDocument, PortableShowObject, PortableShowObjectKey,
    PortableShowObjectRedo, PortableShowObjectUndo, PortableShowRevision,
};
pub use lossless_body::{LosslessBody, PortableJson};
pub use lossless_json::{apply_delta, merge_typed, merge_typed_request, strip_zero_u64_echo};
pub use profile_revision::{
    FixtureProfileDigest, FixtureProfileRevision, FixtureProfileRevisionId,
    FixtureProfileRevisionInsertResult, FixtureProfileRevisionInsertStatus,
    LegacyInlineProfileSnapshot, canonical_fixture_profile_json,
    canonicalize_legacy_inline_profile_snapshots, discover_legacy_inline_profile_snapshots,
};
pub use schedule_occurrence::{
    ScheduleOccurrenceClaim, ScheduleOccurrenceClaimResult, ScheduleOccurrenceRecord,
    ScheduleOccurrenceResolution, ScheduleOccurrenceStatus, SkippedScheduleOccurrence,
};
pub use transaction::{PortableShowCommit, PortableShowTransaction};

pub(crate) use migration::{SHOW_SCHEMA_VERSION, migrate_show, validate_show_connection};
pub(crate) use repository::{
    delete_legacy_object, mutate_legacy_objects, prepare_redo, prepare_undo, put_legacy_object,
    undo_legacy_object,
};
pub(crate) use store::{bump_revision, current_revision, initialise_revision};

#[cfg(test)]
mod candidate_tests;
#[cfg(test)]
mod schedule_occurrence_tests;
#[cfg(test)]
mod tests;
pub use candidate::{
    PortableShowCandidate, PortableShowCandidateObject, PortableShowCandidateObjects,
    PortableShowCandidateProfiles,
};
