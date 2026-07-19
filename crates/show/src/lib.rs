#![forbid(unsafe_code)]
//! Versioned SQLite persistence for desk state and portable, self-contained show files.

mod connection;
mod desk;
mod error;
mod model;
mod portable;
mod show_store;

pub use desk::DeskStore;
pub use error::StoreError;
pub use model::{
    ClientDesk, ControlDesk, DeskUser, PersistedSession, PlaybackSurfaceLayout, PlaybackSurfaceRow,
    RevisionCopySource, ScreenConfiguration, ShowEntry, ShowRevision, VersionedObject,
};
pub use portable::{
    FixtureProfileDigest, FixtureProfileRevision, FixtureProfileRevisionId,
    FixtureProfileRevisionInsertResult, FixtureProfileRevisionInsertStatus,
    LegacyInlineProfileSnapshot, PortablePatchRevision, PortableShowCandidate,
    PortableShowCandidateObject, PortableShowCandidateObjects, PortableShowCandidateProfiles,
    PortableShowCommit, PortableShowDocument, PortableShowObject, PortableShowObjectKey,
    PortableShowRevision, PortableShowTransaction, canonical_fixture_profile_json,
    canonicalize_legacy_inline_profile_snapshots, discover_legacy_inline_profile_snapshots,
};
pub use show_store::{
    AtomicObjectDelete, AtomicObjectWrite, ShowStore, initialise_show, validate_show_file,
};

pub(crate) use connection::set_schema_version;

#[cfg(test)]
mod tests;
