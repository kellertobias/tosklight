#![forbid(unsafe_code)]

//! The Media Server's versioned HTTP API.
//!
//! Output-scoped resources live below a stable output identifier; process-wide catalog and health
//! resources stay process-scoped. Reads load whole-object snapshots. Writes are intent-shaped:
//! a body carries only the fields being changed, never a whole-object overwrite.
//!
//! This is a trusted-LAN service, as the settled decision records. It binds where configuration
//! says and carries no authentication of its own.

pub mod assets;
pub mod data_folder;
pub mod diagnostics;
pub mod error;
pub mod generation;
pub mod replay;
pub mod routes;
pub mod tolerant;
pub mod wire;

pub use data_folder::{DataFolders, FolderChange, FolderEntry, FolderListing, FolderRefusal};
pub use diagnostics::{
    AudioSource, AudioTelemetry, DeskIdentityTelemetry, DeviceLister, Diagnostics, DmxTelemetry,
    FolderPresentation, ImportJob, ImportOutcome, ImportedModel, Imports, LibraryAccess,
    LibraryEdit, LibraryNoteTarget, LogEntry, LogLevelControl, LogPage, LogQuery, LogSource,
    ModelAccess, ModelRejection, MonitorDevice, MonitorLister, PendingImport,
    SpeedGroupReadingTelemetry, SpeedGroupRejectionTelemetry, SpeedGroupSource,
    SpeedGroupTelemetry, UploadStream,
};
pub use error::{ApiError, ApiErrorBody};
pub use generation::{GeneratedArtifact, generated_artifacts, write_generated_artifacts};
pub use replay::Replays;
pub use routes::snapshot::{
    MAX_SNAPSHOT_EDGE, RenderSnapshot, SnapshotCache, SnapshotFailure, SnapshotImage,
    SnapshotRequest, renders_nothing,
};
pub use routes::{
    ApiState, ApplyConfiguration, OutputPreviewFrame, RequestOutputPreview, SettleConfiguration,
    applies_nothing, router, settles_at_once,
};
pub use tolerant::TolerantJson;
