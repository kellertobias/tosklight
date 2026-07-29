//! Capability-owned runtime resources.
//!
//! `AppState` is the Axum composition graph. Mutable state and lifecycle handles live in these
//! capability resources so adapters can migrate to narrow command/query methods without adding
//! another field or lock to the composition root.

use super::*;

#[derive(Clone)]
pub(super) struct InstallationResource {
    desk: Arc<Mutex<DeskStore>>,
    fixture_library: Arc<Mutex<light_fixture::FixtureLibrary>>,
    data_dir: PathBuf,
    configuration: Arc<RwLock<DeskConfiguration>>,
    desk_token: Option<Arc<str>>,
}

impl InstallationResource {
    pub(super) fn open_fixture_library_for_startup(
        data_dir: &std::path::Path,
        fixture_package_dir: Option<&std::path::Path>,
    ) -> Result<light_fixture::FixtureLibrary, light_fixture::FixtureError> {
        tracing::info!(path=%data_dir.join("fixtures.sqlite").display(), "opening fixture library");
        let library = light_fixture::FixtureLibrary::open(data_dir.join("fixtures.sqlite"))?;
        if let Some(path) = fixture_package_dir {
            let report = library.load_fixture_package_directory(path)?;
            tracing::info!(
                path = %path.display(),
                installed = report.installed,
                updated = report.updated,
                unchanged = report.unchanged,
                preserved_operator_revisions = report.preserved_operator_revisions,
                "loaded transferable fixture packages"
            );
        }
        for warning in library.migration_warnings()? {
            tracing::warn!(%warning, "fixture library migration requires operator attention");
        }
        tracing::info!("fixture library ready");
        Ok(library)
    }

    pub(super) fn new(
        desk: DeskStore,
        fixture_library: light_fixture::FixtureLibrary,
        data_dir: PathBuf,
        configuration: DeskConfiguration,
        desk_token: Option<Arc<str>>,
    ) -> Self {
        Self {
            desk: Arc::new(Mutex::new(desk)),
            fixture_library: Arc::new(Mutex::new(fixture_library)),
            data_dir,
            configuration: Arc::new(RwLock::new(configuration)),
            desk_token,
        }
    }

    #[cfg(test)]
    pub(super) fn open_test_installation(data_dir: PathBuf) -> anyhow::Result<Self> {
        let desk = DeskStore::open(data_dir.join("desk.sqlite"))?;
        let fixture_library =
            light_fixture::FixtureLibrary::open(data_dir.join("fixtures.sqlite"))?;
        Ok(Self::new(
            desk,
            fixture_library,
            data_dir,
            DeskConfiguration::default(),
            None,
        ))
    }

    pub(super) fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    pub(super) fn configuration(&self) -> DeskConfiguration {
        self.configuration.read().clone()
    }

    pub(super) fn replace_configuration(&self, configuration: DeskConfiguration) {
        *self.configuration.write() = configuration;
    }

    pub(super) fn update_configuration<R>(
        &self,
        update: impl FnOnce(&mut DeskConfiguration) -> R,
    ) -> R {
        update(&mut self.configuration.write())
    }

    pub(super) fn desk_token_matches(&self, candidate: Option<&str>) -> bool {
        self.desk_token
            .as_deref()
            .is_none_or(|required| candidate == Some(required))
    }

    #[cfg(test)]
    pub(super) fn set_desk_token(&mut self, token: impl Into<Arc<str>>) {
        self.desk_token = Some(token.into());
    }

    pub(super) fn show_library(&self) -> Result<Vec<ShowEntry>, light_show::StoreError> {
        self.desk.lock().library()
    }

    pub(super) fn show(
        &self,
        id: light_core::ShowId,
    ) -> Result<Option<ShowEntry>, light_show::StoreError> {
        self.desk.lock().show(id)
    }

    pub(super) fn upsert_show(
        &self,
        name: &str,
        path: &str,
        overwrite: bool,
    ) -> Result<ShowEntry, light_show::StoreError> {
        self.desk.lock().upsert_show(name, path, overwrite)
    }

    pub(super) fn upsert_show_with_revision_copy(
        &self,
        name: &str,
        path: &str,
        overwrite: bool,
        revision_copy: Option<&RevisionCopySource>,
    ) -> Result<ShowEntry, light_show::StoreError> {
        self.desk
            .lock()
            .upsert_show_with_revision_copy(name, path, overwrite, revision_copy)
    }

    pub(super) fn mark_show_updated(
        &self,
        id: light_core::ShowId,
    ) -> Result<ShowEntry, light_show::StoreError> {
        self.desk.lock().mark_show_updated(id)
    }

    pub(super) fn rename_show(
        &self,
        id: light_core::ShowId,
        name: &str,
        path: &str,
    ) -> Result<ShowEntry, light_show::StoreError> {
        self.desk.lock().rename_show(id, name, path)
    }

    pub(super) fn remove_show(
        &self,
        id: light_core::ShowId,
    ) -> Result<bool, light_show::StoreError> {
        self.desk.lock().remove_show(id)
    }

    pub(super) fn show_revisions(
        &self,
        show_id: light_core::ShowId,
    ) -> Result<Vec<ShowRevision>, light_show::StoreError> {
        self.desk.lock().show_revisions(show_id)
    }

    pub(super) fn show_revision(
        &self,
        show_id: light_core::ShowId,
        revision: light_core::Revision,
    ) -> Result<Option<ShowRevision>, light_show::StoreError> {
        self.desk.lock().show_revision(show_id, revision)
    }

    pub(super) fn add_show_revision(
        &self,
        show_id: light_core::ShowId,
        name: &str,
        path: &str,
    ) -> Result<ShowRevision, light_show::StoreError> {
        self.desk.lock().add_show_revision(show_id, name, path)
    }

    pub(super) fn active_show(&self) -> Result<Option<ShowEntry>, light_show::StoreError> {
        self.desk.lock().active_show()
    }

    pub(super) fn set_active_show(
        &self,
        id: Option<light_core::ShowId>,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().set_active_show(id)
    }

    pub(super) fn setting(&self, key: &str) -> Result<Option<String>, light_show::StoreError> {
        self.desk.lock().setting(key)
    }

    pub(super) fn set_setting(&self, key: &str, value: &str) -> Result<(), light_show::StoreError> {
        self.desk.lock().set_setting(key, value)
    }

    pub(super) fn users(&self) -> Result<Vec<DeskUser>, light_show::StoreError> {
        self.desk.lock().users()
    }

    pub(super) fn add_user(&self, name: &str) -> Result<DeskUser, light_show::StoreError> {
        self.desk.lock().add_user(name)
    }

    pub(super) fn find_user(&self, name: &str) -> Result<Option<DeskUser>, light_show::StoreError> {
        self.desk.lock().find_user(name)
    }

    pub(super) fn update_user(
        &self,
        id: light_core::UserId,
        name: &str,
        enabled: bool,
    ) -> Result<DeskUser, light_show::StoreError> {
        self.desk.lock().update_user(id, name, enabled)
    }

    pub(super) fn save_session(
        &self,
        session: &PersistedSession,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().save_session(session)
    }

    pub(super) fn persisted_sessions(
        &self,
    ) -> Result<Vec<PersistedSession>, light_show::StoreError> {
        self.desk.lock().persisted_sessions()
    }

    pub(super) fn delete_session(&self, id: SessionId) -> Result<bool, light_show::StoreError> {
        self.desk.lock().delete_session(id)
    }

    pub(super) fn desks(&self) -> Result<Vec<ControlDesk>, light_show::StoreError> {
        self.desk.lock().desks()
    }

    pub(super) fn control_desk(
        &self,
        id: Uuid,
    ) -> Result<Option<ControlDesk>, light_show::StoreError> {
        self.desk.lock().control_desk(id)
    }

    pub(super) fn control_desk_by_alias(
        &self,
        alias: &str,
    ) -> Result<Option<ControlDesk>, light_show::StoreError> {
        self.desk.lock().control_desk_by_alias(alias)
    }

    pub(super) fn client_desks(
        &self,
    ) -> Result<Vec<light_show::ClientDesk>, light_show::StoreError> {
        self.desk.lock().client_desks()
    }

    pub(super) fn resolve_client_desk(
        &self,
        client_id: Uuid,
        remembered_desk_id: Option<Uuid>,
    ) -> Result<ControlDesk, light_show::StoreError> {
        self.desk
            .lock()
            .resolve_client_desk(client_id, remembered_desk_id)
    }

    pub(super) fn touch_client(&self, client_id: Uuid) -> Result<(), light_show::StoreError> {
        self.desk.lock().touch_client(client_id)
    }

    pub(super) fn remove_client_desk(&self, desk_id: Uuid) -> Result<bool, light_show::StoreError> {
        self.desk.lock().remove_client_desk(desk_id)
    }

