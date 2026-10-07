#![forbid(unsafe_code)]
//! Tracking cue lists, live playback state, and HTP/LTP arbitration.

mod arbitration;
mod automatic;
mod compiled;
mod contribution;
mod controls;
mod cue_recording;
mod cue_tracking;
mod cue_transfer;
mod engine;
mod model;
mod programming_validation;
mod runtime;
mod source_evidence;
mod timecode;
mod transition;

pub use arbitration::resolve;
pub use automatic::{
    AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause, PlaybackCueReference,
    PlaybackTickResult,
};
pub use contribution::FamilyStartSource;
pub use controls::{PlaybackMutation, PlaybackRuntimeEffect, dynamic_playback_controller_id};
pub use cue_recording::{
    CueListRecordingPlan, CueRecordOperation, CueRecordingContent, CueRecordingPlanError,
    CueRecordingTiming, refresh_cue_only_restorations,
};
pub use cue_transfer::{CueTransferMode, transferred_cue};
pub use engine::PlaybackEngine;
pub use model::CueNumber;
pub use model::{cue::*, playback::*, runtime::*};
pub use runtime::PlaybackTelemetrySample;
pub use source_evidence::{
    PlaybackFamilyEntry, PlaybackFamilyEvidence, PlaybackFamilyFootprint, PlaybackFamilyRole,
    PlaybackRetainedValue, PlaybackSourceHistory, PlaybackSourceOccurrence,
};
pub use timecode::*;
pub use transition::attribute_uses_snap_transition;

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
pub(crate) use engine::DynamicFlashState;
pub(crate) use model::cue::{
    cue_completion_millis, effective_chaser_step_millis, effective_cue_fade_millis,
    effective_cue_out_timing,
};
pub(crate) use model::runtime::{
    PlaybackKey, advance_chaser_steps, new_active_playback, reset_manual_transition,
};
pub(crate) use transition::{interpolate, interpolate_pending};

#[cfg(test)]
mod tests;
