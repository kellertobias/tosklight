#![forbid(unsafe_code)]

//! The Media Server's versioned HTTP API.
//!
//! Output-scoped resources live below a stable output identifier; process-wide catalog and health
//! resources stay process-scoped. Reads load whole-object snapshots. Writes are intent-shaped:
//! a body carries only the fields being changed, never a whole-object overwrite.
//!
//! This is a trusted-LAN service, as the settled decision records. It binds where configuration
//! says and carries no authentication of its own.

pub mod error;
pub mod routes;
pub mod tolerant;
pub mod wire;

pub use error::{ApiError, ApiErrorBody};
pub use routes::{ApiState, router};
pub use tolerant::TolerantJson;
