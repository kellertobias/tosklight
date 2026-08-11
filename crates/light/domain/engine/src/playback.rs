use crate::{
    Engine,
    playback_exclusion::{PoolPlaybackTransition, apply_with_exclusions},
};
use chrono::{DateTime, Utc};
use light_core::CueListId;
use light_playback::{
    ActiveDynamicPlayback, ActivePlayback, PlaybackContribution, PlaybackEngine, PlaybackIdentity,
    PlaybackMutation, PlaybackRuntimeEffect, PlaybackRuntimeStatus, VirtualPlaybackAddress,
};
use std::collections::HashSet;

/// A mutation accepted by the Engine's Playback boundary.
///
/// Callers describe intent without gaining access to the runtime lock or `PlaybackEngine`.
#[derive(Clone, Debug)]
pub enum EnginePlaybackCommand {
    CueList {
        id: CueListId,
        action: CueListPlaybackAction,
    },
    Pool {
        number: u16,
        action: PoolPlaybackAction,
    },
    Virtual {
        address: VirtualPlaybackAddress,
        action: VirtualPlaybackAction,
        exclusion_zones: Vec<Vec<VirtualPlaybackAddress>>,
        activation_origin: Option<light_playback::PlaybackActivationOrigin>,
    },
    ReleasePoolBatch(Vec<u16>),
    ReleaseIdentityBatch(Vec<PlaybackIdentity>),
    RestoreActive(Vec<ActivePlayback>),
    RestoreActiveDynamics(Vec<ActiveDynamicPlayback>),
    RestoreDynamicsPausedSince(Option<DateTime<Utc>>),
    SetDynamicsPaused(bool),
    ToggleDynamicsPaused,
}

#[derive(Clone, Copy, Debug)]
pub enum CueListPlaybackAction {
    Go,
    GoAt(DateTime<Utc>),
    Back,
    Jump(f64),
    Pause,
    Release,
}

#[derive(Clone, Copy, Debug)]
pub enum PoolPlaybackAction {
    Go,
    Back,
    Pause,
    TogglePause,
    FastForward,
    FastRewind,
    On,
    Off,
    Toggle,
    GoTo(f64),
    Load(f64),
    SetMaster(f32),
    SetConfiguredFader {
        mode: light_playback::PlaybackFaderMode,
        value: f32,
    },
    SetGroupMasterFader {
        value: f32,
        authoritative: f32,
    },
    SetMasterTransition {
        value: f32,
        duration_millis: u64,
    },
    SetVirtualMaster(f32),
    SetManualXFade(f32),
    XFade(bool),
    SetTempButton(bool),
    ToggleTemp,
    SetFlash(bool),
    SetSwap(bool),
    DynamicRestart,
    DynamicDoubleSpeed,
    DynamicHalfSpeed,
    DynamicLearnSpeed,
}

/// Actions supported by the dedicated page-qualified Virtual Playback runtime boundary.
///
/// This intentionally stays narrower than `PoolPlaybackAction`: unsupported physical fader and
/// temporary-control semantics must not silently collapse a Virtual address to a pool number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VirtualPlaybackAction {
    Go,
    Back,
    Pause,
    FastForward,
    FastRewind,
    On,
    Off,
    Release,
    Toggle,
    GoTo(f64),
    Load(f64),
    SetMaster(f32),
    XFade(bool),
    SetTempButton(bool),
    ToggleTemp,
    SetFlash(bool),
    SetSwap(bool),
    DynamicRestart,
    DynamicDoubleSpeed,
    DynamicHalfSpeed,
    DynamicLearnSpeed,
}

/// The exact consequence of one accepted Playback action.
///
/// `addressed` belongs to the Playback named by the command. `aggregate` also includes automatic
/// changes to related Playbacks, such as auto-off or exclusion releases.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnginePlaybackEffect {
    pub addressed: PlaybackRuntimeEffect,
    pub aggregate: PlaybackRuntimeEffect,
}

impl EnginePlaybackEffect {
    pub const fn from_addressed(effect: PlaybackRuntimeEffect) -> Self {
        Self {
            addressed: effect,
            aggregate: effect,
        }
    }

    pub const fn with_related(self, related: PlaybackRuntimeEffect) -> Self {
        Self {
            addressed: self.addressed,
            aggregate: self.aggregate.combine(related),
        }
    }

    pub const fn changed(self) -> bool {
        self.aggregate.changed()
    }

