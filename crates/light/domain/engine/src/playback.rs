use crate::{Engine, playback_exclusion::apply_with_solo};
use chrono::{DateTime, Utc};
use light_core::CueListId;
use light_playback::{
    ActiveDynamicPlayback, ActivePlayback, CueNumber, PlaybackAddress, PlaybackContribution,
    PlaybackControlSource, PlaybackEngine, PlaybackMutation, PlaybackRuntimeEffect,
    PlaybackRuntimeStatus, PlaybackSoloRegion,
};

/// A mutation accepted by the Engine's Playback boundary.
///
/// Callers describe intent without gaining access to the runtime lock or `PlaybackEngine`.
#[derive(Clone, Debug)]
pub enum EnginePlaybackCommand {
    CueList {
        id: CueListId,
        action: CueListPlaybackAction,
    },
    Playback {
        address: PlaybackAddress,
        action: PlaybackAction,
        activation_origin: Option<light_playback::PlaybackActivationOrigin>,
        control_source: PlaybackControlSource,
    },
    /// Enforce Solo for an externally owned permanent transport after its accepted start.
    EnforceSoloForTarget {
        address: PlaybackAddress,
    },
    ReleaseAddressBatch(Vec<PlaybackAddress>),
    CancelControlSurface(String),
    /// Install the complete saved permanent state before resolving overlapping Solo regions.
    RestorePlaybackRuntime {
        playbacks: Vec<ActivePlayback>,
        dynamic_playbacks: Vec<ActiveDynamicPlayback>,
    },
    RestoreActive(Vec<ActivePlayback>),
    RestoreActiveDynamics(Vec<ActiveDynamicPlayback>),
    RestoreDynamicsPausedSince(Option<DateTime<Utc>>),
    SetDynamicsPaused(bool),
    ToggleDynamicsPaused,
}

#[derive(Clone, Debug)]
pub enum CueListPlaybackAction {
    Go,
    GoAt(DateTime<Utc>),
    Back,
    Jump(CueNumber),
    ExecuteCueId(uuid::Uuid),
    ReconstructCueId {
        cue_id: uuid::Uuid,
        elapsed_millis: u64,
    },
    /// In-process Timecode history; ages are relative to the requested frame.
    ReconstructTimelineCues(Vec<(uuid::Uuid, u64)>),
    /// External Timecode owns the evaluation clock without setting operator Pause.
    ReconstructSampledTimelineCues(Vec<(uuid::Uuid, u64)>),
    Pause,
    Resume,
    Release,
    /// The level a directly-driven Cuelist contributes at. Timecode clip fades use this; there
    /// is no Playback number to address, so none of the pool fader behaviour applies.
    SetMaster(f32),
}

