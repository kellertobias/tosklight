//! Physical Cue activations begin at the output the operator actually saw.
//! Capture happens before the Playback guard; no producer is sampled while it is held.
use crate::playback::{PoolPlaybackAction, combine_release_effect, execute, execute_pool};
use crate::playback_exclusion::{PoolPlaybackTransition, apply_with_exclusions};
use crate::{
    Engine, EnginePlaybackCommand, EnginePlaybackOutcome, EngineSnapshot, FrameValues,
    RuntimeGeneration,
};
use light_playback::{PlaybackActivationOrigin, PlaybackEngine, PlaybackIdentity};
use std::sync::Arc;

/// Immutable accepted output, including Programmer and Dynamic winners after arbitration.
/// Raw values retain the premaster state, avoiding applying output masters twice.
#[derive(Clone)]
pub struct PlaybackStartFrame {
    snapshot: Arc<EngineSnapshot>,
    values: FrameValues,
}

impl PlaybackStartFrame {
    pub fn from_published(snapshot: Arc<EngineSnapshot>, values: FrameValues) -> Self {
        Self { snapshot, values }
    }

    pub(crate) fn matches(&self, generation: &RuntimeGeneration) -> bool {
        Arc::ptr_eq(&self.snapshot, &generation.snapshot_arc())
    }

    pub(crate) fn adopt(
        &self,
        playback: &mut PlaybackEngine,
        identity: PlaybackIdentity,
    ) -> Result<(), String> {
        playback.adopt_published_color_start(identity, &|fixture, attribute| {
            self.values.raw_value(fixture, attribute).cloned()
        })
    }
}

pub(crate) fn enabled(playback: &PlaybackEngine, identity: PlaybackIdentity) -> bool {
    playback
        .playback_runtime_at(identity)
        .is_some_and(|active| active.enabled)
}

fn starts(action: &PoolPlaybackAction) -> bool {
    matches!(
        action,
        PoolPlaybackAction::On | PoolPlaybackAction::Go | PoolPlaybackAction::Toggle
    )
}

impl Engine {
    pub fn execute_playback_from_published(
        &self,
        command: EnginePlaybackCommand,
        start: Option<&PlaybackStartFrame>,
    ) -> Result<EnginePlaybackOutcome, String> {
        let generation = self.generation.load();
        let start = start.filter(|start| start.matches(&generation));
        let mut playback = generation.playback().write();
        let identity = match &command {
            EnginePlaybackCommand::Pool { number, action }
                if starts(action) && *number < light_playback::MIN_VIRTUAL_PLAYBACK =>
            {
                Some(PlaybackIdentity::physical(*number)?)
            }
            _ => None,
        };
        let seed = identity.filter(|identity| !enabled(&playback, *identity));
        let outcome = execute(&mut playback, command)?;
        if let (Some(start), Some(identity)) = (start, seed) {
            start.adopt(&mut playback, identity)?;
        }
        Ok(outcome)
    }

    pub fn execute_pool_playback_with_published_activation(
        &self,
        number: u16,
        action: PoolPlaybackAction,
        exclusion_zones: &[Vec<u16>],
        activation_origin: Option<PlaybackActivationOrigin>,
        start: Option<&PlaybackStartFrame>,
    ) -> Result<PoolPlaybackTransition, String> {
        if number >= light_playback::MIN_VIRTUAL_PLAYBACK {
            return self.execute_pool_playback_with_activation(
                number,
                action,
                exclusion_zones,
                activation_origin,
            );
        }
        let generation = self.generation.load();
        let start = start.filter(|start| start.matches(&generation));
        let mut playback = generation.playback().write();
        let identity = PlaybackIdentity::physical(number)?;
        let seed = starts(&action) && !enabled(&playback, identity);
        let (outcome, released_playbacks) = apply_with_exclusions(
            &mut playback,
            number,
            exclusion_zones,
            activation_origin,
            |playback| execute_pool(playback, number, action),
        )?;
        if let Some(start) = start.filter(|_| seed) {
            start.adopt(&mut playback, identity)?;
        }
        Ok(PoolPlaybackTransition {
            outcome: combine_release_effect(outcome, &released_playbacks)?,
            released_playbacks,
        })
    }
}
