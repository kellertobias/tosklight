//! One canonical Playback adapter, ordered by the application service.
use super::*;
pub(super) struct ServerPlaybackPorts<'a> {
    pub(super) socket: Option<&'a super::super::software_playback_sources::SoftwarePlaybackSocket>,
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
            socket: None,
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
        if let Some(socket) = self.socket {
            super::super::software_playback_sources::claim_in_order(
                self.state,
                socket,
                context.desk_id,
                control_source,
            )
            .map_err(api_action_error)?;
        }
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
        let solo_targets = if semantics::may_activate_playback(action.clone()) {
            address
                .playback_address()
                .map(|address| solo_related_targets(&snapshot, address))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut related = Vec::new();
        if let Some(light_playback::PlaybackTarget::Group { group_id, .. }) = &target {
            related.push(PlaybackRuntimeIdentity::Group(
                light_application::PlaybackGroupId::new(group_id)
                    .map_err(|error| invalid(error.to_string()))?,
            ));
        }
        if let Some(light_playback::PlaybackTarget::CueList { cue_list_id }) = &target {
            related.push(PlaybackRuntimeIdentity::CueList(*cue_list_id));
            related.push(PlaybackRuntimeIdentity::DirectCueList(*cue_list_id));
        }
        for page in snapshot.playback_pages.iter() {
            for (position, candidate) in &page.assignments {
                let same = target
                    .as_ref()
                    .is_some_and(|target| same_target(target, &candidate.target));
                let peer = light_playback::PlaybackAddress::new(page.number, *position)
                    .map_err(invalid)?;
                let solo = solo_targets
                    .iter()
                    .any(|target| same_target(target, &candidate.target));
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
        // Every affected assignment also changes the target-owned Cuelist projection.
        // Include Solo and auto-off peers in this same before/after publication boundary.
        let aggregate_cue_lists = related
            .iter()
            .filter_map(|identity| {
                let PlaybackRuntimeIdentity::Playback(address) = identity else {
                    return None;
                };
                let definition = snapshot
                    .playback_pages
                    .iter()
                    .find(|page| page.number == address.page())?
                    .assignments
                    .get(&address.position())?;
                match definition.target {
                    light_playback::PlaybackTarget::CueList { cue_list_id } => {
                        Some(PlaybackRuntimeIdentity::CueList(cue_list_id))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        related.extend(aggregate_cue_lists);
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

fn solo_related_targets(
    snapshot: &light_engine::EngineSnapshot,
    incoming: light_playback::PlaybackAddress,
) -> Vec<light_playback::PlaybackTarget> {
    let definition = |address: light_playback::PlaybackAddress| {
        snapshot
            .playback_pages
            .iter()
            .find(|page| page.number == address.page())
            .and_then(|page| page.assignments.get(&address.position()))
    };
    let mut targets = Vec::new();
    for region in snapshot
        .playback_solo_regions
        .iter()
        .filter(|region| region.members.contains(&incoming))
    {
        for target in region
            .members
            .iter()
            .filter_map(|address| definition(*address).map(|candidate| &candidate.target))
        {
            if !targets.iter().any(|existing| same_target(existing, target)) {
                targets.push(target.clone());
            }
        }
    }
    targets
}

#[cfg(test)]
mod solo_projection_tests {
    use super::*;
    #[test]
    fn related_solo_targets_use_exact_member_and_deduplicate_peer_targets() {
        let incoming = light_core::CueListId::new();
        let peer = light_core::CueListId::new();
        let unrelated = light_core::CueListId::new();
        let address =
            |page, position| light_playback::PlaybackAddress::new(page, position).unwrap();
        let assignment =
            |id| light_playback::PlaybackDefinition::new_cue_list("Projection test", id);
        let snapshot = light_engine::EngineSnapshot {
            playback_pages: vec![
                light_playback::PlaybackPage {
                    number: 1,
                    name: "Outside region".into(),
                    assignments: std::collections::HashMap::from([
                        (1, assignment(incoming)),
                        (2, assignment(unrelated)),
                    ]),
                },
                light_playback::PlaybackPage {
                    number: 2,
                    name: "Solo aliases".into(),
                    assignments: std::collections::HashMap::from([
                        (1, assignment(incoming)),
                        (2, assignment(peer)),
                    ]),
                },
                light_playback::PlaybackPage {
                    number: 3,
                    name: "Peer alias".into(),
                    assignments: std::collections::HashMap::from([(1, assignment(peer))]),
                },
            ]
            .into(),
            playback_solo_regions: vec![
                light_playback::PlaybackSoloRegion {
                    id: uuid::Uuid::new_v4(),
                    name: "Solo".into(),
                    members: vec![address(2, 1), address(2, 2)],
                },
                light_playback::PlaybackSoloRegion {
                    id: uuid::Uuid::new_v4(),
                    name: "Overlap".into(),
                    members: vec![address(2, 1), address(2, 2)],
                },
            ]
            .into(),
            ..Default::default()
        };
        let incoming_target = light_playback::PlaybackTarget::CueList {
            cue_list_id: incoming,
        };
        let peer_target = light_playback::PlaybackTarget::CueList { cue_list_id: peer };
        assert!(solo_related_targets(&snapshot, address(1, 1)).is_empty());
        let targets = solo_related_targets(&snapshot, address(2, 1));
        assert_eq!(targets.len(), 2);
        assert!(
            targets
                .iter()
                .any(|target| same_target(target, &incoming_target))
        );
        assert!(
            targets
                .iter()
                .any(|target| same_target(target, &peer_target))
        );
        assert!(!targets.iter().any(|target| same_target(
            target,
            &light_playback::PlaybackTarget::CueList {
                cue_list_id: unrelated
            }
        )));
        assert!(same_target(
            &targets[1],
            &snapshot.playback_pages[2].assignments[&1].target
        ));
    }
}
