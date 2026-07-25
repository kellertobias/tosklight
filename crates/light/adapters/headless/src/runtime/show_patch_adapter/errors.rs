use light_application::{ActionError, ActionErrorKind};
use light_engine::EngineError;
use light_fixture::FixtureError;
use light_show::StoreError;

pub(super) fn store_error(error: StoreError, fallback: Option<u64>) -> ActionError {
    let message = error.to_string();
    let (kind, revision) = match error {
        StoreError::RevisionConflict { current, .. } => {
            (ActionErrorKind::Conflict, fallback.or(Some(current)))
        }
        StoreError::DocumentRevisionConflict { current, .. } => {
            (ActionErrorKind::Conflict, Some(current.value()))
        }
        StoreError::FixtureProfileRevisionConflict { .. } => (ActionErrorKind::Conflict, fallback),
        StoreError::Sql(_) => (ActionErrorKind::Unavailable, fallback),
        StoreError::Uuid(_) | StoreError::Json(_) | StoreError::Invalid(_) => {
            (ActionErrorKind::Invalid, fallback)
        }
    };
    with_revision(ActionError::new(kind, message), revision)
}

pub(super) fn fixture_error(error: FixtureError, fallback: Option<u64>) -> ActionError {
    let message = error.to_string();
    let (kind, revision) = match error {
        FixtureError::RevisionConflict { current, .. } => (
            ActionErrorKind::Conflict,
            fallback.or(Some(u64::from(current))),
        ),
        FixtureError::Invalid(_) | FixtureError::Json(_) => (ActionErrorKind::Invalid, fallback),
        FixtureError::Sql(_) | FixtureError::Io(_) => (ActionErrorKind::Unavailable, fallback),
    };
    with_revision(ActionError::new(kind, message), revision)
}

pub(super) fn engine_error(error: EngineError, revision: Option<u64>) -> ActionError {
    with_revision(
        ActionError::new(ActionErrorKind::Invalid, error.to_string()),
        revision,
    )
}

fn with_revision(mut error: ActionError, revision: Option<u64>) -> ActionError {
    if let Some(revision) = revision {
        error = error.at_revision(revision);
    }
    error
}