    pub(super) fn add_desk(
        &self,
        name: &str,
        alias: &str,
    ) -> Result<ControlDesk, light_show::StoreError> {
        self.desk.lock().add_desk(name, alias)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_desk(
        &self,
        id: Uuid,
        name: &str,
        alias: &str,
        columns: u8,
        rows: u8,
        buttons: u8,
        playback_layout: Option<light_show::PlaybackSurfaceLayout>,
    ) -> Result<ControlDesk, light_show::StoreError> {
        self.desk
            .lock()
            .update_desk(id, name, alias, columns, rows, buttons, playback_layout)
    }

    pub(super) fn desk_page(
        &self,
        desk_id: Uuid,
        show_id: light_core::ShowId,
    ) -> Result<u8, light_show::StoreError> {
        self.desk.lock().desk_page(desk_id, show_id)
    }

    pub(super) fn set_desk_page(
        &self,
        desk_id: Uuid,
        show_id: light_core::ShowId,
        page: u8,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().set_desk_page(desk_id, show_id, page)
    }

    pub(super) fn selected_playback(
        &self,
        desk_id: Uuid,
        show_id: light_core::ShowId,
    ) -> Result<Option<u16>, light_show::StoreError> {
        self.desk.lock().selected_playback(desk_id, show_id)
    }

    pub(super) fn set_selected_playback(
        &self,
        desk_id: Uuid,
        show_id: light_core::ShowId,
        playback: Option<u16>,
    ) -> Result<(), light_show::StoreError> {
        self.desk
            .lock()
            .set_selected_playback(desk_id, show_id, playback)
    }

    pub(super) fn put_screen(
        &self,
        screen: ScreenConfiguration,
    ) -> Result<ScreenConfiguration, light_show::StoreError> {
        self.desk.lock().put_screen(screen)
    }

    pub(super) fn screens(&self) -> Result<Vec<ScreenConfiguration>, light_show::StoreError> {
        self.desk.lock().screens()
    }

    pub(super) fn screen(
        &self,
        id: Uuid,
    ) -> Result<Option<ScreenConfiguration>, light_show::StoreError> {
        self.desk.lock().screen(id)
    }

    pub(super) fn delete_screen(&self, id: Uuid) -> Result<(), light_show::StoreError> {
        self.desk.lock().delete_screen(id)
    }

    pub(super) fn screen_page(
        &self,
        screen_id: Uuid,
        show_id: light_core::ShowId,
    ) -> Result<u8, light_show::StoreError> {
        self.desk.lock().screen_page(screen_id, show_id)
    }

    pub(super) fn set_screen_page(
        &self,
        screen_id: Uuid,
        show_id: light_core::ShowId,
        page: u8,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().set_screen_page(screen_id, show_id, page)
    }

    pub(super) fn bootstrap_desk_data(
        &self,
    ) -> (Vec<DeskUser>, Vec<ControlDesk>, Vec<light_show::ClientDesk>) {
        let desk = self.desk.lock();
        (
            desk.users().unwrap_or_default(),
            desk.desks().unwrap_or_default(),
            desk.client_desks().unwrap_or_default(),
        )
    }

    #[cfg(test)]
    pub(super) fn ensure_default_show_available(&self) -> anyhow::Result<ShowEntry> {
        startup_state::ensure_default_show_available(&self.desk.lock(), &self.data_dir)
    }

    #[cfg(test)]
    pub(super) fn replace_desk_store(&self, desk: DeskStore) {
        *self.desk.lock() = desk;
    }

    pub(super) fn fixture_definitions(
        &self,
    ) -> Result<Vec<light_fixture::FixtureDefinition>, light_fixture::FixtureError> {
        self.fixture_library.lock().definitions()
    }

    pub(super) fn fixture_profiles(
        &self,
    ) -> Result<Vec<light_fixture::FixtureProfile>, light_fixture::FixtureError> {
        self.fixture_library.lock().profiles()
    }

    pub(super) fn fixture_library_warnings(
        &self,
    ) -> Result<Vec<String>, light_fixture::FixtureError> {
        self.fixture_library.lock().migration_warnings()
    }

    pub(super) fn fixture_profile(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<light_fixture::FixtureProfile>, light_fixture::FixtureError> {
        self.fixture_library.lock().profile(id, revision)
    }

    pub(super) fn fixture_profile_source_gdtf(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, light_fixture::FixtureError> {
        self.fixture_library
            .lock()
            .profile_source_gdtf(id, revision)
    }

    pub(super) fn fixture_profile_revisions(
        &self,
        id: light_core::FixtureId,
    ) -> Result<Vec<light_fixture::FixtureProfile>, light_fixture::FixtureError> {
        let library = self.fixture_library.lock();
        library
            .profile_revisions(id)?
            .into_iter()
            .map(|revision| {
                library.profile(id, revision)?.ok_or_else(|| {
                    light_fixture::FixtureError::Invalid(format!(
                        "fixture profile {id:?} revision {revision} disappeared"
                    ))
                })
            })
            .collect()
    }

    pub(super) fn save_fixture_profile(
        &self,
        profile: light_fixture::FixtureProfile,
        expected_revision: u32,
    ) -> Result<light_fixture::FixtureProfile, light_fixture::FixtureError> {
        self.fixture_library
            .lock()
            .save_profile(profile, expected_revision)
    }

    pub(super) fn delete_fixture_profile(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<bool, light_fixture::FixtureError> {
        self.fixture_library.lock().delete_profile(id, revision)
    }

    pub(super) fn import_fixture_package(
        &self,
        package: &[u8],
    ) -> Result<light_fixture::FixtureProfile, light_fixture::FixtureError> {
        self.fixture_library.lock().import_fixture_package(package)
    }

    pub(super) fn attach_fixture_profile_gdtf(
        &self,
        id: light_core::FixtureId,
        revision: u32,
        source: &[u8],
    ) -> Result<bool, light_fixture::FixtureError> {
        self.fixture_library
            .lock()
            .set_profile_source_gdtf(id, revision, source)
    }

    pub(super) fn import_fixture_definition(
        &self,
        definition: &light_fixture::FixtureDefinition,
    ) -> Result<light_fixture::FixtureDefinition, light_fixture::FixtureError> {
        let json = serde_json::to_string(definition)?;
        self.fixture_library.lock().import_json(&json)
    }

    pub(super) fn import_fixture_definition_with_source(
        &self,
        definition: &light_fixture::FixtureDefinition,
        source: &[u8],
    ) -> Result<light_fixture::FixtureDefinition, light_fixture::FixtureError> {
        let json = serde_json::to_string(definition)?;
        self.fixture_library
            .lock()
            .import_json_with_source(&json, Some(source))
    }

    pub(super) fn delete_fixture_definition(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<bool, light_fixture::FixtureError> {
        self.fixture_library.lock().delete(id, revision)
    }

    pub(super) fn export_fixture_package(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<(light_fixture::FixtureProfile, Vec<u8>)>, light_fixture::FixtureError> {
        let library = self.fixture_library.lock();
        let Some(profile) = library.profile(id, revision)? else {
            return Ok(None);
        };
        let Some(package) = library.export_fixture_package(id, revision)? else {
            return Ok(None);
        };
        Ok(Some((profile, package)))
    }

    pub(super) fn fixture_source_gdtf(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, light_fixture::FixtureError> {
        self.fixture_library.lock().source_gdtf(id, revision)
    }

    pub(super) fn fixture_profile_revision_document(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<serde_json::Value>, light_fixture::FixtureError> {
        self.fixture_library
            .lock()
            .profile_revision_document(id, revision)
    }
}

#[derive(Clone)]
pub(super) struct SessionResource {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    session_clients: Arc<RwLock<HashMap<SessionId, Uuid>>>,
    file_input_contexts: Arc<Mutex<HashMap<Uuid, file_manager::FileInputContext>>>,
}

pub(crate) enum SessionFileInputRoute {
    Unclaimed,
    Claimed,
    Dispatch(file_manager::FileInputContext),
}

impl SessionResource {
    pub(super) fn new() -> Self {
        Self {
            sessions: Arc::default(),
            session_clients: Arc::default(),
            file_input_contexts: Arc::default(),
        }
    }

    pub(super) fn session(&self, id: SessionId) -> Option<Session> {
        self.sessions.read().get(&id).cloned()
    }

    pub(super) fn session_for_token(&self, token: &str) -> Option<Session> {
        self.sessions
            .read()
            .values()
            .find(|session| session.token == token)
            .cloned()
    }

    pub(super) fn session_token_matches(&self, id: SessionId, token: &str) -> bool {
        self.sessions
            .read()
            .get(&id)
            .is_some_and(|session| session.token == token)
    }

    pub(super) fn sessions(&self) -> Vec<Session> {
        self.sessions.read().values().cloned().collect()
    }

    pub(super) fn insert_session(&self, session: Session) -> Option<Session> {
        self.sessions.write().insert(session.id, session)
    }

    pub(super) fn remove_session(&self, id: SessionId) -> Option<Session> {
        self.sessions.write().remove(&id)
    }

    #[cfg(test)]
    pub(super) fn contains_session(&self, id: SessionId) -> bool {
        self.sessions.read().contains_key(&id)
    }

    #[cfg(test)]
    pub(super) fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    pub(super) fn update_desk_sessions(&self, desk: &ControlDesk) {
        for session in self
            .sessions
            .write()
            .values_mut()
            .filter(|session| session.desk.id == desk.id)
        {
            session.desk = desk.clone();
        }
    }

    #[cfg(test)]
    pub(super) fn update_session_desk(&self, session_id: SessionId, desk: ControlDesk) -> bool {
        let mut sessions = self.sessions.write();
        let Some(session) = sessions.get_mut(&session_id) else {
            return false;
        };
        session.desk = desk;
        true
    }

    pub(super) fn bind_client(&self, session_id: SessionId, client_id: Uuid) {
        self.session_clients.write().insert(session_id, client_id);
    }

    pub(super) fn unbind_client(&self, session_id: SessionId) -> Option<Uuid> {
        self.session_clients.write().remove(&session_id)
    }

    pub(super) fn client_id(&self, session_id: SessionId) -> Option<Uuid> {
        self.session_clients.read().get(&session_id).copied()
    }

    pub(super) fn has_bound_client(&self, session_id: SessionId) -> bool {
        self.session_clients.read().contains_key(&session_id)
    }

    pub(super) fn client_connected(&self, client_id: Uuid) -> bool {
        let sessions = self.sessions.read();
        let clients = self.session_clients.read();
        sessions
            .keys()
            .any(|session_id| clients.get(session_id) == Some(&client_id))
    }

    pub(super) fn desk_in_use(&self, desk_id: Uuid) -> bool {
        self.sessions
            .read()
            .values()
            .any(|session| session.desk.id == desk_id)
    }

    pub(super) fn client_or_desk_in_use(&self, client_id: Uuid, desk_id: Uuid) -> bool {
        self.client_connected(client_id) || self.desk_in_use(desk_id)
    }

    pub(super) fn same_context_connected(&self, session: &Session) -> bool {
        self.sessions.read().values().any(|candidate| {
            candidate.user.id == session.user.id && candidate.desk.id == session.desk.id
        })
    }

    pub(crate) fn prune_file_input_contexts(&self, now: std::time::Instant) {
        self.file_input_contexts
            .lock()
            .retain(|_, context| context.expires_at > now);
    }

    pub(crate) fn try_claim_file_input_context(
        &self,
        context: file_manager::FileInputContext,
        prepare: impl FnOnce() -> Result<(), ApiError>,
    ) -> Result<(), ApiError> {
        let mut contexts = self.file_input_contexts.lock();
        contexts.retain(|_, current| current.expires_at > std::time::Instant::now());
        if let Some(existing) = contexts.get(&context.desk_id)
            && existing.instance_id != context.instance_id
        {
            return Err(ApiError::conflict(
                "another File Manager instance owns this session's file input context",
            ));
        }
        prepare()?;
        contexts.insert(context.desk_id, context);
        Ok(())
    }

    pub(crate) fn file_input_context(
        &self,
        desk_id: Uuid,
    ) -> Option<file_manager::FileInputContext> {
        self.file_input_contexts.lock().get(&desk_id).cloned()
    }

    pub(crate) fn release_file_input_context(
        &self,
        desk_id: Uuid,
        instance_id: Option<&str>,
    ) -> Option<file_manager::FileInputContext> {
        let mut contexts = self.file_input_contexts.lock();
        let matches = contexts.get(&desk_id).is_some_and(|context| {
            instance_id.is_none_or(|instance_id| instance_id == context.instance_id)
        });
        matches.then(|| contexts.remove(&desk_id)).flatten()
    }

    pub(crate) fn route_file_input(
        &self,
        desk_id: Uuid,
        action: &str,
        expires_at: std::time::Instant,
    ) -> SessionFileInputRoute {
        let mut contexts = self.file_input_contexts.lock();
        let Some(context) = contexts.get_mut(&desk_id) else {
            return SessionFileInputRoute::Unclaimed;
        };
        if context.desk_id != desk_id {
            return SessionFileInputRoute::Unclaimed;
        }
        context.expires_at = expires_at;
        if !matches!(action, "enter" | "escape" | "esc") {
            return SessionFileInputRoute::Claimed;
        }
        let context = context.clone();
        if matches!(action, "escape" | "esc") {
            contexts.remove(&desk_id);
        }
        SessionFileInputRoute::Dispatch(context)
    }

    pub(crate) fn release_session_file_input(
        &self,
        session: &Session,
    ) -> Option<file_manager::FileInputContext> {
        let mut contexts = self.file_input_contexts.lock();
        let owned = contexts
            .get(&session.desk.id)
            .is_some_and(|context| context.session_id == session.id);
        owned.then(|| contexts.remove(&session.desk.id)).flatten()
    }

    #[cfg(test)]
    pub(super) fn file_input_context_count(&self) -> usize {
        self.file_input_contexts.lock().len()
    }
}

#[derive(Clone)]
pub(super) struct ProgrammingResource {
    programmers: ProgrammerRegistry,
    service: ProgrammingService,
    command_history: Arc<Mutex<HashMap<Uuid, VecDeque<CommandHistoryEntry>>>>,
}

impl ProgrammingResource {
    pub(super) fn new(programmers: ProgrammerRegistry, service: ProgrammingService) -> Self {
        Self {
            programmers,
            service,
            command_history: Arc::default(),
        }
    }

    pub(super) fn programmers(&self) -> ProgrammerRegistry {
        self.programmers.clone()
    }

    pub(super) fn record_command_history(&self, entry: CommandHistoryEntry, limit: usize) {
        let mut histories = self.command_history.lock();
        let history = histories.entry(entry.desk_id).or_default();
        history.push_front(entry);
        history.truncate(limit);
    }

    pub(super) fn command_history(&self, desk_id: Uuid) -> Vec<CommandHistoryEntry> {
        self.command_history
            .lock()
            .get(&desk_id)
            .map(|history| history.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub(super) fn run_desk_operation<T>(&self, desk_id: Uuid, operation: impl FnOnce() -> T) -> T {
        self.service.run_desk_operation(desk_id, operation)
    }

    pub(super) fn with_staged_command<T, E>(
        &self,
        session_id: SessionId,
        operation: impl FnOnce(&Self) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<String>,
    {
        self.programmers
            .with_staged_command(session_id, |staged_programmers| {
                let staged = Self {
                    programmers: staged_programmers.clone(),
                    service: self.service.clone(),
                    command_history: Arc::clone(&self.command_history),
                };
                operation(&staged)
            })
    }

    pub(super) fn with_transaction<T, E>(
        &self,
        session_id: SessionId,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        self.programmers.with_transaction(session_id, operation)
    }

    pub(super) fn start(
        &self,
        session_id: SessionId,
        user_id: light_core::UserId,
    ) -> light_programmer::ProgrammerState {
        self.programmers.start(session_id, user_id)
    }

    pub(super) fn restore(&self, state: light_programmer::ProgrammerState) {
        self.programmers.restore(state);
    }

    pub(super) fn disconnect(&self, session_id: SessionId) {
        self.programmers.disconnect(session_id);
    }

    pub(super) fn reset_all(&self) {
        self.programmers.reset_all();
    }

    pub(super) fn get(&self, session_id: SessionId) -> Option<light_programmer::ProgrammerState> {
        self.programmers.get(session_id)
    }

    pub(super) fn selection(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::ProgrammerSelection> {
        self.programmers.selection(session_id)
    }

    pub(super) fn active(&self) -> Vec<light_programmer::ProgrammerState> {
        self.programmers.active()
    }

    pub(super) fn active_for_sessions(&self) -> Vec<light_programmer::ProgrammerState> {
        self.programmers.active_for_sessions()
    }

    pub(super) fn active_for_user_sessions(
        &self,
        user_id: light_core::UserId,
    ) -> Vec<light_programmer::ProgrammerState> {
        self.programmers.active_for_user_sessions(user_id)
    }

    pub(super) fn select(
        &self,
        session_id: SessionId,
        fixtures: impl IntoIterator<Item = light_core::FixtureId>,
    ) -> u64 {
        self.programmers.select(session_id, fixtures)
    }

    pub(super) fn select_expression(
        &self,
        session_id: SessionId,
        fixtures: Vec<light_core::FixtureId>,
        expression: light_programmer::SelectionExpression,
    ) -> u64 {
        self.programmers
            .select_expression(session_id, fixtures, expression)
    }

    pub(super) fn apply_selection_gesture(
        &self,
        session_id: SessionId,
        references: Vec<light_programmer::SelectionReference>,
        groups: &HashMap<String, light_programmer::GroupDefinition>,
    ) -> bool {
        self.programmers
            .apply_selection_gesture(session_id, references, groups)
    }

    pub(super) fn finish_selection_gesture(&self, session_id: SessionId) -> bool {
        self.programmers.finish_selection_gesture(session_id)
    }

    pub(super) fn set(
        &self,
        session_id: SessionId,
        fixture_id: light_core::FixtureId,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
    ) {
        self.programmers
            .set(session_id, fixture_id, attribute, value);
    }

    pub(super) fn set_many(
        &self,
        session_id: SessionId,
        assignments: impl IntoIterator<
            Item = (
                light_core::FixtureId,
                light_core::AttributeKey,
                light_core::AttributeValue,
            ),
        >,
    ) {
        self.programmers.set_many(session_id, assignments);
    }

    pub(super) fn set_many_faded_with_timing(
        &self,
        session_id: SessionId,
        assignments: impl IntoIterator<
            Item = (
                light_core::FixtureId,
                light_core::AttributeKey,
                light_core::AttributeValue,
            ),
        >,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) {
        self.programmers.set_many_faded_with_timing(
            session_id,
            assignments,
            fade_millis,
            delay_millis,
        );
    }

    pub(super) fn set_many_immediate_with_delay(
        &self,
        session_id: SessionId,
        assignments: impl IntoIterator<
            Item = (
                light_core::FixtureId,
                light_core::AttributeKey,
                light_core::AttributeValue,
            ),
        >,
        delay_millis: Option<u64>,
    ) {
        self.programmers
            .set_many_immediate_with_delay(session_id, assignments, delay_millis);
    }

    pub(super) fn set_faded_with_timing(
        &self,
        session_id: SessionId,
        fixture_id: light_core::FixtureId,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) {
        self.programmers.set_faded_with_timing(
            session_id,
            fixture_id,
            attribute,
            value,
            fade_millis,
            delay_millis,
        );
    }

    pub(super) fn set_group(
        &self,
        session_id: SessionId,
        group_id: String,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
    ) -> bool {
        self.programmers
            .set_group(session_id, group_id, attribute, value)
    }

    pub(super) fn set_group_faded_with_timing(
        &self,
        session_id: SessionId,
        group_id: String,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) -> bool {
        self.programmers.set_group_faded_with_timing(
            session_id,
            group_id,
            attribute,
            value,
            fade_millis,
            delay_millis,
        )
    }

    pub(super) fn set_group_immediate_with_delay(
        &self,
        session_id: SessionId,
        group_id: String,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
        delay_millis: Option<u64>,
    ) -> bool {
        self.programmers.set_group_immediate_with_delay(
            session_id,
            group_id,
            attribute,
            value,
            delay_millis,
        )
    }

    pub(super) fn set_transient_action(
        &self,
        session_id: SessionId,
        source: String,
        assignments: impl IntoIterator<
            Item = (
                light_core::FixtureId,
                light_core::AttributeKey,
                light_core::AttributeValue,
            ),
        >,
    ) -> Option<u64> {
        self.programmers
            .set_transient_action(session_id, source, assignments)
    }

    pub(super) fn release_transient_action(
        &self,
        session_id: SessionId,
        source: &str,
        generation: Option<u64>,
    ) -> bool {
        self.programmers
            .release_transient_action(session_id, source, generation)
    }

    pub(super) fn set_modes(
        &self,
        session_id: SessionId,
        blind: Option<bool>,
        preview: Option<bool>,
        highlight: Option<bool>,
        active_context: Option<Option<String>>,
    ) -> bool {
        self.programmers
            .set_modes(session_id, blind, preview, highlight, active_context)
    }

    pub(super) fn clear(&self, session_id: SessionId) -> bool {
        self.programmers.clear(session_id)
    }

    pub(super) fn clear_normal_values(&self, session_id: SessionId) -> bool {
        self.programmers.clear_normal_values(session_id)
    }

    pub(super) fn undo(&self, session_id: SessionId) -> bool {
        self.programmers.undo(session_id)
    }

    pub(super) fn redo(&self, session_id: SessionId) -> bool {
        self.programmers.redo(session_id)
    }

    pub(super) fn clock(&self) -> light_core::SharedClock {
        self.programmers.clock()
    }

    pub(super) fn interaction_version(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::ProgrammerInteractionVersion> {
        self.programmers.interaction_version(session_id)
    }

    pub(super) fn command_line_state(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::CommandLineState> {
        self.programmers.command_line_state(session_id)
    }

    pub(super) fn set_command_line(&self, session_id: SessionId, command_line: String) -> bool {
        self.programmers.set_command_line(session_id, command_line)
    }

    pub(super) fn update_command_line<F>(
        &self,
        session_id: SessionId,
        update: F,
    ) -> Option<light_programmer::CommandLineState>
    where
        F: FnOnce(
            &light_programmer::CommandLineState,
        ) -> (String, light_programmer::CommandTarget, bool),
    {
        self.programmers.update_command_line(session_id, update)
    }

    pub(super) fn set_command_target(&self, session_id: SessionId, target: String) -> bool {
        self.programmers.set_command_target(session_id, target)
    }

    pub(super) fn command_target(&self, session_id: SessionId) -> String {
        self.programmers.command_target(session_id)
    }

    pub(super) fn complete_command_execution(
        &self,
        session_id: SessionId,
        final_text: Option<&str>,
        pending_choice: Option<light_programmer::PendingCommandChoice>,
    ) -> Option<light_programmer::CommandLineState> {
        self.programmers
            .complete_command_execution(session_id, final_text, pending_choice)
    }

    pub(super) fn has_pending_command_choices_except_context(
        &self,
        excluded: Option<SessionId>,
    ) -> bool {
        self.programmers
            .has_pending_command_choices_except_context(excluded)
    }

    pub(super) fn clear_pending_command_choices_except_context(
        &self,
        excluded: Option<SessionId>,
    ) -> usize {
        self.programmers
            .clear_pending_command_choices_except_context(excluded)
    }

    pub(super) fn attach_command_context(&self, session_id: SessionId, context: SessionId) -> bool {
        self.programmers.attach_command_context(session_id, context)
    }

    pub(super) fn finish_selection_gesture_within_interaction(
        &self,
        session_id: SessionId,
    ) -> bool {
        self.programmers
            .finish_selection_gesture_within_interaction(session_id)
    }

    pub(super) fn activate_preload(&self, session_id: SessionId) -> bool {
        self.programmers.activate_preload(session_id)
    }

    pub(super) fn activate_preload_at(
        &self,
        session_id: SessionId,
        committed_at: chrono::DateTime<chrono::Utc>,
    ) -> bool {
        self.programmers
            .activate_preload_at(session_id, committed_at)
    }

    pub(super) fn release_preload(&self, session_id: SessionId) -> bool {
        self.programmers.release_preload(session_id)
    }

    pub(super) fn arm_preload(&self, session_id: SessionId, capture_programmer: bool) -> bool {
        self.programmers.arm_preload(session_id, capture_programmer)
    }

    pub(super) fn set_preload_group(
        &self,
        session_id: SessionId,
        group_id: String,
        attribute: light_core::AttributeKey,
        value: light_core::AttributeValue,
    ) -> bool {
        self.programmers
            .set_preload_group(session_id, group_id, attribute, value)
    }

    pub(super) fn queue_preload_playback_action(
        &self,
        session_id: SessionId,
        playback_number: u16,
        page: Option<u8>,
        action: light_programmer::PreloadPlaybackQueueAction,
        surface: light_programmer::PreloadPlaybackQueueSurface,
    ) -> bool {
        self.programmers.queue_preload_playback_action(
            session_id,
            playback_number,
            page,
            action,
            surface,
        )
    }

    pub(super) fn queue_preload_playback_action_with_origin(
        &self,
        session_id: SessionId,
        playback_number: u16,
        page: Option<u8>,
        action: light_programmer::PreloadPlaybackQueueAction,
        surface: light_programmer::PreloadPlaybackQueueSurface,
        origin_desk_id: Option<Uuid>,
    ) -> bool {
        self.programmers.queue_preload_playback_action_with_origin(
            session_id,
            playback_number,
            page,
            action,
            surface,
            origin_desk_id,
        )
    }

    pub(super) fn preload_playback_actions(
        &self,
        session_id: SessionId,
    ) -> Option<Vec<light_programmer::PreloadPlaybackAction>> {
        self.programmers.preload_playback_actions(session_id)
    }

    pub(super) fn take_preload_playback_actions(
        &self,
        session_id: SessionId,
    ) -> Vec<light_programmer::PreloadPlaybackAction> {
        self.programmers.take_preload_playback_actions(session_id)
    }

    pub(super) fn capture_mode(
        &self,
        session_id: SessionId,
    ) -> Option<light_programmer::ProgrammerCaptureMode> {
        self.programmers.capture_mode(session_id)
    }

    pub(super) fn normal_values_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.normal_values_revision(user_id)
    }

    pub(super) fn preload_values_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.preload_values_revision(user_id)
    }

    pub(super) fn preload_playback_queue_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.preload_playback_queue_revision(user_id)
    }

    pub(super) fn capture_mode_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.capture_mode_revision(user_id)
    }

    pub(super) fn priority_revision(&self, user_id: light_core::UserId) -> u64 {
        self.programmers.priority_revision(user_id)
    }

    pub(super) fn handle(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCommand>,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingResult, light_application::ActionError> {
        self.service.handle(action, ports)
    }

    pub(super) fn snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingLiveSnapshot, light_application::ActionError> {
        self.service.snapshot(context, ports)
    }

    pub(super) fn handle_values(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingValuesRequest>,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingValuesResult, light_application::ActionError> {
        self.service.handle_values(action, ports)
    }

    pub(super) fn values_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingValuesSnapshot, light_application::ActionError> {
        self.service.values_snapshot(context, ports)
    }

    pub(super) fn capture_mode_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingCaptureModeSnapshot, light_application::ActionError>
    {
        self.service.capture_mode_snapshot(context, ports)
    }

    pub(super) fn handle_priority(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingPriorityRequest>,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingPriorityResult, light_application::ActionError> {
        self.service.handle_priority(action, ports)
    }

    pub(super) fn priority_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingPrioritySnapshot, light_application::ActionError>
    {
        self.service.priority_snapshot(context, ports)
    }

    pub(super) fn handle_preload_values(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPreloadValuesRequest,
        >,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingPreloadValuesResult, light_application::ActionError>
    {
        self.service.handle_preload_values(action, ports)
    }

    pub(super) fn preload_values_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingPreloadValuesSnapshot, light_application::ActionError>
    {
        self.service.preload_values_snapshot(context, ports)
    }

    pub(super) fn preload_playback_queue_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<
        light_application::ProgrammingPreloadPlaybackQueueSnapshot,
        light_application::ActionError,
    > {
        self.service.preload_playback_queue_snapshot(context, ports)
    }

    pub(super) fn handle_preload_lifecycle(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPreloadLifecycleRequest,
        >,
        ports: &dyn light_application::ProgrammingPreloadLifecyclePorts,
    ) -> Result<light_application::ProgrammingPreloadLifecycleResult, light_application::ActionError>
    {
        self.service.handle_preload_lifecycle(action, ports)
    }

    pub(super) fn handle_preset_recall(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPresetRecallRequest,
        >,
        ports: &dyn light_application::ProgrammingPresetRecallPorts,
    ) -> Result<light_application::ProgrammingPresetRecallResult, light_application::ActionError>
    {
        self.service.handle_preset_recall(action, ports)
    }

    pub(super) fn handle_preset_recording(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPresetRecordRequest,
        >,
        ports: &dyn light_application::ProgrammingPresetRecordingPorts,
    ) -> Result<light_application::ProgrammingPresetRecordResult, light_application::ActionError>
    {
        self.service.handle_preset_recording(action, ports)
    }

    pub(super) fn record_preset_within_interaction(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ProgrammingPresetRecordRequest,
        >,
        ports: &dyn light_application::ProgrammingPresetRecordingPorts,
    ) -> Result<light_application::ProgrammingPresetRecordResult, light_application::ActionError>
    {
        self.service.record_preset_within_interaction(action, ports)
    }

    pub(super) fn handle_group_recording(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingGroupRecordRequest>,
        ports: &dyn light_application::ProgrammingGroupRecordingPorts,
    ) -> Result<light_application::ProgrammingGroupRecordResult, light_application::ActionError>
    {
        self.service.handle_group_recording(action, ports)
    }

    pub(super) fn record_group_within_interaction(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingGroupRecordRequest>,
        ports: &dyn light_application::ProgrammingGroupRecordingPorts,
    ) -> Result<light_application::ProgrammingGroupRecordResult, light_application::ActionError>
    {
        self.service.record_group_within_interaction(action, ports)
    }

    pub(super) fn handle_group_management(
        &self,
        action: light_application::ActionEnvelope<light_application::GroupManagementRequest>,
        ports: &dyn light_application::GroupManagementPorts,
    ) -> Result<light_application::GroupManagementResult, light_application::ActionError> {
        self.service.handle_group_management(action, ports)
    }

    pub(super) fn install_frozen_group_selection(
        &self,
        context: &light_application::ActionContext,
        session_id: SessionId,
        selection: &light_application::GroupManagementSelection,
    ) {
        self.service
            .install_frozen_group_selection(context, session_id, selection);
    }

    pub(super) fn handle_cue_recording(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCueRecordRequest>,
        ports: &dyn light_application::ProgrammingCueRecordingPorts,
    ) -> Result<light_application::ProgrammingCueRecordResult, light_application::ActionError> {
        self.service.handle_cue_recording(action, ports)
    }

    pub(super) fn record_cue_within_interaction(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCueRecordRequest>,
        ports: &dyn light_application::ProgrammingCueRecordingPorts,
    ) -> Result<light_application::ProgrammingCueRecordResult, light_application::ActionError> {
        self.service.record_cue_within_interaction(action, ports)
    }

    pub(super) fn lifecycle_snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
    ) -> Result<light_application::ProgrammingLifecycleSnapshot, light_application::ActionError>
    {
        self.service.lifecycle_snapshot(context, ports)
    }

    pub(super) fn run_lifecycle_transition<T>(
        &self,
        context: &light_application::ActionContext,
        user_id: light_core::UserId,
        operation: impl FnOnce() -> T,
    ) -> T {
        self.service
            .run_lifecycle_transition(context, user_id, operation)
    }

    pub(super) fn handle_cue_deletion<P: light_application::ProgrammingCueDeletionPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCueDeletionRequest>,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueDeletionResult, light_application::ActionError>
    {
        self.service
            .handle_cue_deletion(action, &active_show.service, ports)
    }

    pub(super) fn delete_cue_within_interaction<
        P: light_application::ProgrammingCueDeletionPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: &light_application::ProgrammingCueDeletionRequest,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueDeletionOutcome, light_application::ActionError>
    {
        self.service
            .delete_cue_within_interaction(context, request, &active_show.service, ports)
    }

    pub(super) fn handle_cue_transfer<P: light_application::ProgrammingCueTransferPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::ProgrammingCueTransferRequest>,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueTransferResult, light_application::ActionError>
    {
        self.service
            .handle_cue_transfer(action, &active_show.service, ports)
    }

    pub(super) fn prepare_cue_transfer_choice_within_interaction<
        P: light_application::ProgrammingCueTransferPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: light_application::ProgrammingCueTransferChoiceRequest,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_programmer::CueMoveCopyChoice, light_application::ActionError> {
        self.service.prepare_cue_transfer_choice_within_interaction(
            context,
            request,
            &active_show.service,
            ports,
        )
    }

    pub(super) fn cue_transfer_within_interaction<
        P: light_application::ProgrammingCueTransferPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: &light_application::ProgrammingCueTransferRequest,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueTransferOutcome, light_application::ActionError>
    {
        self.service
            .cue_transfer_within_interaction(context, request, &active_show.service, ports)
    }

    pub(super) fn current_cue_transfer_within_interaction<
        P: light_application::ProgrammingCueTransferPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: &light_application::ProgrammingCueTransferChoiceRequest,
        mode: light_application::ProgrammingCueTransferMode,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueTransferOutcome, light_application::ActionError>
    {
        self.service.current_cue_transfer_within_interaction(
            context,
            request,
            mode,
            &active_show.service,
            ports,
        )
    }

    pub(super) fn run_external_interaction<T>(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
        operation: impl FnOnce() -> T,
    ) -> Result<light_application::ProgrammingInteractionResult<T>, light_application::ActionError>
    {
        self.service
            .run_external_interaction(context, ports, operation)
    }

    pub(super) fn run_selection_refresh<T>(
        &self,
        context: &light_application::ActionContext,
        targets: impl IntoIterator<Item = light_application::ProgrammingSelectionTarget>,
        operation: impl FnOnce() -> T,
    ) -> light_application::ProgrammingSelectionRefreshResult<T> {
        self.service
            .run_selection_refresh(context, targets, operation)
    }

    pub(super) fn run_selection_refresh_with_owned_target<T>(
        &self,
        context: &light_application::ActionContext,
        owned_target: light_application::ProgrammingSelectionTarget,
        targets: impl IntoIterator<Item = light_application::ProgrammingSelectionTarget>,
        operation: impl FnOnce() -> T,
    ) -> light_application::ProgrammingSelectionRefreshResult<T> {
        self.service.run_selection_refresh_with_owned_target(
            context,
            owned_target,
            targets,
            operation,
        )
    }

    pub(super) fn replace_user_programmer<T>(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::ProgrammingPorts,
        target: light_application::ProgrammingLifecycleTarget,
        operation: impl FnOnce() -> light_application::ProgrammingLifecycleCompletion<T>,
    ) -> Result<light_application::ProgrammingLifecycleResult<T>, light_application::ActionError>
    {
        self.service
            .replace_user_programmer(context, ports, target, operation)
    }

    pub(super) fn update_within_interaction<
        P: light_application::programming_update::ProgrammingUpdatePorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::programming_update::ProgrammingUpdateCommand,
        >,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<
        light_application::programming_update::ProgrammingUpdateResult,
        light_application::ActionError,
    > {
        self.service
            .update_within_interaction(action, &active_show.service, ports)
    }

    pub(super) fn update_targets<
        P: light_application::programming_update::ProgrammingUpdatePorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::programming_update::ProgrammingUpdateTargetsRequest,
        >,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<
        light_application::programming_update::ProgrammingUpdateTargetsResult,
        light_application::ActionError,
    > {
        self.service
            .update_targets(action, &active_show.service, ports)
    }

    pub(super) fn preview_update<
        P: light_application::programming_update::ProgrammingUpdatePorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::programming_update::ProgrammingUpdatePreviewRequest,
        >,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<
        light_application::programming_update::ProgrammingUpdatePreviewResult,
        light_application::ActionError,
    > {
        self.service
            .preview_update(action, &active_show.service, ports)
    }

    pub(super) fn handle_update<
        P: light_application::programming_update::ProgrammingUpdatePorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::programming_update::ProgrammingUpdateCommand,
        >,
        active_show: &ActiveShowResource,
        ports: &P,
    ) -> Result<
        light_application::programming_update::ProgrammingUpdateResult,
        light_application::ActionError,
    > {
        self.service
            .handle_update(action, &active_show.service, ports)
    }
}

#[derive(Clone)]
pub(super) struct PlaybackResource {
    service: PlaybackService,
    topology: PlaybackTopologyService,
    telemetry: Arc<playback_telemetry::PlaybackTelemetrySampler>,
}

impl PlaybackResource {
    pub(super) fn new(
        service: PlaybackService,
        topology: PlaybackTopologyService,
        telemetry: Arc<playback_telemetry::PlaybackTelemetrySampler>,
    ) -> Self {
        Self {
            service,
            topology,
            telemetry,
        }
    }

    pub(super) fn handle(
        &self,
        envelope: light_application::ActionEnvelope<light_application::PlaybackCommand>,
        ports: &dyn light_application::PlaybackPorts,
    ) -> Result<light_application::PlaybackResult, light_application::ActionError> {
        self.service.handle(envelope, ports)
    }

    pub(super) fn snapshot(
        &self,
        context: &light_application::ActionContext,
        identities: &[light_application::PlaybackRuntimeIdentity],
        ports: &dyn light_application::PlaybackPorts,
    ) -> Result<light_application::PlaybackRuntimeSnapshot, light_application::ActionError> {
        self.service.snapshot(context, identities, ports)
    }

    pub(super) fn run_unit_of_work<O>(
        &self,
        operation: O,
    ) -> light_application::PlaybackOperationResult<O::Output>
    where
        O: light_application::PlaybackUnitOfWork,
    {
        self.service.run_unit_of_work(operation)
    }

    pub(super) fn handle_topology<P: light_application::PlaybackTopologyPorts>(
        &self,
        envelope: light_application::ActionEnvelope<light_application::PlaybackTopologyCommand>,
        ports: &P,
    ) -> Result<light_application::PlaybackTopologyResult, light_application::ActionError> {
        self.topology.handle(envelope, ports)
    }

    pub(super) fn render_capability(&self) -> PlaybackRenderCapability {
        PlaybackRenderCapability::new(self.service.clone(), Arc::clone(&self.telemetry))
    }
}

#[derive(Clone)]
pub(super) struct PlaybackRenderCapability {
    service: PlaybackService,
    telemetry: Arc<playback_telemetry::PlaybackTelemetrySampler>,
}

impl PlaybackRenderCapability {
    pub(super) fn new(
        service: PlaybackService,
        telemetry: Arc<playback_telemetry::PlaybackTelemetrySampler>,
    ) -> Self {
        Self { service, telemetry }
    }

    pub(super) fn run_unit_of_work<O>(
        &self,
        operation: O,
    ) -> light_application::PlaybackOperationResult<O::Output>
    where
        O: light_application::PlaybackUnitOfWork,
    {
        self.service.run_unit_of_work(operation)
    }

    pub(super) fn completed_frame(
        &self,
        engine: &Engine,
        show_id: Uuid,
        show_revision: u64,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Option<light_application::EventDraft> {
        self.telemetry
            .completed_frame(engine, show_id, show_revision, at)
    }

    pub(super) fn publish(&self, event: light_application::EventDraft) {
        self.service.events().publish(event);
    }
}

#[derive(Clone)]
pub(super) struct HighlightResource {
    registry: Arc<HighlightRegistry>,
    service: light_application::HighlightService,
    patch_preview: Arc<Mutex<HashMap<SessionId, HashSet<light_core::FixtureId>>>>,
}

impl HighlightResource {
    pub(super) fn new(registry: Arc<HighlightRegistry>) -> Self {
        Self {
            service: light_application::HighlightService::new(Arc::clone(&registry)),
            registry,
            patch_preview: Arc::default(),
        }
    }

    pub(super) fn handle(
        &self,
        envelope: light_application::ActionEnvelope<light_application::HighlightCommand>,
        ports: &dyn light_application::HighlightPorts,
    ) -> Result<light_application::HighlightResult, light_application::ActionError> {
        self.service.handle(envelope, ports)
    }

    pub(super) fn snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::HighlightPorts,
    ) -> Result<HighlightState, light_application::ActionError> {
        self.service.snapshot(context, ports)
    }

    pub(super) fn transition(
        &self,
        desk_id: Uuid,
        user_id: light_core::UserId,
        user_name: Option<&str>,
        selection: &light_programmer::ProgrammerSelection,
        fixtures: &[HighlightFixture],
        groups: &HashMap<String, light_programmer::GroupDefinition>,
        output_suppressed: bool,
    ) -> light_programmer::HighlightTransition {
        self.registry.status(
            desk_id,
            user_id,
            user_name,
            selection,
            fixtures,
            groups,
            output_suppressed,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_action(
        &self,
        desk_id: Uuid,
        user_id: light_core::UserId,
        user_name: Option<&str>,
        action: HighlightAction,
        selection: &light_programmer::ProgrammerSelection,
        fixtures: &[HighlightFixture],
        groups: &HashMap<String, light_programmer::GroupDefinition>,
        output_suppressed: bool,
    ) -> Result<light_programmer::HighlightTransition, light_programmer::HighlightError> {
        self.registry.action(
            desk_id,
            user_id,
            user_name,
            action,
            selection,
            fixtures,
            groups,
            output_suppressed,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_action_guarded(
        &self,
        desk_id: Uuid,
        user_id: light_core::UserId,
        user_name: Option<&str>,
        action: HighlightAction,
        selection: &light_programmer::ProgrammerSelection,
        fixtures: &[HighlightFixture],
        groups: &HashMap<String, light_programmer::GroupDefinition>,
        output_suppressed: bool,
    ) -> Result<light_programmer::HighlightTransition, light_programmer::HighlightError> {
        self.registry.action_guarded(
            desk_id,
            user_id,
            user_name,
            action,
            selection,
            fixtures,
            groups,
            output_suppressed,
        )
    }

    pub(super) fn acknowledge_selection(
        &self,
        desk_id: Uuid,
        user_id: light_core::UserId,
        selection: &light_programmer::ProgrammerSelection,
    ) {
        self.registry
            .acknowledge_internal_selection(desk_id, user_id, selection);
    }

    pub(super) fn clear_all(&self) {
        self.registry.clear_all();
        self.patch_preview.lock().clear();
    }

    pub(super) fn clear_desk(&self, desk_id: Uuid) {
        self.registry.clear_desk(desk_id);
    }

    pub(super) fn clear_context(&self, desk_id: Uuid, user_id: light_core::UserId) {
        self.registry.clear_context(desk_id, user_id);
    }

    pub(super) fn clear_patch_previews(&self) {
        self.patch_preview.lock().clear();
    }

    pub(super) fn remove_patch_preview(&self, session_id: SessionId) {
        self.patch_preview.lock().remove(&session_id);
    }

    pub(super) fn set_patch_preview(
        &self,
        session_id: SessionId,
        fixtures: HashSet<light_core::FixtureId>,
    ) -> bool {
        if fixtures.is_empty() {
            self.remove_patch_preview(session_id);
            false
        } else {
            self.patch_preview.lock().insert(session_id, fixtures);
            true
        }
    }

    pub(super) fn output_fixtures(&self) -> HashSet<light_core::FixtureId> {
        self.include_patch_previews(self.registry.output_fixtures())
    }

    pub(super) fn include_patch_previews(
        &self,
        fixtures: impl IntoIterator<Item = light_core::FixtureId>,
    ) -> HashSet<light_core::FixtureId> {
        let mut fixtures = fixtures.into_iter().collect::<HashSet<_>>();
        for preview in self.patch_preview.lock().values() {
            fixtures.extend(preview.iter().copied());
        }
        fixtures
    }
}

#[derive(Clone)]
pub(super) struct OutputResource {
    runtime_service: OutputRuntimeService,
    speed_group_service: SpeedGroupService,
    engine: Arc<Engine>,
    health: Arc<std::sync::Mutex<OutputHealth>>,
    rate: Arc<AtomicU16>,
    control: OutputControlCapability,
    timecode: Arc<Mutex<TimecodeRouter>>,
    network: Option<Arc<NetworkOutput>>,
    sequences: Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
    manual_clock: Option<Arc<ManualClock>>,
    test_clock_lock: Arc<tokio::sync::Mutex<()>>,
    speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
    dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    dynamic_auto_offs: Arc<Mutex<Vec<u16>>>,
    visualization_frames: Arc<super::visualization_frame::VisualizationFrameHub>,
    sound_capture_owners: Arc<Mutex<[Option<SoundCaptureOwner>; 5]>>,
    #[cfg(test)]
    runtime_persistence_attempts: Arc<AtomicU64>,
    #[cfg(test)]
    runtime_persistence_failure: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    speed_group_persistence_attempts: Arc<AtomicU64>,
    #[cfg(test)]
    speed_group_persistence_failure: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
pub(super) struct OutputControlCapability {
    control: Arc<Mutex<OutputControl>>,
}

impl OutputControlCapability {
    pub(super) fn new(control: Arc<Mutex<OutputControl>>) -> Self {
        Self { control }
    }

    fn lock(&self) -> parking_lot::MutexGuard<'_, OutputControl> {
        self.control.lock()
    }
}

#[derive(Clone, Copy)]
pub(super) struct OutputRuntimeControlProjection {
    pub(super) revision: u64,
    pub(super) grand_master: f32,
    pub(super) blackout: bool,
    pub(super) grand_master_flash: bool,
}

pub(super) struct TestClockSession {
    _guard: tokio::sync::OwnedMutexGuard<()>,
    driver: TestClockDriver,
}

#[derive(Clone)]
pub(super) struct TestClockDriver {
    clock: Arc<ManualClock>,
}

impl TestClockSession {
    pub(super) fn driver(&self) -> TestClockDriver {
        self.driver.clone()
    }

    pub(super) fn set(&self, time: chrono::DateTime<chrono::Utc>) {
        self.driver.set(time);
    }

    pub(super) fn advance_millis(&self, millis: i64) -> chrono::DateTime<chrono::Utc> {
        self.driver.advance_millis(millis)
    }

    pub(super) fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.driver.now()
    }
}

impl TestClockDriver {
    pub(super) fn set(&self, time: chrono::DateTime<chrono::Utc>) {
        self.clock.set(time);
    }

    pub(super) fn advance_millis(&self, millis: i64) -> chrono::DateTime<chrono::Utc> {
        self.clock.advance_millis(millis)
    }

    pub(super) fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now()
    }
}

impl OutputResource {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        runtime_service: OutputRuntimeService,
        speed_group_service: SpeedGroupService,
        engine: Arc<Engine>,
        health: Arc<std::sync::Mutex<OutputHealth>>,
        rate: Arc<AtomicU16>,
        control: OutputControlCapability,
        timecode: Arc<Mutex<TimecodeRouter>>,
        network: Option<Arc<NetworkOutput>>,
        sequences: Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
        manual_clock: Option<Arc<ManualClock>>,
        speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
        dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
        dynamic_auto_offs: Arc<Mutex<Vec<u16>>>,
        visualization_frames: Arc<super::visualization_frame::VisualizationFrameHub>,
    ) -> Self {
        Self {
            runtime_service,
            speed_group_service,
            engine,
            health,
            rate,
            control,
            timecode,
            network,
            sequences,
            manual_clock,
            test_clock_lock: Arc::default(),
            speed_groups,
            dynamics,
            dynamic_auto_offs,
            visualization_frames,
            sound_capture_owners: Arc::new(Mutex::new([None; 5])),
            #[cfg(test)]
            runtime_persistence_attempts: Arc::new(AtomicU64::new(0)),
            #[cfg(test)]
            runtime_persistence_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(test)]
            speed_group_persistence_attempts: Arc::new(AtomicU64::new(0)),
            #[cfg(test)]
            speed_group_persistence_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub(super) fn health_snapshot(&self) -> OutputHealth {
        self.health
            .lock()
            .expect("output health mutex poisoned")
            .clone()
    }

    pub(super) fn latest_visualization_frame(
        &self,
    ) -> Option<Arc<super::visualization_frame::PublishedVisualizationFrame>> {
        self.visualization_frames.latest()
    }

    pub(super) fn visualization_projection(
        &self,
        key: super::visualization_frame::VisualizationProjectionKey,
        source: &super::visualization_frame::PublishedVisualizationFrame,
        build: impl FnOnce()
            -> Result<light_wire::v2::visualization::VisualizationLaneSnapshot, ApiError>,
    ) -> Result<Arc<super::visualization_frame::ProjectedVisualizationFrame>, ApiError> {
        self.visualization_frames.projection(key, source, build)
    }

    pub(super) fn change_visualization_subscribers(
        &self,
        lane: light_wire::v2::visualization::VisualizationLane,
        delta: i8,
    ) {
        self.visualization_frames.change_subscribers(lane, delta);
    }

    pub(super) fn change_visualization_projection_claim(
        &self,
        key: super::visualization_frame::VisualizationProjectionKey,
        delta: i8,
    ) {
        self.visualization_frames
            .change_projection_claim(key, delta);
    }

    pub(super) fn visualization_metrics(&self) -> super::visualization_frame::VisualizationMetrics {
        self.visualization_frames.metrics()
    }

    pub(super) fn record_visualization_snapshot_route(
        &self,
        projection_duration: Duration,
        serialization_duration: Duration,
        payload_bytes: u64,
        source: Option<&super::visualization_frame::PublishedVisualizationFrame>,
    ) {
        self.visualization_frames.record_snapshot_route(
            projection_duration,
            serialization_duration,
            payload_bytes,
            source,
        );
    }

    pub(super) fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.engine.snapshot()
    }

    pub(super) fn start_dynamic(
        &self,
        request: light_dynamics::DynamicStartRequest,
    ) -> Result<Uuid, light_dynamics::DynamicRuntimeError> {
        self.dynamics.lock().start(request)
    }

    pub(super) fn dynamic_runtime_snapshot(&self) -> light_dynamics::DynamicRuntimeSnapshot {
        self.dynamics.lock().snapshot()
    }

    pub(super) fn restore_dynamic_runtime_snapshot(
        &self,
        snapshot: light_dynamics::DynamicRuntimeSnapshot,
    ) -> Result<(), light_dynamics::DynamicRuntimeError> {
        self.dynamics.lock().restore_snapshot(snapshot)
    }

    pub(super) fn off_dynamic_controller(
        &self,
        controller_id: Uuid,
        now_millis: u64,
        release_delay_millis: u64,
        release_duration_millis: u64,
    ) -> Result<(Uuid, bool), light_dynamics::DynamicRuntimeError> {
        self.dynamics.lock().off_controller_by_id(
            controller_id,
            now_millis,
            release_delay_millis,
            release_duration_millis,
        )
    }

    pub(super) fn update_dynamic_controller(
        &self,
        controller_id: Uuid,
        size: Option<f32>,
        speed_multiplier: Option<f32>,
        phase_offset_degrees: Option<f32>,
    ) -> Result<(), light_dynamics::DynamicRuntimeError> {
        self.dynamics.lock().update_controller(
            controller_id,
            size,
            speed_multiplier,
            phase_offset_degrees,
        )
    }

    pub(super) fn is_dynamic_definition_running(&self, definition_id: Uuid) -> bool {
        self.dynamics.lock().is_definition_running(definition_id)
    }

    pub(super) fn set_dynamic_definitions_pinned(&self, pinned: bool) {
        self.dynamics.lock().set_definitions_pinned(pinned);
    }

    pub(super) fn replace_snapshot(&self, snapshot: EngineSnapshot) -> Result<(), EngineError> {
        let definitions = snapshot.dynamics.iter().cloned().collect::<Vec<_>>();
        self.engine.replace_snapshot(snapshot)?;
        self.dynamics
            .lock()
            .install_definitions(definitions)
            .expect("Engine validation and Dynamic validation stay equivalent");
        Ok(())
    }

    pub(super) fn prepare_snapshot(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<PreparedEngineSnapshot, EngineError> {
        self.engine.prepare_snapshot(snapshot)
    }

    pub(super) fn install_prepared_snapshot(&self, prepared: PreparedEngineSnapshot) {
        let definitions = prepared
            .snapshot()
            .dynamics
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.engine.install_prepared_snapshot(prepared);
        self.dynamics
            .lock()
            .install_definitions(definitions)
            .expect("prepared Engine snapshot contains valid Dynamic definitions");
    }

    pub(super) fn resolved_values(
        &self,
    ) -> HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>
    {
        self.engine.resolved_values()
    }

    pub(super) fn visualization_dynamic_values(
        &self,
        extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
        projected: bool,
    ) -> HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>
    {
        self.visualization_dynamic_projection(extra_programmer_values, projected)
            .0
    }

    pub(super) fn visualization_dynamic_projection(
        &self,
        extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
        _projected: bool,
    ) -> (
        HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>,
        light_dynamics::DynamicRuntimeSnapshot,
        Vec<light_dynamics::DynamicRuntimeSample>,
    ) {
        // Visualization is observational. Sampling can retire completed release
        // transitions, so always operate on a clone and leave the authoritative
        // output scheduler responsible for mutating and publishing runtime state.
        let snapshot = self.engine.snapshot();
        let mut visualization_runtime = light_dynamics::DynamicRuntime::default();
        visualization_runtime
            .install_definitions(snapshot.dynamics.iter().cloned())
            .expect("Engine snapshot contains validated Dynamic definitions");
        visualization_runtime
            .restore_snapshot(self.dynamics.lock().snapshot())
            .expect("live Dynamic runtime snapshot remains restorable");
        let visualization_runtime = Mutex::new(visualization_runtime);
        let (sampled, runtime_samples) = output_scheduler::dynamic_projection(
            &self.engine,
            &visualization_runtime,
            &self.speed_groups,
            &self.rate,
            extra_programmer_values,
        );
        let runtime_snapshot = visualization_runtime.lock().snapshot();
        (
            self.engine
                .resolved_values_with_contribution_batches(&sampled),
            runtime_snapshot,
            runtime_samples,
        )
    }

    pub(super) fn dynamic_programmer_values(
        &self,
    ) -> Vec<(Uuid, i16, light_dynamics::DynamicAddressValue)> {
        self.engine.dynamic_programmer_values()
    }

    pub(super) fn active_cue_dynamic_values(&self) -> Vec<light_playback::ActiveCueDynamicValue> {
        self.engine.active_cue_dynamic_values()
    }

    /// Reconciles typed Programmer, Cue, and Playback Dynamic state into the persisted runtime
    /// without sending an output frame. Preload GO uses this inside the active-show exclusion
    /// boundary so the committed Programmer layer and its runtime identity share one timestamp.
    pub(super) fn reconcile_dynamic_runtime(&self) {
        let _ = output_scheduler::dynamic_contributions(
            &self.engine,
            &self.dynamics,
            &self.speed_groups,
            &self.rate,
            &[],
            false,
        );
    }

    pub(super) fn take_dynamic_auto_offs(&self) -> Vec<u16> {
        std::mem::take(&mut *self.dynamic_auto_offs.lock())
    }

    pub(super) fn restore_dynamic_auto_offs(&self, numbers: impl IntoIterator<Item = u16>) {
        let mut pending = self.dynamic_auto_offs.lock();
        for number in numbers {
            if !pending.contains(&number) {
                pending.push(number);
            }
        }
    }

    pub(super) fn playback_runtime(&self) -> Vec<light_playback::ActivePlayback> {
        self.engine.playback_runtime()
    }

    pub(super) fn playback_runtime_status(&self) -> Vec<light_playback::PlaybackRuntimeStatus> {
        self.engine.playback_runtime_status()
    }

    pub(super) fn active_dynamic_playbacks(&self) -> Vec<light_playback::ActiveDynamicPlayback> {
        self.engine.active_dynamic_playbacks()
    }

    pub(super) fn playback_dynamics(&self) -> light_engine::PlaybackDynamicsProjection {
        self.engine.playback_dynamics()
    }

    pub(super) fn set_dynamic_runtime_paused(&self, paused: bool) {
        let now_millis =
            u64::try_from(self.engine.application_time().timestamp_millis()).unwrap_or_default();
        self.dynamics.lock().set_global_paused(paused, now_millis);
    }

    pub(super) fn active_playbacks(&self) -> Vec<light_playback::ActivePlayback> {
        self.engine.active_playbacks()
    }

    pub(super) fn move_in_black_runtime(&self) -> Vec<light_engine::MoveInBlackDiagnostic> {
        self.engine.move_in_black_runtime()
    }

    pub(super) fn enabled_auto_off_playbacks(&self) -> Vec<u16> {
        self.engine.enabled_auto_off_playbacks()
    }

    pub(super) fn application_time(&self) -> chrono::DateTime<chrono::Utc> {
        self.engine.application_time()
    }

    pub(super) fn group_master_flash(&self, group_id: &str) -> f32 {
        self.engine.group_master_flash(group_id)
    }

    pub(super) fn set_highlighted_fixtures(
        &self,
        fixtures: impl IntoIterator<Item = light_core::FixtureId>,
    ) {
        self.engine.set_highlighted_fixtures(fixtures);
    }

    pub(super) fn clear_highlighted_fixtures(&self) {
        self.engine.clear_highlighted_fixtures();
    }

    pub(super) fn highlighted_fixtures(&self) -> Vec<light_core::FixtureId> {
        self.engine.highlighted_fixtures()
    }

    pub(super) fn clear_programmer_transitions(&self) {
        self.engine.clear_programmer_transitions();
    }

    pub(super) fn set_control_timing(
        &self,
        speed_groups_bpm: [f64; 5],
        programmer_fade_millis: u64,
        sequence_master_fade_millis: u64,
    ) {
        self.engine.set_control_timing(
            speed_groups_bpm,
            programmer_fade_millis,
            sequence_master_fade_millis,
        );
    }

    pub(super) fn prepare_playback_batch(
        &self,
        commands: &[light_engine::PlaybackBatchCommand],
        started_at: chrono::DateTime<chrono::Utc>,
        fallback_millis: u64,
    ) -> Result<light_engine::PreparedPlaybackBatch, String> {
        self.engine
            .prepare_playback_batch(commands, started_at, fallback_millis)
    }

    pub(super) fn install_prepared_playback_batch(
        &self,
        prepared: light_engine::PreparedPlaybackBatch,
    ) -> Result<(), String> {
        self.engine.install_prepared_playback_batch(prepared)
    }

    pub(super) fn install_prepared_snapshot_releasing_playback(
        &self,
        prepared: PreparedEngineSnapshot,
    ) {
        let definitions = prepared
            .snapshot()
            .dynamics
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.engine
            .install_prepared_snapshot_releasing_playback(prepared);
        self.dynamics
            .lock()
            .install_definitions(definitions)
            .expect("prepared Engine snapshot contains valid Dynamic definitions");
    }

    pub(super) fn validate_snapshot_for_runtime(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(), EngineError> {
        self.engine.validate_snapshot_for_runtime(snapshot)
    }

    pub(super) fn render(
        &self,
        options: RenderOptions,
    ) -> Result<light_engine::RenderResult, EngineError> {
        self.engine.render(options)
    }

    pub(super) fn profile_visualization_values(
        &self,
        values: &HashMap<
            (light_core::FixtureId, light_core::AttributeKey),
            light_core::AttributeValue,
        >,
        options: RenderOptions,
    ) -> Result<
        HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>,
        EngineError,
    > {
        self.engine.profile_visualization_values(values, options)
    }

    pub(super) fn execute_playback(
        &self,
        command: EnginePlaybackCommand,
    ) -> Result<EnginePlaybackOutcome, String> {
        self.engine.execute_playback(command)
    }

    pub(super) fn execute_pool_playback_with_activation(
        &self,
        number: u16,
        action: PoolPlaybackAction,
        exclusion_zones: &[Vec<u16>],
        activation_origin: Option<light_playback::PlaybackActivationOrigin>,
    ) -> Result<light_engine::PoolPlaybackTransition, String> {
        self.engine.execute_pool_playback_with_activation(
            number,
            action,
            exclusion_zones,
            activation_origin,
        )
    }

    pub(super) fn set_group_master(&self, group_id: &str, value: f32) -> Result<bool, EngineError> {
        self.engine.set_group_master(group_id, value)
    }

    pub(super) fn set_group_master_flash(&self, group_id: String, value: f32) {
        self.engine.set_group_master_flash(group_id, value);
    }

    pub(super) fn set_speed_groups_paused(&self, paused: [bool; 5]) {
        self.engine.set_speed_groups_paused(paused);
    }

    pub(super) fn set_timecode_frame(&self, frame: Option<u64>) {
        self.engine.set_timecode_frame(frame);
    }

    pub(super) fn render_with_playback_events(
        &self,
        active_show: &ActiveShowProjection,
        playback: &PlaybackRenderCapability,
        options: RenderOptions,
    ) -> Result<light_engine::RenderResult, EngineError> {
        let sampled = output_scheduler::dynamic_contributions(
            &self.engine,
            &self.dynamics,
            &self.speed_groups,
            &self.rate,
            &[],
            true,
        );
        output_scheduler::render_with_playback_events(
            &self.engine,
            active_show,
            playback,
            options,
            &sampled,
        )
    }

    #[cfg(test)]
    pub(super) fn dynamic_contributions_for_test(&self) -> Vec<light_engine::ContributionBatch> {
        output_scheduler::dynamic_contributions(
            &self.engine,
            &self.dynamics,
            &self.speed_groups,
            &self.rate,
            &[],
            false,
        )
    }

    pub(super) fn frame_rate_hz(&self) -> u16 {
        self.rate.load(Ordering::Relaxed)
    }

    pub(super) fn set_frame_rate_hz(&self, frame_rate_hz: u16) {
        self.rate.store(frame_rate_hz, Ordering::Relaxed);
    }

    pub(super) fn route_send_errors(&self) -> Vec<light_output::RouteSendError> {
        self.network
            .as_ref()
            .map(|output| output.route_send_errors())
            .unwrap_or_default()
    }

    pub(super) fn take_send_errors(&self) -> u64 {
        self.network
            .as_ref()
            .map(|output| output.take_send_errors())
            .unwrap_or_default()
    }

    pub(super) fn has_network_output(&self) -> bool {
        self.network.is_some()
    }

    pub(super) fn inject_network_failure(
        &self,
        destination: SocketAddr,
        enabled: bool,
    ) -> Result<(), ApiError> {
        self.network
            .as_ref()
            .ok_or_else(|| ApiError::unavailable("network output is unavailable"))?
            .inject_failure(destination, enabled);
        Ok(())
    }

    pub(super) fn clear_runtime_replay(&self) {
        self.runtime_service.clear_replay();
    }

    pub(super) fn handle_runtime_action<P: light_application::OutputRuntimePorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::OutputRuntimeCommand>,
        ports: &P,
    ) -> Result<light_application::OutputRuntimeResult, light_application::ActionError> {
        self.runtime_service.handle(action, ports)
    }

    pub(super) fn runtime_snapshot<P: light_application::OutputRuntimePorts>(
        &self,
        context: &light_application::ActionContext,
        identity: light_application::OutputRuntimeIdentity,
        ports: &P,
    ) -> Result<light_application::OutputRuntimeSnapshot, light_application::ActionError> {
        self.runtime_service.snapshot(context, identity, ports)
    }

    pub(super) fn handle_speed_group_action<P: light_application::SpeedGroupPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::SpeedGroupCommand>,
        ports: &P,
    ) -> Result<light_application::SpeedGroupResult, light_application::ActionError> {
        self.speed_group_service.handle(action, ports)
    }

    pub(super) fn record_speed_group_external_change<P: light_application::SpeedGroupPorts>(
        &self,
        context: &light_application::ActionContext,
        ports: &P,
        changed: &[light_application::SpeedGroupId],
        applied_at_millis: u64,
    ) -> Result<u64, light_application::ActionError> {
        self.speed_group_service
            .record_external_change(context, ports, changed, applied_at_millis)
    }

    pub(super) fn speed_group_service_snapshot<P: light_application::SpeedGroupPorts>(
        &self,
        context: &light_application::ActionContext,
        ports: &P,
    ) -> Result<light_application::SpeedGroupSnapshot, light_application::ActionError> {
        self.speed_group_service.snapshot(context, ports)
    }

    #[cfg(test)]
    pub(super) fn rebind_speed_group_events(&mut self, events: &EventResource) {
        self.speed_group_service = SpeedGroupService::new(events.application.clone());
    }

    pub(super) fn has_test_clock(&self) -> bool {
        self.manual_clock.is_some()
    }

    pub(super) async fn acquire_test_clock(&self) -> Result<TestClockSession, ApiError> {
        let clock = self
            .manual_clock
            .as_ref()
            .cloned()
            .ok_or_else(|| ApiError::not_found("test clock"))?;
        Ok(TestClockSession {
            _guard: Arc::clone(&self.test_clock_lock).lock_owned().await,
            driver: TestClockDriver { clock },
        })
    }

    pub(super) fn record_output_health(&self, packets_sent: u64, send_errors: u64) {
        let mut health = self.health.lock().expect("output health mutex poisoned");
        health.frames_sent += 1;
        health.packets_sent += packets_sent;
        health.send_errors += send_errors;
    }

    pub(super) async fn run_output_scheduler<F, Fut>(
        &self,
        cancellation: CancellationToken,
        tick: F,
    ) where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = std::io::Result<u64>>,
    {
        light_output::run_scheduler_dynamic(
            Arc::clone(&self.rate),
            cancellation,
            Arc::clone(&self.health),
            tick,
        )
        .await;
    }

    pub(super) fn control_projection(&self) -> OutputRuntimeControlProjection {
        let control = self.control.lock();
        OutputRuntimeControlProjection {
            revision: control.revision,
            grand_master: control.options.grand_master,
            blackout: control.options.blackout,
            grand_master_flash: control.grand_master_flash,
        }
    }

    pub(super) fn render_options(&self) -> light_engine::RenderOptions {
        self.control.lock().render_options()
    }

    pub(super) fn restore_runtime_control(&self, runtime: &PersistedOutputRuntime) {
        let mut control = self.control.lock();
        control.options.grand_master = runtime.grand_master;
        control.options.blackout = runtime.blackout;
        control.revision = runtime.revision;
    }

    pub(super) fn apply_runtime_control(
        &self,
        grand_master: Option<f32>,
        blackout: Option<bool>,
    ) -> Result<u64, light_application::ActionError> {
        let mut control = self.control.lock();
        let next_revision = control.revision.checked_add(1).ok_or_else(|| {
            light_application::ActionError::new(
                light_application::ActionErrorKind::Unavailable,
                "output revision is exhausted",
            )
        })?;
        if let Some(grand_master) = grand_master {
            control.options.grand_master = grand_master;
        }
        if let Some(blackout) = blackout {
            control.options.blackout = blackout;
        }
        control.revision = next_revision;
        Ok(next_revision)
    }

    pub(super) fn set_transition_hold(&self, hold: bool) {
        self.control.lock().hold = hold;
    }

    pub(super) fn set_transition_blackout(&self, blackout: bool) {
        self.control.lock().options.blackout = blackout;
    }

    pub(super) fn set_transition_grand_master(&self, grand_master: f32) {
        self.control.lock().options.grand_master = grand_master;
    }

    pub(super) fn set_grand_master_flash(&self, pressed: bool) -> bool {
        let mut control = self.control.lock();
        if control.grand_master_flash == pressed {
            false
        } else {
            control.grand_master_flash = pressed;
            true
        }
    }

    pub(super) fn set_dmx_override(
        &self,
        universe: light_core::Universe,
        address: light_core::DmxAddress,
        value: Option<u8>,
    ) {
        let mut control = self.control.lock();
        if let Some(value) = value {
            control.raw_overrides.insert((universe, address), value);
        } else {
            control.raw_overrides.remove(&(universe, address));
        }
    }

    pub(super) fn dmx_override(
        &self,
        universe: light_core::Universe,
        address: light_core::DmxAddress,
    ) -> Option<u8> {
        self.control
            .lock()
            .raw_overrides
            .get(&(universe, address))
            .copied()
    }

    pub(super) fn dmx_snapshot(&self, revision: u64) -> serde_json::Value {
        let control = self.control.lock();
        let mut universes = control
            .last_frames
            .iter()
            .map(|(&universe, frame)| {
                serde_json::json!({"universe":universe,"slots":frame.to_vec()})
            })
            .collect::<Vec<_>>();
        universes.sort_by_key(|universe| universe["universe"].as_u64().unwrap_or_default());
        serde_json::json!({
            "revision": revision,
            "universes": universes,
            "overrides": control.raw_overrides.iter().map(|(&(universe,address),&value)| {
                serde_json::json!({"universe":universe,"address":address,"value":value})
            }).collect::<Vec<_>>()
        })
    }

    pub(super) fn configure_timecode(&self, sources: Vec<light_control::TimecodeSourceConfig>) {
        self.timecode.lock().configure(sources);
    }

    pub(super) fn ingest_timecode(
        &self,
        timecode: light_control::SmpteTimecode,
    ) -> Option<light_control::SmpteTimecode> {
        self.timecode.lock().ingest(timecode).cloned()
    }

    pub(super) fn timecode_status(&self) -> (Option<String>, Option<light_control::SmpteTimecode>) {
        let router = self.timecode.lock();
        (
            router.active_source().map(str::to_owned),
            router.current().cloned(),
        )
    }

    pub(super) async fn clear_sequences(&self) {
        self.sequences.lock().await.clear();
    }

    pub(super) fn render_frames_and_publish(
        &self,
        rendered: &light_engine::RenderResult,
    ) -> HashMap<light_core::Universe, light_output::DmxFrame> {
        let mut control = self.control.lock();
        if control.hold {
            return control.last_frames.clone();
        }
        let mut frames = rendered.universes.clone();
        for (&(universe, address), &value) in &control.raw_overrides {
            if let Some(frame) = frames.get_mut(&universe) {
                frame[usize::from(address - 1)] = value;
            }
        }
        control.last_frames = frames.clone();
        self.visualization_frames
            .publish(rendered, control.render_options());
        frames
    }

    pub(super) async fn send_network_routes(
        &self,
        routes: &[light_output::OutputRoute],
        frames: &HashMap<light_core::Universe, light_output::DmxFrame>,
        patched_slots: &HashMap<light_core::Universe, u16>,
    ) -> Result<u64, std::io::Error> {
        let output = self
            .network
            .as_ref()
            .ok_or_else(|| std::io::Error::other("network output is unavailable"))?;
        output
            .send_routes(
                routes,
                frames,
                patched_slots,
                &mut *self.sequences.lock().await,
            )
            .await
    }

    pub(super) async fn terminate_routes(&self, routes: &[light_output::OutputRoute]) {
        if let Some(output) = &self.network {
            let _ = output
                .terminate_routes(routes, &mut *self.sequences.lock().await)
                .await;
        }
    }

    pub(super) fn reset_speed_groups(&self, manual_bpm: [f64; 5], sound: [SoundToLightConfig; 5]) {
        *self.speed_groups.lock() = std::array::from_fn(|index| {
            SpeedGroupController::new(manual_bpm[index], sound[index].clone())
                .expect("validated Speed Group configuration")
        });
        *self.sound_capture_owners.lock() = [None; 5];
    }

    pub(super) fn speed_group_snapshots(&self, now: u64) -> [SpeedSnapshot; 5] {
        let controllers = self.speed_groups.lock();
        std::array::from_fn(|index| controllers[index].snapshot(now))
    }

    pub(super) fn speed_group_snapshot(&self, index: usize, now: u64) -> SpeedSnapshot {
        self.speed_groups.lock()[index].snapshot(now)
    }

    pub(super) fn speed_group_controller(&self, index: usize) -> SpeedGroupController {
        self.speed_groups.lock()[index].clone()
    }

    pub(super) fn speed_group_sound_config(&self, index: usize) -> SoundToLightConfig {
        self.speed_groups.lock()[index].sound_config().clone()
    }

    pub(super) fn speed_group_manual_bpm(&self, index: usize) -> f64 {
        self.speed_groups.lock()[index].manual_bpm()
    }

    pub(super) fn configure_speed_groups(
        &self,
        previous_bpm: [f64; 5],
        next_bpm: [f64; 5],
        next_sound: [SoundToLightConfig; 5],
        now: u64,
    ) -> Result<[SoundToLightConfig; 5], ApiError> {
        let mut controllers = self.speed_groups.lock();
        let mut applied_sound = next_sound;
        for index in 0..controllers.len() {
            if next_bpm[index] != previous_bpm[index] {
                speed_groups::unlink_speed_group(&mut controllers, index, now);
                controllers[index]
                    .set_manual_bpm(next_bpm[index])
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                controllers[index]
                    .set_speed_master_scale(1.0)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                controllers[index].set_paused_at(false, now);
                applied_sound[index].enabled = false;
                self.sound_capture_owners.lock()[index] = None;
            } else {
                controllers[index]
                    .set_manual_fallback_bpm(next_bpm[index])
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            }
            controllers[index]
                .set_sound_config(applied_sound[index].clone())
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        Ok(applied_sound)
    }

    pub(super) fn tap_speed_group(
        &self,
        index: usize,
        now: u64,
    ) -> light_control::speed::LearnResult {
        let mut controllers = self.speed_groups.lock();
        speed_groups::unlink_speed_group(&mut controllers, index, now);
        let result = controllers[index].tap_learn(now);
        self.sound_capture_owners.lock()[index] = None;
        result
    }

    pub(super) fn set_manual_speed_group(
        &self,
        index: usize,
        bpm: f64,
        now: u64,
        disable_sound: bool,
    ) -> Result<(), ApiError> {
        let mut controllers = self.speed_groups.lock();
        speed_groups::unlink_speed_group(&mut controllers, index, now);
        controllers[index]
            .set_manual_bpm(bpm)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        controllers[index]
            .set_speed_master_scale(1.0)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        controllers[index].set_paused_at(false, now);
        if disable_sound {
            let mut sound = controllers[index].sound_config().clone();
            sound.enabled = false;
            controllers[index]
                .set_sound_config(sound)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        self.sound_capture_owners.lock()[index] = None;
        Ok(())
    }

    pub(super) fn set_speed_group_sound_config(
        &self,
        index: usize,
        configuration: SoundToLightConfig,
    ) -> Result<(), ApiError> {
        self.speed_groups.lock()[index]
            .set_sound_config(configuration)
            .map_err(|error| ApiError::bad_request(error.to_string()))
    }

    pub(super) fn set_speed_group_manual_fallback(
        &self,
        index: usize,
        bpm: f64,
    ) -> Result<(), ApiError> {
        self.speed_groups.lock()[index]
            .set_manual_fallback_bpm(bpm)
            .map_err(|error| ApiError::bad_request(error.to_string()))
    }

    #[cfg(test)]
    pub(super) fn configure_speed_group_test_state(
        &self,
        index: usize,
        sound: SoundToLightConfig,
        scale: f64,
        paused: bool,
        now: u64,
    ) {
        let mut controllers = self.speed_groups.lock();
        controllers[index].set_sound_config(sound).unwrap();
        controllers[index].set_speed_master_scale(scale).unwrap();
        controllers[index].set_paused_at(paused, now);
    }

    #[cfg(test)]
    pub(super) fn set_speed_group_scale_for_test(&self, index: usize, scale: f64) {
        self.speed_groups.lock()[index]
            .set_speed_master_scale(scale)
            .unwrap();
    }

    pub(super) fn observe_speed_group_sound(
        &self,
        index: usize,
        desk_id: Uuid,
        now: u64,
        mut observation: SoundObservation,
    ) -> Result<(), ApiError> {
        if !self.speed_groups.lock()[index].sound_config().enabled {
            return Err(ApiError::conflict(
                "enable Sound to Light before submitting observations",
            ));
        }
        {
            let mut owners = self.sound_capture_owners.lock();
            if owners[index].is_some_and(|owner| {
                owner.desk_id != desk_id && now.saturating_sub(owner.last_seen_millis) <= 3_000
            }) {
                return Err(ApiError::conflict(
                    "this Speed Group is receiving audio from another desk",
                ));
            }
            owners[index] = Some(SoundCaptureOwner {
                desk_id,
                last_seen_millis: now,
            });
        }
        observation.captured_at_millis = now;
        self.speed_groups.lock()[index].observe_sound(observation);
        Ok(())
    }

    pub(super) fn apply_speed_group_action(
        &self,
        index: usize,
        now: u64,
        action: &str,
    ) -> Result<Vec<usize>, ApiError> {
        let mut controllers = self.speed_groups.lock();
        let affected = match action {
            "learn" => {
                speed_groups::unlink_speed_group(&mut controllers, index, now);
                controllers[index].tap_learn(now);
                vec![index]
            }
            "double" => {
                let affected = speed_groups::speed_group_action_indices(&controllers, index);
                for &affected_index in &affected {
                    controllers[affected_index].double();
                }
                affected
            }
            "half" => {
                let affected = speed_groups::speed_group_action_indices(&controllers, index);
                for &affected_index in &affected {
                    controllers[affected_index].half();
                }
                affected
            }
            "pause" => {
                let paused = !controllers[index].snapshot(now).paused;
                let affected = speed_groups::speed_group_action_indices(&controllers, index);
                for &affected_index in &affected {
                    controllers[affected_index].set_paused_at(paused, now);
                }
                affected
            }
            _ => {
                return Err(ApiError::bad_request(
                    "Speed Group action must be learn, double, half, or pause",
                ));
            }
        };
        if action == "learn" {
            self.sound_capture_owners.lock()[index] = None;
        }
        Ok(affected)
    }

    pub(super) fn speed_group_port_state(&self) -> light_application::SpeedGroupPortState {
        let controllers = self.speed_groups.lock();
        let owners = self.sound_capture_owners.lock();
        let groups = controllers
            .iter()
            .enumerate()
            .map(|(index, controller)| {
                let snapshot = controller.snapshot(0);
                light_application::SpeedGroupProjection {
                    group: light_application::SpeedGroupId::new((index + 1) as u8)
                        .expect("fixed Speed Group index"),
                    manual_bpm: snapshot.manual_bpm,
                    paused: snapshot.paused,
                    speed_master_scale: snapshot.speed_master_scale,
                    synchronized_with: snapshot
                        .synchronized_with
                        .and_then(light_application::SpeedGroupId::new),
                    phase_origin_millis: snapshot.phase_origin_millis,
                }
            })
            .collect();
        let manual_control_clean = controllers
            .iter()
            .enumerate()
            .filter(|(index, controller)| {
                controller.manual_entry_is_current(controller.manual_bpm())
                    && owners[*index].is_none()
            })
            .filter_map(|(index, _)| light_application::SpeedGroupId::new((index + 1) as u8))
            .collect();
        light_application::SpeedGroupPortState {
            groups,
            manual_control_clean,
        }
    }

    pub(super) fn apply_resolved_speed_group_action(
        &self,
        action: light_application::SpeedGroupResolvedAction,
    ) -> Result<Vec<usize>, light_application::ActionError> {
        use light_application::{ActionError, ActionErrorKind, SpeedGroupResolvedAction};
        let mut controllers = self.speed_groups.lock();
        let affected = match action {
            SpeedGroupResolvedAction::SetManualBpm {
                group,
                bpm,
                applied_at_millis,
            } => {
                let index = group.index();
                speed_groups::unlink_speed_group(&mut controllers, index, applied_at_millis);
                controllers[index].set_manual_bpm(bpm).map_err(|error| {
                    ActionError::new(ActionErrorKind::Invalid, error.to_string())
                })?;
                controllers[index]
                    .set_speed_master_scale(1.0)
                    .map_err(|error| {
                        ActionError::new(ActionErrorKind::Invalid, error.to_string())
                    })?;
                controllers[index].set_paused_at(false, applied_at_millis);
                vec![index]
            }
            SpeedGroupResolvedAction::Synchronize {
                source,
                target,
                applied_at_millis,
            } => {
                speed_groups::synchronize_speed_groups(
                    &mut controllers,
                    source.index(),
                    target.index(),
                    applied_at_millis,
                )
                .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.message))?;
                vec![source.index(), target.index()]
            }
        };
        self.clear_sound_capture_owners(&affected);
        Ok(affected)
    }

    pub(super) fn apply_speed_group_playback(
        &self,
        index: usize,
        now: u64,
        action: &str,
        input: &PoolPlaybackInput,
        fader: light_playback::PlaybackFaderMode,
        configured_source: SpeedGroupSource,
        linked_fallback: Option<f64>,
    ) -> Result<(bool, Vec<usize>, bool), ApiError> {
        let takes_manual_control = action != "pause";
        let mut controllers = self.speed_groups.lock();
        let owner_present = self.sound_capture_owners.lock()[index].is_some();
        let manual_ownership_changed = takes_manual_control
            && (configured_source != SpeedGroupSource::Manual
                || controllers[index].sound_config().enabled
                || owner_present);
        if let Some(effective_bpm) = linked_fallback {
            controllers[index]
                .set_manual_fallback_bpm(effective_bpm)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        if takes_manual_control {
            let mut sound = controllers[index].sound_config().clone();
            sound.enabled = false;
            controllers[index]
                .set_sound_config(sound)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        let before = playback_speed_groups::controller_snapshots(&controllers, now);
        let (affected, clear_owner) = playback_speed_groups::apply_speed_action(
            &mut controllers,
            index,
            now,
            action,
            input,
            fader,
            owner_present,
        )?;
        let changed = action == "learn"
            || manual_ownership_changed
            || playback_speed_groups::speed_group_changed(&before, &controllers, &affected, now);
        drop(controllers);
        if clear_owner || takes_manual_control {
            self.clear_sound_capture_owner(index);
        }
        Ok((changed, affected, takes_manual_control))
    }

    pub(super) fn sound_capture_owner(&self, index: usize) -> Option<SoundCaptureOwner> {
        self.sound_capture_owners.lock()[index]
    }

    pub(super) fn clear_sound_capture_owner(&self, index: usize) {
        self.sound_capture_owners.lock()[index] = None;
    }

    pub(super) fn set_sound_capture_owner(&self, index: usize, owner: Option<SoundCaptureOwner>) {
        self.sound_capture_owners.lock()[index] = owner;
    }

    pub(super) fn clear_sound_capture_owners(&self, indices: &[usize]) {
        let mut owners = self.sound_capture_owners.lock();
        for &index in indices {
            owners[index] = None;
        }
    }

    #[cfg(test)]
    pub(super) fn runtime_persistence_attempts(&self) -> u64 {
        self.runtime_persistence_attempts.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub(super) fn record_runtime_persistence_attempt(&self) -> Result<(), ApiError> {
        self.runtime_persistence_attempts
            .fetch_add(1, Ordering::SeqCst);
        if self.runtime_persistence_failure.load(Ordering::SeqCst) {
            Err(ApiError::unavailable(
                "injected output runtime persistence failure",
            ))
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub(super) fn force_runtime_persistence_failure(&self, fail: bool) {
        self.runtime_persistence_failure
            .store(fail, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(super) fn speed_group_persistence_attempts(&self) -> u64 {
        self.speed_group_persistence_attempts.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub(super) fn record_speed_group_persistence_attempt(&self) -> Result<(), ApiError> {
        self.speed_group_persistence_attempts
            .fetch_add(1, Ordering::SeqCst);
        if self.speed_group_persistence_failure.load(Ordering::SeqCst) {
            Err(ApiError::internal("forced Speed Group persistence failure"))
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub(super) fn force_speed_group_persistence_failure(&self, fail: bool) {
        self.speed_group_persistence_failure
            .store(fail, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub(super) struct ActiveShowResource {
    activation: ActiveShowCoordinator,
    active: Arc<RwLock<Option<ShowEntry>>>,
    document: Arc<Mutex<Option<light_show::PortableShowDocument>>>,
    backup_checkpoint: Arc<Mutex<Option<(light_core::ShowId, u64)>>>,
    error: Arc<RwLock<Option<String>>>,
    service: ActiveShowService,
    patch: ShowPatchService,
    selective_import: SelectiveShowImportService,
    mvr_imports: Arc<Mutex<HashMap<Uuid, StagedMvrImport>>>,
    #[cfg(test)]
    patch_profile_resolution: Arc<PatchProfileResolutionPause>,
    #[cfg(test)]
    http_lifecycle: Arc<ActiveShowLifecyclePause>,
    #[cfg(test)]
    preload_store_release_lifecycle: Arc<ActiveShowLifecyclePause>,
    #[cfg(test)]
    patch_lifecycle: Arc<ActiveShowLifecyclePause>,
}

impl ActiveShowResource {
    pub(super) fn new(
        activation: ActiveShowCoordinator,
        active: Arc<RwLock<Option<ShowEntry>>>,
        error: Option<String>,
        service: ActiveShowService,
        patch: ShowPatchService,
        selective_import: SelectiveShowImportService,
    ) -> Self {
        Self {
            activation,
            active,
            document: Arc::default(),
            backup_checkpoint: Arc::default(),
            error: Arc::new(RwLock::new(error)),
            service,
            patch,
            selective_import,
            mvr_imports: Arc::default(),
            #[cfg(test)]
            patch_profile_resolution: Arc::default(),
            #[cfg(test)]
            http_lifecycle: Arc::default(),
            #[cfg(test)]
            preload_store_release_lifecycle: Arc::default(),
            #[cfg(test)]
            patch_lifecycle: Arc::default(),
        }
    }

    pub(super) fn current(&self) -> Option<ShowEntry> {
        self.active.read().clone()
    }

    pub(super) fn mutate_objects<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<
            light_application::MutateActiveShowObjectsCommand,
        >,
        ports: &P,
    ) -> Result<light_application::MutateActiveShowObjectsResult, light_application::ActionError>
    {
        self.service.mutate_objects(action, ports)
    }

    pub(super) fn undo_object<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::UndoActiveShowObjectCommand>,
        ports: &P,
    ) -> Result<light_application::UndoActiveShowObjectResult, light_application::ActionError> {
        self.service.undo_object(action, ports)
    }

    pub(super) fn undo_recording<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<
            light_application::UndoActiveShowRecordingCommand,
        >,
        ports: &P,
    ) -> Result<light_application::MutateActiveShowObjectsResult, light_application::ActionError>
    {
        self.service.undo_recording(action, ports)
    }

    pub(super) fn mutate_output_route<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::MutateOutputRouteCommand>,
        ports: &P,
    ) -> Result<light_application::MutateOutputRouteResult, light_application::ActionError> {
        self.service.mutate_output_route(action, ports)
    }

    pub(super) fn commit_programming_cue<P: light_application::ProgrammingCueActiveShowPorts>(
        &self,
        context: &light_application::ActionContext,
        commit: &light_application::ProgrammingCueCommit,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueCommitResult, light_application::ActionError> {
        self.service.commit_programming_cue(context, commit, ports)
    }

    pub(super) fn commit_programming_group<
        P: light_application::ProgrammingGroupActiveShowPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        commit: &light_application::ProgrammingGroupCommit,
        ports: &P,
    ) -> Result<light_application::ProgrammingGroupCommitResult, light_application::ActionError>
    {
        self.service
            .commit_programming_group(context, commit, ports)
    }

    pub(super) fn commit_programming_preset<
        P: light_application::ProgrammingPresetActiveShowPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        commit: &light_application::ProgrammingPresetCommit,
        ports: &P,
    ) -> Result<light_application::ProgrammingPresetCommitResult, light_application::ActionError>
    {
        self.service
            .commit_programming_preset(context, commit, ports)
    }

    pub(super) fn commit_group_management<P: light_application::GroupManagementActiveShowPorts>(
        &self,
        context: &light_application::ActionContext,
        commit: &light_application::GroupManagementCommit,
        ports: &P,
    ) -> Result<light_application::GroupManagementCommitResult, light_application::ActionError>
    {
        self.service.commit_group_management(context, commit, ports)
    }

    pub(super) fn patch_snapshot<P: light_application::ShowPatchPorts>(
        &self,
        context: &light_application::ActionContext,
        show_id: light_core::ShowId,
        ports: &P,
    ) -> Result<light_application::PatchSnapshot, light_application::ActionError> {
        self.patch.snapshot(context, show_id, ports)
    }

    pub(super) fn patch_fixtures<P: light_application::ShowPatchPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::PatchFixturesCommand>,
        ports: &P,
    ) -> Result<light_application::PatchFixturesResult, light_application::ActionError> {
        self.patch.handle(action, ports)
    }

    pub(super) fn preview_selective_import<P: light_application::SelectiveShowImportPorts>(
        &self,
        context: &light_application::ActionContext,
        request: light_application::SelectiveShowImportRequest,
        ports: &P,
    ) -> Result<light_application::SelectiveShowImportPreview, light_application::ActionError> {
        self.selective_import.preview(context, request, ports)
    }

    pub(super) fn apply_selective_import<P: light_application::SelectiveShowImportPorts>(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ApplySelectiveShowImportCommand,
        >,
        ports: &P,
    ) -> Result<light_application::SelectiveShowImportResult, light_application::ActionError> {
        self.selective_import.apply(action, ports)
    }

    pub(super) fn apply_mvr_import<P: light_application::ShowPatchPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::ApplyActiveMvrImportCommand>,
        ports: &P,
    ) -> Result<light_application::ActiveMvrImportResult, light_application::ActionError> {
        light_application::MvrImportService::new(self.service.clone()).apply(action, ports)
    }

    #[cfg(test)]
    pub(super) fn patch_profile_resolution_probe(&self) -> Arc<PatchProfileResolutionPause> {
        Arc::clone(&self.patch_profile_resolution)
    }

    #[cfg(test)]
    pub(super) fn http_lifecycle_probe(&self) -> Arc<ActiveShowLifecyclePause> {
        Arc::clone(&self.http_lifecycle)
    }

    #[cfg(test)]
    pub(super) fn preload_store_release_lifecycle_probe(&self) -> Arc<ActiveShowLifecyclePause> {
        Arc::clone(&self.preload_store_release_lifecycle)
    }

    #[cfg(test)]
    pub(super) fn patch_lifecycle_probe(&self) -> Arc<ActiveShowLifecyclePause> {
        Arc::clone(&self.patch_lifecycle)
    }

    #[cfg(test)]
    pub(super) fn pause_http_lifecycle_if_armed(&self) {
        self.http_lifecycle.pause_if_armed();
    }

    #[cfg(test)]
    pub(super) fn pause_patch_lifecycle_if_armed(&self) {
        self.patch_lifecycle.pause_if_armed();
    }

    pub(super) async fn acquire(&self) -> ActiveShowPermit {
        self.activation.acquire().await
    }

    pub(super) fn acquire_blocking(&self) -> ActiveShowPermit {
        self.activation.acquire_blocking()
    }

    pub(super) fn try_acquire(&self) -> Result<ActiveShowPermit, tokio::sync::TryLockError> {
        self.activation.try_acquire()
    }

    pub(super) fn coordinator(&self) -> ActiveShowCoordinator {
        self.activation.clone()
    }

    pub(super) fn replace_current(&self, show: Option<ShowEntry>) {
        *self.active.write() = show;
    }

    pub(super) fn update_current(&self, update: impl FnOnce(&mut ShowEntry)) -> bool {
        let mut active = self.active.write();
        let Some(show) = active.as_mut() else {
            return false;
        };
        update(show);
        true
    }

    pub(super) fn output_projection(&self) -> ActiveShowProjection {
        ActiveShowProjection {
            active: Arc::clone(&self.active),
        }
    }

    pub(super) fn error(&self) -> Option<String> {
        self.error.read().clone()
    }

    pub(super) fn set_error(&self, error: Option<String>) {
        *self.error.write() = error;
    }

    pub(super) fn clear_document_cache(&self) {
        *self.document.lock() = None;
    }

    pub(super) fn document_cache(&self) -> ActiveShowDocumentCache {
        ActiveShowDocumentCache {
            document: Arc::clone(&self.document),
        }
    }

    pub(super) fn stage_mvr_import(&self, token: Uuid, import: StagedMvrImport) {
        let now = Instant::now();
        let mut imports = self.mvr_imports.lock();
        imports.retain(|_, item| now.duration_since(item.created) < Duration::from_secs(30 * 60));
        imports.insert(token, import);
    }

    pub(super) fn take_mvr_import(&self, token: Uuid) -> Option<StagedMvrImport> {
        self.mvr_imports.lock().remove(&token)
    }

    pub(super) fn update_backup_checkpoint(
        &self,
        update: impl FnOnce(&mut Option<(light_core::ShowId, u64)>),
    ) {
        update(&mut self.backup_checkpoint.lock());
    }
}

#[derive(Clone)]
pub(super) struct ActiveShowCoordinator {
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl ActiveShowCoordinator {
    pub(super) fn new() -> Self {
        Self {
            lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub(super) async fn acquire(&self) -> ActiveShowPermit {
        ActiveShowPermit(self.lock.clone().lock_owned().await)
    }

    pub(super) fn acquire_blocking(&self) -> ActiveShowPermit {
        ActiveShowPermit(self.lock.clone().blocking_lock_owned())
    }

    pub(super) fn try_acquire(&self) -> Result<ActiveShowPermit, tokio::sync::TryLockError> {
        self.lock.clone().try_lock_owned().map(ActiveShowPermit)
    }
}

pub(super) struct ActiveShowPermit(#[allow(dead_code)] tokio::sync::OwnedMutexGuard<()>);

#[derive(Clone)]
pub(super) struct ActiveShowProjection {
    active: Arc<RwLock<Option<ShowEntry>>>,
}

impl ActiveShowProjection {
    pub(super) fn new(active: Arc<RwLock<Option<ShowEntry>>>) -> Self {
        Self { active }
    }

    pub(super) fn current(&self) -> Option<ShowEntry> {
        self.active.read().clone()
    }
}

#[derive(Clone)]
pub(super) struct ActiveShowDocumentCache {
    document: Arc<Mutex<Option<light_show::PortableShowDocument>>>,
}

impl ActiveShowDocumentCache {
    pub(super) fn take(&self) -> Option<light_show::PortableShowDocument> {
        self.document.lock().take()
    }

    pub(super) fn replace(&self, document: Option<light_show::PortableShowDocument>) {
        *self.document.lock() = document;
    }

    #[cfg(test)]
    pub(super) fn snapshot(&self) -> Option<light_show::PortableShowDocument> {
        self.document.lock().clone()
    }
}

#[derive(Clone)]
pub(super) struct EventResource {
    application: EventBus,
    audit: Arc<Mutex<VecDeque<Event>>>,
    revision: Arc<AtomicU64>,
}

impl EventResource {
    const AUDIT_CAPACITY: usize = 2_048;

    pub(super) fn new(application: EventBus) -> Self {
        Self {
            application,
            audit: Arc::new(Mutex::new(VecDeque::with_capacity(Self::AUDIT_CAPACITY))),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn latest_sequence(&self) -> u64 {
        self.application.latest_sequence()
    }

    pub(super) fn publish(
        &self,
        event: light_application::EventDraft,
    ) -> Arc<light_application::EventEnvelope> {
        self.application.publish(event)
    }

    pub(super) fn replay(
        &self,
        after_sequence: u64,
        filter: &light_application::EventFilter,
    ) -> light_application::EventReplay {
        self.application.replay(after_sequence, filter)
    }

    pub(super) fn subscribe(
        &self,
        filter: light_application::EventFilter,
        options: light_application::SubscriptionOptions,
    ) -> light_application::EventSubscription {
        self.application.subscribe(filter, options)
    }

    pub(super) fn publish_automatic_playback_events(
        &self,
        changes: impl IntoIterator<Item = light_application::AutomaticPlaybackProjection>,
    ) -> Vec<Arc<light_application::EventEnvelope>> {
        light_application::publish_automatic_playback_events(&self.application, changes)
    }

    pub(super) fn record_audit(&self, kind: &str, payload: serde_json::Value) -> u64 {
        let revision = self.revision.fetch_add(1, Ordering::Relaxed) + 1;
        let event = Event {
            revision,
            kind: kind.into(),
            payload,
        };
        let mut audit = self.audit.lock();
        if audit.len() == Self::AUDIT_CAPACITY {
            audit.pop_front();
        }
        audit.push_back(event);
        revision
    }

    pub(super) fn audit_after(&self, after_revision: u64) -> Vec<Event> {
        self.audit
            .lock()
            .iter()
            .filter(|event| event.revision > after_revision)
            .cloned()
            .collect()
    }

    #[cfg(test)]
    pub(super) fn audit_events(&self) -> Vec<Event> {
        self.audit.lock().iter().cloned().collect()
    }

    #[cfg(test)]
    pub(super) fn audit_revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
}

#[derive(Clone)]
pub(super) struct IntegrationResource {
    matter_bridge: Arc<matter::MatterBridgeAdapter>,
    matter_transport: Option<Arc<matter::MatterTransport>>,
    osc_subscribers: Arc<Mutex<HashMap<String, OscSubscriber>>>,
    osc_cue_record_suppression: Arc<Mutex<osc_cue_record_suppression::OscCueRecordSuppression>>,
    osc_feedback: Option<Arc<std::net::UdpSocket>>,
    #[cfg(test)]
    osc_feedback_capture: Arc<Mutex<Vec<CapturedOscMessage>>>,
}

impl IntegrationResource {
    pub(super) fn new(
        matter_bridge: Arc<matter::MatterBridgeAdapter>,
        matter_transport: Option<Arc<matter::MatterTransport>>,
        osc_feedback: Option<Arc<std::net::UdpSocket>>,
    ) -> Self {
        Self {
            matter_bridge,
            matter_transport,
            osc_subscribers: Arc::default(),
            osc_cue_record_suppression: Arc::default(),
            osc_feedback,
            #[cfg(test)]
            osc_feedback_capture: Arc::default(),
        }
    }

    pub(super) fn matter_bridge(&self) -> &matter::MatterBridgeAdapter {
        &self.matter_bridge
    }

    pub(super) fn matter_transport(&self) -> Option<&matter::MatterTransport> {
        self.matter_transport.as_deref()
    }

    pub(super) fn hardware_connected(&self) -> bool {
        !self.osc_subscribers.lock().is_empty()
    }

    pub(super) fn osc_subscriber(&self, client_id: &str) -> Option<OscSubscriber> {
        self.osc_subscribers.lock().get(client_id).cloned()
    }

    #[cfg(test)]
    pub(super) fn set_osc_last_seen(&self, client_id: &str, last_seen: Instant) {
        if let Some(subscriber) = self.osc_subscribers.lock().get_mut(client_id) {
            subscriber.last_seen = last_seen;
        }
    }

    #[cfg(test)]
    pub(super) fn set_osc_record_started(&self, client_id: &str, started: Instant) {
        if let Some(subscriber) = self.osc_subscribers.lock().get_mut(client_id) {
            subscriber.update_record_started = Some(started);
        }
    }

    pub(super) fn osc_subscriber_for_source(&self, source: SocketAddr) -> Option<OscSubscriber> {
        self.osc_subscribers
            .lock()
            .values()
            .find(|subscriber| subscriber.command_source == source)
            .cloned()
    }

    pub(super) fn accept_highlight_action(
        &self,
        source: SocketAddr,
        desk_alias: &str,
        action: HighlightAction,
        now: Instant,
    ) -> Option<SessionId> {
        let mut subscribers = self.osc_subscribers.lock();
        let subscriber = subscribers.values_mut().find(|subscriber| {
            subscriber.command_source == source && subscriber.desk_alias == desk_alias
        })?;
        if is_duplicate_osc_action(
            subscriber
                .last_highlight_action
                .as_ref()
                .map(|(previous, received_at)| (previous.as_str(), *received_at)),
            action,
            now,
        ) {
            return None;
        }
        subscriber.last_highlight_action = Some((action.osc_dedupe_key().to_owned(), now));
        Some(subscriber.session_id)
    }

    pub(super) fn register_osc_subscriber(
        &self,
        client_id: String,
        subscriber: OscSubscriber,
    ) -> Option<OscSubscriber> {
        self.osc_subscribers.lock().insert(client_id, subscriber)
    }

    pub(super) fn unregister_osc_subscriber(&self, client_id: &str) -> Option<OscSubscriber> {
        self.osc_subscribers.lock().remove(client_id)
    }

    pub(super) fn clear_osc_subscribers(&self) {
        self.osc_subscribers.lock().clear();
    }

    pub(super) fn has_osc_session(&self, session_id: SessionId) -> bool {
        self.osc_subscribers
            .lock()
            .values()
            .any(|subscriber| subscriber.session_id == session_id)
    }

    pub(super) fn osc_session_is_exclusive_to_client(
        &self,
        client_id: &str,
        session_id: SessionId,
    ) -> bool {
        self.osc_subscribers
            .lock()
            .iter()
            .all(|(id, subscriber)| id == client_id || subscriber.session_id != session_id)
    }

    pub(super) fn set_shift(&self, source: SocketAddr, pressed: bool) {
        let mut subscribers = self.osc_subscribers.lock();
        let Some(target) = subscribers
            .values_mut()
            .find(|candidate| candidate.command_source == source)
        else {
            return;
        };
        if pressed {
            target.shifted = !target.shifted;
            target.shift_held = true;
        } else {
            target.shift_held = false;
            if target.update_first_release.is_some() {
                target.shifted = false;
            }
        }
    }

    pub(super) fn clear_shift(&self, source: SocketAddr) {
        if let Some(target) = self
            .osc_subscribers
            .lock()
            .values_mut()
            .find(|candidate| candidate.command_source == source)
        {
            target.shifted = false;
        }
    }

    pub(super) fn record_gesture(
        &self,
        source: SocketAddr,
        pressed: bool,
    ) -> osc_highlight::OscRecordGesture {
        self.osc_subscribers
            .lock()
            .values_mut()
            .find(|candidate| candidate.command_source == source)
            .map(|target| osc_highlight::record_gesture(target, pressed))
            .unwrap_or(osc_highlight::OscRecordGesture::None)
    }

    pub(super) fn active_osc_subscribers(
        &self,
        now: Instant,
        timeout: Duration,
    ) -> (Vec<OscSubscriber>, Vec<(SessionId, SocketAddr)>, bool) {
        let mut subscribers = self.osc_subscribers.lock();
        let before = subscribers.len();
        let expired = subscribers
            .values()
            .filter(|subscriber| now.duration_since(subscriber.last_seen) >= timeout)
            .map(|subscriber| (subscriber.session_id, subscriber.command_source))
            .collect::<Vec<_>>();
        subscribers.retain(|_, subscriber| now.duration_since(subscriber.last_seen) < timeout);
        let changed = before != subscribers.len();
        let active = subscribers.values().cloned().collect();
        (active, expired, changed)
    }

    pub(super) fn remove_suppression_source(&self, session_id: SessionId, source: SocketAddr) {
        self.osc_cue_record_suppression
            .lock()
            .remove_source(session_id, source);
    }

    pub(super) fn remove_session_suppression(&self, session_id: SessionId) {
        self.osc_cue_record_suppression
            .lock()
            .remove_session(session_id);
    }

    pub(super) fn suppresses_osc_input(
        &self,
        input: osc_cue_record_suppression::OscSuppressionInput<'_>,
        now: Instant,
    ) -> bool {
        self.osc_cue_record_suppression
            .lock()
            .suppresses_input(input, now)
    }

    pub(super) fn remember_osc_intercept(
        &self,
        input: osc_cue_record_suppression::OscSuppressionInput<'_>,
        now: Instant,
    ) {
        self.osc_cue_record_suppression
            .lock()
            .remember_intercept(input, now);
    }

    pub(super) fn send_osc(
        &self,
        target: SocketAddr,
        address: String,
        arguments: Vec<OscArgument>,
    ) {
        #[cfg(test)]
        self.osc_feedback_capture
            .lock()
            .push((target, address.clone(), arguments.clone()));
        if let (Some(socket), Ok(packet)) =
            (&self.osc_feedback, encode_osc_message(&address, &arguments))
        {
            let _ = socket.send_to(&packet, target);
        }
    }

    #[cfg(test)]
    pub(super) fn captured_osc_feedback(&self) -> Vec<CapturedOscMessage> {
        self.osc_feedback_capture.lock().clone()
    }
}

#[derive(Clone)]
pub(super) struct MediaResource {
    cache: Arc<Mutex<MediaCache>>,
    status: Arc<RwLock<HashMap<light_core::FixtureId, MediaServerStatus>>>,
}

impl MediaResource {
    pub(super) fn new(cache: MediaCache) -> Self {
        Self {
            cache: Arc::new(Mutex::new(cache)),
            status: Arc::default(),
        }
    }

    pub(super) fn statuses(&self) -> HashMap<light_core::FixtureId, MediaServerStatus> {
        self.status.read().clone()
    }

    pub(super) fn status(&self, fixture_id: light_core::FixtureId) -> MediaServerStatus {
        self.status
            .read()
            .get(&fixture_id)
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn record_status(&self, fixture_id: light_core::FixtureId, error: Option<String>) {
        let mut statuses = self.status.write();
        let status = statuses.entry(fixture_id).or_default();
        status.online = error.is_none();
        if let Some(error) = error {
            status.last_error = Some(error);
        } else {
            status.last_success = Some(chrono::Utc::now().to_rfc3339());
            status.last_error = None;
        }
    }

    pub(super) fn put_thumbnails(
        &self,
        images: impl IntoIterator<Item = (ThumbnailKey, light_media::MediaImage)>,
    ) -> Result<(), light_media::MediaError> {
        let mut cache = self.cache.lock();
        for (key, image) in images {
            cache.put_thumbnail(key, image)?;
        }
        Ok(())
    }

    pub(super) fn thumbnail(&self, key: &ThumbnailKey) -> Option<light_media::CachedImage> {
        self.cache.lock().thumbnail(key)
    }

    pub(super) fn put_preview(
        &self,
        key: PreviewKey,
        image: light_media::MediaImage,
    ) -> Result<(), light_media::MediaError> {
        self.cache.lock().put_preview(key, image)
    }

    pub(super) fn preview(&self, key: &PreviewKey) -> Option<light_media::CachedImage> {
        self.cache.lock().preview(key)
    }

    pub(super) fn invalidate_fixture(&self, fixture_id: light_core::FixtureId) {
        self.cache.lock().clear_fixture(&fixture_id.0.to_string());
        self.status.write().remove(&fixture_id);
    }

    pub(super) fn invalidate_fixtures(
        &self,
        fixture_ids: impl IntoIterator<Item = light_core::FixtureId>,
    ) {
        let fixture_ids = fixture_ids.into_iter().collect::<Vec<_>>();
        {
            let mut cache = self.cache.lock();
            for fixture_id in &fixture_ids {
                cache.clear_fixture(&fixture_id.0.to_string());
            }
        }
        let mut statuses = self.status.write();
        for fixture_id in fixture_ids {
            statuses.remove(&fixture_id);
        }
    }

    pub(super) fn retain_fixtures(&self, fixture_ids: &HashSet<light_core::FixtureId>) {
        self.status
            .write()
            .retain(|fixture_id, _| fixture_ids.contains(fixture_id));
        self.cache.lock().retain_fixtures(
            &fixture_ids
                .iter()
                .map(|fixture_id| fixture_id.0.to_string())
                .collect(),
        );
    }
}

impl Default for MediaResource {
    fn default() -> Self {
        Self::new(MediaCache::default())
    }
}

#[derive(Clone)]
pub(super) struct ReplayResource {
    show_library: Arc<tokio::sync::Mutex<show_library_v2::ShowLibraryReplayCache>>,
    fixture_library: Arc<tokio::sync::Mutex<fixture_api_replay::FixtureLibraryReplayCache>>,
    show_object: Arc<tokio::sync::Mutex<show_objects_v2::ShowObjectReplayCache>>,
    show_object_intent:
        Arc<tokio::sync::Mutex<show_object_intents_v2::ShowObjectIntentReplayCache>>,
    preset_generation: Arc<tokio::sync::Mutex<live_action_http::PresetGenerationReplayCache>>,
    screen_configuration:
        Arc<tokio::sync::Mutex<screen_configuration_v2::ScreenConfigurationReplayCache>>,
    control_desk_configuration:
        Arc<tokio::sync::Mutex<control_desk_configuration_v2::ControlDeskConfigurationReplayCache>>,
    desk_management: Arc<tokio::sync::Mutex<desk_management_v2::DeskManagementReplayCache>>,
    stage_layout: Arc<Mutex<stage_layout_http::StageLayoutReplayCache>>,
    virtual_playback_zones:
        Arc<Mutex<virtual_playback_zones_http::VirtualPlaybackZonesReplayCache>>,
}

impl Default for ReplayResource {
    fn default() -> Self {
        Self {
            show_library: Arc::default(),
            fixture_library: Arc::default(),
            show_object: Arc::default(),
            show_object_intent: Arc::default(),
            preset_generation: Arc::default(),
            screen_configuration: Arc::default(),
            control_desk_configuration: Arc::default(),
            desk_management: Arc::default(),
            stage_layout: Arc::default(),
            virtual_playback_zones: Arc::default(),
        }
    }
}

impl ReplayResource {
    pub(super) async fn lookup_show_library(
        &self,
        key: &show_library_v2::ReplayKey,
        signature: &[u8; 32],
    ) -> Result<Option<light_wire::v2::show_library::ShowLibraryActionOutcome>, ApiError> {
        self.show_library.lock().await.get(key, signature)
    }

    pub(super) async fn insert_show_library(
        &self,
        key: show_library_v2::ReplayKey,
        signature: [u8; 32],
        outcome: light_wire::v2::show_library::ShowLibraryActionOutcome,
    ) {
        self.show_library
            .lock()
            .await
            .insert(key, signature, outcome);
    }

    pub(super) async fn lookup_fixture_library(
        &self,
        key: &fixture_api_replay::ReplayKey,
        signature: &[u8; 32],
    ) -> Result<Option<light_wire::v2::fixture_library::FixtureLibraryActionOutcome>, ApiError>
    {
        self.fixture_library.lock().await.get(key, signature)
    }

    pub(super) async fn insert_fixture_library(
        &self,
        key: fixture_api_replay::ReplayKey,
        signature: [u8; 32],
        outcome: light_wire::v2::fixture_library::FixtureLibraryActionOutcome,
    ) {
        self.fixture_library
            .lock()
            .await
            .insert(key, signature, outcome);
    }

    pub(super) async fn lookup_show_object(
        &self,
        key: &show_objects_v2::ReplayKey,
        action: &light_wire::v2::show_objects::OutputRouteAction,
    ) -> Result<Option<light_wire::v2::show_objects::OutputRouteActionOutcome>, ApiError> {
        self.show_object.lock().await.get(key, action)
    }

    pub(super) async fn insert_show_object(
        &self,
        key: show_objects_v2::ReplayKey,
        action: light_wire::v2::show_objects::OutputRouteAction,
        outcome: light_wire::v2::show_objects::OutputRouteActionOutcome,
    ) {
        self.show_object.lock().await.insert(key, action, outcome);
    }

    pub(super) async fn lookup_show_object_intent(
        &self,
        key: &show_object_intents_v2::ReplayKey,
        action: &show_object_intents_v2::ReplayAction,
    ) -> Result<Option<light_wire::v2::show_objects::ShowObjectActionOutcome>, ApiError> {
        self.show_object_intent.lock().await.get(key, action)
    }

    pub(super) async fn insert_show_object_intent(
        &self,
        key: show_object_intents_v2::ReplayKey,
        action: show_object_intents_v2::ReplayAction,
        outcome: light_wire::v2::show_objects::ShowObjectActionOutcome,
    ) {
        self.show_object_intent
            .lock()
            .await
            .insert(key, action, outcome);
    }

    pub(super) async fn lookup_preset_generation(
        &self,
        key: &live_action_http::PresetGenerationReplayKey,
        request: &light_wire::v2::live_action::GenerateFixturePresetsRequest,
    ) -> Result<Option<light_wire::v2::live_action::GenerateFixturePresetsOutcome>, ApiError> {
        self.preset_generation.lock().await.get(key, request)
    }

    pub(super) async fn insert_preset_generation(
        &self,
        key: live_action_http::PresetGenerationReplayKey,
        request: light_wire::v2::live_action::GenerateFixturePresetsRequest,
        outcome: light_wire::v2::live_action::GenerateFixturePresetsOutcome,
    ) {
        self.preset_generation
            .lock()
            .await
            .insert(key, request, outcome);
    }

    pub(super) async fn lookup_screen_configuration(
        &self,
        key: &screen_configuration_v2::ReplayKey,
        action: &light_wire::v2::screen_configuration::ScreenConfigurationAction,
    ) -> Result<
        Option<light_wire::v2::screen_configuration::ScreenConfigurationActionOutcome>,
        ApiError,
    > {
        self.screen_configuration.lock().await.get(key, action)
    }

    pub(super) async fn insert_screen_configuration(
        &self,
        key: screen_configuration_v2::ReplayKey,
        action: light_wire::v2::screen_configuration::ScreenConfigurationAction,
        outcome: light_wire::v2::screen_configuration::ScreenConfigurationActionOutcome,
    ) {
        self.screen_configuration
            .lock()
            .await
            .insert(key, action, outcome);
    }

    pub(super) async fn lookup_control_desk_configuration(
        &self,
        key: &control_desk_configuration_v2::ReplayKey,
        action: &light_wire::v2::control_desk_configuration::ControlDeskConfigurationAction,
    ) -> Result<
        Option<light_wire::v2::control_desk_configuration::ControlDeskConfigurationActionOutcome>,
        ApiError,
    > {
        self.control_desk_configuration
            .lock()
            .await
            .get(key, action)
    }

    pub(super) async fn insert_control_desk_configuration(
        &self,
        key: control_desk_configuration_v2::ReplayKey,
        action: light_wire::v2::control_desk_configuration::ControlDeskConfigurationAction,
        outcome: light_wire::v2::control_desk_configuration::ControlDeskConfigurationActionOutcome,
    ) {
        self.control_desk_configuration
            .lock()
            .await
            .insert(key, action, outcome);
    }

    pub(super) async fn lookup_desk_management(
        &self,
        key: &desk_management_v2::ReplayKey,
        fingerprint: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        self.desk_management.lock().await.get(key, fingerprint)
    }

    pub(super) async fn insert_desk_management(
        &self,
        key: desk_management_v2::ReplayKey,
        fingerprint: serde_json::Value,
        outcome: serde_json::Value,
    ) {
        self.desk_management
            .lock()
            .await
            .insert(key, fingerprint, outcome);
    }

    pub(super) async fn lookup_stage_layout(
        &self,
        key: &stage_layout_http::ReplayKey,
        action: &light_wire::v2::stage_layout::StageLayoutAction,
    ) -> Result<
        Option<light_wire::v2::stage_layout::StageLayoutActionOutcome>,
        stage_layout_http::StageLayoutHttpError,
    > {
        self.stage_layout.lock().get(key, action)
    }

    pub(super) async fn insert_stage_layout(
        &self,
        key: stage_layout_http::ReplayKey,
        action: light_wire::v2::stage_layout::StageLayoutAction,
        outcome: light_wire::v2::stage_layout::StageLayoutActionOutcome,
    ) {
        self.stage_layout.lock().insert(key, action, outcome);
    }

    pub(super) async fn lookup_virtual_playback_zones(
        &self,
        key: &virtual_playback_zones_http::ReplayKey,
        action: &virtual_playback_zones_http::ReplayAction,
    ) -> Result<
        Option<light_wire::v2::virtual_playback_zones::VirtualPlaybackExclusionUpdateOutcome>,
        ApiError,
    > {
        self.virtual_playback_zones.lock().get(key, action)
    }

    pub(super) async fn insert_virtual_playback_zones(
        &self,
        key: virtual_playback_zones_http::ReplayKey,
        action: virtual_playback_zones_http::ReplayAction,
        outcome: light_wire::v2::virtual_playback_zones::VirtualPlaybackExclusionUpdateOutcome,
    ) {
        self.virtual_playback_zones
            .lock()
            .insert(key, action, outcome);
    }

    #[cfg(test)]
    pub(super) fn show_object_cache_is_available(&self) -> bool {
        self.show_object.try_lock().is_ok()
    }
}

#[derive(Clone)]
pub(super) struct LifecycleResource {
    shutdown: CancellationToken,
    task_sender: tokio::sync::mpsc::Sender<OwnedRuntimeTask>,
    task_receiver: Arc<Mutex<Option<tokio::sync::mpsc::Receiver<OwnedRuntimeTask>>>>,
}

const RUNTIME_TASK_QUEUE_CAPACITY: usize = 256;

pub(super) type OwnedRuntimeTask =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'static>>;

impl LifecycleResource {
    pub(super) fn new(shutdown: CancellationToken) -> Self {
        let (task_sender, task_receiver) = tokio::sync::mpsc::channel(RUNTIME_TASK_QUEUE_CAPACITY);
        Self {
            shutdown,
            task_sender,
            task_receiver: Arc::new(Mutex::new(Some(task_receiver))),
        }
    }

    pub(super) fn cancellation_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    pub(super) fn request_shutdown(&self) {
        self.shutdown.cancel();
    }

    pub(super) fn schedule(
        &self,
        task: impl std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    ) -> Result<(), anyhow::Error> {
        self.task_sender
            .try_send(Box::pin(task))
            .map_err(|error| match error {
                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    anyhow::anyhow!("runtime task supervisor queue is full")
                }
                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    anyhow::anyhow!("runtime task supervisor is unavailable")
                }
            })
    }

    pub(super) fn take_task_receiver(
        &self,
    ) -> Option<tokio::sync::mpsc::Receiver<OwnedRuntimeTask>> {
        self.task_receiver.lock().take()
    }

    #[cfg(test)]
    pub(super) fn is_shutdown_requested(&self) -> bool {
        self.shutdown.is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standalone_session(token: &str, desk_id: Uuid) -> Session {
        Session {
            id: SessionId::new(),
            user: DeskUser {
                id: light_core::UserId(Uuid::new_v4()),
                name: "Standalone operator".into(),
                enabled: true,
            },
            token: token.into(),
            connected: true,
            desk: ControlDesk {
                id: desk_id,
                name: "Standalone desk".into(),
                osc_alias: "standalone".into(),
                columns: 8,
                rows: 1,
                buttons: 3,
                playback_layout: None,
            },
        }
    }

    #[test]
    fn session_resource_operates_without_the_server_state_bag() {
        let sessions = SessionResource::new();
        let desk_id = Uuid::new_v4();
        let session = standalone_session("standalone-token", desk_id);
        let client_id = Uuid::new_v4();

        sessions.insert_session(session.clone());
        sessions.bind_client(session.id, client_id);
        assert_eq!(
            sessions.session_for_token("standalone-token").unwrap().id,
            session.id
        );
        assert!(sessions.session_token_matches(session.id, "standalone-token"));
        assert!(sessions.client_connected(client_id));
        assert!(sessions.desk_in_use(desk_id));

        let context = file_manager::FileInputContext {
            instance_id: "standalone-files".into(),
            action: file_manager::FileInputAction::Copy,
            session_id: session.id,
            desk_id,
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(60),
        };
        sessions
            .try_claim_file_input_context(context, || Ok(()))
            .unwrap();
        assert!(sessions.file_input_context(desk_id).is_some());
        assert!(matches!(
            sessions.route_file_input(
                desk_id,
                "enter",
                std::time::Instant::now() + std::time::Duration::from_secs(60),
            ),
            SessionFileInputRoute::Dispatch(_)
        ));
        assert!(sessions.release_session_file_input(&session).is_some());

        assert_eq!(sessions.unbind_client(session.id), Some(client_id));
        assert_eq!(
            sessions.remove_session(session.id).unwrap().token,
            "standalone-token"
        );
        assert_eq!(sessions.session_count(), 0);
    }

    #[test]
    fn event_resource_operates_without_the_server_state_bag() {
        let events = EventResource::new(EventBus::new(4));
        assert_eq!(events.latest_sequence(), 0);
        assert_eq!(
            events.record_audit("standalone", serde_json::json!({"value": 1})),
            1
        );
        assert_eq!(events.audit_after(0)[0].kind, "standalone");

        let filter = light_application::EventFilter::for_desk(Uuid::new_v4());
        let subscription = events.subscribe(
            filter.clone(),
            light_application::SubscriptionOptions::default(),
        );
        events.publish(light_application::EventDraft::configuration_changed(
            light_application::NotificationRevision { revision: 1 },
        ));
        assert_eq!(events.latest_sequence(), 1);
        let light_application::EventReplay::Events(replayed) = events.replay(0, &filter) else {
            panic!("standalone event resource should retain its event");
        };
        assert_eq!(replayed.len(), 1);
        assert!(subscription.try_next().is_some());
    }

    #[test]
    fn installation_resource_operates_without_the_server_state_bag() {
        let mut installation = InstallationResource::new(
            DeskStore::open(":memory:").unwrap(),
            light_fixture::FixtureLibrary::open(":memory:").unwrap(),
            PathBuf::new(),
            DeskConfiguration::default(),
            None,
        );

        assert!(installation.fixture_definitions().unwrap().is_empty());
        assert!(installation.fixture_profiles().unwrap().is_empty());
        assert!(installation.fixture_library_warnings().unwrap().is_empty());
        assert_eq!(installation.data_dir(), std::path::Path::new(""));
        assert_eq!(
            installation.configuration().frame_rate_hz,
            DeskConfiguration::default().frame_rate_hz
        );
        assert!(installation.desk_token_matches(None));
        installation.set_desk_token("standalone-token");
        assert!(installation.desk_token_matches(Some("standalone-token")));
        assert!(!installation.desk_token_matches(Some("wrong-token")));

        installation.set_setting("standalone", "available").unwrap();
        assert_eq!(
            installation.setting("standalone").unwrap().as_deref(),
            Some("available")
        );
    }

    #[test]
    fn media_resource_operates_without_the_server_state_bag() {
        let media = MediaResource::default();
        let fixture_id = light_core::FixtureId::new();
        let key = ThumbnailKey {
            fixture: fixture_id.0.to_string(),
            library_type: 1,
            library: LibraryId::ROOT,
            element: 7,
        };
        media
            .put_thumbnails([(
                key.clone(),
                light_media::MediaImage {
                    format: light_media::ImageFormat::Jpeg,
                    width: 1,
                    height: 1,
                    bytes: vec![1],
                },
            )])
            .unwrap();
        media.record_status(fixture_id, None);

        assert_eq!(media.thumbnail(&key).unwrap().image.bytes, vec![1]);
        assert!(media.status(fixture_id).online);

        media.invalidate_fixture(fixture_id);
        assert!(media.thumbnail(&key).is_none());
        assert!(!media.status(fixture_id).online);
    }

    #[test]
    fn highlight_resource_operates_without_the_server_state_bag() {
        let highlight = HighlightResource::new(Arc::new(HighlightRegistry::default()));
        let session_id = SessionId::new();
        let fixture_id = light_core::FixtureId::new();

        assert!(highlight.set_patch_preview(session_id, HashSet::from([fixture_id])));
        assert_eq!(highlight.output_fixtures(), HashSet::from([fixture_id]));

        highlight.remove_patch_preview(session_id);
        assert!(highlight.output_fixtures().is_empty());
    }

    #[test]
    fn integration_resource_operates_without_the_server_state_bag() {
        let integrations =
            IntegrationResource::new(Arc::new(matter::MatterBridgeAdapter::default()), None, None);
        let source: SocketAddr = "127.0.0.1:19010".parse().unwrap();
        let session_id = SessionId::new();
        integrations.register_osc_subscriber(
            "standalone".into(),
            OscSubscriber {
                desk_alias: "main".into(),
                target: source,
                command_source: source,
                session_id,
                last_seen: Instant::now(),
                shifted: false,
                shift_held: false,
                update_record_started: None,
                update_first_release: None,
                last_highlight_action: None,
            },
        );

        assert!(integrations.hardware_connected());
        assert_eq!(
            integrations
                .osc_subscriber_for_source(source)
                .unwrap()
                .session_id,
            session_id
        );
        integrations.set_shift(source, true);
        assert!(integrations.osc_subscriber("standalone").unwrap().shifted);
        assert_eq!(
            integrations
                .unregister_osc_subscriber("standalone")
                .unwrap()
                .session_id,
            session_id
        );
        assert!(!integrations.hardware_connected());
    }

    #[test]
    fn output_resource_operates_without_the_server_state_bag() {
        let events = EventBus::default();
        let output = OutputResource::new(
            OutputRuntimeService::new(events.clone()),
            SpeedGroupService::new(events),
            Arc::new(Engine::new(ProgrammerRegistry::default())),
            Arc::new(std::sync::Mutex::new(OutputHealth::default())),
            Arc::new(AtomicU16::new(44)),
            OutputControlCapability::new(Arc::new(Mutex::new(OutputControl::default()))),
            Arc::new(Mutex::new(TimecodeRouter::default())),
            None,
            Arc::default(),
            None,
            Arc::new(Mutex::new(std::array::from_fn(|index| {
                SpeedGroupController::new(
                    default_speed_groups()[index],
                    SoundToLightConfig::default(),
                )
                .unwrap()
            }))),
            Arc::new(Mutex::new(light_dynamics::DynamicRuntime::default())),
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(super::visualization_frame::VisualizationFrameHub::default()),
        );

        output.apply_runtime_control(Some(0.5), Some(true)).unwrap();
        assert_eq!(output.control_projection().revision, 1);
        assert_eq!(output.control_projection().grand_master, 0.5);
        assert!(output.control_projection().blackout);

        output.set_dmx_override(1, 7, Some(201));
        assert_eq!(output.dmx_override(1, 7), Some(201));
        output.set_manual_speed_group(0, 128.5, 0, true).unwrap();
        assert_eq!(output.speed_group_manual_bpm(0), 128.5);
    }

    #[tokio::test]
    async fn lifecycle_resource_operates_without_the_server_state_bag() {
        let lifecycle = LifecycleResource::new(CancellationToken::new());
        let cancellation = lifecycle.cancellation_token();
        assert!(!lifecycle.is_shutdown_requested());

        lifecycle.request_shutdown();

        cancellation.cancelled().await;
        assert!(lifecycle.is_shutdown_requested());
    }

    #[test]
    fn lifecycle_resource_rejects_work_when_supervisor_queue_is_full() {
        let lifecycle = LifecycleResource::new(CancellationToken::new());
        for _ in 0..RUNTIME_TASK_QUEUE_CAPACITY {
            lifecycle.schedule(async { Ok(()) }).unwrap();
        }

        let error = lifecycle.schedule(async { Ok(()) }).unwrap_err();

        assert_eq!(error.to_string(), "runtime task supervisor queue is full");
    }

    #[tokio::test]
    async fn replay_resource_preserves_fingerprint_and_replayed_metadata() {
        let resource = ReplayResource::default();
        let key = desk_management_v2::ReplayKey::new(
            SessionId(Uuid::from_u128(1)),
            "test-edit",
            "request-1",
        );
        let fingerprint = serde_json::json!({"value": 1});
        resource
            .insert_desk_management(
                key.clone(),
                fingerprint.clone(),
                serde_json::json!({"request_id":"request-1","replayed":false}),
            )
            .await;

        let replayed = resource
            .lookup_desk_management(&key, &fingerprint)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(replayed["replayed"], true);

        let conflict = resource
            .lookup_desk_management(&key, &serde_json::json!({"value": 2}))
            .await
            .unwrap_err();
        assert_eq!(conflict.status, StatusCode::CONFLICT);
        assert_eq!(
            conflict.message,
            "request_id was already used for a different edit"
        );
    }

    #[tokio::test]
    async fn independent_replay_capabilities_do_not_share_a_lock() {
        let resource = ReplayResource::default();
        let _show_library_guard = resource.show_library.lock().await;
        let key = desk_management_v2::ReplayKey::new(
            SessionId(Uuid::from_u128(2)),
            "test-edit",
            "request-2",
        );

        tokio::time::timeout(
            Duration::from_millis(100),
            resource.lookup_desk_management(&key, &serde_json::json!({"value": 1})),
        )
        .await
        .expect("an unrelated replay capability must not wait for the show-library cache")
        .unwrap();
    }

    #[test]
    fn programming_resource_operates_without_app_state_or_registry_access() {
        let programmers = ProgrammerRegistry::default();
        let service = ProgrammingService::new(
            programmers.clone(),
            EventBus::default(),
            Arc::new(HighlightRegistry::default()),
        );
        let resource = ProgrammingResource::new(programmers, service);
        let session_id = SessionId(Uuid::from_u128(31));
        let user_id = light_core::UserId(Uuid::from_u128(32));
        let fixture_id = light_core::FixtureId(Uuid::from_u128(33));

        resource.start(session_id, user_id);
        resource.select(session_id, [fixture_id]);
        assert_eq!(
            resource
                .selection(session_id)
                .expect("standalone selection")
                .selected,
            vec![fixture_id]
        );

        resource
            .with_staged_command(session_id, |staged| {
                staged.set_command_line(session_id, "FIXTURE 1".into());
                Ok::<_, String>(())
            })
            .unwrap();
        assert_eq!(
            resource
                .get(session_id)
                .expect("standalone programmer")
                .command_line,
            "FIXTURE 1"
        );
    }

    #[tokio::test]
    async fn active_show_resource_operates_without_the_server_state_bag() {
        let events = EventBus::default();
        let service = ActiveShowService::new(events);
        let resource = ActiveShowResource::new(
            ActiveShowCoordinator::new(),
            Arc::default(),
            Some("recovery".into()),
            service.clone(),
            ShowPatchService::new(service.clone()),
            SelectiveShowImportService::new(service),
        );
        let show = ShowEntry {
            id: light_core::ShowId::new(),
            name: "Standalone".into(),
            path: "standalone.show".into(),
            revision: 3,
            updated_at: String::new(),
            revision_copy: None,
        };

        resource.replace_current(Some(show.clone()));
        let current = resource.current().expect("standalone show");
        assert_eq!(current.id, show.id);
        assert_eq!(current.name, show.name);
        assert_eq!(resource.error().as_deref(), Some("recovery"));
        resource.set_error(None);
        assert!(resource.error().is_none());

        let permit = resource.acquire().await;
        assert!(resource.try_acquire().is_err());
        drop(permit);
        assert!(resource.try_acquire().is_ok());
    }
}
