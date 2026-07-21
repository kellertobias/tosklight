//! Server adapter for the transport-independent Playback application boundary.

use super::*;
use std::collections::BTreeSet;

pub(super) struct ServerPlaybackPorts<'a> {
    pub(super) state: &'a AppState,
    pub(super) session: Option<&'a Session>,
    pub(super) desk: Option<&'a ControlDesk>,
    pub(super) persistence_pending: std::sync::atomic::AtomicBool,
    addressed_event_required: std::sync::atomic::AtomicBool,
    exclusion_zones: std::sync::OnceLock<CachedExclusionZones>,
}

struct CachedExclusionZones {
    addressed_page: Option<u8>,
    zones: Vec<Vec<u16>>,
    scope: light_playback::PlaybackExclusionScope,
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
            exclusion_zones: std::sync::OnceLock::new(),
        }
    }

    fn exclusion_context(
        &self,
        address: ResolvedPlaybackAddress,
    ) -> (&[Vec<u16>], light_playback::PlaybackExclusionScope) {
        let ResolvedPlaybackAddress::Pool { page, .. } = address else {
            return (&[], light_playback::PlaybackExclusionScope::None);
        };
        let cached = self.exclusion_zones.get_or_init(|| {
            let resolver = self.desk.map(|desk| {
                super::super::VirtualPlaybackExclusionResolver::read(self.state, desk.id)
            });
            CachedExclusionZones {
                addressed_page: page,
                zones: resolver
                    .as_ref()
                    .map(|resolver| resolver.zone_numbers(page))
                    .unwrap_or_default(),
                scope: resolver.map_or(light_playback::PlaybackExclusionScope::None, |resolver| {
                    if resolver.applies_to_page(page) {
                        light_playback::PlaybackExclusionScope::OriginatingDesk
                    } else {
                        light_playback::PlaybackExclusionScope::None
                    }
                }),
            }
        });
        debug_assert_eq!(cached.addressed_page, page);
        (&cached.zones, cached.scope)
    }
}