    pub const fn durable(self) -> bool {
        self.aggregate.durable()
    }
}

impl<T> From<PlaybackMutation<T>> for EnginePlaybackEffect {
    fn from(mutation: PlaybackMutation<T>) -> Self {
        Self {
            addressed: mutation.addressed_effect,
            aggregate: mutation.effect,
        }
    }
}

#[derive(Clone, Debug)]
pub enum EnginePlaybackOutcome {
    Active(Box<ActivePlayback>),
    ActiveList {
        active: Vec<ActivePlayback>,
        effect: PlaybackRuntimeEffect,
    },
    Changed(EnginePlaybackEffect),
    ChangedPlaybacks(Vec<u16>),
    DynamicsPaused(bool),
    Applied,
}

/// Immutable runtime metadata needed by persistence and output-control projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackDynamicsProjection {
    pub paused: bool,
    pub paused_since: Option<DateTime<Utc>>,
}

impl Engine {
    pub fn application_time(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    pub fn execute_playback(
        &self,
        command: EnginePlaybackCommand,
    ) -> Result<EnginePlaybackOutcome, String> {
        let generation = self.generation.load();
        execute(&mut generation.playback().write(), command)
    }

    pub fn execute_pool_playback_with_exclusions(
        &self,
        number: u16,
        action: PoolPlaybackAction,
        exclusion_zones: &[Vec<u16>],
    ) -> Result<PoolPlaybackTransition, String> {
        self.execute_pool_playback_with_activation(number, action, exclusion_zones, None)
    }

    pub fn execute_pool_playback_with_activation(
        &self,
        number: u16,
        action: PoolPlaybackAction,
        exclusion_zones: &[Vec<u16>],
        activation_origin: Option<light_playback::PlaybackActivationOrigin>,
    ) -> Result<PoolPlaybackTransition, String> {
        let generation = self.generation.load();
        let (outcome, released_playbacks) = apply_with_exclusions(
            &mut generation.playback().write(),
            number,
            exclusion_zones,
            activation_origin,
            |playback| execute_pool(playback, number, action),
        )?;
        let outcome = combine_release_effect(outcome, &released_playbacks)?;
        Ok(PoolPlaybackTransition {
            outcome,
            released_playbacks,
        })
    }

    pub fn active_playbacks(&self) -> Vec<ActivePlayback> {
        self.generation.load().playback().read().active()
    }

    pub fn playback_runtime(&self) -> Vec<ActivePlayback> {
        self.generation.load().playback().read().runtime()
    }

    pub fn playback_runtime_status(&self) -> Vec<PlaybackRuntimeStatus> {
        self.generation.load().playback().read().runtime_status()
    }

    pub fn playback_runtime_status_at(
        &self,
        identity: light_playback::PlaybackIdentity,
    ) -> Option<PlaybackRuntimeStatus> {
        self.generation
            .load()
            .playback()
            .read()
            .runtime_status_at(identity)
    }

    pub fn playback_control_state_at(
        &self,
        identity: PlaybackIdentity,
    ) -> light_playback::PlaybackControlState {
        self.generation
            .load()
            .playback()
            .read()
            .control_state_at(identity)
    }

    pub fn playback_runtime_status_for_cue_list(
        &self,
        cue_list_id: light_core::CueListId,
    ) -> Option<PlaybackRuntimeStatus> {
        self.generation
            .load()
            .playback()
            .read()
            .runtime_status_for_cue_list(cue_list_id)
    }

    pub fn set_cue_external_completion_millis(
        &self,
        cue_list_id: light_core::CueListId,
        duration_millis: u64,
    ) -> bool {
        self.generation
            .load()
            .playback()
            .write()
            .set_external_completion_millis(cue_list_id, duration_millis)
    }

    pub fn active_dynamic_playbacks(&self) -> Vec<ActiveDynamicPlayback> {
        self.generation
            .load()
            .playback()
            .read()
            .active_dynamic_playbacks()
    }

    pub fn active_dynamic_playback_at(
        &self,
        identity: PlaybackIdentity,
    ) -> Option<ActiveDynamicPlayback> {
        self.generation
            .load()
            .playback()
            .read()
            .active_dynamic_playback_at(identity)
    }

    pub fn active_dynamic_playbacks_for_persistence(&self) -> Vec<ActiveDynamicPlayback> {
        self.generation
            .load()
            .playback()
            .read()
            .active_dynamic_playbacks_for_persistence()
    }

    /// Applies output-resolved full-control auto-off as one durable Playback mutation.
    ///
    /// The scheduler determines complete address coverage from authoritative contributions; the
    /// engine only accepts exact currently-enabled Dynamic Playback numbers whose assignment has
    /// the opt-out setting enabled.
    pub fn auto_off_fully_controlled_dynamic_playbacks(
        &self,
        numbers: impl IntoIterator<Item = u16>,
    ) -> Vec<u16> {
        self.auto_off_fully_controlled_dynamic_playbacks_at(
            numbers
                .into_iter()
                .filter_map(|number| PlaybackIdentity::physical(number).ok()),
        )
        .into_iter()
        .filter_map(|identity| match identity {
            PlaybackIdentity::Physical(number) => Some(number.get()),
            PlaybackIdentity::Virtual(_) => None,
        })
        .collect()
    }

    pub fn auto_off_fully_controlled_dynamic_playbacks_at(
        &self,
        identities: impl IntoIterator<Item = PlaybackIdentity>,
    ) -> Vec<PlaybackIdentity> {
        let generation = self.generation.load();
        let mut playback = generation.playback().write();
        let mut changed = Vec::new();
        for identity in identities {
            if !playback
                .dynamic_assignment_at(identity)
                .is_some_and(|assignment| assignment.auto_off_full_control)
                || !playback
                    .active_dynamic_playback_at(identity)
                    .is_some_and(|active| active.enabled)
            {
                continue;
            }
            let turned_off = if playback.dynamic_assignment_at(identity).is_some() {
                playback
                    .off_dynamic_at_mutation(identity)
                    .is_ok_and(|mutation| mutation.value)
            } else {
                playback.off_at(identity).unwrap_or(false)
            };
            if turned_off {
                changed.push(identity);
            }
        }
        changed
    }

    pub fn active_cue_dynamic_values(&self) -> Vec<light_playback::ActiveCueDynamicValue> {
        self.generation
            .load()
            .playback()
            .read()
            .active_cue_dynamic_values()
    }

    /// Volatile per-Playback telemetry rows derived from already-published runtime state.
    pub fn playback_telemetry_at(
        &self,
        at: DateTime<Utc>,
    ) -> Vec<light_playback::PlaybackTelemetrySample> {
        self.generation
            .load()
            .playback()
            .read()
            .telemetry_samples_at(at)
    }

    pub fn playback_contributions_at(&self, at: DateTime<Utc>) -> Vec<PlaybackContribution> {
        self.generation
            .load()
            .playback()
            .read()
            .contributions_with_context_at(at, |_, _| false)
    }

    pub fn playback_dynamics(&self) -> PlaybackDynamicsProjection {
        let generation = self.generation.load();
        let playback = generation.playback().read();
        PlaybackDynamicsProjection {
            paused: playback.dynamics_paused(),
            paused_since: playback.dynamics_paused_since(),
        }
    }
}

fn execute(
    playback: &mut PlaybackEngine,
    command: EnginePlaybackCommand,
) -> Result<EnginePlaybackOutcome, String> {
    match command {
        EnginePlaybackCommand::CueList { id, action } => execute_cue_list(playback, id, action),
        EnginePlaybackCommand::Pool { number, action } => execute_pool(playback, number, action),
        EnginePlaybackCommand::Virtual {
            address,
            action,
            exclusion_zones,
            activation_origin,
        } => execute_virtual(
            playback,
            address,
            action,
            &exclusion_zones,
            activation_origin,
        ),
        EnginePlaybackCommand::ReleasePoolBatch(numbers) => Ok(
            EnginePlaybackOutcome::ChangedPlaybacks(release_pool_batch(playback, numbers)),
        ),
        EnginePlaybackCommand::ReleaseIdentityBatch(identities) => {
            let mut released = Vec::new();
            for identity in identities {
                if playback
                    .release_at_mutation(identity)
                    .is_ok_and(|mutation| mutation.effect.changed())
                {
                    released.push(identity.number());
                }
            }
            released.sort_unstable();
            released.dedup();
            Ok(EnginePlaybackOutcome::ChangedPlaybacks(released))
        }
        EnginePlaybackCommand::RestoreActive(active) => {
            playback.restore_active(active);
            Ok(EnginePlaybackOutcome::Applied)
        }
        EnginePlaybackCommand::RestoreActiveDynamics(active) => {
            playback.restore_active_dynamics(active);
            Ok(EnginePlaybackOutcome::Applied)
        }
        EnginePlaybackCommand::RestoreDynamicsPausedSince(paused_at) => {
            playback.restore_dynamics_paused_since(paused_at);
            Ok(EnginePlaybackOutcome::Applied)
        }
        EnginePlaybackCommand::SetDynamicsPaused(paused) => {
            playback.set_dynamics_paused(paused);
            Ok(EnginePlaybackOutcome::DynamicsPaused(paused))
        }
        EnginePlaybackCommand::ToggleDynamicsPaused => Ok(EnginePlaybackOutcome::DynamicsPaused(
            playback.toggle_dynamics_paused(),
        )),
    }
}

pub(crate) fn execute_virtual(
    playback: &mut PlaybackEngine,
    address: VirtualPlaybackAddress,
    action: VirtualPlaybackAction,
    exclusion_zones: &[Vec<VirtualPlaybackAddress>],
    activation_origin: Option<light_playback::PlaybackActivationOrigin>,
) -> Result<EnginePlaybackOutcome, String> {
    let identity = PlaybackIdentity::Virtual(address);
    let was_enabled = playback.is_active_at(identity);
    let addressed_effect = match action {
        VirtualPlaybackAction::Go => {
            playback.go_playback_at(identity)?;
            PlaybackRuntimeEffect::Durable
        }
        VirtualPlaybackAction::Back => {
            playback.back_playback_at(identity)?;
            PlaybackRuntimeEffect::Durable
        }
        VirtualPlaybackAction::Pause => {
            if playback.dynamic_assignment_at(identity).is_some() {
                playback.toggle_dynamic_pause_at_mutation(identity)?.effect
            } else {
                playback.pause_playback_at_mutation(identity)?.effect
            }
        }
        VirtualPlaybackAction::FastForward => {
            playback.fast_forward_playback_at(identity)?;
            PlaybackRuntimeEffect::Durable
        }
        VirtualPlaybackAction::FastRewind => {
            playback.fast_rewind_playback_at(identity)?;
            PlaybackRuntimeEffect::Durable
        }
        VirtualPlaybackAction::On => {
            if playback.dynamic_assignment_at(identity).is_some() {
                playback.on_dynamic_at_mutation(identity)?;
            } else {
                playback.on_at(identity)?;
            }
            if was_enabled {
                PlaybackRuntimeEffect::None
            } else {
                PlaybackRuntimeEffect::Durable
            }
        }
        VirtualPlaybackAction::Off => {
            let changed = if playback.dynamic_assignment_at(identity).is_some() {
                playback.off_dynamic_at_mutation(identity)?.value
            } else {
                playback.off_at(identity)?
            };
            if changed {
                PlaybackRuntimeEffect::Durable
            } else {
                PlaybackRuntimeEffect::None
            }
        }
        VirtualPlaybackAction::Release => playback.release_at_mutation(identity)?.effect,
        VirtualPlaybackAction::Toggle => {
            if playback.dynamic_assignment_at(identity).is_some() {
                playback.toggle_dynamic_at_mutation(identity)?;
            } else if was_enabled {
                playback.off_at(identity)?;
            } else {
                playback.on_at(identity)?;
            }
            PlaybackRuntimeEffect::Durable
        }
        VirtualPlaybackAction::GoTo(cue) => {
            playback.goto_playback_at_mutation(identity, cue)?.effect
        }
        VirtualPlaybackAction::Load(cue) => {
            playback.load_playback_at_mutation(identity, cue)?.effect
        }
        VirtualPlaybackAction::SetMaster(value) => {
            if playback.dynamic_assignment_at(identity).is_some() {
                playback
                    .set_dynamic_fader_at_mutation(identity, value)?
                    .effect
            } else {
                playback
                    .set_virtual_master_at_mutation(identity, value)?
                    .effect
            }
        }
        VirtualPlaybackAction::XFade(on) => playback.xfade_at_mutation(identity, on)?.effect,
        VirtualPlaybackAction::SetTempButton(active) => {
            playback
                .set_temp_button_at_mutation(identity, active)?
                .effect
        }
        VirtualPlaybackAction::ToggleTemp => playback.toggle_temp_at_mutation(identity)?.effect,
        VirtualPlaybackAction::SetFlash(pressed) => {
            if playback.dynamic_assignment_at(identity).is_some() {
                playback
                    .set_dynamic_flash_at_mutation(identity, pressed)?
                    .effect
            } else {
                playback.set_flash_at_mutation(identity, pressed)?.effect
            }
        }
        VirtualPlaybackAction::SetSwap(pressed) => {
            playback.set_swap_at_mutation(identity, pressed)?.effect
        }
        VirtualPlaybackAction::DynamicRestart => {
            playback.restart_dynamic_at_mutation(identity)?.effect
        }
        VirtualPlaybackAction::DynamicDoubleSpeed => {
            playback
                .scale_dynamic_speed_at_mutation(identity, true)?
                .effect
        }
        VirtualPlaybackAction::DynamicHalfSpeed => {
            playback
                .scale_dynamic_speed_at_mutation(identity, false)?
                .effect
        }
        VirtualPlaybackAction::DynamicLearnSpeed => {
            playback.tap_dynamic_speed_at_mutation(identity)?.effect
        }
    };
    let is_enabled = playback.is_active_at(identity);
    let mut peer_effect = PlaybackRuntimeEffect::None;
    if !was_enabled && is_enabled {
        if let Some(origin) = activation_origin {
            playback.record_activation_at(identity, origin);
        }
        for peer in virtual_exclusion_peers(exclusion_zones, address) {
            peer_effect = peer_effect.combine(
                playback
                    .release_at_mutation(PlaybackIdentity::Virtual(peer))?
                    .effect,
            );
        }
    }
    Ok(EnginePlaybackOutcome::Changed(
        EnginePlaybackEffect::from_addressed(addressed_effect).with_related(peer_effect),
    ))
}

fn virtual_exclusion_peers(
    zones: &[Vec<VirtualPlaybackAddress>],
    activated: VirtualPlaybackAddress,
) -> std::collections::BTreeSet<VirtualPlaybackAddress> {
    zones
        .iter()
        .filter(|zone| zone.contains(&activated))
        .flat_map(|zone| zone.iter().copied())
        .filter(|address| *address != activated)
        .collect()
}

fn release_pool_batch(playback: &mut PlaybackEngine, numbers: Vec<u16>) -> Vec<u16> {
    let mut seen = HashSet::with_capacity(numbers.len());
    let mut changed = numbers
        .into_iter()
        .filter(|number| seen.insert(*number))
        .filter(|number| playback.off(*number).unwrap_or(false))
        .collect::<Vec<_>>();
    changed.sort_unstable();
    changed
}

fn execute_cue_list(
    playback: &mut PlaybackEngine,
    id: CueListId,
    action: CueListPlaybackAction,
) -> Result<EnginePlaybackOutcome, String> {
    match action {
        CueListPlaybackAction::Go => playback
            .go(id)
            .cloned()
            .map(Box::new)
            .map(EnginePlaybackOutcome::Active),
        CueListPlaybackAction::GoAt(started_at) => playback
            .go_at(id, started_at)
            .cloned()
            .map(Box::new)
            .map(EnginePlaybackOutcome::Active),
        CueListPlaybackAction::Back => playback
            .back(id)
            .cloned()
            .map(Box::new)
            .map(EnginePlaybackOutcome::Active),
        CueListPlaybackAction::Jump(cue) => playback
            .jump(id, cue)
            .cloned()
            .map(Box::new)
            .map(EnginePlaybackOutcome::Active),
        CueListPlaybackAction::Pause => {
            let effect = playback.pause_mutation(id)?.effect;
            Ok(EnginePlaybackOutcome::ActiveList {
                active: playback.active(),
                effect,
            })
        }
        CueListPlaybackAction::Release => Ok(EnginePlaybackOutcome::Changed(addressed_effect(
            durable_effect(playback.release(id)),
        ))),
    }
}

fn execute_pool(
    playback: &mut PlaybackEngine,
    number: u16,
    action: PoolPlaybackAction,
) -> Result<EnginePlaybackOutcome, String> {
    let effects = match action {
        PoolPlaybackAction::Go => playback
            .go_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))?,
        PoolPlaybackAction::Back => playback
            .back_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))?,
        PoolPlaybackAction::Pause if playback.dynamic_assignment(number).is_some() => {
            playback.toggle_dynamic_pause_mutation(number)?.into()
        }
        PoolPlaybackAction::Pause => playback.pause_playback_mutation(number)?.into(),
        PoolPlaybackAction::TogglePause => toggle_pause(playback, number)?,
        PoolPlaybackAction::FastForward => playback
            .fast_forward_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))?,
        PoolPlaybackAction::FastRewind => playback
            .fast_rewind_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))?,
        PoolPlaybackAction::On => playback.on_mutation(number)?.into(),
        PoolPlaybackAction::Off => playback.off_mutation(number)?.into(),
        PoolPlaybackAction::Toggle => playback.toggle_mutation(number)?.into(),
        PoolPlaybackAction::GoTo(cue) => playback.goto_playback_mutation(number, cue)?.into(),
        PoolPlaybackAction::Load(cue) => playback.load_playback_mutation(number, cue)?.into(),
        PoolPlaybackAction::SetMaster(value) => playback.set_master_mutation(number, value)?.into(),
        PoolPlaybackAction::SetConfiguredFader { mode, value } => playback
            .set_configured_fader_mutation(number, mode, value)?
            .into(),
        PoolPlaybackAction::SetGroupMasterFader {
            value,
            authoritative,
        } => playback
            .set_group_master_fader_mutation(number, value, authoritative)?
            .into(),
        PoolPlaybackAction::SetMasterTransition {
            value,
            duration_millis,
        } => playback
            .set_master_transition_mutation(number, value, duration_millis)?
            .into(),
        PoolPlaybackAction::SetVirtualMaster(value) => {
            playback.set_virtual_master_mutation(number, value)?.into()
        }
        PoolPlaybackAction::SetManualXFade(value) => {
            playback.set_manual_xfade_mutation(number, value)?.into()
        }
        PoolPlaybackAction::XFade(on) => playback.xfade_mutation(number, on)?.into(),
        PoolPlaybackAction::SetTempButton(active) => {
            playback.set_temp_button_mutation(number, active)?.into()
        }
        PoolPlaybackAction::ToggleTemp => playback.toggle_temp_mutation(number)?.into(),
        PoolPlaybackAction::SetFlash(pressed) => {
            if playback.dynamic_assignment(number).is_some() {
                playback.set_dynamic_flash_mutation(number, pressed)?.into()
            } else {
                playback.set_flash_mutation(number, pressed)?.into()
            }
        }
        PoolPlaybackAction::SetSwap(pressed) => playback.set_swap_mutation(number, pressed)?.into(),
        PoolPlaybackAction::DynamicRestart => playback.restart_dynamic_mutation(number)?.into(),
        PoolPlaybackAction::DynamicDoubleSpeed => {
            playback.scale_dynamic_speed_mutation(number, true)?.into()
        }
        PoolPlaybackAction::DynamicHalfSpeed => {
            playback.scale_dynamic_speed_mutation(number, false)?.into()
        }
        PoolPlaybackAction::DynamicLearnSpeed => {
            playback.tap_dynamic_speed_mutation(number)?.into()
        }
    };
    Ok(EnginePlaybackOutcome::Changed(effects))
}

