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
    ClientDesk, ControlDesk, DeskUser, FixedScreenFixtureColumn, FixedScreenFixtureCompactMode,
    FixedScreenFixtureIncludedHeads, FixedScreenFixtureOrder, FixedScreenPane,
    FixedScreenStageRenderQuality, FixedScreenTextMode, PersistedSession, PlaybackSurfaceLayout,
    PlaybackSurfaceRow, RevisionCopySource, ScreenConfiguration, ScreenContent, ShowEntry,
    ShowRevision, VersionedObject,
};
pub use portable::{
    FixtureProfileDigest, FixtureProfileRevision, FixtureProfileRevisionId,
    FixtureProfileRevisionInsertResult, FixtureProfileRevisionInsertStatus,
    LegacyInlineProfileSnapshot, LosslessBody, PortableJson, PortablePatchRevision,
    PortableShowCandidate, PortableShowCandidateObject, PortableShowCandidateObjects,
    PortableShowCandidateProfiles, PortableShowCommit, PortableShowDocument, PortableShowObject,
    PortableShowObjectKey, PortableShowObjectUndo, PortableShowRevision, PortableShowTransaction,
    ScheduleOccurrenceClaim, ScheduleOccurrenceClaimResult, ScheduleOccurrenceRecord,
    ScheduleOccurrenceResolution, ScheduleOccurrenceStatus, SkippedScheduleOccurrence, apply_delta,
    canonical_fixture_profile_json, canonicalize_legacy_inline_profile_snapshots,
    discover_legacy_inline_profile_snapshots, merge_typed, merge_typed_request,
    strip_zero_u64_echo,
};
pub use show_store::{
    AtomicObjectDelete, AtomicObjectWrite, ShowStore, initialise_show, validate_show_file,
};

pub(crate) use connection::set_schema_version;

#[cfg(test)]
mod tests;
