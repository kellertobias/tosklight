use crate::{
    Engine,
    playback_exclusion::{PoolPlaybackTransition, apply_with_exclusions},
};
use chrono::{DateTime, Utc};
use light_core::CueListId;
use light_playback::{
    ActivePlayback, PlaybackContribution, PlaybackEngine, PlaybackMutation, PlaybackRuntimeEffect,
    PlaybackRuntimeStatus,
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
    ReleasePoolBatch(Vec<u16>),
    RestoreActive(Vec<ActivePlayback>),
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
    SetVirtualMaster(f32),
    SetManualXFade(f32),
    XFade(bool),
    SetTempButton(bool),
    ToggleTemp,
    SetFlash(bool),
    SetSwap(bool),
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
        let generation = self.generation.load();
        let (outcome, released_playbacks) = apply_with_exclusions(
            &mut generation.playback().write(),
            number,
            exclusion_zones,
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
        EnginePlaybackCommand::ReleasePoolBatch(numbers) => Ok(
            EnginePlaybackOutcome::ChangedPlaybacks(release_pool_batch(playback, numbers)),
        ),
        EnginePlaybackCommand::RestoreActive(active) => {
            playback.restore_active(active);
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
            playback.set_flash_mutation(number, pressed)?.into()
        }
        PoolPlaybackAction::SetSwap(pressed) => playback.set_swap_mutation(number, pressed)?.into(),
    };
    Ok(EnginePlaybackOutcome::Changed(effects))
}

fn toggle_pause(
    playback: &mut PlaybackEngine,
    number: u16,
) -> Result<EnginePlaybackEffect, String> {
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
