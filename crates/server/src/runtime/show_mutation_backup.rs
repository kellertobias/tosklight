use super::{AppState, ShowEntry};
use light_application::{ActionError, ActionErrorKind, BackupIdentity};
use light_show::{ShowStore, StoreError};
use std::path::PathBuf;

/// Retained same-store backup for a show mutation that has already been prepared.
pub(in crate::runtime) struct ShowMutationBackupPlan {
    directory: PathBuf,
    filename_prefix: String,
    retention: usize,
}

impl ShowMutationBackupPlan {
    pub(in crate::runtime) fn patch(state: &AppState, entry: &ShowEntry) -> Self {
        Self::new(
            state.data_dir.join("backups"),
            entry,
            "patch",
            state.configuration.read().backup_retention,
        )
    }

    pub(in crate::runtime) fn output_route(state: &AppState, entry: &ShowEntry) -> Self {
        Self::new(
            state.data_dir.join("backups"),
            entry,
            "output-route",
            state.configuration.read().backup_retention,
        )
    }

    pub(in crate::runtime) fn show_objects(state: &AppState, entry: &ShowEntry) -> Self {
        Self::new(
            state.data_dir.join("backups"),
            entry,
            "show-object",
            state.configuration.read().backup_retention,
        )
    }

    pub(in crate::runtime) fn migration(
        data_dir: &std::path::Path,
        entry: &ShowEntry,
        retention: usize,
    ) -> Self {
        Self::new(data_dir.join("backups"), entry, "migration", retention)
    }

    fn new(directory: PathBuf, entry: &ShowEntry, operation: &str, retention: usize) -> Self {
        Self {
            directory,
            filename_prefix: format!(
                "{}-{}-{}-",
                filename_component(&entry.name, 80),
                entry.id.0,
                operation,
            ),
            retention: retention.max(1),
        }
    }

    pub(in crate::runtime) fn create_mutation(
        &self,
        store: &ShowStore,
        identity: &BackupIdentity,
        current_revision: Option<u64>,
    ) -> Result<(), ActionError> {
        let identity = format!(
            "{}-{}",
            identity.correlation_id,
            filename_component(&identity.request_id, 48)
        );
        self.create(store, &identity, current_revision)
    }

    pub(in crate::runtime) fn create_migration(
        &self,
        store: &ShowStore,
        source_revision: u64,
    ) -> Result<(), ActionError> {
        self.create(
            store,
            &format!("source-revision-{source_revision}"),
            Some(source_revision),
        )
    }

    fn create(
        &self,
        store: &ShowStore,
        identity: &str,
        current_revision: Option<u64>,
    ) -> Result<(), ActionError> {
        std::fs::create_dir_all(&self.directory)
            .map_err(|error| unavailable(error, current_revision))?;
        store
            .backup_to(self.destination(identity))
            .map_err(|error| store_error(error, current_revision))?;
        self.enforce_retention(current_revision)
    }

    fn destination(&self, identity: &str) -> PathBuf {
        self.directory.join(format!(
            "{}{}-{}.show",
            self.filename_prefix,
            chrono::Utc::now().timestamp_millis(),
            filename_component(identity, 96),
        ))
    }

    fn enforce_retention(&self, current_revision: Option<u64>) -> Result<(), ActionError> {
        let mut backups = std::fs::read_dir(&self.directory)
            .map_err(|error| unavailable(error, current_revision))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| self.is_managed_backup(path))
            .collect::<Vec<_>>();
        backups.sort();
        let remove_count = backups.len().saturating_sub(self.retention);
        for path in backups.into_iter().take(remove_count) {
            std::fs::remove_file(path).map_err(|error| unavailable(error, current_revision))?;
        }
        Ok(())
    }

    fn is_managed_backup(&self, path: &std::path::Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(&self.filename_prefix) && name.ends_with(".show"))
    }
}

fn unavailable(error: impl std::fmt::Display, revision: Option<u64>) -> ActionError {
    with_revision(
        ActionError::new(ActionErrorKind::Unavailable, error.to_string()),
        revision,
    )
}

fn store_error(error: StoreError, fallback: Option<u64>) -> ActionError {
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

fn with_revision(mut error: ActionError, revision: Option<u64>) -> ActionError {
    if let Some(revision) = revision {
        error = error.at_revision(revision);
    }
    error
}

fn filename_component(value: &str, limit: usize) -> String {
    let component = value
        .chars()
        .take(limit)
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => character,
            _ => '_',
        })
        .collect::<String>();
    if component.is_empty() {
        "unnamed".into()
    } else {
        component
    }
}
