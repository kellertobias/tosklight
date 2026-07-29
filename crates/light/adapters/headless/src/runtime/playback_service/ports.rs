//! Server adapter for the transport-independent Playback application boundary.

use super::*;
pub(super) struct ServerPlaybackPorts<'a> {
    pub(super) state: &'a AppState,
    pub(super) session: Option<&'a Session>,
    pub(super) desk: Option<&'a ControlDesk>,
    pub(super) persistence_pending: std::sync::atomic::AtomicBool,
    addressed_event_required: std::sync::atomic::AtomicBool,
    exclusion_zones: std::sync::OnceLock<CachedExclusionZones>,
}

struct CachedExclusionZones {
    addressed_page: u8,
    zones: Vec<Vec<light_playback::VirtualPlaybackAddress>>,
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
    ) -> (
        &[Vec<light_playback::VirtualPlaybackAddress>],
        light_playback::PlaybackExclusionScope,
    ) {
        let ResolvedPlaybackAddress::Virtual(address) = address else {
            return (&[], light_playback::PlaybackExclusionScope::None);
        };
        let page = address.page();
        let cached = self.exclusion_zones.get_or_init(|| {
            let zones = super::super::VirtualPlaybackExclusionResolver::read(self.state)
                .map(|resolver| resolver.zone_addresses())
                .unwrap_or_default();
            let applies = zones.iter().any(|zone| zone.contains(&address));
            CachedExclusionZones {
                addressed_page: page,
                zones,
                scope: if applies {
                    light_playback::PlaybackExclusionScope::Show
                } else {
                    light_playback::PlaybackExclusionScope::None
                },
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
            .current()
            .clone()
            .ok_or_else(|| invalid("no show is open"))?;
        self.state
            .installation
            .desk_page(context.desk_id, show.id)
            .map_err(|error| invalid(error.to_string()))
    }

    fn playback_at(&self, page: u8, slot: u8) -> Result<Option<u16>, ActionError> {
        Ok(cuelist_for_page_playback(
            &self.state.output.snapshot(),
            page,
            slot,
        ))
    }

    fn group_playback(
        &self,
        _context: &ActionContext,
        group_id: PlaybackGroupId,
    ) -> Result<Option<u16>, ActionError> {
        resolve_group_playback(&self.state.output.snapshot(), group_id.as_str())
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
            ResolvedPlaybackAddress::Virtual(address) => {
                self.execute_virtual(context, address, action, surface)
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
        let definition = match address {
            ResolvedPlaybackAddress::Virtual(address) => {
                virtual_playback_definition(self.state, address)?
            }
            _ => {
                let Some(number) = address.playback_number() else {
                    return Ok(Vec::new());
                };
                playback_definition(self.state, number)?
            }
        };
        if !matches!(
            definition.target,
            light_playback::PlaybackTarget::CueList { .. }
        ) {
            return Ok(Vec::new());
        }
        let mut related = Vec::new();
        if semantics::may_activate_playback(action) {
            if let ResolvedPlaybackAddress::Virtual(activated) = address {
                related.extend(
                    self.exclusion_context(address)
                        .0
                        .iter()
                        .filter(|zone| zone.contains(&activated))
                        .flatten()
                        .copied()
                        .filter(|peer| *peer != activated)
                        .map(PlaybackRuntimeIdentity::Virtual),
                );
            }
        }
        if semantics::may_trigger_auto_off(action, &definition) {
            related.extend(
                self.state
                    .output
                    .enabled_auto_off_playbacks()
                    .into_iter()
                    .map(PlaybackRuntimeIdentity::Playback),
            );
        }
        let mut unique = Vec::with_capacity(related.len());
        for identity in related {
            if !unique.contains(&identity) {
                unique.push(identity);
            }
        }
        Ok(unique)
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
    fn execute_virtual(
        &self,
        context: &ActionContext,
        address: light_playback::VirtualPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        let definition = virtual_playback_definition(self.state, address)?;
        let (action_name, input) = legacy_action(action);
        if captures_preload(context.source)
            && let Some(pending) = self.capture(
                context,
                &definition,
                action_name,
                &input,
                surface,
                Some(address.page()),
            )?
        {
            return Ok(PlaybackExecution::Pool {
                changed: false,
                pending: Some(pending),
            });
        }
        if !matches!(
            definition.target,
            light_playback::PlaybackTarget::CueList { .. }
                | light_playback::PlaybackTarget::Dynamic { .. }
        ) {
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
                    exclusion_zones: &[],
                    activation_origin: Some(light_playback::PlaybackActivationOrigin {
                        at: self.state.output.application_time(),
                        desk_id: self.desk.map(|desk| desk.id),
                        surface: activation_surface(surface),
                        exclusion_scope: light_playback::PlaybackExclusionScope::None,
                    }),
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
            return Ok(PlaybackExecution::Pool {
                changed: dispatch.changed,
                pending: None,
            });
        }
        let action = virtual_runtime_action(&definition, action)?;
        let Some(action) = action else {
            return Ok(PlaybackExecution::Pool {
                changed: false,
                pending: None,
            });
        };
        let resolved = ResolvedPlaybackAddress::Virtual(address);
        let (exclusion_zones, exclusion_scope) = self.exclusion_context(resolved);
        let activation_origin = Some(light_playback::PlaybackActivationOrigin {
            at: self.state.output.application_time(),
            desk_id: self.desk.map(|desk| desk.id),
            surface: activation_surface(surface),
            exclusion_scope,
        });
        let enabled_peers_before =
            enabled_virtual_exclusion_peers(self.state, address, exclusion_zones);
        let outcome = self
            .state
            .output
            .execute_playback(EnginePlaybackCommand::Virtual {
                address,
                action,
                exclusion_zones: exclusion_zones.to_vec(),
                activation_origin,
            })
            .map_err(invalid)?;
        let EnginePlaybackOutcome::Changed(effect) = outcome else {
            return Err(invalid("unexpected Virtual Playback outcome"));
        };
        let enabled_after = enabled_virtual_identities(self.state);
        let released_playbacks = enabled_peers_before
            .into_iter()
            .filter(|peer| !enabled_after.contains(peer))
            .map(|peer| peer.number().get())
            .collect::<Vec<_>>();
        if !released_playbacks.is_empty()
            && let Some(desk) = self.desk
        {
            emit(
                self.state,
                "playback_exclusion_applied",
                serde_json::json!({
                    "desk_id": desk.id,
                    "activated_page": address.page(),
                    "activated_playback": address.number().get(),
                    "released_playbacks": released_playbacks,
                    "source": source_name(context.source),
                }),
            );
        }
        if effect.durable()
            && let Err(error) = persist_active_playbacks(self.state)
        {
            self.mark_persistence_pending(context, "active_playbacks", error);
        }
        Ok(PlaybackExecution::Pool {
            changed: effect.changed(),
            pending: None,
        })
    }

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
            .output
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
        let (action_name, input) = legacy_action(action);
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
        let activation_origin = Some(light_playback::PlaybackActivationOrigin {
            at: self.state.output.application_time(),
            desk_id: self.desk.map(|desk| desk.id),
            surface: activation_surface(surface),
            exclusion_scope: light_playback::PlaybackExclusionScope::None,
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
                exclusion_zones: &[],
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

fn enabled_virtual_exclusion_peers(
    state: &AppState,
    activated: light_playback::VirtualPlaybackAddress,
    zones: &[Vec<light_playback::VirtualPlaybackAddress>],
) -> std::collections::BTreeSet<light_playback::VirtualPlaybackAddress> {
    let enabled = enabled_virtual_identities(state);
    zones
        .iter()
        .filter(|zone| zone.contains(&activated))
        .flatten()
        .copied()
        .filter(|peer| *peer != activated && enabled.contains(peer))
        .collect()
}

fn enabled_virtual_identities(
    state: &AppState,
) -> std::collections::HashSet<light_playback::VirtualPlaybackAddress> {
    state
        .output
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter_map(|playback| match playback.playback_identity {
            Some(light_playback::PlaybackIdentity::Virtual(address)) => Some(address),
            _ => None,
        })
        .collect()
}

fn virtual_runtime_action(
    definition: &light_playback::PlaybackDefinition,
    action: PlaybackAction,
) -> Result<Option<VirtualPlaybackAction>, ActionError> {
    use light_playback::PlaybackButtonAction as Button;
    let action = match action {
        PlaybackAction::On { pressed: true } => Some(VirtualPlaybackAction::On),
        PlaybackAction::Go { pressed: true } => Some(VirtualPlaybackAction::Go),
        PlaybackAction::Back { pressed: true } => Some(VirtualPlaybackAction::Back),
        PlaybackAction::Pause { pressed: true } => Some(VirtualPlaybackAction::Pause),
        PlaybackAction::FastForward { pressed: true } => Some(VirtualPlaybackAction::FastForward),
        PlaybackAction::FastRewind { pressed: true } => Some(VirtualPlaybackAction::FastRewind),
        PlaybackAction::Off { pressed: true } => Some(VirtualPlaybackAction::Off),
        PlaybackAction::Release => Some(VirtualPlaybackAction::Release),
        PlaybackAction::Toggle { pressed: true } => Some(VirtualPlaybackAction::Toggle),
        PlaybackAction::Master(level) => Some(VirtualPlaybackAction::SetMaster(level.value())),
        PlaybackAction::GoTo(cue) => Some(VirtualPlaybackAction::GoTo(cue.value())),
        PlaybackAction::Load(cue) => Some(VirtualPlaybackAction::Load(cue.value())),
        PlaybackAction::Crossfade { enabled } => Some(VirtualPlaybackAction::XFade(enabled)),
        PlaybackAction::Temporary { enabled, .. } => {
            Some(VirtualPlaybackAction::SetTempButton(enabled))
        }
        PlaybackAction::Flash { pressed } => Some(VirtualPlaybackAction::SetFlash(pressed)),
        PlaybackAction::Swap { pressed } => Some(VirtualPlaybackAction::SetSwap(pressed)),
        PlaybackAction::Temp { pressed: true } => Some(VirtualPlaybackAction::ToggleTemp),
        PlaybackAction::DynamicRestart { pressed: true } => {
            Some(VirtualPlaybackAction::DynamicRestart)
        }
        PlaybackAction::DynamicDoubleSpeed { pressed: true } => {
            Some(VirtualPlaybackAction::DynamicDoubleSpeed)
        }
        PlaybackAction::DynamicHalfSpeed { pressed: true } => {
            Some(VirtualPlaybackAction::DynamicHalfSpeed)
        }
        PlaybackAction::DynamicLearnSpeed { pressed: true } => {
            Some(VirtualPlaybackAction::DynamicLearnSpeed)
        }
        PlaybackAction::ConfiguredButton {
            number,
            pressed: true,
        } => {
            if number == 0 || number > definition.button_count {
                return Err(invalid("button is not present on this virtual playback"));
            }
            match definition.buttons[usize::from(number - 1)] {
                Button::On => Some(VirtualPlaybackAction::On),
                Button::Off => Some(VirtualPlaybackAction::Off),
                Button::Toggle => Some(VirtualPlaybackAction::Toggle),
                Button::Go => Some(VirtualPlaybackAction::Go),
                Button::GoMinus => Some(VirtualPlaybackAction::Back),
                Button::Pause => Some(VirtualPlaybackAction::Pause),
                Button::FastForward => Some(VirtualPlaybackAction::FastForward),
                Button::FastRewind => Some(VirtualPlaybackAction::FastRewind),
                Button::Flash => Some(VirtualPlaybackAction::SetFlash(true)),
                Button::Temp => Some(VirtualPlaybackAction::ToggleTemp),
                Button::Swap => Some(VirtualPlaybackAction::SetSwap(true)),
                Button::DynamicRestart => Some(VirtualPlaybackAction::DynamicRestart),
                Button::DynamicDoubleSpeed => Some(VirtualPlaybackAction::DynamicDoubleSpeed),
                Button::DynamicHalfSpeed => Some(VirtualPlaybackAction::DynamicHalfSpeed),
                Button::DynamicLearnSpeed => Some(VirtualPlaybackAction::DynamicLearnSpeed),
                Button::None => None,
                _ => {
                    return Err(invalid(
                        "configured Virtual Playback action is not supported by the dedicated runtime",
                    ));
                }
            }
        }
        PlaybackAction::ConfiguredButton {
            number,
            pressed: false,
        } => {
            if number == 0 || number > definition.button_count {
                return Err(invalid("button is not present on this virtual playback"));
            }
            match definition.buttons[usize::from(number - 1)] {
                Button::Flash => Some(VirtualPlaybackAction::SetFlash(false)),
                Button::Swap => Some(VirtualPlaybackAction::SetSwap(false)),
                _ => None,
            }
        }
        action if action.pressed() == Some(false) => None,
        _ => {
            return Err(invalid(
                "action is not supported by the dedicated Virtual Playback runtime",
            ));
        }
    };
    Ok(action)
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
