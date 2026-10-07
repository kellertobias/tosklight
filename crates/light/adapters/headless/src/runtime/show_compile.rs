use super::attribute_configuration::InstalledAttributeConfiguration;
use super::*;
use light_application::{ActionError, ActionErrorKind};
use light_engine::EngineError;
use light_show::{PortableShowDocument, PortableShowTransaction, StoreError};

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
///
/// `document` is the accepted source document the candidate was staged against. It is read once
/// and carried through commit so activation configuration never needs a second path read.
pub(super) struct PreparedShowLoad {
    store: ActiveShowRepository,
    source_revision: u64,
    document: PortableShowDocument,
    transaction: PortableShowTransaction,
    snapshot: EngineSnapshot,
}

pub(super) struct PreparedRuntimeShowLoad<T> {
    store: ActiveShowRepository,
    source_revision: u64,
    document: PortableShowDocument,
    transaction: PortableShowTransaction,
    candidate_revision: u64,
    runtime: T,
}

/// Show-owned configuration derived, before any activation effect, from the exact portable
/// document the installed snapshot was compiled from.
///
/// Derivation is pure: it reads neither the show file nor live desk state, and it keeps the
/// current passive policies. An invalid Attribute configuration becomes recommended defaults
/// with `validation_error` while the stored object is preserved; an undecodable PSN body becomes
/// the default (off) configuration and is reported through `PsnConfigurationOrigin`.
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))] // Installed by TL-584's owned activation workflow.
pub(super) struct PreparedShowConfiguration {
    show_id: light_core::ShowId,
    show_revision: u64,
    attributes: InstalledAttributeConfiguration,
    psn: super::psn_http::PreparedPsnConfiguration,
}

#[cfg_attr(not(test), allow(dead_code))] // Installed by TL-584's owned activation workflow.
impl PreparedShowConfiguration {
    pub(super) fn for_document(document: &PortableShowDocument) -> Self {
        Self {
            show_id: document.id(),
            show_revision: document.revision().value(),
            attributes: InstalledAttributeConfiguration::for_document(document),
            psn: super::psn_http::PreparedPsnConfiguration::for_document(document),
        }
    }

    pub(super) const fn show_id(&self) -> light_core::ShowId {
        self.show_id
    }

    pub(super) const fn show_revision(&self) -> u64 {
        self.show_revision
    }

    pub(super) const fn attributes(&self) -> &InstalledAttributeConfiguration {
        &self.attributes
    }

    pub(super) const fn psn(&self) -> &super::psn_http::PreparedPsnConfiguration {
        &self.psn
    }

    /// Every part names the same show and portable revision as the compiled snapshot.
    fn ensure_matches(
        &self,
        document: &PortableShowDocument,
        compiled_revision: u64,
    ) -> Result<(), ShowLoadError> {
        let document_revision = document.revision().value();
        let coherent = self.show_id == document.id()
            && self.show_revision == document_revision
            && document_revision == compiled_revision
            && self.attributes.show_id == Some(self.show_id)
            && self.attributes.show_revision == document_revision
            && self.psn.show_id == self.show_id
            && self.psn.show_revision == document_revision;
        if coherent {
            Ok(())
        } else {
            Err(ShowLoadError::Invariant(format!(
                "prepared show configuration for {} revision {document_revision} differs from \
                 the compiled snapshot revision {compiled_revision}",
                document.id().0
            )))
        }
    }

    /// Memory-only installation of the prepared values. It never reopens the show file, so a
    /// later file replacement or deletion cannot substitute another document or silent defaults.
    /// PSN ownership is always reset, including for an equal configuration or the same show ID.
    pub(super) fn install_memory_only(&self, state: &AppState) {
        state.attributes.install_prepared(self.attributes.clone());
        super::psn_http::install_prepared(state, &self.psn);
        state
            .output
            .engine()
            .set_color_model(self.attributes.configuration.color_model);
    }
}

/// A committed (or unchanged) show load: the prepared runtime, the exact portable document it was
/// compiled from and the configuration derived from that document. No live state was touched.
#[cfg_attr(not(test), allow(dead_code))] // Installed by TL-584's owned activation workflow.
pub(super) struct PreparedShowActivation<T> {
    runtime: T,
    document: PortableShowDocument,
    configuration: PreparedShowConfiguration,
}

