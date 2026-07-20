use crate::{
    Engine, RuntimeGeneration,
    playback::{EnginePlaybackEffect, addressed_effect},
    playback_exclusion::apply_with_exclusions,
};
use chrono::{DateTime, Utc};
use light_playback::{PlaybackEngine, PlaybackRuntimeEffect};
use std::{collections::BTreeMap, sync::Arc};

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
    pub exclusion_zones: Arc<[Vec<u16>]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackBatchOutcome {
    pub number: u16,
    pub released_playbacks: Vec<u16>,
    pub addressed_effect: PlaybackRuntimeEffect,
    pub effect: PlaybackRuntimeEffect,
}

/// A validated, isolated Playback batch tied to the Engine generation it was prepared from.
#[must_use = "a prepared Playback batch must be installed to affect live output"]
pub struct PreparedPlaybackBatch {
    generation: Arc<RuntimeGeneration>,
    playback: PlaybackEngine,
    outcomes: Vec<PlaybackBatchOutcome>,
    effect: PlaybackRuntimeEffect,
    numbered_effects: BTreeMap<u16, PlaybackRuntimeEffect>,
}

impl PreparedPlaybackBatch {
    pub fn outcomes(&self) -> &[PlaybackBatchOutcome] {
        &self.outcomes
    }

    pub const fn effect(&self) -> PlaybackRuntimeEffect {
        self.effect
    }

    pub fn changed_playback_numbers(&self) -> impl Iterator<Item = u16> + '_ {
        self.numbered_effects.keys().copied()
    }

    pub fn effect_for(&self, number: u16) -> PlaybackRuntimeEffect {
        self.numbered_effects
            .get(&number)
            .copied()
            .unwrap_or_default()
    }
}

impl Engine {
    pub fn prepare_playback_batch(
        &self,
        commands: &[PlaybackBatchCommand],
        started_at: DateTime<Utc>,
        fallback_millis: u64,
    ) -> Result<PreparedPlaybackBatch, String> {
        let generation = self.generation.load_full();
        let mut playback = generation.playback().read().clone();
        let outcomes: Vec<PlaybackBatchOutcome> = commands
            .iter()
            .map(|command| apply_command(&mut playback, command, started_at, fallback_millis))
            .collect::<Result<_, _>>()?;
        let before = generation.playback().read();
        let effect = playback.retained_runtime_effect_since(&before);
        let numbered_effects = playback
            .numbered_runtime_effects_since(&before)
            .into_iter()
            .collect();
        drop(before);
        Ok(PreparedPlaybackBatch {
            generation,
            playback,
            outcomes,
            effect,
            numbered_effects,
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
        if prepared.effect.changed() {
            *prepared.generation.playback().write() = prepared.playback;
        }
        Ok(())
    }
}

fn apply_command(
    playback: &mut PlaybackEngine,
    command: &PlaybackBatchCommand,
    started_at: DateTime<Utc>,
    fallback_millis: u64,
) -> Result<PlaybackBatchOutcome, String> {
    let previous = playback
        .playback_runtime(command.number)
        .map(|runtime| (runtime.enabled, runtime.master));
    let (effects, released_playbacks) = apply_with_exclusions(
        playback,
        command.number,
        &command.exclusion_zones,
        |playback| apply_action(playback, command),
    )?;
    let timing_effect = if effects.addressed.changed() {
        playback
            .apply_preload_timing_mutation(
                command.number,
                action_name(command.action),
                started_at,
                fallback_millis,
                previous,
            )?
            .effect
    } else {
        PlaybackRuntimeEffect::None
    };
    let addressed_effect = effects.addressed.combine(timing_effect);
    let effect = effects
        .aggregate
        .combine(release_effect(&released_playbacks))
        .combine(timing_effect);
    Ok(PlaybackBatchOutcome {
        number: command.number,
        released_playbacks,
        addressed_effect,
        effect,
    })
}

fn apply_action(
    playback: &mut PlaybackEngine,
    command: &PlaybackBatchCommand,
) -> Result<EnginePlaybackEffect, String> {
    match command.action {
        PlaybackBatchAction::Toggle => playback
            .toggle_mutation(command.number)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::Go => playback
            .go_playback(command.number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable)),
        PlaybackBatchAction::Back => playback
            .back_playback(command.number)
            .map(|_| addressed_effect(PlaybackRuntimeEffect::Durable)),
        PlaybackBatchAction::Off => playback
            .off_mutation(command.number)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::On => playback
            .on_mutation(command.number)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::SetTempButton(active) => playback
            .set_temp_button_mutation(command.number, active)
            .map(EnginePlaybackEffect::from),
    }
}

const fn release_effect(released: &[u16]) -> PlaybackRuntimeEffect {
    if released.is_empty() {
        PlaybackRuntimeEffect::None
    } else {
        PlaybackRuntimeEffect::Durable
    }
}

const fn action_name(action: PlaybackBatchAction) -> &'static str {
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
