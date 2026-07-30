use super::*;
use light_application::{ActionError, ActionErrorKind};
use light_engine::{EngineError, PreparedEngineSnapshot};
use light_show::{PortableShowTransaction, StoreError};

#[derive(Debug)]
pub(super) enum ShowLoadError {
    Store(StoreError),
    Application(ActionError),
    Engine(EngineError),
    Invariant(String),
}

impl std::fmt::Display for ShowLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(error) => error.fmt(formatter),
            Self::Application(error) => formatter.write_str(&error.message),
            Self::Engine(error) => error.fmt(formatter),
            Self::Invariant(message) => formatter.write_str(message),
        }
    }
}

/// One exact portable document, its staged compatibility migration, and the snapshot compiled
/// from that same candidate. No persistence changes occur until `prepare_runtime` succeeds.
pub(super) struct PreparedShowLoad {
    store: ActiveShowRepository,
    source_revision: u64,
    transaction: PortableShowTransaction,
    snapshot: EngineSnapshot,
}

pub(super) struct PreparedRuntimeShowLoad {
    store: ActiveShowRepository,
    source_revision: u64,
    transaction: PortableShowTransaction,
    runtime: PreparedEngineSnapshot,
}

impl PreparedShowLoad {
    pub(super) fn prepare_runtime(
        self,
        prepare: impl FnOnce(EngineSnapshot) -> Result<PreparedEngineSnapshot, EngineError>,
    ) -> Result<PreparedRuntimeShowLoad, ShowLoadError> {
        let runtime = prepare(self.snapshot).map_err(ShowLoadError::Engine)?;
        Ok(PreparedRuntimeShowLoad {
            store: self.store,
            source_revision: self.source_revision,
            transaction: self.transaction,
            runtime,
        })
    }

    fn into_snapshot(self) -> EngineSnapshot {
        self.snapshot
    }
}

impl PreparedRuntimeShowLoad {
    pub(super) fn commit_migration(
        self,
        backup: &ShowMutationBackupPlan,
    ) -> Result<PreparedEngineSnapshot, ShowLoadError> {
        if self.transaction.is_empty() {
            return Ok(self.runtime);
        }
        backup
            .create_migration(&self.store, self.source_revision)
            .map_err(ShowLoadError::Application)?;
        let candidate_revision = self.runtime.snapshot().revision;
        let committed = self
            .store
            .apply_portable_transaction(self.transaction)
            .map_err(ShowLoadError::Store)?;
        debug_assert_eq!(committed.revision().value(), candidate_revision);
        Ok(self.runtime)
    }
}

pub(super) fn prepare_show_load(
    entry: &ShowEntry,
    override_value: Option<(&str, &str, &serde_json::Value)>,
) -> Result<PreparedShowLoad, ShowLoadError> {
    let store = ActiveShowRepository::open(&entry.path).map_err(ShowLoadError::Store)?;
    let document = store.portable_document().map_err(ShowLoadError::Store)?;
    let source_revision = document.revision().value();
    let mut transaction = document.transaction();
    if entry.name == default_show::name() {
        default_show::stage_upgrade(&document, &mut transaction).map_err(ShowLoadError::Store)?;
    }
    if let Some((kind, id, body)) = override_value {
        transaction.put(kind, id, body.clone());
    }
    let prepared = light_application::prepare_show_candidate(&document, transaction)
        .map_err(ShowLoadError::Application)?;
    let (transaction, snapshot) = prepared.into_parts();
    let expected_revision = source_revision
        .checked_add(u64::from(!transaction.is_empty()))
        .ok_or_else(|| ShowLoadError::Invariant("portable show revision overflow".into()))?;
    if snapshot.revision != expected_revision {
        return Err(ShowLoadError::Invariant(format!(
            "compiled show revision {} differs from predicted revision {expected_revision}",
            snapshot.revision
        )));
    }
    Ok(PreparedShowLoad {
        store,
        source_revision,
        transaction,
        snapshot,
    })
}

/// Read-only compatibility helper for call sites that have not yet moved to ActiveShowService.
/// Startup and show activation use `prepare_show_for_runtime` so staged migrations are persisted.
pub(super) fn load_engine_snapshot(entry: &ShowEntry) -> Result<EngineSnapshot, String> {
    prepare_show_load(entry, None)
        .map(PreparedShowLoad::into_snapshot)
        .map_err(|error| error.to_string())
}

pub(super) fn load_engine_snapshot_with_override(
    entry: &ShowEntry,
    override_value: Option<(&str, &str, &serde_json::Value)>,
) -> Result<EngineSnapshot, ShowLoadError> {
    prepare_show_load(entry, override_value).map(PreparedShowLoad::into_snapshot)
}

/// Compiles and migrates a show before the short active-runtime installation boundary. Callers
/// serialize competing show-change workflows with `ActiveShowResource::acquire_show_change`.
pub(super) fn prepare_show_for_runtime(
    state: &AppState,
    entry: &ShowEntry,
) -> Result<PreparedEngineSnapshot, ApiError> {
    highlight_compatibility::require_review_for_show(state, entry)?;
    let backup = ShowMutationBackupPlan::migration(
        state.installation.data_dir(),
        entry,
        state.installation.configuration().backup_retention,
    );
    prepare_show_load(entry, None)
        .and_then(|prepared| {
            prepared.prepare_runtime(|snapshot| state.output.prepare_snapshot(snapshot))
        })
        .and_then(|prepared| prepared.commit_migration(&backup))
        .map_err(show_load_api_error)
}

pub(super) fn show_load_api_error(error: ShowLoadError) -> ApiError {
    match error {
        ShowLoadError::Store(error) => ApiError::store(error),
        ShowLoadError::Engine(error) => ApiError::bad_request(error.to_string()),
        ShowLoadError::Invariant(message) => ApiError::internal(message),
        ShowLoadError::Application(error) => match error.kind {
            ActionErrorKind::Invalid => ApiError::bad_request(error.message),
            ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
            ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
            ActionErrorKind::NotFound => ApiError::not_found(error.message),
            ActionErrorKind::Conflict | ActionErrorKind::Busy => ApiError::conflict(error.message),
            ActionErrorKind::Unavailable => ApiError::unavailable(error.message),
            ActionErrorKind::Internal => ApiError::internal(error.message),
        },
    }
}
