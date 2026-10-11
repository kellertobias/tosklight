//! One canonical Playback adapter, ordered by the application service.
use super::*;
pub(super) struct ServerPlaybackPorts<'a> {
    pub(super) state: &'a AppState,
    pub(super) session: Option<&'a Session>,
    pub(super) desk: Option<&'a ControlDesk>,
    pub(super) persistence_pending: std::sync::atomic::AtomicBool,
    pub(super) addressed_event_required: std::sync::atomic::AtomicBool,
}
impl<'a> ServerPlaybackPorts<'a> {
    pub(super) fn new(
        state: &'a AppState,
        session: Option<&'a Session>,
        desk: Option<&'a ControlDesk>,
    ) -> Self {
        Self {
            state,
            session,
            desk,
            persistence_pending: std::sync::atomic::AtomicBool::new(false),
            addressed_event_required: std::sync::atomic::AtomicBool::new(false),
        }
    }
}
impl PlaybackPorts for ServerPlaybackPorts<'_> {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let Some(id) = context.session_id else {
            return Ok(());
        };
        self.session
            .filter(|session| session.id.0 == id)
            .map(|_| ())
            .ok_or_else(|| ActionError::new(ActionErrorKind::Unauthorized, "invalid session"))
    }
    fn current_page(&self, context: &ActionContext) -> Result<u8, ActionError> {
        self.state
            .installation
            .command_page(context.desk_id)
            .map_err(|error| invalid(error.to_string()))
    }
    fn playback_exists(
        &self,
        address: light_playback::PlaybackAddress,
    ) -> Result<bool, ActionError> {
        Ok(self
            .state
            .output
            .snapshot()
            .playback_pages
            .iter()
            .find(|page| page.number == address.page())
            .is_some_and(|page| page.assignments.contains_key(&address.position())))
    }
    fn group_playback(
        &self,
        _context: &ActionContext,
        group_id: PlaybackGroupId,
    ) -> Result<Option<light_playback::PlaybackAddress>, ActionError> {
        resolve_group_playback(&self.state.output.snapshot(), group_id.as_str())
    }
    fn validate_runtime_dynamic_stop(
        &self,
        _context: &ActionContext,
        address: &ResolvedPlaybackAddress,
        dynamic_id: uuid::Uuid,
        instance_id: uuid::Uuid,
        controller_id: uuid::Uuid,
    ) -> Result<(), ActionError> {
        runtime_dynamic_stop::validate(self.state, address, dynamic_id, instance_id, controller_id)
    }
    fn execute(
        &self,
        context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        control_source: &light_playback::PlaybackControlSource,
    ) -> Result<PlaybackExecution, ActionError> {
        control_source.validate().map_err(invalid)?;
        match address {
            ResolvedPlaybackAddress::CueList(id) => self.execute_cue_list(context, id, action),
            ResolvedPlaybackAddress::Group {
                group_id,
                playback_address,
            } => group::execute(
                self,
                context,
                group_id,
                playback_address,
                action,
                control_source,
            ),
            ResolvedPlaybackAddress::Playback(address) => {
                self.execute_playback(context, address, action, control_source)
            }
        }
    }
    fn durability(&self) -> PlaybackDurability {
        if self
            .persistence_pending
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            PlaybackDurability::PersistencePending
        } else {
            PlaybackDurability::Durable
        }
    }
    fn addressed_runtime_event_required(&self) -> bool {
        self.addressed_event_required
            .load(std::sync::atomic::Ordering::Relaxed)
    }
    fn transition_cause(
        &self,
        context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
    ) -> Result<Option<light_application::PlaybackTransitionCause>, ActionError> {
        semantics::transition_cause(self, context, address, action)
    }
    fn related_runtime_identities(
        &self,
        _context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        _source: &light_playback::PlaybackControlSource,
    ) -> Result<Vec<PlaybackRuntimeIdentity>, ActionError> {
        let snapshot = self.state.output.snapshot();
        let definition = address
            .playback_address()
            .map(|address| playback_definition(self.state, address))
            .transpose()?;
        let action = definition
            .as_ref()
            .map(|definition| {
                configured_control_definition(definition, action.clone()).map(|(_, action)| action)
            })
            .transpose()?
            .unwrap_or(action);
        let target = match (&address, &definition) {
            (ResolvedPlaybackAddress::CueList(id), _) => {
                Some(light_playback::PlaybackTarget::CueList { cue_list_id: *id })
            }
            (_, Some(definition)) => Some(definition.target.clone()),
            _ => None,
        };
        let mut related = Vec::new();
        if let Some(light_playback::PlaybackTarget::CueList { cue_list_id }) = &target {
            related.push(PlaybackRuntimeIdentity::DirectCueList(*cue_list_id));
        }
        for page in snapshot.playback_pages.iter() {
            for (position, candidate) in &page.assignments {
                let same = target
                    .as_ref()
                    .is_some_and(|target| same_target(target, &candidate.target));
                let peer = light_playback::PlaybackAddress::new(page.number, *position)
                    .map_err(invalid)?;
                let solo = semantics::may_activate_playback(action.clone())
                    && address.playback_address().is_some_and(|address| {
                        snapshot.playback_solo_regions.iter().any(|region| {
                            region.members.contains(&address) && region.members.contains(&peer)
                        })
                    });
                if same || solo {
                    related.push(PlaybackRuntimeIdentity::Playback(peer));
                }
            }
        }
        if definition
            .as_ref()
            .is_some_and(|definition| semantics::may_trigger_auto_off(action, definition))
        {
            related.extend(
                self.state
                    .output
                    .enabled_auto_off_playbacks()
                    .into_iter()
                    .map(PlaybackRuntimeIdentity::Playback),
            );
        }
        related.sort_by_key(|identity| format!("{identity:?}"));
        related.dedup();
        Ok(related)
    }
    fn projection(
        &self,
        context: &ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<light_application::PlaybackRuntimeProjection, ActionError> {
        projection::projection(self, context, identity)
    }
    fn projections(
        &self,
        context: &ActionContext,
        identities: &[PlaybackRuntimeIdentity],
    ) -> Result<Vec<light_application::PlaybackRuntimeProjection>, ActionError> {
        projection::projections(self, context, identities)
    }
    fn desk_projection(
        &self,
        context: &ActionContext,
    ) -> Result<Option<light_application::PlaybackDeskProjection>, ActionError> {
        projection::desk_projection(self, context)
    }
}
fn same_target(
    left: &light_playback::PlaybackTarget,
    right: &light_playback::PlaybackTarget,
) -> bool {
    match (left, right) {
        (
            light_playback::PlaybackTarget::Dynamic { assignment: a },
            light_playback::PlaybackTarget::Dynamic { assignment: b },
        ) => a.target_id() == b.target_id(),
        (
            light_playback::PlaybackTarget::Group { group_id: a, .. },
            light_playback::PlaybackTarget::Group { group_id: b, .. },
        ) => a == b,
        _ => left == right,
    }
}
