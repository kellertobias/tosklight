use crate::{Engine, RuntimeGeneration, playback_exclusion::apply_with_solo};
use chrono::{DateTime, Utc};
use light_playback::{
    PlaybackAddress, PlaybackControlSource, PlaybackEngine, PlaybackRuntimeEffect,
    PlaybackSoloRegion,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackBatchAction {
    Toggle,
    Go,
    Back,
    Off,
    On,
    SetTempButton(bool),
    TogglePause,
    DynamicRestart,
    DynamicDoubleSpeed,
    DynamicHalfSpeed,
    DynamicLearnSpeed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackBatchCommand {
    pub address: PlaybackAddress,
    pub action: PlaybackBatchAction,
    pub control_source: PlaybackControlSource,
    pub activation_origin: Option<light_playback::PlaybackActivationOrigin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackBatchOutcome {
    /// Internal ordered-batch fact: this step started a permanent owner, never a Flash hold.
    pub permanent_started: bool,
    pub address: PlaybackAddress,
    pub released_playbacks: Vec<PlaybackAddress>,
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
    numbered_effects: BTreeMap<PlaybackAddress, PlaybackRuntimeEffect>,
}

impl PreparedPlaybackBatch {
    pub fn matches_snapshot(&self, snapshot: &Arc<crate::EngineSnapshot>) -> bool {
        Arc::ptr_eq(&self.generation.snapshot_arc(), snapshot)
    }

    pub fn outcomes(&self) -> &[PlaybackBatchOutcome] {
        &self.outcomes
    }

    pub const fn effect(&self) -> PlaybackRuntimeEffect {
        self.effect
    }

    pub fn changed_playback_addresses(&self) -> impl Iterator<Item = PlaybackAddress> + '_ {
        self.numbered_effects.keys().copied()
    }

    pub fn effect_for(&self, number: PlaybackAddress) -> PlaybackRuntimeEffect {
        self.numbered_effects
            .get(&number)
            .copied()
            .unwrap_or_default()
    }
}

/// An isolated execution of one ordered queue against its captured Playback source.
///
/// This type cannot be installed by `install_prepared_playback_batch`. It is a queue executor
/// primitive, not a retained preview episode: overlay/rebase lifecycle, intermediate Dynamic
/// restart effects and scheduler publication remain the caller's separate integration work.
pub struct PreparedPreloadPlaybackBatch {
    playback: PlaybackEngine,
    outcomes: Vec<PlaybackBatchOutcome>,
    effect: PlaybackRuntimeEffect,
}

impl PreparedPreloadPlaybackBatch {
    pub fn runtime(&self) -> &PlaybackEngine {
        &self.playback
    }

    pub fn outcomes(&self) -> &[PlaybackBatchOutcome] {
        &self.outcomes
    }

    pub const fn effect(&self) -> PlaybackRuntimeEffect {
        self.effect
    }
}

impl Engine {
    /// Execute a complete hypothetical Preload GO using only the supplied immutable frame.
    /// Commands retain their order and duplicates; failure discards the whole isolated result.
    /// Neither Live state nor the captured queue is read again, drained, ticked or installed.
    pub fn prepare_preload_playback_batch(
        &self,
        frame: &crate::PreparedOutputFrame,
        commands: &[PlaybackBatchCommand],
    ) -> Result<PreparedPreloadPlaybackBatch, String> {
        let source = frame
            .preload_playback_source()
            .ok_or("the output frame has no captured Preload Playback source")?;
        let started_at = frame.sampled_at();
        let mut playback = source.fork_for_preview(started_at);
        let outcomes = commands
            .iter()
            .map(|command| {
                let mut command = command.clone();
                if let Some(origin) = command.activation_origin.as_mut() {
                    origin.at = started_at;
                }
                apply_command(
                    &mut playback,
                    &command,
                    started_at,
                    frame.programmer_fade_millis,
                    &frame.snapshot().playback_solo_regions,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let effect = playback.retained_runtime_effect_since(source);
        Ok(PreparedPreloadPlaybackBatch {
            playback,
            outcomes,
            effect,
        })
    }

    pub fn prepare_playback_batch(
        &self,
        commands: &[PlaybackBatchCommand],
        started_at: DateTime<Utc>,
        fallback_millis: u64,
    ) -> Result<PreparedPlaybackBatch, String> {
        self.prepare_playback_batch_from_published(commands, started_at, fallback_millis, None)
    }

    pub fn prepare_playback_batch_from_published(
        &self,
        commands: &[PlaybackBatchCommand],
        started_at: DateTime<Utc>,
        fallback_millis: u64,
        start: Option<&crate::PlaybackStartFrame>,
    ) -> Result<PreparedPlaybackBatch, String> {
        self.prepare_playback_batch_with_steps(
            commands,
            started_at,
            fallback_millis,
            start,
            |playback, command, regions| {
                apply_command(playback, command, started_at, fallback_millis, regions)
            },
        )
    }

    /// Extend one isolated, ordered batch with an authority owned outside Playback. The callback
    /// may mutate only the supplied candidate and its own isolated participants.
    pub fn prepare_playback_batch_with_steps(
        &self,
        commands: &[PlaybackBatchCommand],
        started_at: DateTime<Utc>,
        fallback_millis: u64,
        start: Option<&crate::PlaybackStartFrame>,
        mut step: impl FnMut(
            &mut PlaybackEngine,
            &PlaybackBatchCommand,
            &[PlaybackSoloRegion],
        ) -> Result<PlaybackBatchOutcome, String>,
    ) -> Result<PreparedPlaybackBatch, String> {
        let _ = fallback_millis;
        let generation = self.generation.load_full();
        let start = start.filter(|start| start.matches(&generation));
        let mut playback = generation.playback().read().clone();
        let outcomes: Vec<PlaybackBatchOutcome> = commands
            .iter()
            .map(|command| {
                let identity = command.address;
                let seed = start
                    .filter(|_| {
                        matches!(
                            command.action,
                            PlaybackBatchAction::On
                                | PlaybackBatchAction::Go
                                | PlaybackBatchAction::Toggle
                        )
                    })
                    .filter(|_| !crate::playback_start::enabled(&playback, identity))
                    .filter(|_| {
                        !playback.definition(identity).is_some_and(|definition| {
                            matches!(
                                definition.target,
                                light_playback::PlaybackTarget::Timecode { .. }
                            )
                        })
                    });
                playback.with_captured_time(started_at, |playback| {
                    let outcome = step(
                        playback,
                        command,
                        &generation.snapshot().playback_solo_regions,
                    )?;
                    if let Some(start) = seed {
                        start.adopt(playback, identity)?;
                    }
                    Ok(outcome)
                })
            })
            .collect::<Result<_, _>>()?;
        let before = generation.playback().read();
        let mut effect = playback.retained_runtime_effect_since(&before);
        let mut numbered_effects: BTreeMap<_, _> = playback
            .numbered_runtime_effects_since(&before)
            .into_iter()
            .collect();
        for outcome in &outcomes {
            if playback
                .definition(outcome.address)
                .is_some_and(|definition| {
                    matches!(
                        definition.target,
                        light_playback::PlaybackTarget::Timecode { .. }
                    )
                })
            {
                effect = effect.combine(outcome.effect);
                numbered_effects
                    .entry(outcome.address)
                    .and_modify(|effect| *effect = effect.combine(outcome.addressed_effect))
                    .or_insert(outcome.addressed_effect);
            }
        }
        drop(before);
        Ok(PreparedPlaybackBatch {
            generation,
            playback,
            outcomes,
            effect,
            numbered_effects,
        })
    }

    pub fn apply_prepared_playback_step(
        playback: &mut PlaybackEngine,
        command: &PlaybackBatchCommand,
        started_at: DateTime<Utc>,
        fallback_millis: u64,
        regions: &[PlaybackSoloRegion],
    ) -> Result<PlaybackBatchOutcome, String> {
        apply_command(playback, command, started_at, fallback_millis, regions)
    }

    pub fn execute_prepared_cue_command(
        playback: &mut PlaybackEngine,
        command: crate::EnginePlaybackCommand,
    ) -> Result<crate::EnginePlaybackOutcome, String> {
        if !matches!(command, crate::EnginePlaybackCommand::CueList { .. }) {
            return Err("prepared Timecode executor accepts only direct Cue commands".into());
        }
        crate::playback::execute(playback, command, &[])
    }

    pub fn enforce_prepared_target_solo(
        playback: &mut PlaybackEngine,
        address: PlaybackAddress,
        regions: &[PlaybackSoloRegion],
    ) -> Result<PlaybackRuntimeEffect, String> {
        crate::playback_exclusion::enforce_target_solo(playback, address, regions)
    }

    pub fn install_prepared_playback_batch(
        &self,
        prepared: PreparedPlaybackBatch,
    ) -> Result<(), String> {
        self.install_prepared_playback_batch_with(prepared, |_| Ok(()), |()| ())
    }

    /// Validate the generation, then prepare dependent state under the Playback write guard.
    /// The preparation must leave live state untouched on error. Installation is infallible;
    /// adapters can retain a later-ranked runtime guard in `T` across the Playback swap.
    pub fn install_prepared_playback_batch_with<T, R>(
        &self,
        prepared: PreparedPlaybackBatch,
        prepare: impl FnOnce(&PlaybackEngine) -> Result<T, String>,
        install: impl FnOnce(T) -> R,
    ) -> Result<R, String> {
        let current = self.generation.load_full();
        if !Arc::ptr_eq(&current, &prepared.generation) {
            return Err("the compiled show changed while Playback was being prepared".into());
        }
        let mut live = current.playback().write();
        let dependent = prepare(&prepared.playback)?;
        if prepared.effect.changed() {
            *live = prepared.playback;
        }
        Ok(install(dependent))
    }
}

fn apply_command(
    playback: &mut PlaybackEngine,
    command: &PlaybackBatchCommand,
    started_at: DateTime<Utc>,
    fallback_millis: u64,
    regions: &[PlaybackSoloRegion],
) -> Result<PlaybackBatchOutcome, String> {
    let was_enabled = playback.permanent_enabled(command.address);
    let previous = playback.preload_timing_state(command.address);
    let before: Vec<_> =
        crate::playback_exclusion::target_solo_members(playback, command.address, regions)
            .into_iter()
            .filter(|address| playback.permanent_enabled(*address))
            .collect();
    let outcome = apply_with_solo(
        playback,
        command.address,
        &action(command.action),
        regions,
        command.activation_origin,
        &command.control_source,
    )?;
    let crate::EnginePlaybackOutcome::Changed(effects) = outcome else {
        return Err("unexpected Preload Playback action outcome".into());
    };
    let timing_effect = if effects.addressed.changed()
        && matches!(
            playback
                .definition(command.address)
                .map(|definition| &definition.target),
            Some(light_playback::PlaybackTarget::CueList { .. })
        ) {
        playback
            .apply_preload_timing_mutation(
                command.address,
                action_name(command.action),
                started_at,
                fallback_millis,
                previous,
            )?
            .effect
    } else {
        PlaybackRuntimeEffect::None
    };
    let released_playbacks = before
        .into_iter()
        .filter(|address| !playback.permanent_enabled(*address))
        .collect();
    Ok(PlaybackBatchOutcome {
        permanent_started: !was_enabled && playback.permanent_enabled(command.address),
        address: command.address,
        released_playbacks,
        addressed_effect: effects.addressed.combine(timing_effect),
        effect: effects.aggregate.combine(timing_effect),
    })
}
fn action(action: PlaybackBatchAction) -> crate::PlaybackAction {
    use crate::PlaybackAction;
    match action {
        PlaybackBatchAction::Toggle => PlaybackAction::Toggle,
        PlaybackBatchAction::Go => PlaybackAction::Go,
        PlaybackBatchAction::Back => PlaybackAction::Back,
        PlaybackBatchAction::Off => PlaybackAction::Off,
        PlaybackBatchAction::On => PlaybackAction::On,
        PlaybackBatchAction::SetTempButton(value) => PlaybackAction::SetTempButton(value),
        PlaybackBatchAction::TogglePause => PlaybackAction::TogglePause,
        PlaybackBatchAction::DynamicRestart => PlaybackAction::DynamicRestart,
        PlaybackBatchAction::DynamicDoubleSpeed => PlaybackAction::DynamicDoubleSpeed,
        PlaybackBatchAction::DynamicHalfSpeed => PlaybackAction::DynamicHalfSpeed,
        PlaybackBatchAction::DynamicLearnSpeed => PlaybackAction::DynamicLearnSpeed,
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
        PlaybackBatchAction::TogglePause => "dynamic-pause",
        PlaybackBatchAction::DynamicRestart => "dynamic-restart",
        PlaybackBatchAction::DynamicDoubleSpeed => "dynamic-double-speed",
        PlaybackBatchAction::DynamicHalfSpeed => "dynamic-half-speed",
        PlaybackBatchAction::DynamicLearnSpeed => "dynamic-learn-speed",
    }
}
