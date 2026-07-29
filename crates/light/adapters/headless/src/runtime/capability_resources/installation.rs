use super::*;

impl InstallationResource {
    pub(in crate::runtime) fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    pub(in crate::runtime) fn configuration(&self) -> DeskConfiguration {
        self.configuration.read().clone()
    }

    pub(in crate::runtime) fn replace_configuration(&self, configuration: DeskConfiguration) {
        *self.configuration.write() = configuration;
    }

    pub(in crate::runtime) fn update_configuration<R>(
        &self,
        update: impl FnOnce(&mut DeskConfiguration) -> R,
    ) -> R {
        update(&mut self.configuration.write())
    }

    pub(in crate::runtime) fn desk_token_matches(&self, candidate: Option<&str>) -> bool {
        self.desk_token
            .as_deref()
            .is_none_or(|required| candidate == Some(required))
    }

    #[cfg(test)]
    pub(in crate::runtime) fn set_desk_token(&mut self, token: impl Into<Arc<str>>) {
        self.desk_token = Some(token.into());
    }

    pub(in crate::runtime) fn show_library(
        &self,
    ) -> Result<Vec<ShowEntry>, light_show::StoreError> {
        self.desk.lock().library()
    }

    pub(in crate::runtime) fn show(
        &self,
        id: light_core::ShowId,
    ) -> Result<Option<ShowEntry>, light_show::StoreError> {
        self.desk.lock().show(id)
    }

    pub(in crate::runtime) fn upsert_show(
        &self,
        name: &str,
        path: &str,
        overwrite: bool,
    ) -> Result<ShowEntry, light_show::StoreError> {
        self.desk.lock().upsert_show(name, path, overwrite)
    }

