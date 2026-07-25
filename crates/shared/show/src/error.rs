use crate::portable::PortableShowRevision;
use light_core::Revision;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("revision conflict: expected {expected}, current {current}")]
    RevisionConflict {
        expected: Revision,
        current: Revision,
    },
    #[error("portable show revision conflict: expected {expected}, current {current}")]
    DocumentRevisionConflict {
        expected: PortableShowRevision,
        current: PortableShowRevision,
    },
    #[error(
        "fixture profile {profile_id} revision {revision} conflicts: stored digest {existing_digest}, candidate digest {candidate_digest}"
    )]
    FixtureProfileRevisionConflict {
        profile_id: String,
        revision: Revision,
        existing_digest: String,
        candidate_digest: String,
    },
}
