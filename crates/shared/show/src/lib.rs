#![forbid(unsafe_code)]
//! Versioned SQLite persistence for desk state and portable, self-contained show files.

mod connection;
mod desk;
mod error;
mod model;
mod portable;
mod programming_contract;
mod show_store;

pub use desk::DeskStore;
pub use error::StoreError;
pub use model::{
    ClientDesk, ControlDesk, FixedScreenFixtureColumn, FixedScreenFixtureCompactMode,
    FixedScreenFixtureIncludedHeads, FixedScreenFixtureOrder, FixedScreenPane, FixedScreenSide,
    FixedScreenStageRenderQuality, FixedScreenTextMode, PersistedSession, PlaybackSurfaceLayout,
    PlaybackSurfaceRow, ProgrammerControlSurfaceConfiguration, RevisionCopySource,
    ScreenConfiguration, ScreenContent, ShowEntry, ShowRevision, VersionedObject,
};
pub use portable::{
    CueThumbnail, CueThumbnailEntry, FixtureProfileDigest, FixtureProfileRevision,
    FixtureProfileRevisionId, FixtureProfileRevisionInsertResult,
    FixtureProfileRevisionInsertStatus, LegacyInlineProfileSnapshot, LosslessBody, PortableJson,
    PortablePatchRevision, PortableShowCandidate, PortableShowCandidateObject,
    PortableShowCandidateObjects, PortableShowCandidateProfiles, PortableShowCommit,
    PortableShowDocument, PortableShowObject, PortableShowObjectKey, PortableShowObjectRedo,
    PortableShowObjectUndo, PortableShowRevision, PortableShowTransaction,
    SYNC_APPLIED_REQUEST_RETENTION, ScheduleOccurrenceClaim, ScheduleOccurrenceClaimResult,
    ScheduleOccurrenceRecord, ScheduleOccurrenceResolution, ScheduleOccurrenceStatus,
    SkippedScheduleOccurrence, SyncAppliedRequest, SyncRequestRecord, apply_delta,
    canonical_fixture_profile_json, canonicalize_legacy_inline_profile_snapshots,
    discover_legacy_inline_profile_snapshots, merge_typed, merge_typed_request,
    strip_zero_u64_echo,
};
pub use programming_contract::{
    LegacyProgrammingFamily, LegacyProgrammingFinding, PROGRAMMING_CONTRACT_METADATA_KEY,
    PROGRAMMING_OBJECT_KINDS, ProgrammingContractMarker, ProgrammingContractRejection,
    ShowProgrammingContractReport, check_programming_object_writes,
    inspect_show_programming_contract, legacy_attribute_value, legacy_live_write_message,
    legacy_programming_address, legacy_programming_attributes, legacy_programming_family,
    required_object_programming_contract, validate_show_programming_contract,
    writer_stamps_programming_contract,
};
pub use show_store::{
    AtomicObjectDelete, AtomicObjectWrite, ObjectStamp, ShowStore, initialise_show,
    validate_show_file,
};

pub(crate) use connection::set_schema_version;

#[cfg(test)]
mod tests;
