//! Cue activations begin at the output the operator actually saw.
//! Capture happens before the Playback guard; no producer is sampled while it is held.
use crate::playback::{PlaybackAction, execute};
use crate::{
    Engine, EnginePlaybackCommand, EnginePlaybackOutcome, EngineSnapshot, FrameValues,
    RuntimeGeneration,
};
use light_playback::{PlaybackAddress, PlaybackEngine};
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
        identity: PlaybackAddress,
    ) -> Result<(), String> {
        playback.adopt_published_color_start(identity, &|fixture, attribute| {
            self.values.raw_value(fixture, attribute).cloned()
        })
    }
}

pub(crate) fn enabled(playback: &PlaybackEngine, identity: PlaybackAddress) -> bool {
    playback
        .playback_runtime(identity)
        .is_some_and(|active| active.enabled)
}

fn starts(action: &PlaybackAction) -> bool {
    matches!(
        action,
        PlaybackAction::On | PlaybackAction::Go | PlaybackAction::Toggle
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
            EnginePlaybackCommand::Playback {
                address, action, ..
            } if starts(action) => Some(*address),
            _ => None,
        };
        let seed = identity.filter(|identity| !enabled(&playback, *identity));
        let outcome = execute(
            &mut playback,
            command,
            &generation.snapshot().playback_solo_regions,
        )?;
        if let (Some(start), Some(identity)) = (start, seed) {
            start.adopt(&mut playback, identity)?;
        }
        Ok(outcome)
    }
}
