#![forbid(unsafe_code)]
//! Keeping an Architect document and the Control show it came from in step.
//!
//! A document opened from a desk is *bound* to that desk's show: the binding names the desk
//! installation, the show UUID and an association identity that scopes every sync request. The
//! binding is installation state on this computer — never part of the portable show — so a copy
//! of the file carries no binding and can never write into the desk by accident.
//!
//! The protocol is `docs/engineering/show-sync.md`. This crate is the client: the binding store,
//! the durable journal of unconfirmed edits, the confirmed mirror of the desk's show, the desk
//! client, and the engine that ties them to the open document. It has no window code, so the
//! Architect and the end-to-end harness run exactly the same synchronization.

pub mod binding;
pub mod desk;
pub mod engine;
pub mod intent;
pub mod journal;
pub mod mirror;
pub mod state;
pub mod status;

pub use binding::{SyncBinding, SyncBindingStore};
pub use engine::{ConflictView, DocumentHost, Resolution, Start, SyncEngine};
pub use status::{SyncPhase, SyncStatus};
