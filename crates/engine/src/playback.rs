use crate::{
    Engine, RuntimeGeneration,
    playback_exclusion::{PoolPlaybackTransition, apply_with_exclusions},
};
use chrono::{DateTime, Utc};
use light_core::CueListId;
use light_playback::{ActivePlayback, PlaybackContribution, PlaybackEngine, PlaybackRuntimeStatus};
use std::{collections::HashSet, sync::Arc};

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

#[derive(Clone, Debug)]
pub enum EnginePlaybackOutcome {
    Active(Box<ActivePlayback>),
    ActiveList(Vec<ActivePlayback>),
    Changed(bool),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackBatchAction {
    Toggle,
    Go,
    Back,
    Off,
    On,
    SetTempButton(bool),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackBatchCommand {
    pub number: u16,
    pub action: PlaybackBatchAction,
    pub exclusion_zones: Vec<Vec<u16>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackBatchOutcome {
    pub number: u16,
    pub released_playbacks: Vec<u16>,
}

/// A validated, isolated Playback batch tied to the Engine generation it was prepared from.
#[must_use = "a prepared Playback batch must be installed to affect live output"]
pub struct PreparedPlaybackBatch {
    generation: Arc<RuntimeGeneration>,
    playback: PlaybackEngine,
    outcomes: Vec<PlaybackBatchOutcome>,
}

impl PreparedPlaybackBatch {
    pub fn outcomes(&self) -> &[PlaybackBatchOutcome] {
        &self.outcomes
    }
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

    pub fn prepare_playback_batch(
        &self,
        commands: &[PlaybackBatchCommand],
        started_at: DateTime<Utc>,
        fallback_millis: u64,
    ) -> Result<PreparedPlaybackBatch, String> {
        let generation = self.generation.load_full();
        let mut playback = generation.playback().read().clone();
        let mut outcomes = Vec::with_capacity(commands.len());
        for command in commands {
            outcomes.push(apply_batch_command(
                &mut playback,
                command,
                started_at,
                fallback_millis,
            )?);
        }
        Ok(PreparedPlaybackBatch {
            generation,
            playback,
            outcomes,
        })
    }

    pub fn install_prepared_playback_batch(
        &self,
        prepared: PreparedPlaybackBatch,
    ) -> Result<(), String> {
        let current = self.generation.load_full();
        if !Arc::ptr_eq(&current, &prepared.generation) {
            return Err("the compiled show changed while Playback was being prepared".into());
        }
        *prepared.generation.playback().write() = prepared.playback;
        Ok(())
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
            playback.pause(id)?;
            Ok(EnginePlaybackOutcome::ActiveList(playback.active()))
        }
        CueListPlaybackAction::Release => Ok(EnginePlaybackOutcome::Changed(playback.release(id))),
    }
}

fn execute_pool(
    playback: &mut PlaybackEngine,
    number: u16,
    action: PoolPlaybackAction,
) -> Result<EnginePlaybackOutcome, String> {
    let changed = match action {
        PoolPlaybackAction::Go => playback.go_playback(number).map(|_| true)?,
        PoolPlaybackAction::Back => playback.back_playback(number).map(|_| true)?,
        PoolPlaybackAction::Pause => playback.pause_playback(number).map(|()| true)?,
        PoolPlaybackAction::TogglePause => toggle_pause(playback, number)?,
        PoolPlaybackAction::FastForward => playback.fast_forward_playback(number).map(|_| true)?,
        PoolPlaybackAction::FastRewind => playback.fast_rewind_playback(number).map(|_| true)?,
        PoolPlaybackAction::On => playback.on(number).map(|()| true)?,
        PoolPlaybackAction::Off => playback.off(number)?,
        // `PlaybackEngine::toggle` returns the resulting enabled state, not whether the
        // operation changed. A valid toggle always performs one semantic transition.
        PoolPlaybackAction::Toggle => playback.toggle(number).map(|_| true)?,
        PoolPlaybackAction::GoTo(cue) => playback.goto_playback(number, cue).map(|_| true)?,
        PoolPlaybackAction::Load(cue) => playback.load_playback(number, cue).map(|_| true)?,
        PoolPlaybackAction::SetMaster(value) => {
            playback.set_master(number, value).map(|()| true)?
        }
        PoolPlaybackAction::SetVirtualMaster(value) => {
            playback.set_virtual_master(number, value).map(|()| true)?
        }
        PoolPlaybackAction::SetManualXFade(value) => {
            playback.set_manual_xfade(number, value).map(|()| true)?
        }
        PoolPlaybackAction::XFade(on) => playback.xfade(number, on).map(|()| true)?,
        PoolPlaybackAction::SetTempButton(active) => {
            playback.set_temp_button(number, active).map(|()| true)?
        }
        PoolPlaybackAction::ToggleTemp => playback.toggle_temp(number).map(|_| true)?,
        PoolPlaybackAction::SetFlash(pressed) => {
            playback.set_flash(number, pressed).map(|()| true)?
        }
        PoolPlaybackAction::SetSwap(pressed) => {
            playback.set_swap(number, pressed).map(|()| true)?
        }
    };
    Ok(EnginePlaybackOutcome::Changed(changed))
}

fn toggle_pause(playback: &mut PlaybackEngine, number: u16) -> Result<bool, String> {
    let paused = playback
        .runtime()
        .iter()
        .any(|runtime| runtime.playback_number == Some(number) && runtime.paused);
    if paused {
        playback.go_playback(number).map(|_| true)
    } else {
        playback.pause_playback(number).map(|()| true)
    }
}

fn apply_batch_command(
    playback: &mut PlaybackEngine,
    command: &PlaybackBatchCommand,
    started_at: DateTime<Utc>,
    fallback_millis: u64,
) -> Result<PlaybackBatchOutcome, String> {
    let previous = playback
        .runtime()
        .into_iter()
        .find(|runtime| runtime.playback_number == Some(command.number))
        .map(|runtime| (runtime.enabled, runtime.master));
    let (_, released_playbacks) = apply_with_exclusions(
        playback,
        command.number,
        &command.exclusion_zones,
        |playback| apply_batch_action(playback, command),
    )?;
    playback.apply_preload_timing(
        command.number,
        batch_action_name(command.action),
        started_at,
        fallback_millis,
        previous,
    )?;
    Ok(PlaybackBatchOutcome {
        number: command.number,
        released_playbacks,
    })
}

fn apply_batch_action(
    playback: &mut PlaybackEngine,
    command: &PlaybackBatchCommand,
) -> Result<(), String> {
    match command.action {
        PlaybackBatchAction::Toggle => playback.toggle(command.number).map(|_| ()),
        PlaybackBatchAction::Go => playback.go_playback(command.number).map(|_| ()),
        PlaybackBatchAction::Back => playback.back_playback(command.number).map(|_| ()),
        PlaybackBatchAction::Off => playback.off(command.number).map(|_| ()),
        PlaybackBatchAction::On => playback.on(command.number),
        PlaybackBatchAction::SetTempButton(active) => {
            playback.set_temp_button(command.number, active)
        }
    }
}

const fn batch_action_name(action: PlaybackBatchAction) -> &'static str {
    match action {
        PlaybackBatchAction::Toggle => "toggle",
        PlaybackBatchAction::Go => "go",
        PlaybackBatchAction::Back => "go-minus",
        PlaybackBatchAction::Off => "off",
        PlaybackBatchAction::On => "on",
        PlaybackBatchAction::SetTempButton(true) => "temp-on",
        PlaybackBatchAction::SetTempButton(false) => "temp-off",
    }
}