#[cfg_attr(not(test), allow(dead_code))] // Installed by TL-584's owned activation workflow.
impl<T> PreparedShowActivation<T> {
    pub(super) const fn runtime(&self) -> &T {
        &self.runtime
    }

    pub(super) const fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    pub(super) const fn configuration(&self) -> &PreparedShowConfiguration {
        &self.configuration
    }

    pub(super) fn into_parts(self) -> (T, PortableShowDocument, PreparedShowConfiguration) {
        (self.runtime, self.document, self.configuration)
    }
}

impl PreparedShowLoad {
    pub(super) fn prepare_runtime<T>(
        self,
        prepare: impl FnOnce(EngineSnapshot) -> Result<T, EngineError>,
    ) -> Result<PreparedRuntimeShowLoad<T>, ShowLoadError> {
        let candidate_revision = self.snapshot.revision;
        let runtime = prepare(self.snapshot).map_err(ShowLoadError::Engine)?;
        Ok(PreparedRuntimeShowLoad {
            store: self.store,
            source_revision: self.source_revision,
            document: self.document,
            transaction: self.transaction,
            candidate_revision,
            runtime,
        })
    }

    fn into_snapshot(self) -> EngineSnapshot {
        self.snapshot
    }
}

impl<T> PreparedRuntimeShowLoad<T> {
    /// Compatibility wrapper for startup and unaffected callers: commits a staged migration and
    /// returns only the prepared runtime.
    pub(super) fn commit_migration(
        self,
        backup: &ShowMutationBackupPlan,
    ) -> Result<T, ShowLoadError> {
        let candidate_revision = self.candidate_revision;
        let (runtime, document) = self.commit_document(backup)?;
        debug_assert_eq!(document.revision().value(), candidate_revision);
        Ok(runtime)
    }

    /// Commits a staged migration and derives activation configuration from the exact compiled
    /// document: the accepted source document when nothing migrated, otherwise that document
    /// advanced by the transaction's own committed changes. Nothing rereads the show file.
    ///
    /// An identity/revision mismatch is reported before any activation effect. After a migration
    /// commit that report necessarily follows persistence; no post-commit atomicity is claimed.
    #[cfg_attr(not(test), allow(dead_code))] // Called by TL-584's owned activation workflow.
    pub(super) fn commit_activation(
        self,
        backup: &ShowMutationBackupPlan,
    ) -> Result<PreparedShowActivation<T>, ShowLoadError> {
        let candidate_revision = self.candidate_revision;
        let (runtime, document) = self.commit_document(backup)?;
        let configuration = PreparedShowConfiguration::for_document(&document);
        configuration.ensure_matches(&document, candidate_revision)?;
        Ok(PreparedShowActivation {
            runtime,
            document,
            configuration,
        })
    }

    fn commit_document(
        self,
        backup: &ShowMutationBackupPlan,
    ) -> Result<(T, PortableShowDocument), ShowLoadError> {
        let Self {
            store,
            source_revision,
            mut document,
            transaction,
            candidate_revision: _,
            runtime,
        } = self;
        if transaction.is_empty() {
            return Ok((runtime, document));
        }
        backup
            .create_migration(&store, source_revision)
            .map_err(ShowLoadError::Application)?;
        let committed = store
            .apply_portable_transaction(transaction)
            .map_err(ShowLoadError::Store)?;
        document.apply_commit(&committed);
        Ok((runtime, document))
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
        document,
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
) -> Result<PreparedOutputSnapshot, ApiError> {
    super::show_programming_contract::require_for_show(state, entry)?;
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

/// Activation-bundle sibling of `prepare_show_for_runtime` for TL-584. It performs the same
/// review gate, compilation, output preparation and migration commit, and additionally returns
/// the exact compiled document with its prepared Attribute and PSN configuration. It installs
/// nothing; the caller installs the runtime and `install_memory_only` inside its own boundary.
#[cfg_attr(not(test), allow(dead_code))] // Called by TL-584's owned activation workflow.
pub(super) fn prepare_show_activation_for_runtime(
    state: &AppState,
    entry: &ShowEntry,
) -> Result<PreparedShowActivation<PreparedOutputSnapshot>, ApiError> {
    super::show_programming_contract::require_for_show(state, entry)?;
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
        .and_then(|prepared| prepared.commit_activation(&backup))
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