    pub(in crate::runtime) fn upsert_show_with_revision_copy(
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

    pub(in crate::runtime) fn mark_show_updated(
        &self,
        id: light_core::ShowId,
    ) -> Result<ShowEntry, light_show::StoreError> {
        self.desk.lock().mark_show_updated(id)
    }

    pub(in crate::runtime) fn rename_show(
        &self,
        id: light_core::ShowId,
        name: &str,
        path: &str,
    ) -> Result<ShowEntry, light_show::StoreError> {
        self.desk.lock().rename_show(id, name, path)
    }

    pub(in crate::runtime) fn remove_show(
        &self,
        id: light_core::ShowId,
    ) -> Result<bool, light_show::StoreError> {
        self.desk.lock().remove_show(id)
    }

    pub(in crate::runtime) fn show_revisions(
        &self,
        show_id: light_core::ShowId,
    ) -> Result<Vec<ShowRevision>, light_show::StoreError> {
        self.desk.lock().show_revisions(show_id)
    }

    pub(in crate::runtime) fn show_revision(
        &self,
        show_id: light_core::ShowId,
        revision: light_core::Revision,
    ) -> Result<Option<ShowRevision>, light_show::StoreError> {
        self.desk.lock().show_revision(show_id, revision)
    }

    pub(in crate::runtime) fn add_show_revision(
        &self,
        show_id: light_core::ShowId,
        name: &str,
        path: &str,
    ) -> Result<ShowRevision, light_show::StoreError> {
        self.desk.lock().add_show_revision(show_id, name, path)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn active_show(
        &self,
    ) -> Result<Option<ShowEntry>, light_show::StoreError> {
        self.desk.lock().active_show()
    }

    pub(in crate::runtime) fn set_active_show(
        &self,
        id: Option<light_core::ShowId>,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().set_active_show(id)
    }

    pub(in crate::runtime) fn setting(
        &self,
        key: &str,
    ) -> Result<Option<String>, light_show::StoreError> {
        self.desk.lock().setting(key)
    }

    pub(in crate::runtime) fn set_setting(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().set_setting(key, value)
    }

    pub(in crate::runtime) fn users(&self) -> Result<Vec<DeskUser>, light_show::StoreError> {
        self.desk.lock().users()
    }

    pub(in crate::runtime) fn add_user(
        &self,
        name: &str,
    ) -> Result<DeskUser, light_show::StoreError> {
        self.desk.lock().add_user(name)
    }

    pub(in crate::runtime) fn find_user(
        &self,
        name: &str,
    ) -> Result<Option<DeskUser>, light_show::StoreError> {
        self.desk.lock().find_user(name)
    }

    pub(in crate::runtime) fn update_user(
        &self,
        id: light_core::UserId,
        name: &str,
        enabled: bool,
    ) -> Result<DeskUser, light_show::StoreError> {
        self.desk.lock().update_user(id, name, enabled)
    }

    pub(in crate::runtime) fn save_session(
        &self,
        session: &PersistedSession,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().save_session(session)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn persisted_sessions(
        &self,
    ) -> Result<Vec<PersistedSession>, light_show::StoreError> {
        self.desk.lock().persisted_sessions()
    }

    pub(in crate::runtime) fn delete_session(
        &self,
        id: SessionId,
    ) -> Result<bool, light_show::StoreError> {
        self.desk.lock().delete_session(id)
    }

    pub(in crate::runtime) fn desks(&self) -> Result<Vec<ControlDesk>, light_show::StoreError> {
        self.desk.lock().desks()
    }

    pub(in crate::runtime) fn control_desk(
        &self,
        id: Uuid,
    ) -> Result<Option<ControlDesk>, light_show::StoreError> {
        self.desk.lock().control_desk(id)
    }

    pub(in crate::runtime) fn control_desk_by_alias(
        &self,
        alias: &str,
    ) -> Result<Option<ControlDesk>, light_show::StoreError> {
        self.desk.lock().control_desk_by_alias(alias)
    }

    pub(in crate::runtime) fn client_desks(
        &self,
    ) -> Result<Vec<light_show::ClientDesk>, light_show::StoreError> {
        self.desk.lock().client_desks()
    }

    pub(in crate::runtime) fn resolve_client_desk(
        &self,
        client_id: Uuid,
        remembered_desk_id: Option<Uuid>,
    ) -> Result<ControlDesk, light_show::StoreError> {
        self.desk
            .lock()
            .resolve_client_desk(client_id, remembered_desk_id)
    }

    pub(in crate::runtime) fn touch_client(
        &self,
        client_id: Uuid,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().touch_client(client_id)
    }

    pub(in crate::runtime) fn remove_client_desk(
        &self,
        desk_id: Uuid,
    ) -> Result<bool, light_show::StoreError> {
        self.desk.lock().remove_client_desk(desk_id)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn add_desk(
        &self,
        name: &str,
        alias: &str,
    ) -> Result<ControlDesk, light_show::StoreError> {
        self.desk.lock().add_desk(name, alias)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::runtime) fn update_desk(
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

    pub(in crate::runtime) fn desk_page(
        &self,
        desk_id: Uuid,
        show_id: light_core::ShowId,
    ) -> Result<u8, light_show::StoreError> {
        self.desk.lock().desk_page(desk_id, show_id)
    }

    pub(in crate::runtime) fn set_desk_page(
        &self,
        desk_id: Uuid,
        show_id: light_core::ShowId,
        page: u8,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().set_desk_page(desk_id, show_id, page)
    }

    pub(in crate::runtime) fn selected_playback(
        &self,
        desk_id: Uuid,
        show_id: light_core::ShowId,
    ) -> Result<Option<u16>, light_show::StoreError> {
        self.desk.lock().selected_playback(desk_id, show_id)
    }

    pub(in crate::runtime) fn set_selected_playback(
        &self,
        desk_id: Uuid,
        show_id: light_core::ShowId,
        playback: Option<u16>,
    ) -> Result<(), light_show::StoreError> {
        self.desk
            .lock()
            .set_selected_playback(desk_id, show_id, playback)
    }

    pub(in crate::runtime) fn put_screen(
        &self,
        screen: ScreenConfiguration,
    ) -> Result<ScreenConfiguration, light_show::StoreError> {
        self.desk.lock().put_screen(screen)
    }

    pub(in crate::runtime) fn screens(
        &self,
    ) -> Result<Vec<ScreenConfiguration>, light_show::StoreError> {
        self.desk.lock().screens()
    }

    pub(in crate::runtime) fn screen(
        &self,
        id: Uuid,
    ) -> Result<Option<ScreenConfiguration>, light_show::StoreError> {
        self.desk.lock().screen(id)
    }

    pub(in crate::runtime) fn delete_screen(&self, id: Uuid) -> Result<(), light_show::StoreError> {
        self.desk.lock().delete_screen(id)
    }

    pub(in crate::runtime) fn screen_page(
        &self,
        screen_id: Uuid,
        show_id: light_core::ShowId,
    ) -> Result<u8, light_show::StoreError> {
        self.desk.lock().screen_page(screen_id, show_id)
    }

    pub(in crate::runtime) fn set_screen_page(
        &self,
        screen_id: Uuid,
        show_id: light_core::ShowId,
        page: u8,
    ) -> Result<(), light_show::StoreError> {
        self.desk.lock().set_screen_page(screen_id, show_id, page)
    }

    pub(in crate::runtime) fn bootstrap_desk_data(
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
    pub(in crate::runtime) fn ensure_default_show_available(&self) -> anyhow::Result<ShowEntry> {
        startup_state::ensure_default_show_available(&self.desk.lock(), &self.data_dir)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn replace_desk_store(&self, desk: InstallationDeskStore) {
        *self.desk.lock() = desk;
    }

    pub(in crate::runtime) fn fixture_definitions(
        &self,
    ) -> Result<Vec<light_fixture::FixtureDefinition>, light_fixture::FixtureError> {
        self.fixture_library.lock().definitions()
    }

    pub(in crate::runtime) fn fixture_profiles(
        &self,
    ) -> Result<Vec<light_fixture::FixtureProfile>, light_fixture::FixtureError> {
        self.fixture_library.lock().profiles()
    }

    pub(in crate::runtime) fn fixture_library_warnings(
        &self,
    ) -> Result<Vec<String>, light_fixture::FixtureError> {
        self.fixture_library.lock().migration_warnings()
    }

    #[cfg(test)]
    pub(in crate::runtime) fn fixture_profile(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<light_fixture::FixtureProfile>, light_fixture::FixtureError> {
        self.fixture_library.lock().profile(id, revision)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn fixture_profile_source_gdtf(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, light_fixture::FixtureError> {
        self.fixture_library
            .lock()
            .profile_source_gdtf(id, revision)
    }

    pub(in crate::runtime) fn fixture_profile_revisions(
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

    pub(in crate::runtime) fn save_fixture_profile(
        &self,
        profile: light_fixture::FixtureProfile,
        expected_revision: u32,
    ) -> Result<light_fixture::FixtureProfile, light_fixture::FixtureError> {
        self.fixture_library
            .lock()
            .save_profile(profile, expected_revision)
    }

    pub(in crate::runtime) fn delete_fixture_profile(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<bool, light_fixture::FixtureError> {
        self.fixture_library.lock().delete_profile(id, revision)
    }

    pub(in crate::runtime) fn import_fixture_package(
        &self,
        package: &[u8],
    ) -> Result<light_fixture::FixtureProfile, light_fixture::FixtureError> {
        self.fixture_library.lock().import_fixture_package(package)
    }

    pub(in crate::runtime) fn attach_fixture_profile_gdtf(
        &self,
        id: light_core::FixtureId,
        revision: u32,
        source: &[u8],
    ) -> Result<bool, light_fixture::FixtureError> {
        self.fixture_library
            .lock()
            .set_profile_source_gdtf(id, revision, source)
    }

    pub(in crate::runtime) fn import_fixture_definition(
        &self,
        definition: &light_fixture::FixtureDefinition,
    ) -> Result<light_fixture::FixtureDefinition, light_fixture::FixtureError> {
        let json = serde_json::to_string(definition)?;
        self.fixture_library.lock().import_json(&json)
    }

    pub(in crate::runtime) fn import_fixture_definition_with_source(
        &self,
        definition: &light_fixture::FixtureDefinition,
        source: &[u8],
    ) -> Result<light_fixture::FixtureDefinition, light_fixture::FixtureError> {
        let json = serde_json::to_string(definition)?;
        self.fixture_library
            .lock()
            .import_json_with_source(&json, Some(source))
    }

    pub(in crate::runtime) fn delete_fixture_definition(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<bool, light_fixture::FixtureError> {
        self.fixture_library.lock().delete(id, revision)
    }

    pub(in crate::runtime) fn export_fixture_package(
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

    pub(in crate::runtime) fn fixture_source_gdtf(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, light_fixture::FixtureError> {
        self.fixture_library.lock().source_gdtf(id, revision)
    }

    pub(in crate::runtime) fn fixture_profile_revision_document(
        &self,
        id: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<serde_json::Value>, light_fixture::FixtureError> {
        self.fixture_library
            .lock()
            .profile_revision_document(id, revision)
    }
}