#[derive(Clone, Debug)]
pub enum PlaybackAction {
    Go,
    Back,
    Pause,
    TogglePause,
    FastForward,
    FastRewind,
    On,
    Off,
    Release,
    Toggle,
    GoTo(CueNumber),
    Load(CueNumber),
    SetMaster(f32),
    /// An automation fader whose zero/non-zero value is also authoritative Off/On state.
    SetMasterWithExplicitActivation(f32),
    SetConfiguredFader {
        mode: light_playback::PlaybackFaderMode,
        value: f32,
    },
    SetConfiguredFaderAbsolute {
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
    ChangedPlaybacks(Vec<PlaybackAddress>),
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
    /// Exact provenance allocation is independent of Playback transport and LTP ordering.
    pub fn playback_source_occurrence_watermark(&self) -> u64 {
        self.generation
            .load()
            .playback()
            .read()
            .source_occurrence_watermark()
    }

    /// Cold restore reserves historical source IDs before their catalogue becomes visible.
    /// Call outside the Dynamics runtime guard; this takes only the Playback write lock.
    /// The caller must serialize generation installation (startup before workers, or the
    /// desk's exclusive show activation permit), just as for installing the restored runtime.
    pub fn reserve_playback_source_occurrence_watermark(&self, watermark: u64) {
        self.generation
            .load()
            .playback()
            .write()
            .reserve_source_occurrence_watermark(watermark);
    }

    pub fn application_time(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    pub fn execute_playback(
        &self,
        command: EnginePlaybackCommand,
    ) -> Result<EnginePlaybackOutcome, String> {
        let generation = self.generation.load();
        execute(
            &mut generation.playback().write(),
            command,
            &generation.snapshot().playback_solo_regions,
        )
    }

    pub fn enabled_auto_off_playbacks(&self) -> Vec<PlaybackAddress> {
        let generation = self.generation.load();
        let playback = generation.playback().read();
        let mut addresses = generation
            .snapshot()
            .playback_pages
            .iter()
            .flat_map(|page| {
                page.assignments
                    .iter()
                    .filter_map(|(position, definition)| {
                        let address = PlaybackAddress::new(page.number, *position).ok()?;
                        (definition.auto_off
                            && playback
                                .playback_runtime(address)
                                .is_some_and(|runtime| runtime.enabled))
                        .then_some(address)
                    })
            })
            .collect::<Vec<_>>();
        addresses.sort_unstable();
        addresses.dedup();
        addresses
    }

    pub fn permanent_playback_enabled(&self, address: PlaybackAddress) -> bool {
        self.generation
            .load()
            .playback()
            .read()
            .permanent_enabled(address)
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
        identity: light_playback::PlaybackAddress,
    ) -> Option<PlaybackRuntimeStatus> {
        self.generation
            .load()
            .playback()
            .read()
            .runtime_status_for_playback(identity)
    }

    pub fn playback_control_state(
        &self,
        address: PlaybackAddress,
        control_source: &light_playback::PlaybackControlSource,
    ) -> light_playback::PlaybackControlState {
        self.generation
            .load()
            .playback()
            .read()
            .control_state(address, control_source)
    }

    pub fn playback_control_state_at(
        &self,
        identity: PlaybackAddress,
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

    pub fn playback_runtime_status_for_direct_cue_list(
        &self,
        cue_list_id: light_core::CueListId,
    ) -> Option<PlaybackRuntimeStatus> {
        self.generation
            .load()
            .playback()
            .read()
            .runtime_status_for_direct_cue_list(cue_list_id)
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

    /// Install a transient start receipt after a successful output transaction.
    pub fn acknowledge_dynamic_playback_start(
        &self,
        target: uuid::Uuid,
        incarnation: uuid::Uuid,
        outcome: light_playback::DynamicPlaybackStartOutcome,
    ) -> bool {
        self.generation
            .load()
            .playback()
            .write()
            .acknowledge_dynamic_playback_start(target, incarnation, outcome)
    }

    pub fn active_dynamic_playbacks(&self) -> Vec<ActiveDynamicPlayback> {
        self.generation
            .load()
            .playback()
            .read()
            .active_dynamic_playbacks()
    }

    pub fn active_dynamic_playback(
        &self,
        identity: PlaybackAddress,
    ) -> Option<ActiveDynamicPlayback> {
        self.generation
            .load()
            .playback()
            .read()
            .active_dynamic_playback(identity)
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
        identities: impl IntoIterator<Item = PlaybackAddress>,
    ) -> Vec<PlaybackAddress> {
        let generation = self.generation.load();
        let mut playback = generation.playback().write();
        let mut changed = Vec::new();
        for identity in identities {
            if !playback
                .dynamic_assignment(identity)
                .is_some_and(|assignment| assignment.auto_off_full_control)
                || !playback
                    .active_dynamic_playback(identity)
                    .is_some_and(|active| active.enabled)
            {
                continue;
            }
            let turned_off = if playback.dynamic_assignment(identity).is_some() {
                playback
                    .off_dynamic_mutation(identity)
                    .is_ok_and(|mutation| mutation.value)
            } else {
                playback.off(identity).unwrap_or(false)
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
            .contributions_with_context(
                at,
                Some(&|_, attribute| light_playback::attribute_uses_snap_transition(attribute)),
            )
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

pub(crate) fn execute(
    playback: &mut PlaybackEngine,
    command: EnginePlaybackCommand,
    regions: &[PlaybackSoloRegion],
) -> Result<EnginePlaybackOutcome, String> {
    match command {
        // Direct Cue control (including Timecode clips) does not activate a Playback member.
        EnginePlaybackCommand::CueList { id, action } => execute_cue_list(playback, id, action),
        EnginePlaybackCommand::Playback {
            address,
            action,
            activation_origin,
            control_source,
        } => apply_with_solo(
            playback,
            address,
            &action,
            regions,
            activation_origin,
            &control_source,
        ),
        EnginePlaybackCommand::EnforceSoloForTarget { address } => {
            let effect =
                crate::playback_exclusion::enforce_target_solo(playback, address, regions)?;
            Ok(EnginePlaybackOutcome::Changed(
                EnginePlaybackEffect::from_addressed(effect),
            ))
        }
        EnginePlaybackCommand::CancelControlSurface(surface_id) => Ok(
            EnginePlaybackOutcome::Changed(playback.cancel_control_surface(&surface_id).into()),
        ),
        EnginePlaybackCommand::ReleaseAddressBatch(addresses) => {
            let mut released = Vec::new();
            for address in addresses {
                if playback
                    .release_mutation(address)
                    .is_ok_and(|mutation| mutation.effect.changed())
                {
                    released.push(address);
                }
            }
            released.sort_unstable();
            released.dedup();
            Ok(EnginePlaybackOutcome::ChangedPlaybacks(released))
        }
        EnginePlaybackCommand::RestorePlaybackRuntime {
            playbacks,
            dynamic_playbacks,
        } => {
            playback.restore_permanent_runtime(playbacks, dynamic_playbacks);
            crate::playback_exclusion::enforce_restored_solo(playback, regions)?;
            Ok(EnginePlaybackOutcome::Applied)
        }
        EnginePlaybackCommand::RestoreActive(active) => {
            playback.restore_active(active);
            crate::playback_exclusion::enforce_restored_solo(playback, regions)?;
            Ok(EnginePlaybackOutcome::Applied)
        }
        EnginePlaybackCommand::RestoreActiveDynamics(active) => {
            playback.restore_active_dynamics(active);
            crate::playback_exclusion::enforce_restored_solo(playback, regions)?;
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
        CueListPlaybackAction::ExecuteCueId(cue_id) => playback
            .execute_timeline_cue_id(id, cue_id)
            .cloned()
            .map(Box::new)
            .map(EnginePlaybackOutcome::Active),
        CueListPlaybackAction::ReconstructCueId {
            cue_id,
            elapsed_millis,
        } => playback
            .reconstruct_to_cue_id(id, cue_id, elapsed_millis)
            .cloned()
            .map(Box::new)
            .map(EnginePlaybackOutcome::Active),
        CueListPlaybackAction::ReconstructTimelineCues(cues) => playback
            .reconstruct_timeline_cues(id, &cues)
            .cloned()
            .map(Box::new)
            .map(EnginePlaybackOutcome::Active),
        CueListPlaybackAction::ReconstructSampledTimelineCues(cues) => playback
            .reconstruct_sampled_timeline_cues(id, &cues)
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
        CueListPlaybackAction::Resume => {
            playback.resume(id).map(|()| EnginePlaybackOutcome::Applied)
        }
        CueListPlaybackAction::Release => Ok(EnginePlaybackOutcome::Changed(addressed_effect(
            durable_effect(playback.release(id)),
        ))),
        CueListPlaybackAction::SetMaster(value) => {
            let effect = playback.set_cue_list_master_mutation(id, value)?.effect;
            Ok(EnginePlaybackOutcome::Changed(addressed_effect(effect)))
        }
    }
}

pub(crate) fn execute_addressed(
    playback: &mut PlaybackEngine,
    number: PlaybackAddress,
    action: PlaybackAction,
    source: &PlaybackControlSource,
) -> Result<EnginePlaybackOutcome, String> {
    let effects = match action {
        PlaybackAction::Go => playback
            .go_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))?,
        PlaybackAction::Back => playback
            .back_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))?,
        PlaybackAction::Pause if playback.dynamic_assignment(number).is_some() => {
            playback.toggle_dynamic_pause_mutation(number)?.into()
        }
        PlaybackAction::Pause => playback.pause_playback_mutation(number)?.into(),
        PlaybackAction::TogglePause => toggle_pause(playback, number)?,
        PlaybackAction::FastForward => playback
            .fast_forward_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))?,
        PlaybackAction::FastRewind => playback
            .fast_rewind_playback(number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable))?,
        PlaybackAction::On => playback.on_mutation(number)?.into(),
        PlaybackAction::Off => playback.off_mutation(number)?.into(),
        PlaybackAction::Release => playback.release_mutation(number)?.into(),
        PlaybackAction::Toggle => playback.toggle_mutation(number)?.into(),
        PlaybackAction::GoTo(cue) => playback.goto_playback_mutation(number, cue)?.into(),
        PlaybackAction::Load(cue) => playback.load_playback_mutation(number, cue)?.into(),
        PlaybackAction::SetMaster(value) => {
            playback.set_master_mutation(number, source, value)?.into()
        }
        PlaybackAction::SetMasterWithExplicitActivation(value) => playback
            .set_master_with_explicit_activation_mutation(number, value)?
            .into(),
        PlaybackAction::SetConfiguredFader { mode, value } => playback
            .set_configured_fader_mutation(number, source, mode, value)?
            .into(),
        PlaybackAction::SetConfiguredFaderAbsolute { mode, value } => playback
            .set_configured_fader_absolute_mutation(number, source, mode, value)?
            .into(),
        PlaybackAction::SetGroupMasterFader {
            value,
            authoritative,
        } => playback
            .set_group_master_fader_mutation(number, source, value, authoritative)?
            .into(),
        PlaybackAction::SetMasterTransition {
            value,
            duration_millis,
        } => playback
            .set_master_transition_mutation(number, value, duration_millis)?
            .into(),
        PlaybackAction::SetManualXFade(value) => {
            playback.set_manual_xfade_mutation(number, value)?.into()
        }
        PlaybackAction::XFade(on) => playback.xfade_mutation(number, on)?.into(),
        PlaybackAction::SetTempButton(active) => playback
            .set_temp_button_mutation(number, source, active)?
            .into(),
        PlaybackAction::ToggleTemp => playback.toggle_temp_mutation(number, source)?.into(),
        PlaybackAction::SetFlash(pressed) => {
            if playback.dynamic_assignment(number).is_some() {
                playback
                    .set_dynamic_flash_mutation(number, source, pressed)?
                    .into()
            } else {
                playback.set_flash_mutation(number, source, pressed)?.into()
            }
        }
        PlaybackAction::SetSwap(pressed) => {
            playback.set_swap_mutation(number, source, pressed)?.into()
        }
        PlaybackAction::DynamicRestart => playback.restart_dynamic_mutation(number)?.into(),
        PlaybackAction::DynamicDoubleSpeed => {
            playback.scale_dynamic_speed_mutation(number, true)?.into()
        }
        PlaybackAction::DynamicHalfSpeed => {
            playback.scale_dynamic_speed_mutation(number, false)?.into()
        }
        PlaybackAction::DynamicLearnSpeed => playback.tap_dynamic_speed_mutation(number)?.into(),
    };
    Ok(EnginePlaybackOutcome::Changed(effects))
}

fn toggle_pause(
    playback: &mut PlaybackEngine,
    number: PlaybackAddress,
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
