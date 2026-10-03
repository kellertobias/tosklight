//! Control ↔ Architect automatic show synchronization.
//!
//! One sync transaction is one Architect gesture: generic object, patch and metadata edits that
//! commit together, compared field by field against the client's base. See
//! `docs/engineering/show-sync.md` for the kind matrix and the protocol.

mod events;
mod fields;
mod model;
mod ports;
mod service;
mod staging;

pub use events::{
    SHOW_SYNC_EVENT_BODY_BYTES, SHOW_SYNC_EVENT_BYTES, SHOW_SYNC_EVENT_OBJECTS,
    SHOW_SYNC_EVENT_OMITTED_BODIES, ShowSyncCommitChange, ShowSyncCommittedObject,
    ShowSyncGapChange, ShowSyncGapReason, ShowSyncProfileReference, ShowSyncPublication,
    is_mirrored_kind,
};
pub use model::{
    METADATA_KIND, PATCHED_FIXTURE_KIND, REQUEST_REUSED_MESSAGE, SHOW_SYNC_METADATA_PREFIXES,
    SHOW_SYNC_OBJECT_KINDS, ShowSyncAppliedObject, ShowSyncCommand, ShowSyncConflict,
    ShowSyncConflictReason, ShowSyncFieldEdit, ShowSyncOperation, ShowSyncResult,
    is_sync_metadata_key, is_sync_object_kind,
};
pub use ports::ShowSyncPorts;

#[cfg(test)]
mod tests;
