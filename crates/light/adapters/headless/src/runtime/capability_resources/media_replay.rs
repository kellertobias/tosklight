use super::*;

#[derive(Clone)]
pub(in crate::runtime) struct MediaResource {
    cache: Arc<Mutex<MediaCache>>,
    status: Arc<RwLock<HashMap<light_core::FixtureId, MediaServerStatus>>>,
}

impl MediaResource {
    pub(in crate::runtime) fn new(cache: MediaCache) -> Self {
        Self {
            cache: Arc::new(Mutex::new(cache)),
            status: Arc::default(),
        }
    }

    pub(in crate::runtime) fn statuses(&self) -> HashMap<light_core::FixtureId, MediaServerStatus> {
        self.status.read().clone()
    }

    pub(in crate::runtime) fn status(
        &self,
        fixture_id: light_core::FixtureId,
    ) -> MediaServerStatus {
        self.status
            .read()
            .get(&fixture_id)
            .cloned()
            .unwrap_or_default()
    }

    pub(in crate::runtime) fn record_status(
        &self,
        fixture_id: light_core::FixtureId,
        error: Option<String>,
    ) {
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

    pub(in crate::runtime) fn put_thumbnails(
        &self,
        images: impl IntoIterator<Item = (ThumbnailKey, light_media::MediaImage)>,
    ) -> Result<(), light_media::MediaError> {
        let mut cache = self.cache.lock();
        for (key, image) in images {
            cache.put_thumbnail(key, image)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn thumbnail(
        &self,
        key: &ThumbnailKey,
    ) -> Option<light_media::CachedImage> {
        self.cache.lock().thumbnail(key)
    }

    pub(in crate::runtime) fn put_preview(
        &self,
        key: PreviewKey,
        image: light_media::MediaImage,
    ) -> Result<(), light_media::MediaError> {
        self.cache.lock().put_preview(key, image)
    }

    pub(in crate::runtime) fn preview(&self, key: &PreviewKey) -> Option<light_media::CachedImage> {
        self.cache.lock().preview(key)
    }

    pub(in crate::runtime) fn invalidate_fixture(&self, fixture_id: light_core::FixtureId) {
        self.cache.lock().clear_fixture(&fixture_id.0.to_string());
        self.status.write().remove(&fixture_id);
    }

    pub(in crate::runtime) fn invalidate_fixtures(
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

    pub(in crate::runtime) fn retain_fixtures(&self, fixture_ids: &HashSet<light_core::FixtureId>) {
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

impl ReplayResource {
    pub(in crate::runtime) async fn lookup_show_library(
        &self,
        key: &show_library_v2::ReplayKey,
        signature: &[u8; 32],
    ) -> Result<Option<light_wire::v2::show_library::ShowLibraryActionOutcome>, ApiError> {
        self.show_library.lock().await.get(key, signature)
    }

    pub(in crate::runtime) async fn insert_show_library(
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

    pub(in crate::runtime) async fn lookup_fixture_library(
        &self,
        key: &fixture_api_replay::ReplayKey,
        signature: &[u8; 32],
    ) -> Result<Option<light_wire::v2::fixture_library::FixtureLibraryActionOutcome>, ApiError>
    {
        self.fixture_library.lock().await.get(key, signature)
    }

    pub(in crate::runtime) async fn insert_fixture_library(
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

    pub(in crate::runtime) async fn lookup_show_object(
        &self,
        key: &show_objects_v2::ReplayKey,
        action: &light_wire::v2::show_objects::OutputRouteAction,
    ) -> Result<Option<light_wire::v2::show_objects::OutputRouteActionOutcome>, ApiError> {
        self.show_object.lock().await.get(key, action)
    }

    pub(in crate::runtime) async fn insert_show_object(
        &self,
        key: show_objects_v2::ReplayKey,
        action: light_wire::v2::show_objects::OutputRouteAction,
        outcome: light_wire::v2::show_objects::OutputRouteActionOutcome,
    ) {
        self.show_object.lock().await.insert(key, action, outcome);
    }

    pub(in crate::runtime) async fn lookup_show_object_intent(
        &self,
        key: &show_object_intents_v2::ReplayKey,
        action: &show_object_intents_v2::ReplayAction,
    ) -> Result<Option<light_wire::v2::show_objects::ShowObjectActionOutcome>, ApiError> {
        self.show_object_intent.lock().await.get(key, action)
    }

    pub(in crate::runtime) async fn insert_show_object_intent(
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

    pub(in crate::runtime) async fn lookup_schedule(
        &self,
        key: &schedules_v2::ReplayKey,
        action: &schedules_v2::ReplayAction,
    ) -> Result<Option<light_wire::v2::schedules::ScheduleMutationOutcome>, ApiError> {
        self.schedules.lock().await.get(key, action)
    }

    pub(in crate::runtime) async fn insert_schedule(
        &self,
        key: schedules_v2::ReplayKey,
        action: schedules_v2::ReplayAction,
        outcome: light_wire::v2::schedules::ScheduleMutationOutcome,
    ) {
        self.schedules.lock().await.insert(key, action, outcome);
    }

    pub(in crate::runtime) async fn lookup_preset_generation(
        &self,
        key: &live_action_http::PresetGenerationReplayKey,
        request: &light_wire::v2::live_action::GenerateFixturePresetsRequest,
    ) -> Result<Option<light_wire::v2::live_action::GenerateFixturePresetsOutcome>, ApiError> {
        self.preset_generation.lock().await.get(key, request)
    }

    pub(in crate::runtime) async fn insert_preset_generation(
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

    pub(in crate::runtime) async fn lookup_screen_configuration(
        &self,
        key: &screen_configuration_v2::ReplayKey,
        action: &light_wire::v2::screen_configuration::ScreenConfigurationAction,
    ) -> Result<
        Option<light_wire::v2::screen_configuration::ScreenConfigurationActionOutcome>,
        ApiError,
    > {
        self.screen_configuration.lock().await.get(key, action)
    }

    pub(in crate::runtime) async fn insert_screen_configuration(
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

    pub(in crate::runtime) async fn lookup_control_desk_configuration(
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

    pub(in crate::runtime) async fn insert_control_desk_configuration(
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

    pub(in crate::runtime) async fn lookup_desk_management(
        &self,
        key: &desk_management_v2::ReplayKey,
        fingerprint: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        self.desk_management.lock().await.get(key, fingerprint)
    }

    pub(in crate::runtime) async fn insert_desk_management(
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

    pub(in crate::runtime) async fn lookup_cue_thumbnails(
        &self,
        key: &cue_thumbnails_http::ReplayKey,
        thumbnails: &[light_wire::v2::cue_thumbnails::CueThumbnailUpload],
    ) -> Result<Option<light_wire::v2::cue_thumbnails::CueThumbnailUpdateOutcome>, ApiError> {
        self.cue_thumbnails.lock().get(key, thumbnails)
    }

    pub(in crate::runtime) async fn insert_cue_thumbnails(
        &self,
        key: cue_thumbnails_http::ReplayKey,
        thumbnails: Vec<light_wire::v2::cue_thumbnails::CueThumbnailUpload>,
        outcome: light_wire::v2::cue_thumbnails::CueThumbnailUpdateOutcome,
    ) {
        self.cue_thumbnails.lock().insert(key, thumbnails, outcome);
    }

    pub(in crate::runtime) async fn lookup_stage_layout(
        &self,
        key: &stage_layout_http::ReplayKey,
        action: &light_wire::v2::stage_layout::StageLayoutAction,
    ) -> Result<
        Option<light_wire::v2::stage_layout::StageLayoutActionOutcome>,
        stage_layout_http::StageLayoutHttpError,
    > {
        self.stage_layout.lock().get(key, action)
    }

    pub(in crate::runtime) async fn insert_stage_layout(
        &self,
        key: stage_layout_http::ReplayKey,
        action: light_wire::v2::stage_layout::StageLayoutAction,
        outcome: light_wire::v2::stage_layout::StageLayoutActionOutcome,
    ) {
        self.stage_layout.lock().insert(key, action, outcome);
    }

    pub(in crate::runtime) async fn lookup_visualizer_view(
        &self,
        key: &visualizer_view_http::ReplayKey,
        action: &visualizer_view_http::ReplayAction,
    ) -> Result<Option<light_wire::v2::visualizer_view::VisualizerViewUpdateOutcome>, ApiError>
    {
        self.visualizer_view.lock().get(key, action)
    }

    pub(in crate::runtime) async fn insert_visualizer_view(
        &self,
        key: visualizer_view_http::ReplayKey,
        action: visualizer_view_http::ReplayAction,
        outcome: light_wire::v2::visualizer_view::VisualizerViewUpdateOutcome,
    ) {
        self.visualizer_view.lock().insert(key, action, outcome);
    }

    pub(in crate::runtime) async fn lookup_virtual_playback_zones(
        &self,
        key: &virtual_playback_zones_http::ReplayKey,
        action: &virtual_playback_zones_http::ReplayAction,
    ) -> Result<
        Option<light_wire::v2::virtual_playback_zones::VirtualPlaybackExclusionUpdateOutcome>,
        ApiError,
    > {
        self.virtual_playback_zones.lock().get(key, action)
    }

    pub(in crate::runtime) async fn insert_virtual_playback_zones(
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
    pub(in crate::runtime) fn show_object_cache_is_available(&self) -> bool {
        self.show_object.try_lock().is_ok()
    }
}