impl PlaybackPorts for ServerPlaybackPorts<'_> {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let Some(session_id) = context.session_id else {
            return Ok(());
        };
        self.session
            .filter(|session| session.id.0 == session_id)
            .map(|_| ())
            .ok_or_else(|| ActionError::new(ActionErrorKind::Unauthorized, "invalid session"))
    }

    fn current_page(&self, context: &ActionContext) -> Result<u8, ActionError> {
        let show = self
            .state
            .active_show
            .read()
            .clone()
            .ok_or_else(|| invalid("no show is open"))?;
        self.state
            .desk
            .lock()
            .desk_page(context.desk_id, show.id)
            .map_err(|error| invalid(error.to_string()))
    }

    fn playback_at(&self, page: u8, slot: u8) -> Result<Option<u16>, ActionError> {
        Ok(cuelist_for_page_playback(
            &self.state.engine.snapshot(),
            page,
            slot,
        ))
    }

    fn group_playback(
        &self,
        _context: &ActionContext,
        group_id: PlaybackGroupId,
    ) -> Result<Option<u16>, ActionError> {
        resolve_group_playback(&self.state.engine.snapshot(), group_id.as_str())
    }

    fn execute(
        &self,
        context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        match address {
            ResolvedPlaybackAddress::CueList(id) => self.execute_cue_list(context, id, action),
            ResolvedPlaybackAddress::Group {
                group_id,
                playback_number,
            } => group::execute(self, context, group_id, playback_number, action, surface),
            ResolvedPlaybackAddress::Pool { .. } => {
                self.execute_pool(context, address, action, surface)
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
        _surface: PlaybackSurface,
    ) -> Result<Vec<PlaybackRuntimeIdentity>, ActionError> {
        let Some(number) = address.playback_number() else {
            return Ok(Vec::new());
        };
        let definition = playback_definition(self.state, number)?;
        if !matches!(
            definition.target,
            light_playback::PlaybackTarget::CueList { .. }
        ) {
            return Ok(Vec::new());
        }
        let mut related = BTreeSet::new();
        if semantics::may_activate_playback(action) {
            related.extend(super::super::virtual_playback_peer_numbers(
                self.exclusion_context(address).0,
                number,
            ));
        }
        if semantics::may_trigger_auto_off(action, &definition) {
            related.extend(self.state.engine.enabled_auto_off_playbacks());
        }
        related.remove(&number);
        Ok(related
            .into_iter()
            .map(PlaybackRuntimeIdentity::Playback)
            .collect())
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

impl ServerPlaybackPorts<'_> {
    fn execute_cue_list(
        &self,
        context: &ActionContext,
        id: light_core::CueListId,
        action: PlaybackAction,
    ) -> Result<PlaybackExecution, ActionError> {
        let command = match action {
            PlaybackAction::Go { pressed: true } => CueListPlaybackAction::Go,
            PlaybackAction::Back { pressed: true } => CueListPlaybackAction::Back,
            PlaybackAction::Pause { pressed: true } => CueListPlaybackAction::Pause,
            PlaybackAction::Release => CueListPlaybackAction::Release,
            _ => return Err(invalid("action is incompatible with a cue list")),
        };
        let outcome = self
            .state
            .engine
            .execute_playback(EnginePlaybackCommand::CueList {
                id,
                action: command,
            })
            .map_err(invalid)?;
        let (execution, durable) = match outcome {
            EnginePlaybackOutcome::Active(active) => (PlaybackExecution::Active(active), true),
            EnginePlaybackOutcome::ActiveList { active, effect } => (
                PlaybackExecution::ActiveList {
                    active,
                    changed: effect.changed(),
                },
                effect.durable(),
            ),
            EnginePlaybackOutcome::Changed(effect) => (
                PlaybackExecution::Released(effect.changed()),
                effect.durable(),
            ),
            _ => return Err(invalid("unexpected cue-list Playback outcome")),
        };
        if durable && let Err(error) = persist_active_playbacks(self.state) {
            self.mark_persistence_pending(context, "active_playbacks", error);
        }
        Ok(execution)
    }

    pub(super) fn execute_pool(
        &self,
        context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        let number = address
            .playback_number()
            .ok_or_else(|| invalid("pool Playback address is required"))?;
        let ResolvedPlaybackAddress::Pool { page, .. } = address else {
            unreachable!("pool Playback address was validated")
        };
        let definition = playback_definition(self.state, number)?;
        let (action_name, input) = legacy_action(action, surface);
        if captures_preload(context.source)
            && let Some(pending) =
                self.capture(context, &definition, action_name, &input, surface, page)?
        {
            return Ok(PlaybackExecution::Pool {
                changed: false,
                pending: Some(pending),
            });
        }
        if self.intercept_update(context, &definition, action) {
            return Ok(PlaybackExecution::Pool {
                changed: false,
                pending: None,
            });
        }
        let (exclusion_zones, exclusion_scope) = self.exclusion_context(address);
        let activation_origin = Some(light_playback::PlaybackActivationOrigin {
            at: self.state.engine.application_time(),
            desk_id: self.desk.map(|desk| desk.id),
            surface: activation_surface(surface),
            exclusion_scope,
        });
        let dispatch = dispatch_playback_action(
            self.state,
            &definition,
            action_name,
            &input,
            PlaybackDispatchContext {
                action: context,
                session: self.session,
                desk: self.desk,
                source: source_name(context.source),
                exclusion_zones,
                activation_origin,
            },
        )
        .map_err(api_action_error)?;
        if dispatch.persistence_pending {
            self.persistence_pending
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if dispatch.addressed_event_required {
            self.addressed_event_required
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(PlaybackExecution::Pool {
            changed: dispatch.changed,
            pending: None,
        })
    }

    fn intercept_update(
        &self,
        context: &ActionContext,
        definition: &light_playback::PlaybackDefinition,
        action: PlaybackAction,
    ) -> bool {
        context.source == ActionSource::Osc
            && self.session.is_some_and(|session| {
                intercept_update_playback_target(
                    self.state,
                    session,
                    definition,
                    action_touched(action),
                )
            })
    }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
