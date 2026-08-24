use super::*;

#[derive(Clone)]
pub(in crate::runtime) struct PlaybackResource {
    service: PlaybackService,
    topology: PlaybackTopologyService,
    telemetry: Arc<playback_telemetry::PlaybackTelemetrySampler>,
}

impl PlaybackResource {
    pub(in crate::runtime) fn new(
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

    pub(in crate::runtime) fn handle(
        &self,
        envelope: light_application::ActionEnvelope<light_application::PlaybackCommand>,
        ports: &dyn light_application::PlaybackPorts,
    ) -> Result<light_application::PlaybackResult, light_application::ActionError> {
        self.service.handle(envelope, ports)
    }

    pub(in crate::runtime) fn snapshot(
        &self,
        context: &light_application::ActionContext,
        identities: &[light_application::PlaybackRuntimeIdentity],
        ports: &dyn light_application::PlaybackPorts,
    ) -> Result<light_application::PlaybackRuntimeSnapshot, light_application::ActionError> {
        self.service.snapshot(context, identities, ports)
    }

    pub(in crate::runtime) fn run_unit_of_work<O>(
        &self,
        operation: O,
    ) -> light_application::PlaybackOperationResult<O::Output>
    where
        O: light_application::PlaybackUnitOfWork,
    {
        self.service.run_unit_of_work(operation)
    }

    pub(in crate::runtime) fn handle_topology<P: light_application::PlaybackTopologyPorts>(
        &self,
        envelope: light_application::ActionEnvelope<light_application::PlaybackTopologyCommand>,
        ports: &P,
    ) -> Result<light_application::PlaybackTopologyResult, light_application::ActionError> {
        self.topology.handle(envelope, ports)
    }

    pub(in crate::runtime) fn render_capability(&self) -> PlaybackRenderCapability {
        PlaybackRenderCapability::new(self.service.clone(), Arc::clone(&self.telemetry))
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct PlaybackRenderCapability {
    service: PlaybackService,
    telemetry: Arc<playback_telemetry::PlaybackTelemetrySampler>,
}

impl PlaybackRenderCapability {
    pub(in crate::runtime) fn new(
        service: PlaybackService,
        telemetry: Arc<playback_telemetry::PlaybackTelemetrySampler>,
    ) -> Self {
        Self { service, telemetry }
    }

    pub(in crate::runtime) fn run_unit_of_work<O>(
        &self,
        operation: O,
    ) -> light_application::PlaybackOperationResult<O::Output>
    where
        O: light_application::PlaybackUnitOfWork,
    {
        self.service.run_unit_of_work(operation)
    }

    pub(in crate::runtime) fn completed_frame(
        &self,
        engine: &Engine,
        show_id: Uuid,
        show_revision: u64,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Option<light_application::EventDraft> {
        self.telemetry
            .completed_frame(engine, show_id, show_revision, at)
    }

    pub(in crate::runtime) fn publish(&self, event: light_application::EventDraft) {
        self.service.events().publish(event);
    }
}

impl HighlightResource {
    pub(in crate::runtime) fn handle(
        &self,
        envelope: light_application::ActionEnvelope<light_application::HighlightCommand>,
        ports: &dyn light_application::HighlightPorts,
    ) -> Result<light_application::HighlightResult, light_application::ActionError> {
        self.service.handle(envelope, ports)
    }

    pub(in crate::runtime) fn snapshot(
        &self,
        context: &light_application::ActionContext,
        ports: &dyn light_application::HighlightPorts,
    ) -> Result<HighlightState, light_application::ActionError> {
        self.service.snapshot(context, ports)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn transition(
        &self,
        selection: &light_programmer::ProgrammerSelection,
        fixtures: &[HighlightFixture],
        groups: &HashMap<String, light_programmer::GroupDefinition>,
        output_suppressed: bool,
    ) -> light_programmer::HighlightTransition {
        self.registry
            .status(selection, fixtures, groups, output_suppressed)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn apply_action(
        &self,
        action: HighlightAction,
        selection: &light_programmer::ProgrammerSelection,
        fixtures: &[HighlightFixture],
        groups: &HashMap<String, light_programmer::GroupDefinition>,
        output_suppressed: bool,
    ) -> light_programmer::HighlightTransition {
        self.registry
            .action(action, selection, fixtures, groups, output_suppressed)
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(in crate::runtime) fn apply_action_guarded(
        &self,
        desk_id: Uuid,
        user_id: light_core::UserId,
        action: HighlightAction,
        selection: &light_programmer::ProgrammerSelection,
        fixtures: &[HighlightFixture],
        groups: &HashMap<String, light_programmer::GroupDefinition>,
        output_suppressed: bool,
    ) -> light_programmer::HighlightTransition {
        self.registry.action_guarded(
            desk_id,
            user_id,
            action,
            selection,
            fixtures,
            groups,
            output_suppressed,
        )
    }

    pub(in crate::runtime) fn acknowledge_selection(
        &self,
        desk_id: Uuid,
        user_id: light_core::UserId,
        selection: &light_programmer::ProgrammerSelection,
    ) {
        self.registry
            .acknowledge_internal_selection(desk_id, user_id, selection);
    }

    pub(in crate::runtime) fn clear_all(&self) {
        self.registry.clear_all();
        self.patch_preview.lock().clear();
    }

    pub(in crate::runtime) fn clear_context(&self, desk_id: Uuid, user_id: light_core::UserId) {
        self.registry.clear_context(desk_id, user_id);
    }

    pub(in crate::runtime) fn clear_patch_previews(&self) {
        self.patch_preview.lock().clear();
    }

    pub(in crate::runtime) fn remove_patch_preview(&self, session_id: SessionId) {
        self.patch_preview.lock().remove(&session_id);
    }

    pub(in crate::runtime) fn set_patch_preview(
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

    #[cfg(test)]
    pub(in crate::runtime) fn output_fixtures(&self) -> HashSet<light_core::FixtureId> {
        self.include_patch_previews(self.registry.output_fixtures())
    }

    pub(in crate::runtime) fn output_layers(&self) -> Vec<light_programmer::HighlightOutputLayer> {
        self.include_patch_preview_layers(self.registry.output_layers())
    }

    pub(in crate::runtime) fn mark_explicit_fixture_attributes(
        &self,
        desk_id: Uuid,
        user_id: light_core::UserId,
        touched: impl IntoIterator<Item = (light_core::FixtureId, light_core::AttributeKey)>,
    ) -> bool {
        self.registry
            .mark_explicit_fixture_attributes(desk_id, user_id, touched)
    }

    pub(in crate::runtime) fn include_patch_preview_layers(
        &self,
        layers: impl IntoIterator<Item = light_programmer::HighlightOutputLayer>,
    ) -> Vec<light_programmer::HighlightOutputLayer> {
        let mut layers = layers
            .into_iter()
            .map(|layer| (layer.fixture_id, layer))
            .collect::<HashMap<_, _>>();
        for fixture_id in self
            .patch_preview
            .lock()
            .values()
            .flat_map(|preview| preview.iter().copied())
        {
            layers.insert(
                fixture_id,
                light_programmer::HighlightOutputLayer {
                    fixture_id,
                    role: light_programmer::HighlightOutputRole::Highlight,
                    suppressed_attributes: HashSet::new(),
                },
            );
        }
        let mut layers = layers.into_values().collect::<Vec<_>>();
        layers.sort_by_key(|layer| layer.fixture_id.0);
        layers
    }

    pub(in crate::runtime) fn include_patch_previews(
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
