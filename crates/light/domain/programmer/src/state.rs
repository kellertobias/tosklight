use crate::GroupReleaseProgrammerValue;
use crate::groups::GroupProgrammerValues;
use crate::preload::PreloadPlaybackAction;
use crate::selection::SelectionExpression;
use chrono::{DateTime, Utc};
use light_core::{
    AttributeKey, AttributeValue, FixtureId, ProgrammerId, SessionId, TimedValue, UserId,
};
use light_dynamics::DynamicAddressValue;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ProgrammerValueTiming {
    pub(crate) fade: bool,
    pub(crate) fade_millis: Option<u64>,
    pub(crate) delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProgrammerSnapshot {
    pub selected: Vec<FixtureId>,
    pub selection_expression: Option<SelectionExpression>,
    /// Shared with the live state, so taking an Undo checkpoint costs a reference count rather
    /// than a copy of everything the operator has programmed.
    pub values: Arc<Vec<TimedValue>>,
    pub dynamic_values: Arc<Vec<DynamicAddressValue>>,
    pub group_values: Arc<GroupProgrammerValues>,
    pub group_release_values: Vec<GroupReleaseProgrammerValue>,
    pub preload_pending: Vec<TimedValue>,
    pub preload_active: Arc<Vec<TimedValue>>,
    pub preload_dynamic_pending: Arc<Vec<DynamicAddressValue>>,
    pub preload_dynamic_active: Arc<Vec<DynamicAddressValue>>,
    pub preload_group_pending: GroupProgrammerValues,
    pub preload_group_active: Arc<GroupProgrammerValues>,
    pub preload_group_release_pending: Vec<GroupReleaseProgrammerValue>,
    pub preload_group_release_active: Vec<GroupReleaseProgrammerValue>,
    pub preload_playback_pending: Vec<PreloadPlaybackAction>,
    pub command_line: String,
    pub blind: bool,
    pub preload_capture_programmer: bool,
    pub preview: bool,
    pub active_context: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProgrammerState {
    pub id: ProgrammerId,
    pub session_id: SessionId,
    pub user_id: UserId,
    pub priority: i16,
    pub selected: Vec<FixtureId>,
    #[serde(default)]
    pub selection_expression: Option<SelectionExpression>,
    /// Shared rather than owned: the render reads these every frame and must not pay for a copy of
    /// the operator's programming to do it. A write clones once, through `Arc::make_mut`, and only
    /// while a reader still holds the previous contents.
    pub values: Arc<Vec<TimedValue>>,
    #[serde(default)]
    pub dynamic_values: Arc<Vec<DynamicAddressValue>>,
    /// Runtime-only fixture-control overrides. These sit above normal programmer values while a
    /// momentary or timed action is active, but are never recorded, persisted, or added to Undo.
    #[serde(skip)]
    pub transient_values: Arc<Vec<TransientProgrammerAction>>,
    #[serde(default)]
    pub group_values: Arc<GroupProgrammerValues>,
    #[serde(default)]
    pub group_release_values: Vec<GroupReleaseProgrammerValue>,
    #[serde(default)]
    pub preload_pending: Vec<TimedValue>,
    #[serde(default)]
    pub preload_active: Arc<Vec<TimedValue>>,
    #[serde(default)]
    pub preload_dynamic_pending: Arc<Vec<DynamicAddressValue>>,
    #[serde(default)]
    pub preload_dynamic_active: Arc<Vec<DynamicAddressValue>>,
    #[serde(default)]
    pub preload_group_pending: GroupProgrammerValues,
    #[serde(default)]
    pub preload_group_active: Arc<GroupProgrammerValues>,
    #[serde(default)]
    pub preload_group_release_pending: Vec<GroupReleaseProgrammerValue>,
    #[serde(default)]
    pub preload_group_release_active: Vec<GroupReleaseProgrammerValue>,
    #[serde(default)]
    pub preload_playback_pending: Vec<PreloadPlaybackAction>,
    /// True while a Preload GO has committed queued Playback activations that hold-to-release has
    /// not released yet. Outside undo history: the activations themselves are not undoable.
    #[serde(default)]
    pub preload_playback_active: bool,
    pub connected: bool,
    pub last_activity: DateTime<Utc>,
    #[serde(default)]
    pub command_line: String,
    #[serde(default)]
    pub blind: bool,
    #[serde(default = "default_true")]
    pub preload_capture_programmer: bool,
    #[serde(default)]
    pub preview: bool,
    /// Legacy compatibility field. Live Highlight is owned by the server's transient output
    /// registry and is never serialized, restored, recorded, or included in undo history.
    #[serde(skip)]
    pub highlight: bool,
    #[serde(default)]
    pub active_context: Option<String>,
    /// Undo is live desk interaction history, not durable Programmer content. Persisting every
    /// snapshot duplicates large Dynamic payloads and puts whole-history JSON serialization on
    /// every operator action. Older persisted sessions may still contain this field and remain
    /// readable, but new session writes deliberately omit it.
    #[serde(default, skip_serializing)]
    pub undo: Vec<Arc<ProgrammerSnapshot>>,
    /// Redo has the same live-session lifetime as Undo.
    #[serde(default, skip_serializing)]
    pub redo: Vec<Arc<ProgrammerSnapshot>>,
    /// Runtime-only identity for coalescing samples from one continuous encoder gesture.
    #[serde(skip)]
    pub active_value_undo_group: Option<String>,
}

/// Minimal owned Programmer projection consumed by the output renderer.
///
/// Keeping this separate from `ProgrammerState` prevents a render tick from cloning selection,
/// command, pending Preload, and up to 100 Undo snapshots while it holds the registry read lock.
#[derive(Clone, Debug)]
pub struct ProgrammerOutputState {
    pub id: ProgrammerId,
    pub priority: i16,
    pub values: Arc<Vec<TimedValue>>,
    pub transient_values: Arc<Vec<TransientProgrammerAction>>,
    pub group_values: Arc<GroupProgrammerValues>,
    pub preload_active: Arc<Vec<TimedValue>>,
    pub preload_group_active: Arc<GroupProgrammerValues>,
}

#[derive(Clone, Debug)]
pub struct TransientProgrammerAction {
    pub source: String,
    pub generation: u64,
    pub values: Vec<TimedValue>,
}

impl ProgrammerState {
    // @tour recording-and-live-references:10 Capture only recordable Programmer content
    // Record and Update consume operator-authored values while excluding Highlight, transient
    // output, defaults, Preload buffers, and resolved stage output.

    /// Capture only the operator-authored content that Update and Record-style storage workflows
    /// may consume. This deliberately excludes resolved output, Highlight, defaults, and Preload
    /// buffers. The returned value is owned, so planning an Update never clears or otherwise
    /// mutates the live programmer.
    pub fn update_content(&self) -> ProgrammerUpdateContent {
        self.update_content_with_selection(&self.selected)
    }

    pub(crate) fn update_content_with_selection(
        &self,
        selected_fixtures: &[FixtureId],
    ) -> ProgrammerUpdateContent {
        let (fixture_values, group_values) = self.update_projection_values();
        ProgrammerUpdateContent {
            fixture_values,
            group_values,
            dynamic_values: self.dynamic_values.to_vec(),
            selected_fixtures: selected_fixtures.to_vec(),
        }
    }

    /// Value-event diff input sharing the immutable Dynamic layer across ordinary commands.
    pub fn update_projection_content(
        &self,
    ) -> (
        Vec<ProgrammerFixtureUpdate>,
        Vec<ProgrammerGroupUpdate>,
        Arc<Vec<DynamicAddressValue>>,
    ) {
        let (fixture_values, group_values) = self.update_projection_values();
        (
            fixture_values,
            group_values,
            Arc::clone(&self.dynamic_values),
        )
    }

    fn update_projection_values(
        &self,
    ) -> (Vec<ProgrammerFixtureUpdate>, Vec<ProgrammerGroupUpdate>) {
        let mut fixture_values = self
            .values
            .iter()
            .map(|value| ProgrammerFixtureUpdate {
                fixture_id: value.fixture_id,
                attribute: value.attribute.clone(),
                value: value.value.clone(),
                programmer_order: value.programmer_order,
                fade: value.fade,
                fade_millis: value.fade_millis,
                delay_millis: value.delay_millis,
            })
            .collect::<Vec<_>>();
        fixture_values.sort_by(|left, right| {
            left.programmer_order
                .cmp(&right.programmer_order)
                .then_with(|| left.fixture_id.0.cmp(&right.fixture_id.0))
                .then_with(|| left.attribute.cmp(&right.attribute))
        });

        let mut group_values = self
            .group_values
            .iter()
            .flat_map(|(group_id, attributes)| {
                attributes
                    .iter()
                    .map(move |(attribute, value)| ProgrammerGroupUpdate {
                        group_id: group_id.clone(),
                        attribute: attribute.clone(),
                        value: value.value.clone(),
                        programmer_order: value.programmer_order,
                        fade: value.fade,
                        fade_millis: value.fade_millis,
                        delay_millis: value.delay_millis,
                    })
            })
            .collect::<Vec<_>>();
        group_values.sort_by(|left, right| {
            left.programmer_order
                .cmp(&right.programmer_order)
                .then_with(|| left.group_id.cmp(&right.group_id))
                .then_with(|| left.attribute.cmp(&right.attribute))
        });

        (fixture_values, group_values)
    }
}

/// One exact fixture/attribute value authored in the normal programmer.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProgrammerFixtureUpdate {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    pub programmer_order: u64,
    #[serde(default)]
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_millis: Option<u64>,
}

/// One exact Group/attribute value authored in the normal programmer.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProgrammerGroupUpdate {
    pub group_id: String,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    pub programmer_order: u64,
    #[serde(default)]
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_millis: Option<u64>,
}

/// Stable, owned Update input. Fixture and Group values are kept separate because their exact
/// stored addresses and tracking sources are different. Selection is included solely for Group
/// membership updates.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ProgrammerUpdateContent {
    pub fixture_values: Vec<ProgrammerFixtureUpdate>,
    pub group_values: Vec<ProgrammerGroupUpdate>,
    pub dynamic_values: Vec<DynamicAddressValue>,
    pub selected_fixtures: Vec<FixtureId>,
}

impl ProgrammerUpdateContent {
    pub fn has_values(&self) -> bool {
        !self.fixture_values.is_empty()
            || !self.group_values.is_empty()
            || !self.dynamic_values.is_empty()
    }

    pub fn has_selection(&self) -> bool {
        !self.selected_fixtures.is_empty()
    }
}
