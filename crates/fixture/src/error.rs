use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("invalid fixture: {0}")]
    Invalid(String),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("fixture revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: u32, current: u32 },
}
