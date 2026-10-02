//! One immutable Programmer source bundle for a desk output frame.
//!
//! The live state is read once under the mutation gate. Normal and active values already use
//! copy-on-write Arcs; pending fixture and Group values still use mutable collections, so their
//! owned copy is reused until the pending generation changes. The Playback action queue has an
//! independent generation so queue-only and value-only edits retain each other's immutable copy.

use std::sync::Arc;

use light_core::{ProgrammerId, TimedValue};
use light_dynamics::DynamicAddressValue;

use crate::groups::GroupProgrammerValues;
use crate::{
    ActiveDynamicSessionSource, GroupReleaseProgrammerValue, PreloadPlaybackAction,
    ProgrammerOutputState, ReleasedPreloadColors,
};

#[derive(Clone, Debug)]
pub struct ProgrammerPreloadPendingOutput {
    pub fixture_values: Arc<Vec<TimedValue>>,
    pub group_values: Arc<GroupProgrammerValues>,
    pub group_release_values: Arc<Vec<GroupReleaseProgrammerValue>>,
    pub dynamic_values: Arc<Vec<DynamicAddressValue>>,
    pub released_colors: Arc<ReleasedPreloadColors>,
}

#[derive(Clone, Debug)]
pub struct ProgrammerPreloadOutputSource {
    pub pending: Arc<ProgrammerPreloadPendingOutput>,
    pub active_values: Arc<Vec<TimedValue>>,
    pub active_groups: Arc<GroupProgrammerValues>,
    pub active_dynamics: Arc<Vec<DynamicAddressValue>>,
    pub active_group_releases: Arc<Vec<GroupReleaseProgrammerValue>>,
}

/// All Programmer inputs observed at one mutation boundary. `identity` and generations let a
/// downstream retained frame tell an edit from a repeated read without inspecting the values.
#[derive(Clone, Debug)]
pub struct ProgrammerOutputSourceCapture {
    pub identity: Option<ProgrammerId>,
    pub normal_values_generation: u64,
    pub preload_values_generation: u64,
    pub preload_playback_queue_generation: u64,
    pub preload_playback_actions: Arc<Vec<PreloadPlaybackAction>>,
    pub priority: Option<i16>,
    pub output_states: Vec<ProgrammerOutputState>,
    pub normal_dynamics: Vec<ActiveDynamicSessionSource>,
    pub preload: Option<ProgrammerPreloadOutputSource>,
}

#[derive(Clone)]
pub(crate) struct PendingOutputCache {
    pub(crate) identity: ProgrammerId,
    pub(crate) generation: u64,
    pub(crate) priority: i16,
    pub(crate) value: Arc<ProgrammerPreloadPendingOutput>,
    pub(crate) playback_queue_generation: u64,
    pub(crate) playback_actions: Arc<Vec<PreloadPlaybackAction>>,
}
