use crate::{
    Engine, RuntimeGeneration,
    playback::{EnginePlaybackEffect, addressed_effect},
    playback_exclusion::apply_with_exclusions,
};
use chrono::{DateTime, Utc};
use light_playback::{
    PlaybackEngine, PlaybackIdentity, PlaybackRuntimeEffect, VirtualPlaybackAddress,
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
    SetFader { value_permyriad: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackBatchCommand {
    pub number: u16,
    pub page: Option<u8>,
    pub action: PlaybackBatchAction,
    pub exclusion_zones: Arc<[Vec<u16>]>,
    pub activation_origin: Option<light_playback::PlaybackActivationOrigin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackBatchOutcome {
    pub number: u16,
    pub page: Option<u8>,
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
    if command.number >= light_playback::MIN_VIRTUAL_PLAYBACK {
        return apply_virtual_command(playback, command, started_at, fallback_millis);
    }
    let previous = playback.preload_timing_state(command.number);
    let (effects, released_playbacks) = apply_with_exclusions(
        playback,
        command.number,
        &command.exclusion_zones,
        command.activation_origin,
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
        page: command.page,
        released_playbacks,
        addressed_effect,
        effect,
    })
}

fn apply_virtual_command(
    playback: &mut PlaybackEngine,
    command: &PlaybackBatchCommand,
    started_at: DateTime<Utc>,
    fallback_millis: u64,
) -> Result<PlaybackBatchOutcome, String> {
    let page = command
        .page
        .ok_or("virtual Preload Playback action requires a page")?;
    let address = VirtualPlaybackAddress::new(page, command.number)?;
    let identity = PlaybackIdentity::Virtual(address);
    let previous = playback.preload_timing_state_at(identity);
    let zones = command
        .exclusion_zones
        .iter()
        .map(|zone| {
            zone.iter()
                .copied()
                .map(|number| VirtualPlaybackAddress::new(page, number))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let peers = zones
        .iter()
        .filter(|zone| zone.contains(&address))
        .flatten()
        .copied()
        .filter(|peer| *peer != address)
        .filter(|peer| playback.is_active_at(PlaybackIdentity::Virtual(*peer)))
        .collect::<std::collections::BTreeSet<_>>();
    let outcome = crate::playback::execute_virtual(
        playback,
        address,
        virtual_action(command.action)?,
        &zones,
        command.activation_origin,
    )?;
    let effects = match outcome {
        crate::EnginePlaybackOutcome::Changed(effect) => effect,
        _ => EnginePlaybackEffect::default(),
    };
    let timing_effect = if effects.addressed.changed() {
        playback
            .apply_preload_timing_at_mutation(
                identity,
                action_name(command.action),
                started_at,
                fallback_millis,
                previous,
            )?
            .effect
    } else {
        PlaybackRuntimeEffect::None
    };
    let released_playbacks = peers
        .into_iter()
        .filter(|peer| !playback.is_active_at(PlaybackIdentity::Virtual(*peer)))
        .map(|peer| peer.number().get())
        .collect::<Vec<_>>();
    Ok(PlaybackBatchOutcome {
        number: command.number,
        page: command.page,
        released_playbacks,
        addressed_effect: effects.addressed.combine(timing_effect),
        effect: effects.aggregate.combine(timing_effect),
    })
}

fn virtual_action(action: PlaybackBatchAction) -> Result<crate::VirtualPlaybackAction, String> {
    use crate::VirtualPlaybackAction as Virtual;
    Ok(match action {
        PlaybackBatchAction::Toggle => Virtual::Toggle,
        PlaybackBatchAction::Go => Virtual::Go,
        PlaybackBatchAction::Back => Virtual::Back,
        PlaybackBatchAction::Off => Virtual::Off,
        PlaybackBatchAction::On => Virtual::On,
        PlaybackBatchAction::SetTempButton(active) => Virtual::SetTempButton(active),
        PlaybackBatchAction::TogglePause => Virtual::Pause,
        PlaybackBatchAction::DynamicRestart => Virtual::DynamicRestart,
        PlaybackBatchAction::DynamicDoubleSpeed => Virtual::DynamicDoubleSpeed,
        PlaybackBatchAction::DynamicHalfSpeed => Virtual::DynamicHalfSpeed,
        PlaybackBatchAction::DynamicLearnSpeed => Virtual::DynamicLearnSpeed,
        PlaybackBatchAction::SetFader { value_permyriad } if value_permyriad <= 10_000 => {
            Virtual::SetMaster(f32::from(value_permyriad) / 10_000.0)
        }
        PlaybackBatchAction::SetFader { .. } => {
            return Err("Preload Playback fader value must be within 0-10000".into());
        }
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
        PlaybackBatchAction::TogglePause => playback
            .toggle_dynamic_pause_mutation(command.number)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::DynamicRestart => playback
            .restart_dynamic_mutation(command.number)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::DynamicDoubleSpeed => playback
            .scale_dynamic_speed_mutation(command.number, true)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::DynamicHalfSpeed => playback
            .scale_dynamic_speed_mutation(command.number, false)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::DynamicLearnSpeed => playback
            .tap_dynamic_speed_mutation(command.number)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::SetFader { value_permyriad } if value_permyriad <= 10_000 => playback
            .set_master_mutation(command.number, f32::from(value_permyriad) / 10_000.0)
            .map(EnginePlaybackEffect::from),
        PlaybackBatchAction::SetFader { .. } => {
            Err("Preload Playback fader value must be within 0-10000".into())
        }
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
        PlaybackBatchAction::TogglePause => "dynamic-pause",
        PlaybackBatchAction::DynamicRestart => "dynamic-restart",
        PlaybackBatchAction::DynamicDoubleSpeed => "dynamic-double-speed",
        PlaybackBatchAction::DynamicHalfSpeed => "dynamic-half-speed",
        PlaybackBatchAction::DynamicLearnSpeed => "dynamic-learn-speed",
        PlaybackBatchAction::SetFader { .. } => "fader",
    }
}
