#![forbid(unsafe_code)]
//! Tracking cue lists, live playback state, phasers, and HTP/LTP arbitration.

mod arbitration;
mod automatic;
mod compiled;
mod contribution;
mod controls;
mod cue_tracking;
mod engine;
mod model;
mod phaser;
mod runtime;
mod transition;

pub use arbitration::resolve;
pub use automatic::{
    AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause, PlaybackCueReference,
    PlaybackTickResult,
};
pub use engine::PlaybackEngine;
pub use model::{cue::*, playback::*, runtime::*};
pub use phaser::*;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use light_core::{
    AttributeKey, AttributeValue, CueListId, FixtureId, MergeMode, SharedClock, SystemClock,
    TimedValue,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

type AttributeAddress = (FixtureId, AttributeKey);

pub(crate) use compiled::{CompiledAttribute, CompiledCueList};
pub(crate) use model::cue::{cue_completion_millis, effective_chaser_step_millis};
pub(crate) use model::runtime::{
    PlaybackKey, TemporaryPlaybackKind, advance_chaser_steps, new_active_playback,
    reset_manual_transition,
};
pub(crate) use transition::interpolate;

#[cfg(test)]
mod tests;
