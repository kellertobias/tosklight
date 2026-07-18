mod legacy;
mod model;
mod repository;

pub use legacy::{
    LegacyInlineProfileSnapshot, canonicalize_legacy_inline_profile_snapshots,
    discover_legacy_inline_profile_snapshots,
};
pub use model::{
    FixtureProfileDigest, FixtureProfileRevision, FixtureProfileRevisionId,
    canonical_fixture_profile_json,
};
pub use repository::{FixtureProfileRevisionInsertResult, FixtureProfileRevisionInsertStatus};

pub(crate) use model::profile_conflict;
pub(crate) use repository::{insert_fixture_profile_revision_in, load_fixture_profile_revisions};

#[cfg(test)]
mod tests;