fn toggle_pause(
    playback: &mut PlaybackEngine,
    number: u16,
) -> Result<EnginePlaybackEffect, String> {
    if playback.dynamic_assignment(number).is_some() {
        return playback
            .toggle_dynamic_pause_mutation(number)
            .map(EnginePlaybackEffect::from);
    }
    let paused = playback
        .playback_runtime(number)
        .is_some_and(|runtime| runtime.paused);
    if paused {
        playback
            .go_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))
    } else {
        playback
            .pause_playback_mutation(number)
            .map(EnginePlaybackEffect::from)
    }
}

pub(crate) fn combine_release_effect(
    outcome: EnginePlaybackOutcome,
    released: &[u16],
) -> Result<EnginePlaybackOutcome, String> {
    let EnginePlaybackOutcome::Changed(effects) = outcome else {
        return Err("unexpected pool Playback outcome".into());
    };
    Ok(EnginePlaybackOutcome::Changed(
        effects.with_related(durable_effect(!released.is_empty())),
    ))
}

pub(crate) const fn addressed_effect(effect: PlaybackRuntimeEffect) -> EnginePlaybackEffect {
    EnginePlaybackEffect::from_addressed(effect)
}

const fn durable_effect(changed: bool) -> PlaybackRuntimeEffect {
    if changed {
        PlaybackRuntimeEffect::Durable
    } else {
        PlaybackRuntimeEffect::None
    }
}
