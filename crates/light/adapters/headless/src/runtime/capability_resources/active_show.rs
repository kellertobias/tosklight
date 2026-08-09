use super::*;

impl ActiveShowResource {
    pub(in crate::runtime) fn current(&self) -> Option<ShowEntry> {
        self.active.read().clone()
    }

    pub(in crate::runtime) fn compatibility_reports(
        &self,
    ) -> Result<Vec<serde_json::Value>, light_show::StoreError> {
        let Some(show) = self.current() else {
            return Ok(Vec::new());
        };
        let document =
            super::super::capabilities::active_show::repository::ActiveShowRepository::open(
                &show.path,
            )?
            .portable_document()?;
        Ok(document
            .objects_of_kind("compatibility_report")
            .map(|object| object.body().clone())
            .collect())
    }

    pub(in crate::runtime) fn mutate_objects<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<
            light_application::MutateActiveShowObjectsCommand,
        >,
        ports: &P,
    ) -> Result<light_application::MutateActiveShowObjectsResult, light_application::ActionError>
    {
        self.service.mutate_objects(action, ports)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn undo_object<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::UndoActiveShowObjectCommand>,
        ports: &P,
    ) -> Result<light_application::UndoActiveShowObjectResult, light_application::ActionError> {
        self.service.undo_object(action, ports)
    }

    pub(in crate::runtime) fn undo_recording<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<
            light_application::UndoActiveShowRecordingCommand,
        >,
        ports: &P,
    ) -> Result<light_application::MutateActiveShowObjectsResult, light_application::ActionError>
    {
        self.service.undo_recording(action, ports)
    }

    pub(in crate::runtime) fn mutate_output_route<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::MutateOutputRouteCommand>,
        ports: &P,
    ) -> Result<light_application::MutateOutputRouteResult, light_application::ActionError> {
        self.service.mutate_output_route(action, ports)
    }

    pub(in crate::runtime) fn create_output_route_range<P: light_application::ActiveShowPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::CreateOutputRouteRangeCommand>,
        ports: &P,
    ) -> Result<light_application::CreateOutputRouteRangeResult, light_application::ActionError>
    {
        self.service.create_output_route_range(action, ports)
    }

    pub(in crate::runtime) fn commit_programming_cue<
        P: light_application::ProgrammingCueActiveShowPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        commit: &light_application::ProgrammingCueCommit,
        ports: &P,
    ) -> Result<light_application::ProgrammingCueCommitResult, light_application::ActionError> {
        self.service.commit_programming_cue(context, commit, ports)
    }

    pub(in crate::runtime) fn commit_programming_group<
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

    pub(in crate::runtime) fn commit_programming_preset<
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

    pub(in crate::runtime) fn commit_group_management<
        P: light_application::GroupManagementActiveShowPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        commit: &light_application::GroupManagementCommit,
        ports: &P,
    ) -> Result<light_application::GroupManagementCommitResult, light_application::ActionError>
    {
        self.service.commit_group_management(context, commit, ports)
    }

    pub(in crate::runtime) fn patch_snapshot<P: light_application::ShowPatchPorts>(
        &self,
        context: &light_application::ActionContext,
        show_id: light_core::ShowId,
        ports: &P,
    ) -> Result<light_application::PatchSnapshot, light_application::ActionError> {
        self.patch.snapshot(context, show_id, ports)
    }

    pub(in crate::runtime) fn patch_fixtures<P: light_application::ShowPatchPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::PatchFixturesCommand>,
        ports: &P,
    ) -> Result<light_application::PatchFixturesResult, light_application::ActionError> {
        self.patch.handle(action, ports)
    }

    pub(in crate::runtime) fn preview_selective_import<
        P: light_application::SelectiveShowImportPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        request: light_application::SelectiveShowImportRequest,
        ports: &P,
    ) -> Result<light_application::SelectiveShowImportPreview, light_application::ActionError> {
        self.selective_import.preview(context, request, ports)
    }

    pub(in crate::runtime) fn apply_selective_import<
        P: light_application::SelectiveShowImportPorts,
    >(
        &self,
        action: light_application::ActionEnvelope<
            light_application::ApplySelectiveShowImportCommand,
        >,
        ports: &P,
    ) -> Result<light_application::SelectiveShowImportResult, light_application::ActionError> {
        self.selective_import.apply(action, ports)
    }

    pub(in crate::runtime) fn undo_selective_import<
        P: light_application::SelectiveShowImportPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        target: &light_application::SelectiveShowImportUndoTarget,
        ports: &P,
    ) -> Result<light_core::Revision, light_application::ActionError> {
        self.selective_import.undo(context, target, ports)
    }

    pub(in crate::runtime) fn apply_mvr_import<P: light_application::ShowPatchPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::ApplyActiveMvrImportCommand>,
        ports: &P,
    ) -> Result<light_application::ActiveMvrImportResult, light_application::ActionError> {
        light_application::MvrImportService::new(self.service.clone()).apply(action, ports)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn patch_profile_resolution_probe(
        &self,
    ) -> Arc<PatchProfileResolutionPause> {
        Arc::clone(&self.patch_profile_resolution)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn http_lifecycle_probe(&self) -> Arc<ActiveShowLifecyclePause> {
        Arc::clone(&self.http_lifecycle)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn preload_store_release_lifecycle_probe(
        &self,
    ) -> Arc<ActiveShowLifecyclePause> {
        Arc::clone(&self.preload_store_release_lifecycle)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn patch_lifecycle_probe(&self) -> Arc<ActiveShowLifecyclePause> {
        Arc::clone(&self.patch_lifecycle)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn pause_http_lifecycle_if_armed(&self) {
        self.http_lifecycle.pause_if_armed();
    }

    #[cfg(test)]
    pub(in crate::runtime) fn pause_patch_lifecycle_if_armed(&self) {
        self.patch_lifecycle.pause_if_armed();
    }

    pub(in crate::runtime) async fn acquire(&self) -> ActiveShowPermit {
        self.activation.acquire().await
    }

    pub(in crate::runtime) async fn acquire_shared(&self) -> ActiveShowPermit {
        self.activation.acquire_shared().await
    }

    /// Serializes whole show-change workflows without excluding output rendering. The output
    /// coordinator is acquired separately only for the prepared runtime and identity swap.
    pub(in crate::runtime) async fn acquire_show_change(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.show_change.clone().lock_owned().await
    }

    pub(in crate::runtime) fn acquire_blocking(&self) -> ActiveShowPermit {
        self.activation.acquire_blocking()
    }

    pub(in crate::runtime) fn try_acquire(
        &self,
    ) -> Result<ActiveShowPermit, tokio::sync::TryLockError> {
        self.activation.try_acquire()
    }

    pub(in crate::runtime) fn replace_current(&self, show: Option<ShowEntry>) {
        *self.active.write() = show;
    }

    pub(in crate::runtime) fn update_current(&self, update: impl FnOnce(&mut ShowEntry)) -> bool {
        let mut active = self.active.write();
        let Some(show) = active.as_mut() else {
            return false;
        };
        update(show);
        true
    }

    pub(in crate::runtime) fn output_projection(&self) -> ActiveShowProjection {
        ActiveShowProjection {
            active: Arc::clone(&self.active),
        }
    }

    pub(in crate::runtime) fn error(&self) -> Option<String> {
        self.error.read().clone()
    }

    pub(in crate::runtime) fn set_error(&self, error: Option<String>) {
        *self.error.write() = error;
    }

    pub(in crate::runtime) fn clear_document_cache(&self) {
        *self.document.lock() = None;
    }

    pub(in crate::runtime) fn document_cache(&self) -> ActiveShowDocumentCache {
        ActiveShowDocumentCache {
            document: Arc::clone(&self.document),
        }
    }

    pub(in crate::runtime) fn stage_mvr_import(&self, token: Uuid, import: StagedMvrImport) {
        let now = Instant::now();
        let mut imports = self.mvr_imports.lock();
        imports.retain(|_, item| now.duration_since(item.created) < Duration::from_secs(30 * 60));
        imports.insert(token, import);
    }

    pub(in crate::runtime) fn take_mvr_import(&self, token: Uuid) -> Option<StagedMvrImport> {
        self.mvr_imports.lock().remove(&token)
    }

    pub(in crate::runtime) fn update_backup_checkpoint(
        &self,
        update: impl FnOnce(&mut Option<(light_core::ShowId, u64)>),
    ) {
        update(&mut self.backup_checkpoint.lock());
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct ActiveShowCoordinator {
    lock: Arc<tokio::sync::RwLock<()>>,
}

impl ActiveShowCoordinator {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            lock: Arc::new(tokio::sync::RwLock::new(())),
        }
    }

    pub(in crate::runtime) async fn acquire(&self) -> ActiveShowPermit {
        ActiveShowPermit::Exclusive(self.lock.clone().write_owned().await)
    }

    pub(in crate::runtime) async fn acquire_shared(&self) -> ActiveShowPermit {
        ActiveShowPermit::Shared(self.lock.clone().read_owned().await)
    }

    pub(in crate::runtime) fn acquire_blocking(&self) -> ActiveShowPermit {
        ActiveShowPermit::Exclusive(futures_lite::future::block_on(
            self.lock.clone().write_owned(),
        ))
    }

    pub(in crate::runtime) fn try_acquire(
        &self,
    ) -> Result<ActiveShowPermit, tokio::sync::TryLockError> {
        self.lock
            .clone()
            .try_read_owned()
            .map(ActiveShowPermit::Shared)
    }
}

pub(in crate::runtime) enum ActiveShowPermit {
    Shared(#[allow(dead_code)] tokio::sync::OwnedRwLockReadGuard<()>),
    Exclusive(#[allow(dead_code)] tokio::sync::OwnedRwLockWriteGuard<()>),
}

#[derive(Clone)]
pub(in crate::runtime) struct ActiveShowProjection {
    active: Arc<RwLock<Option<ShowEntry>>>,
}

impl ActiveShowProjection {
    pub(in crate::runtime) fn new(active: Arc<RwLock<Option<ShowEntry>>>) -> Self {
        Self { active }
    }

    pub(in crate::runtime) fn current(&self) -> Option<ShowEntry> {
        self.active.read().clone()
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct ActiveShowDocumentCache {
    document: Arc<Mutex<Option<light_show::PortableShowDocument>>>,
}

impl ActiveShowDocumentCache {
    pub(in crate::runtime) fn take(&self) -> Option<light_show::PortableShowDocument> {
        self.document.lock().take()
    }

    pub(in crate::runtime) fn replace(&self, document: Option<light_show::PortableShowDocument>) {
        *self.document.lock() = document;
    }

    #[cfg(test)]
    pub(in crate::runtime) fn snapshot(&self) -> Option<light_show::PortableShowDocument> {
        self.document.lock().clone()
    }
}
