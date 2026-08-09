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
pub mod diagnostics;
pub mod error;
pub mod generation;
pub mod replay;
pub mod routes;
pub mod tolerant;
pub mod wire;

pub use diagnostics::{
    AudioSource, AudioTelemetry, DeviceLister, Diagnostics, DmxTelemetry, ImportJob, ImportOutcome,
    Imports, LibraryAccess, LibraryEdit, LogEntry, LogLevelControl, LogPage, LogQuery, LogSource,
    PendingImport, UploadStream,
};
pub use error::{ApiError, ApiErrorBody};
pub use generation::{GeneratedArtifact, generated_artifacts, write_generated_artifacts};
pub use replay::Replays;
pub use routes::{ApiState, ApplyConfiguration, applies_nothing, router};
pub use tolerant::TolerantJson;
