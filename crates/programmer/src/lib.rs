#![forbid(unsafe_code)]
//! User-scoped selection and programmer state, shared by all of a user's sessions.

use chrono::{DateTime, Utc};
use light_core::{
    AttributeKey, AttributeValue, FixtureId, ProgrammerId, SessionId, SharedClock, SystemClock,
    TimedValue, UserId,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const HISTORY_LIMIT: usize = 100;
fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize)]
pub struct GroupProgrammerValue {
    pub value: AttributeValue,
    pub changed_at: DateTime<Utc>,
    #[serde(default)]
    pub programmer_order: u64,
    #[serde(default)]
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_millis: Option<u64>,
}
impl<'de> Deserialize<'de> for GroupProgrammerValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Scoped {
                value: AttributeValue,
                changed_at: DateTime<Utc>,
                #[serde(default)]
                programmer_order: u64,
                #[serde(default)]
                fade: bool,
                #[serde(default)]
                fade_millis: Option<u64>,
                #[serde(default)]
                delay_millis: Option<u64>,
            },
            Legacy(AttributeValue),
        }
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Scoped {
                value,
                changed_at,
                programmer_order,
                fade,
                fade_millis,
                delay_millis,
            } => Self {
                value,
                changed_at,
                programmer_order,
                fade,
                fade_millis,
                delay_millis,
            },
            Repr::Legacy(value) => Self {
                value,
                changed_at: Utc::now(),
                programmer_order: 0,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            },
        })
    }
}
type GroupProgrammerValues = HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;

#[derive(Clone, Copy)]
struct ProgrammerValueTiming {
    fade: bool,
    fade_millis: Option<u64>,
    delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PreloadPlaybackAction {
    pub playback_number: u16,
    pub action: String,
    pub surface: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProgrammerSnapshot {
    pub selected: Vec<FixtureId>,
    pub selection_expression: Option<SelectionExpression>,
    pub values: Vec<TimedValue>,
    pub group_values: GroupProgrammerValues,
    pub preload_pending: Vec<TimedValue>,
    pub preload_active: Vec<TimedValue>,
    pub preload_group_pending: GroupProgrammerValues,
    pub preload_group_active: GroupProgrammerValues,
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
    pub values: Vec<TimedValue>,
    #[serde(default)]
    pub group_values: GroupProgrammerValues,
    #[serde(default)]
    pub preload_pending: Vec<TimedValue>,
    #[serde(default)]
    pub preload_active: Vec<TimedValue>,
    #[serde(default)]
    pub preload_group_pending: GroupProgrammerValues,
    #[serde(default)]
    pub preload_group_active: GroupProgrammerValues,
    #[serde(default)]
    pub preload_playback_pending: Vec<PreloadPlaybackAction>,
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
    #[serde(default)]
    pub undo: Vec<ProgrammerSnapshot>,
    #[serde(default)]
    pub redo: Vec<ProgrammerSnapshot>,
}

impl ProgrammerState {
    /// Capture only the operator-authored content that Update and Record-style storage workflows
    /// may consume. This deliberately excludes resolved output, Highlight, defaults, and Preload
    /// buffers. The returned value is owned, so planning an Update never clears or otherwise
    /// mutates the live programmer.
    pub fn update_content(&self) -> ProgrammerUpdateContent {
        let mut fixture_values = self
            .values
            .iter()
            .map(|value| ProgrammerFixtureUpdate {
                fixture_id: value.fixture_id,
                attribute: value.attribute.clone(),
                value: value.value.clone(),
                programmer_order: value.programmer_order,
                fade_millis: value.fade_millis,
                delay_millis: value.delay_millis,
            })
            .collect::<Vec<_>>();
        fixture_values.sort_by_key(|value| value.programmer_order);

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

        ProgrammerUpdateContent {
            fixture_values,
            group_values,
            selected_fixtures: self.selected.clone(),
        }
    }

    fn snapshot(&self) -> ProgrammerSnapshot {
        ProgrammerSnapshot {
            selected: self.selected.clone(),
            selection_expression: self.selection_expression.clone(),
            values: self.values.clone(),
            group_values: self.group_values.clone(),
            preload_pending: self.preload_pending.clone(),
            preload_active: self.preload_active.clone(),
            preload_group_pending: self.preload_group_pending.clone(),
            preload_group_active: self.preload_group_active.clone(),
            preload_playback_pending: self.preload_playback_pending.clone(),
            command_line: self.command_line.clone(),
            blind: self.blind,
            preload_capture_programmer: self.preload_capture_programmer,
            preview: self.preview,
            active_context: self.active_context.clone(),
        }
    }

    fn restore_snapshot(&mut self, snapshot: ProgrammerSnapshot, now: DateTime<Utc>) {
        self.selected = snapshot.selected;
        self.selection_expression = snapshot.selection_expression;
        self.values = snapshot.values;
        self.group_values = snapshot.group_values;
        self.preload_pending = snapshot.preload_pending;
        self.preload_active = snapshot.preload_active;
        self.preload_group_pending = snapshot.preload_group_pending;
        self.preload_group_active = snapshot.preload_group_active;
        self.preload_playback_pending = snapshot.preload_playback_pending;
        self.command_line = snapshot.command_line;
        self.blind = snapshot.blind;
        self.preload_capture_programmer = snapshot.preload_capture_programmer;
        self.preview = snapshot.preview;
        self.highlight = false;
        self.active_context = snapshot.active_context;
        self.last_activity = now;
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
}

/// One exact fixture/attribute value authored in the normal programmer.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProgrammerFixtureUpdate {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    pub programmer_order: u64,
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
    pub selected_fixtures: Vec<FixtureId>,
}

impl ProgrammerUpdateContent {
    pub fn has_values(&self) -> bool {
        !self.fixture_values.is_empty() || !self.group_values.is_empty()
    }

    pub fn has_selection(&self) -> bool {
        !self.selected_fixtures.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectionRule {
    All,
    Odd,
    Even,
    EveryNth { n: usize, offset: usize },
}
impl SelectionRule {
    pub fn validate(&self) -> Result<(), String> {
        if matches!(self, Self::EveryNth { n: 0, .. }) {
            Err("every-Nth selection requires N to be at least 1".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectionExpression {
    Static,
    LiveGroup {
        group_id: String,
        rule: SelectionRule,
    },
    FrozenGroup {
        group_id: String,
        source_revision: u64,
    },
    PlaybackContents {
        items: Vec<SelectionReference>,
    },
    /// Ordered operator sources from a mixed command or consecutive surface gestures. References
    /// remain live and add/remove operations are replayed left-to-right whenever Groups change.
    Sources {
        items: Vec<SelectionReference>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectionReference {
    Fixture { fixture_id: FixtureId },
    LiveGroup { group_id: String },
    RemoveFixture { fixture_id: FixtureId },
    RemoveLiveGroup { group_id: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct GroupDefinition {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub fixtures: Vec<FixtureId>,
    pub derived_from: Option<DerivedGroup>,
    pub frozen_from: Option<FrozenGroup>,
    pub programming: HashMap<AttributeKey, AttributeValue>,
    pub master: f32,
    pub playback_fader: Option<u8>,
}

impl Default for GroupDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            color: None,
            icon: None,
            fixtures: vec![],
            derived_from: None,
            frozen_from: None,
            programming: HashMap::new(),
            master: 1.0,
            playback_fader: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DerivedGroup {
    pub source_group_id: String,
    pub rule: SelectionRule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FrozenGroup {
    pub source_group_id: String,
    pub source_revision: u64,
    pub captured_at: chrono::DateTime<Utc>,
}

pub fn apply_selection_rule(fixtures: &[FixtureId], rule: &SelectionRule) -> Vec<FixtureId> {
    fixtures
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, fixture)| {
            let one_based = index + 1;
            let selected = match rule {
                SelectionRule::All => true,
                SelectionRule::Odd => one_based % 2 == 1,
                SelectionRule::Even => one_based % 2 == 0,
                SelectionRule::EveryNth { n, offset } => {
                    *n > 0 && index >= *offset && (index - *offset) % *n == 0
                }
            };
            selected.then_some(fixture)
        })
        .collect()
}

pub fn resolve_group(
    id: &str,
    groups: &HashMap<String, GroupDefinition>,
) -> Result<Vec<FixtureId>, String> {
    fn visit(
        id: &str,
        groups: &HashMap<String, GroupDefinition>,
        visiting: &mut HashSet<String>,
    ) -> Result<Vec<FixtureId>, String> {
        if !visiting.insert(id.to_owned()) {
            return Err(format!("derived group cycle detected at {id}"));
        }
        let group = groups
            .get(id)
            .ok_or_else(|| format!("group {id} does not exist"))?;
        let resolved = if let Some(derived) = &group.derived_from {
            apply_selection_rule(
                &visit(&derived.source_group_id, groups, visiting)?,
                &derived.rule,
            )
        } else {
            group.fixtures.clone()
        };
        visiting.remove(id);
        Ok(resolved)
    }
    visit(id, groups, &mut HashSet::new())
}

pub fn resolve_selection_references(
    items: &[SelectionReference],
    groups: &HashMap<String, GroupDefinition>,
) -> Vec<FixtureId> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        match item {
            SelectionReference::Fixture { fixture_id } => {
                if seen.insert(*fixture_id) {
                    selected.push(*fixture_id);
                }
            }
            SelectionReference::LiveGroup { group_id } => {
                if let Ok(fixtures) = resolve_group(group_id, groups) {
                    for fixture_id in fixtures {
                        if seen.insert(fixture_id) {
                            selected.push(fixture_id);
                        }
                    }
                }
            }
            SelectionReference::RemoveFixture { fixture_id } => {
                selected.retain(|candidate| candidate != fixture_id);
                seen.remove(fixture_id);
            }
            SelectionReference::RemoveLiveGroup { group_id } => {
                if let Ok(fixtures) = resolve_group(group_id, groups) {
                    for fixture_id in fixtures {
                        selected.retain(|candidate| *candidate != fixture_id);
                        seen.remove(&fixture_id);
                    }
                }
            }
        }
    }
    selected
}

/// Apply the desk's ordered Group Merge rule: retain the existing membership exactly, then append
/// each previously absent incoming fixture in operator selection order. Duplicate incoming
/// fixtures do not reorder or duplicate an existing member.
pub fn merge_ordered_group_membership(
    existing: &[FixtureId],
    incoming: &[FixtureId],
) -> Vec<FixtureId> {
    let mut merged = existing.to_vec();
    let mut seen = existing.iter().copied().collect::<HashSet<_>>();
    for fixture_id in incoming {
        if seen.insert(*fixture_id) {
            merged.push(*fixture_id);
        }
    }
    merged
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetStoreMode {
    Merge,
    Overwrite,
    AddMissingFixtures,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Preset {
    pub name: String,
    pub values: HashMap<FixtureId, HashMap<AttributeKey, AttributeValue>>,
    pub group_values: HashMap<String, HashMap<AttributeKey, AttributeValue>>,
}

impl Preset {
    pub fn store(&mut self, incoming: Preset, mode: PresetStoreMode) {
        if !incoming.name.is_empty() {
            self.name = incoming.name;
        }
        match mode {
            PresetStoreMode::Overwrite => {
                self.values = incoming.values;
                self.group_values = incoming.group_values;
            }
            PresetStoreMode::Merge => {
                for (fixture, attributes) in incoming.values {
                    self.values.entry(fixture).or_default().extend(attributes);
                }
                for (group, attributes) in incoming.group_values {
                    self.group_values
                        .entry(group)
                        .or_default()
                        .extend(attributes);
                }
            }
            PresetStoreMode::AddMissingFixtures => {
                for (fixture, attributes) in incoming.values {
                    self.values.entry(fixture).or_insert(attributes);
                }
                for (group, attributes) in incoming.group_values {
                    self.group_values.entry(group).or_insert(attributes);
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SelectionContext {
    selected: Vec<FixtureId>,
    expression: Option<SelectionExpression>,
    /// True only while consecutive ordinary surface selections are being accumulated. A value
    /// entry or an explicit selection/clear operation closes the gesture.
    gesture_open: bool,
}

#[derive(Clone)]
pub struct ProgrammerRegistry {
    states: Arc<RwLock<HashMap<SessionId, ProgrammerState>>>,
    sessions: Arc<RwLock<HashMap<SessionId, SessionId>>>,
    command_contexts: Arc<RwLock<HashMap<SessionId, SessionId>>>,
    command_lines: Arc<RwLock<HashMap<SessionId, String>>>,
    command_targets: Arc<RwLock<HashMap<SessionId, String>>>,
    selection_contexts: Arc<RwLock<HashMap<SessionId, SelectionContext>>>,
    programmer_order: Arc<AtomicU64>,
    clock: SharedClock,
}
impl Default for ProgrammerRegistry {
    fn default() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }
}
impl ProgrammerRegistry {
    pub fn with_clock(clock: SharedClock) -> Self {
        Self {
            states: Arc::default(),
            sessions: Arc::default(),
            command_contexts: Arc::default(),
            command_lines: Arc::default(),
            command_targets: Arc::default(),
            selection_contexts: Arc::default(),
            programmer_order: Arc::default(),
            clock,
        }
    }

    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.clock)
    }

    pub fn set_priority(&self, session: SessionId, priority: i16) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.priority = priority;
        for value in state
            .values
            .iter_mut()
            .chain(&mut state.preload_pending)
            .chain(&mut state.preload_active)
        {
            value.priority = priority;
        }
        state.last_activity = self.clock.now();
        true
    }

    pub fn reset_all(&self) {
        self.states.write().clear();
        self.sessions.write().clear();
        self.command_contexts.write().clear();
        self.command_lines.write().clear();
        self.command_targets.write().clear();
        self.selection_contexts.write().clear();
        self.programmer_order.store(0, Ordering::Relaxed);
    }

    fn next_programmer_order(&self) -> u64 {
        self.programmer_order.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn start(&self, session_id: SessionId, user_id: UserId) -> ProgrammerState {
        let existing = self
            .states
            .read()
            .iter()
            .find_map(|(key, state)| (state.user_id == user_id).then_some(*key));
        if let Some(key) = existing {
            self.sessions.write().insert(session_id, key);
            self.command_contexts
                .write()
                .entry(session_id)
                .or_insert(session_id);
            let command_context = self.command_context(session_id);
            self.command_lines
                .write()
                .entry(command_context)
                .or_default();
            self.command_targets
                .write()
                .entry(command_context)
                .or_insert_with(|| "FIXTURE".into());
            self.selection_contexts
                .write()
                .entry(command_context)
                .or_default();
            let mut states = self.states.write();
            let state = states.get_mut(&key).expect("programmer disappeared");
            state.connected = true;
            state.last_activity = self.clock.now();
            let mut projected = state.clone();
            projected.session_id = session_id;
            projected.command_line = self
                .command_lines
                .read()
                .get(&command_context)
                .cloned()
                .unwrap_or_default();
            self.project_selection(&mut projected, command_context);
            return projected;
        }
        self.sessions.write().insert(session_id, session_id);
        let state = ProgrammerState {
            id: ProgrammerId::new(),
            session_id,
            user_id,
            priority: 100,
            selected: vec![],
            selection_expression: None,
            values: vec![],
            group_values: HashMap::new(),
            preload_pending: vec![],
            preload_active: vec![],
            preload_group_pending: HashMap::new(),
            preload_group_active: HashMap::new(),
            preload_playback_pending: vec![],
            connected: true,
            last_activity: self.clock.now(),
            command_line: String::new(),
            blind: false,
            preload_capture_programmer: true,
            preview: false,
            highlight: false,
            active_context: None,
            undo: vec![],
            redo: vec![],
        };
        self.states.write().insert(session_id, state.clone());
        self.command_contexts
            .write()
            .entry(session_id)
            .or_insert(session_id);
        let command_context = self.command_context(session_id);
        self.command_lines
            .write()
            .entry(command_context)
            .or_default();
        self.command_targets
            .write()
            .entry(command_context)
            .or_insert_with(|| "FIXTURE".into());
        self.selection_contexts
            .write()
            .entry(command_context)
            .or_default();
        state
    }
    pub fn restore(&self, state: ProgrammerState) {
        let restored_order = state
            .values
            .iter()
            .chain(&state.preload_pending)
            .chain(&state.preload_active)
            .map(|value| value.programmer_order)
            .chain(
                state
                    .group_values
                    .values()
                    .chain(state.preload_group_pending.values())
                    .chain(state.preload_group_active.values())
                    .flat_map(|attributes| attributes.values().map(|value| value.programmer_order)),
            )
            .max()
            .unwrap_or(0);
        self.programmer_order
            .fetch_max(restored_order, Ordering::Relaxed);
        let session_id = state.session_id;
        self.selection_contexts.write().insert(
            session_id,
            SelectionContext {
                selected: state.selected.clone(),
                expression: state.selection_expression.clone(),
                gesture_open: false,
            },
        );
        self.command_contexts
            .write()
            .entry(session_id)
            .or_insert(session_id);
        self.command_lines
            .write()
            .insert(session_id, state.command_line.clone());
        self.command_targets.write().insert(
            session_id,
            if state.command_line.trim().eq_ignore_ascii_case("GROUP") {
                "GROUP".into()
            } else {
                "FIXTURE".into()
            },
        );
        let existing = self
            .states
            .read()
            .iter()
            .find_map(|(key, current)| (current.user_id == state.user_id).then_some(*key));
        if let Some(existing) = existing {
            self.sessions.write().insert(session_id, existing);
            let mut shared = state;
            shared.session_id = existing;
            shared.command_line.clear();
            self.states.write().insert(existing, shared);
        } else {
            self.sessions.write().insert(session_id, session_id);
            let mut shared = state;
            shared.command_line.clear();
            self.states.write().insert(session_id, shared);
        }
    }
    fn key(&self, session: SessionId) -> SessionId {
        self.sessions
            .read()
            .get(&session)
            .copied()
            .unwrap_or(session)
    }
    fn command_context(&self, session: SessionId) -> SessionId {
        self.command_contexts
            .read()
            .get(&session)
            .copied()
            .unwrap_or(session)
    }

    fn project_selection(&self, state: &mut ProgrammerState, context: SessionId) {
        let selections = self.selection_contexts.read();
        let selection = selections.get(&context);
        state.selected = selection
            .map(|selection| selection.selected.clone())
            .unwrap_or_default();
        state.selection_expression = selection.and_then(|selection| selection.expression.clone());
    }

    fn close_selection_gesture(&self, session: SessionId) {
        if let Some(selection) = self
            .selection_contexts
            .write()
            .get_mut(&self.command_context(session))
        {
            selection.gesture_open = false;
        }
    }

    /// Finish the current desk-local sequence of ordinary selection presses without clearing its
    /// visible selection. Recording a target uses this boundary so the next fixture or Group press
    /// starts a fresh selection while the just-recorded source remains inspectable.
    pub fn finish_selection_gesture(&self, session: SessionId) {
        self.close_selection_gesture(session);
    }

    /// Bind a controller session to the command interaction context for its desk.
    /// Programmer values remain shared by user identity, while button presses,
    /// partial command lines, selection gestures, and the active command target are shared only by
    /// sessions attached to this same context.
    pub fn attach_command_context(&self, session: SessionId, context: SessionId) -> bool {
        if !self.sessions.read().contains_key(&session) {
            return false;
        }
        let previous = self.command_context(session);
        if previous == context {
            return true;
        }

        let previous_line = self
            .command_lines
            .read()
            .get(&previous)
            .cloned()
            .unwrap_or_default();
        let previous_target = self
            .command_targets
            .read()
            .get(&previous)
            .cloned()
            .unwrap_or_else(|| "FIXTURE".into());
        let previous_selection = self
            .selection_contexts
            .read()
            .get(&previous)
            .cloned()
            .unwrap_or_default();
        let promote_previous = self
            .command_lines
            .read()
            .get(&context)
            .is_none_or(|current| current.is_empty() && !previous_line.is_empty());

        self.command_contexts.write().insert(session, context);
        {
            let mut command_lines = self.command_lines.write();
            if promote_previous {
                command_lines.insert(context, previous_line);
            } else {
                command_lines.entry(context).or_default();
            }
        }
        {
            let mut command_targets = self.command_targets.write();
            if promote_previous {
                command_targets.insert(context, previous_target);
            } else {
                command_targets
                    .entry(context)
                    .or_insert_with(|| "FIXTURE".into());
            }
        }
        {
            let mut selection_contexts = self.selection_contexts.write();
            selection_contexts
                .entry(context)
                .or_insert(previous_selection);
        }

        if previous == session
            && !self
                .command_contexts
                .read()
                .values()
                .any(|candidate| *candidate == previous)
        {
            self.command_lines.write().remove(&previous);
            self.command_targets.write().remove(&previous);
            self.selection_contexts.write().remove(&previous);
        }
        true
    }
    pub fn select(&self, session: SessionId, fixtures: impl IntoIterator<Item = FixtureId>) {
        let mut seen = HashSet::new();
        let selected = fixtures
            .into_iter()
            .filter(|fixture| seen.insert(*fixture))
            .collect::<Vec<_>>();
        let expression = Some(SelectionExpression::Static);
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            // Keep a serializable projection for legacy persistence. Reads are projected from the
            // desk-local selection context below.
            state.selected = selected.clone();
            state.selection_expression = expression.clone();
            state.last_activity = self.clock.now();
        }
        self.selection_contexts.write().insert(
            self.command_context(session),
            SelectionContext {
                selected,
                expression,
                gesture_open: false,
            },
        );
    }
    pub fn select_expression(
        &self,
        session: SessionId,
        fixtures: Vec<FixtureId>,
        expression: SelectionExpression,
    ) {
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selected = fixtures.clone();
            state.selection_expression = Some(expression.clone());
            state.last_activity = self.clock.now();
        }
        self.selection_contexts.write().insert(
            self.command_context(session),
            SelectionContext {
                selected: fixtures,
                expression: Some(expression),
                gesture_open: false,
            },
        );
    }

    /// Apply one ordinary UI selection gesture. Consecutive calls on the same desk accumulate;
    /// selection on another desk is independent even when both sessions share programmer values.
    pub fn apply_selection_gesture(
        &self,
        session: SessionId,
        references: Vec<SelectionReference>,
        groups: &HashMap<String, GroupDefinition>,
    ) -> bool {
        if !self.sessions.read().contains_key(&session) {
            return false;
        }
        let context = self.command_context(session);
        let (selected, expression) = {
            let mut selections = self.selection_contexts.write();
            let selection = selections.entry(context).or_default();
            let mut items = if selection.gesture_open {
                match selection.expression.clone() {
                    Some(SelectionExpression::Sources { items }) => items,
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            items.extend(references);
            let selected = resolve_selection_references(&items, groups);
            let expression = SelectionExpression::Sources { items };
            selection.selected = selected.clone();
            selection.expression = Some(expression.clone());
            selection.gesture_open = true;
            (selected, expression)
        };
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selected = selected;
            state.selection_expression = Some(expression);
            state.last_activity = self.clock.now();
        }
        true
    }
    pub fn set(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
    ) {
        self.set_with_fade(session, fixture_id, attribute, value, false);
    }

    /// Apply a fixture-level macro as one Programmer mutation. All values share one undo
    /// checkpoint and become visible to the renderer together, so a multi-channel control action
    /// can never be observed half-applied.
    pub fn set_many(
        &self,
        session: SessionId,
        assignments: impl IntoIterator<Item = (FixtureId, AttributeKey, AttributeValue)>,
    ) {
        self.set_many_with_checkpoint(session, assignments, true);
    }

    /// Complete a momentary/timed action without creating a second Undo point. The active edge
    /// already captured the pre-action state; Undo after the inactive edge therefore returns to
    /// that state instead of unexpectedly firing the action again.
    pub fn set_many_transient(
        &self,
        session: SessionId,
        assignments: impl IntoIterator<Item = (FixtureId, AttributeKey, AttributeValue)>,
    ) {
        self.set_many_with_checkpoint(session, assignments, false);
    }

    fn set_many_with_checkpoint(
        &self,
        session: SessionId,
        assignments: impl IntoIterator<Item = (FixtureId, AttributeKey, AttributeValue)>,
        checkpoint: bool,
    ) {
        let assignments = assignments.into_iter().collect::<Vec<_>>();
        if assignments.is_empty() {
            return;
        }
        self.close_selection_gesture(session);
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            if checkpoint {
                state.checkpoint();
            }
            let values = if state.blind && state.preload_capture_programmer {
                &mut state.preload_pending
            } else {
                &mut state.values
            };
            let changed_at = self.clock.now();
            for (fixture_id, attribute, value) in assignments {
                values.retain(|existing| {
                    existing.fixture_id != fixture_id || existing.attribute != attribute
                });
                values.push(TimedValue {
                    fixture_id,
                    attribute,
                    value,
                    priority: state.priority,
                    changed_at,
                    programmer_order: self.next_programmer_order(),
                    merge_mode: light_core::MergeMode::Ltp,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                });
            }
            state.last_activity = changed_at;
        }
    }
    pub fn set_faded(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
    ) {
        self.set_with_fade(session, fixture_id, attribute, value, true);
    }
    pub fn set_faded_with_timing(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) {
        self.set_with_timing(
            session,
            fixture_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade: true,
                fade_millis,
                delay_millis,
            },
        );
    }
    fn set_with_fade(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        fade: bool,
    ) {
        self.set_with_timing(
            session,
            fixture_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade,
                fade_millis: None,
                delay_millis: None,
            },
        );
    }
    fn set_with_timing(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammerValueTiming,
    ) {
        self.close_selection_gesture(session);
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            let merge_mode = light_core::MergeMode::Ltp;
            let values = if state.blind && state.preload_capture_programmer {
                &mut state.preload_pending
            } else {
                &mut state.values
            };
            values.retain(|v| !(v.fixture_id == fixture_id && v.attribute == attribute));
            values.push(TimedValue {
                fixture_id,
                attribute,
                value,
                priority: state.priority,
                changed_at: self.clock.now(),
                programmer_order: self.next_programmer_order(),
                merge_mode,
                fade: timing.fade,
                fade_millis: timing.fade_millis,
                delay_millis: timing.delay_millis,
            });
            state.last_activity = self.clock.now();
        }
    }
    pub fn set_group(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        self.set_group_with_fade(session, group_id, attribute, value, false)
    }
    pub fn set_group_faded(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        self.set_group_with_fade(session, group_id, attribute, value, true)
    }
    pub fn set_group_faded_with_timing(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) -> bool {
        self.set_group_with_timing(
            session,
            group_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade: true,
                fade_millis,
                delay_millis,
            },
        )
    }
    fn set_group_with_fade(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        fade: bool,
    ) -> bool {
        self.set_group_with_timing(
            session,
            group_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade,
                fade_millis: None,
                delay_millis: None,
            },
        )
    }
    fn set_group_with_timing(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammerValueTiming,
    ) -> bool {
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        let programmer_order = self.next_programmer_order();
        let target = if state.blind && state.preload_capture_programmer {
            &mut state.preload_group_pending
        } else {
            &mut state.group_values
        };
        target.entry(group_id).or_default().insert(
            attribute,
            GroupProgrammerValue {
                value,
                changed_at: self.clock.now(),
                programmer_order,
                fade: timing.fade,
                fade_millis: timing.fade_millis,
                delay_millis: timing.delay_millis,
            },
        );
        state.last_activity = self.clock.now();
        true
    }

    /// Release exactly one fixture-scoped programmer attribute. Contributions at every other
    /// fixture, Group, and attribute remain intact so resolved output falls back naturally.
    pub fn release_fixture_attribute(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> bool {
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let values = if state.blind && state.preload_capture_programmer {
            &mut state.preload_pending
        } else {
            &mut state.values
        };
        let before = values.len();
        if values
            .iter()
            .all(|value| value.fixture_id != fixture_id || value.attribute != *attribute)
        {
            return false;
        }
        state.checkpoint();
        let values = if state.blind && state.preload_capture_programmer {
            &mut state.preload_pending
        } else {
            &mut state.values
        };
        values.retain(|value| value.fixture_id != fixture_id || value.attribute != *attribute);
        debug_assert!(values.len() < before);
        state.last_activity = self.clock.now();
        true
    }

    /// Release exactly one Group-scoped programmer attribute. The Group entry itself is removed
    /// only when it has no remaining attributes.
    pub fn release_group_attribute(
        &self,
        session: SessionId,
        group_id: &str,
        attribute: &AttributeKey,
    ) -> bool {
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let target = if state.blind && state.preload_capture_programmer {
            &mut state.preload_group_pending
        } else {
            &mut state.group_values
        };
        if !target
            .get(group_id)
            .is_some_and(|attributes| attributes.contains_key(attribute))
        {
            return false;
        }
        state.checkpoint();
        let target = if state.blind && state.preload_capture_programmer {
            &mut state.preload_group_pending
        } else {
            &mut state.group_values
        };
        if let Some(attributes) = target.get_mut(group_id) {
            attributes.remove(attribute);
            if attributes.is_empty() {
                target.remove(group_id);
            }
        }
        state.last_activity = self.clock.now();
        true
    }
    pub fn activate_preload(&self, session: SessionId) -> bool {
        self.activate_preload_at(session, self.clock.now())
    }

    /// Publishes every pending programmer value at the one application timestamp owned by
    /// Preload GO. Values deliberately keep their explicit fade/delay metadata; only their
    /// transition origin moves from the blind-edit time to the commit time.
    pub fn activate_preload_at(&self, session: SessionId, committed_at: DateTime<Utc>) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        for mut incoming in std::mem::take(&mut state.preload_pending) {
            incoming.changed_at = committed_at;
            state.preload_active.retain(|value| {
                !(value.fixture_id == incoming.fixture_id && value.attribute == incoming.attribute)
            });
            state.preload_active.push(incoming);
        }
        for (group, mut attributes) in std::mem::take(&mut state.preload_group_pending) {
            for value in attributes.values_mut() {
                value.changed_at = committed_at;
            }
            state
                .preload_group_active
                .entry(group)
                .or_default()
                .extend(attributes);
        }
        // GO publishes the prepared values, then returns input to the live
        // programmer. Entering preload again starts the next blind edit.
        state.blind = false;
        state.last_activity = committed_at;
        true
    }

    pub fn queue_preload_playback_action(
        &self,
        session: SessionId,
        playback_number: u16,
        action: String,
        surface: String,
    ) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        state.preload_playback_pending.push(PreloadPlaybackAction {
            playback_number,
            action,
            surface,
        });
        state.last_activity = self.clock.now();
        true
    }

    pub fn take_preload_playback_actions(&self, session: SessionId) -> Vec<PreloadPlaybackAction> {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return Vec::new();
        };
        std::mem::take(&mut state.preload_playback_pending)
    }
    pub fn clear_preload_pending(&self, session: SessionId) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        state.preload_pending.clear();
        state.preload_group_pending.clear();
        state.preload_playback_pending.clear();
        state.last_activity = self.clock.now();
        true
    }
    pub fn release_preload(&self, session: SessionId) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let changed = state.blind
            || !state.preload_pending.is_empty()
            || !state.preload_active.is_empty()
            || !state.preload_group_pending.is_empty()
            || !state.preload_group_active.is_empty()
            || !state.preload_playback_pending.is_empty();
        if !changed {
            return false;
        }
        state.checkpoint();
        state.preload_pending.clear();
        state.preload_active.clear();
        state.preload_group_pending.clear();
        state.preload_group_active.clear();
        state.preload_playback_pending.clear();
        state.blind = false;
        state.last_activity = self.clock.now();
        true
    }
    pub fn set_preload_group(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        let programmer_order = self.next_programmer_order();
        state
            .preload_group_pending
            .entry(group_id)
            .or_default()
            .insert(
                attribute,
                GroupProgrammerValue {
                    value,
                    changed_at: self.clock.now(),
                    programmer_order,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                },
            );
        state.last_activity = self.clock.now();
        true
    }
    pub fn set_command_line(&self, session: SessionId, command_line: String) -> bool {
        if !self.sessions.read().contains_key(&session) {
            return false;
        }
        self.command_lines
            .write()
            .insert(self.command_context(session), command_line);
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.last_activity = self.clock.now();
        }
        true
    }
    pub fn command_target(&self, session: SessionId) -> String {
        self.command_targets
            .read()
            .get(&self.command_context(session))
            .cloned()
            .unwrap_or_else(|| "FIXTURE".into())
    }
    pub fn set_command_target(&self, session: SessionId, target: String) -> bool {
        if !self.sessions.read().contains_key(&session)
            || !matches!(target.as_str(), "FIXTURE" | "GROUP")
        {
            return false;
        }
        self.command_targets
            .write()
            .insert(self.command_context(session), target);
        true
    }
    pub fn set_modes(
        &self,
        session: SessionId,
        blind: Option<bool>,
        preview: Option<bool>,
        highlight: Option<bool>,
        active_context: Option<Option<String>>,
    ) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        if let Some(value) = blind {
            state.blind = value;
        }
        if let Some(value) = preview {
            state.preview = value;
        }
        let _ = highlight;
        state.highlight = false;
        if let Some(value) = active_context {
            state.active_context = value;
        }
        state.last_activity = self.clock.now();
        true
    }

    pub fn arm_preload(&self, session: SessionId, capture_programmer: bool) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        state.blind = true;
        state.preload_capture_programmer = capture_programmer;
        state.last_activity = self.clock.now();
        true
    }
    pub fn clear_values(&self, session: SessionId) -> bool {
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        state.values.clear();
        state.group_values.clear();
        state.last_activity = self.clock.now();
        true
    }
    pub fn undo(&self, session: SessionId) -> bool {
        let (selected, expression) = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return false;
            };
            let Some(previous) = state.undo.pop() else {
                return false;
            };
            state.redo.push(state.snapshot());
            state.restore_snapshot(previous, self.clock.now());
            (state.selected.clone(), state.selection_expression.clone())
        };
        self.selection_contexts.write().insert(
            self.command_context(session),
            SelectionContext {
                selected,
                expression,
                gesture_open: false,
            },
        );
        true
    }
    pub fn redo(&self, session: SessionId) -> bool {
        let (selected, expression) = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return false;
            };
            let Some(next) = state.redo.pop() else {
                return false;
            };
            state.undo.push(state.snapshot());
            state.restore_snapshot(next, self.clock.now());
            (state.selected.clone(), state.selection_expression.clone())
        };
        self.selection_contexts.write().insert(
            self.command_context(session),
            SelectionContext {
                selected,
                expression,
                gesture_open: false,
            },
        );
        true
    }
    pub fn disconnect(&self, session: SessionId) {
        let key = self.key(session);
        self.sessions.write().remove(&session);
        let still_connected = self.sessions.read().values().any(|bound| *bound == key);
        if let Some(state) = self.states.write().get_mut(&key) {
            state.connected = still_connected;
        }
    }
    pub fn connect(&self, session: SessionId) {
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.connected = true;
            state.last_activity = self.clock.now();
        }
    }
    pub fn clear(&self, session: SessionId) -> bool {
        let key = self.key(session);
        self.sessions.write().retain(|_, bound| *bound != key);
        self.states.write().remove(&key).is_some()
    }
    pub fn active(&self) -> Vec<ProgrammerState> {
        self.states.read().values().cloned().collect()
    }
    pub fn active_for_sessions(&self) -> Vec<ProgrammerState> {
        let states = self.states.read();
        let command_contexts = self.command_contexts.read();
        let command_lines = self.command_lines.read();
        let selection_contexts = self.selection_contexts.read();
        self.sessions
            .read()
            .iter()
            .filter_map(|(session, key)| {
                let mut state = states.get(key)?.clone();
                state.session_id = *session;
                let command_context = command_contexts.get(session).unwrap_or(session);
                state.command_line = command_lines
                    .get(command_context)
                    .cloned()
                    .unwrap_or_default();
                if let Some(selection) = selection_contexts.get(command_context) {
                    state.selected = selection.selected.clone();
                    state.selection_expression = selection.expression.clone();
                } else {
                    state.selected.clear();
                    state.selection_expression = None;
                }
                Some(state)
            })
            .collect()
    }
    pub fn get(&self, session: SessionId) -> Option<ProgrammerState> {
        let mut state = self.states.read().get(&self.key(session)).cloned()?;
        state.session_id = session;
        let command_context = self.command_context(session);
        state.command_line = self
            .command_lines
            .read()
            .get(&command_context)
            .cloned()
            .unwrap_or_default();
        self.project_selection(&mut state, command_context);
        Some(state)
    }
    pub fn refresh_live_selections(&self, groups: &HashMap<String, GroupDefinition>) {
        for selection in self.selection_contexts.write().values_mut() {
            match selection.expression.clone() {
                Some(SelectionExpression::LiveGroup { group_id, rule }) => {
                    if let Ok(fixtures) = resolve_group(&group_id, groups) {
                        selection.selected = apply_selection_rule(&fixtures, &rule);
                    }
                }
                Some(
                    SelectionExpression::PlaybackContents { items }
                    | SelectionExpression::Sources { items },
                ) => {
                    selection.selected = resolve_selection_references(&items, groups);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::ManualClock;

    #[test]
    fn users_are_isolated() {
        let registry = ProgrammerRegistry::default();
        let first = SessionId::new();
        let second = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(first, UserId::new());
        registry.start(second, UserId::new());
        registry.select(first, [fixture]);
        registry.set(
            first,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        assert_eq!(registry.get(first).unwrap().selected.len(), 1);
        assert!(registry.get(second).unwrap().selected.is_empty());
        assert!(registry.get(second).unwrap().values.is_empty());
        registry.set_group(
            first,
            "front".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        assert!(
            registry
                .get(first)
                .unwrap()
                .group_values
                .contains_key("front")
        );
        assert!(registry.get(second).unwrap().group_values.is_empty());
    }
    #[test]
    fn sessions_for_the_same_user_share_values_but_keep_command_lines_local() {
        let registry = ProgrammerRegistry::default();
        let user = UserId::new();
        let first = SessionId::new();
        let second = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(first, user);
        registry.select(first, [fixture]);
        registry.start(second, user);
        assert_eq!(registry.active().len(), 1);
        assert!(registry.get(second).unwrap().selected.is_empty());
        assert!(registry.set_command_line(first, "GROUP 1 +".into()));
        assert!(registry.set_command_line(second, "GROUP 2 +".into()));
        assert!(registry.set_command_target(first, "GROUP".into()));
        assert_eq!(registry.command_target(first), "GROUP");
        assert_eq!(registry.command_target(second), "FIXTURE");
        let mut command_lines = registry
            .active_for_sessions()
            .into_iter()
            .map(|state| state.command_line)
            .collect::<Vec<_>>();
        command_lines.sort();
        assert_eq!(command_lines, ["GROUP 1 +", "GROUP 2 +"]);
        assert_eq!(registry.get(second).unwrap().command_line, "GROUP 2 +");
        registry.disconnect(first);
        assert!(registry.get(second).unwrap().connected);
        registry.disconnect(second);
        assert!(!registry.active()[0].connected);
    }

    #[test]
    fn sessions_share_programmer_values_by_user_and_command_interactions_by_desk() {
        let registry = ProgrammerRegistry::default();
        let user = UserId::new();
        let first = SessionId::new();
        let second = SessionId::new();
        let other_desk_session = SessionId::new();
        let desk = SessionId::new();
        let other_desk = SessionId::new();
        let fixture = FixtureId::new();

        registry.start(first, user);
        registry.start(second, user);
        registry.start(other_desk_session, user);
        assert!(registry.attach_command_context(first, desk));
        assert!(registry.attach_command_context(second, desk));
        assert!(registry.attach_command_context(other_desk_session, other_desk));

        registry.select(first, [fixture]);
        registry.set(
            first,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.75),
        );
        assert_eq!(registry.get(second).unwrap().selected, vec![fixture]);
        assert!(
            registry
                .get(other_desk_session)
                .unwrap()
                .selected
                .is_empty()
        );
        assert_eq!(registry.get(other_desk_session).unwrap().values.len(), 1);

        assert!(registry.set_command_line(first, "GROUP 1 +".into()));
        assert!(registry.set_command_target(first, "GROUP".into()));
        assert_eq!(registry.get(second).unwrap().command_line, "GROUP 1 +");
        assert_eq!(registry.command_target(second), "GROUP");
        assert!(
            registry
                .get(other_desk_session)
                .unwrap()
                .command_line
                .is_empty()
        );
        assert_eq!(registry.command_target(other_desk_session), "FIXTURE");

        assert!(registry.set_command_line(other_desk_session, "FIXTURE 9".into()));
        assert_eq!(registry.get(first).unwrap().command_line, "GROUP 1 +");
    }

    #[test]
    fn ordered_selection_sources_remove_and_readd_left_to_right_and_stay_live() {
        let first = FixtureId::new();
        let second = FixtureId::new();
        let third = FixtureId::new();
        let fourth = FixtureId::new();
        let mut groups = HashMap::from([(
            "3".into(),
            GroupDefinition {
                id: "3".into(),
                name: "Group 3".into(),
                fixtures: vec![first, second, third],
                ..Default::default()
            },
        )]);
        let sources = vec![
            SelectionReference::LiveGroup {
                group_id: "3".into(),
            },
            SelectionReference::RemoveFixture { fixture_id: second },
            SelectionReference::Fixture { fixture_id: second },
            SelectionReference::Fixture { fixture_id: fourth },
        ];
        assert_eq!(
            resolve_selection_references(&sources, &groups),
            vec![first, third, second, fourth]
        );

        groups.get_mut("3").unwrap().fixtures = vec![third, first];
        assert_eq!(
            resolve_selection_references(&sources, &groups),
            vec![third, first, second, fourth]
        );
    }

    #[test]
    fn ordinary_selection_gestures_accumulate_per_desk_until_a_value_lands() {
        let registry = ProgrammerRegistry::default();
        let user = UserId::new();
        let first = SessionId::new();
        let same_desk = SessionId::new();
        let other_desk = SessionId::new();
        let desk_context = SessionId::new();
        let other_context = SessionId::new();
        let first_fixture = FixtureId::new();
        let second_fixture = FixtureId::new();
        let third_fixture = FixtureId::new();
        registry.start(first, user);
        registry.start(same_desk, user);
        registry.start(other_desk, user);
        registry.attach_command_context(first, desk_context);
        registry.attach_command_context(same_desk, desk_context);
        registry.attach_command_context(other_desk, other_context);

        assert!(registry.apply_selection_gesture(
            first,
            vec![SelectionReference::Fixture {
                fixture_id: first_fixture,
            }],
            &HashMap::new(),
        ));
        assert!(registry.apply_selection_gesture(
            same_desk,
            vec![SelectionReference::Fixture {
                fixture_id: second_fixture,
            }],
            &HashMap::new(),
        ));
        assert_eq!(
            registry.get(first).unwrap().selected,
            vec![first_fixture, second_fixture]
        );
        assert!(registry.get(other_desk).unwrap().selected.is_empty());

        registry.set(
            first,
            first_fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        assert!(registry.apply_selection_gesture(
            first,
            vec![SelectionReference::Fixture {
                fixture_id: third_fixture,
            }],
            &HashMap::new(),
        ));
        assert_eq!(registry.get(first).unwrap().selected, vec![third_fixture]);
        assert_eq!(registry.get(other_desk).unwrap().values.len(), 1);
    }

    #[test]
    fn releasing_one_scoped_attribute_preserves_every_other_contribution() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(session, UserId::new());
        registry.set(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        registry.set(
            session,
            fixture,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.25),
        );
        registry.set_group(
            session,
            "1".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.75),
        );
        registry.set_group(
            session,
            "1".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.8),
        );

        assert!(registry.release_fixture_attribute(session, fixture, &AttributeKey::intensity(),));
        assert!(!registry.release_fixture_attribute(session, fixture, &AttributeKey::intensity(),));
        assert_eq!(registry.get(session).unwrap().values.len(), 1);
        assert!(registry.release_group_attribute(session, "1", &AttributeKey::intensity(),));
        let state = registry.get(session).unwrap();
        assert_eq!(state.group_values["1"].len(), 1);
        assert!(state.group_values["1"].contains_key(&AttributeKey("pan".into())));
    }

    #[test]
    fn restoring_multiple_sessions_for_one_user_does_not_deadlock() {
        let source = ProgrammerRegistry::default();
        let user = UserId::new();
        let first = SessionId::new();
        let mut first_state = source.start(first, user);
        first_state.connected = false;
        let mut second_state = first_state.clone();
        second_state.session_id = SessionId::new();
        second_state.id = ProgrammerId::new();

        let restored = ProgrammerRegistry::default();
        restored.restore(first_state);
        restored.restore(second_state);
        assert_eq!(restored.active().len(), 1);
        assert_eq!(restored.active_for_sessions().len(), 2);
    }
    #[test]
    fn legacy_group_programmer_values_migrate_with_a_timestamp() {
        let value: GroupProgrammerValue =
            serde_json::from_value(serde_json::json!({"kind":"normalized","value":0.5})).unwrap();
        assert_eq!(value.value.normalized(), Some(0.5));
    }

    #[test]
    fn ordered_selection_macros_derived_groups_and_cycles_are_deterministic() {
        let fixtures = (0..6).map(|_| FixtureId::new()).collect::<Vec<_>>();
        assert_eq!(
            apply_selection_rule(&fixtures, &SelectionRule::Odd),
            vec![fixtures[0], fixtures[2], fixtures[4]]
        );
        assert_eq!(
            apply_selection_rule(&fixtures, &SelectionRule::EveryNth { n: 3, offset: 1 }),
            vec![fixtures[1], fixtures[4]]
        );
        let mut groups = HashMap::from([
            (
                "source".into(),
                GroupDefinition {
                    id: "source".into(),
                    fixtures: fixtures.clone(),
                    ..Default::default()
                },
            ),
            (
                "odd".into(),
                GroupDefinition {
                    id: "odd".into(),
                    derived_from: Some(DerivedGroup {
                        source_group_id: "source".into(),
                        rule: SelectionRule::Odd,
                    }),
                    ..Default::default()
                },
            ),
        ]);
        assert_eq!(
            resolve_group("odd", &groups).unwrap(),
            vec![fixtures[0], fixtures[2], fixtures[4]]
        );
        groups
            .get_mut("source")
            .unwrap()
            .fixtures
            .push(FixtureId::new());
        assert_eq!(resolve_group("odd", &groups).unwrap().len(), 4);
        groups.insert(
            "cycle".into(),
            GroupDefinition {
                id: "cycle".into(),
                derived_from: Some(DerivedGroup {
                    source_group_id: "cycle".into(),
                    rule: SelectionRule::All,
                }),
                ..Default::default()
            },
        );
        assert!(
            resolve_group("cycle", &groups)
                .unwrap_err()
                .contains("cycle")
        );
    }

    #[test]
    fn preload_clear_does_not_release_active_preload() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(session, UserId::new());
        registry.set_modes(session, Some(true), None, None, None);
        registry.set(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.7),
        );
        assert!(registry.activate_preload(session));
        registry.set(
            session,
            fixture,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.2),
        );
        assert!(registry.clear_preload_pending(session));
        let state = registry.get(session).unwrap();
        assert_eq!(state.preload_active.len(), 1);
        assert!(state.preload_pending.is_empty());
        assert_eq!(state.values.len(), 1);
        assert_eq!(state.values[0].attribute, AttributeKey("pan".into()));
    }
    #[test]
    fn preload_retains_multiple_group_scopes_with_edit_timestamps() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        registry.start(session, UserId::new());
        registry.set_modes(session, Some(true), None, None, None);
        registry.set_preload_group(
            session,
            "a".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.4),
        );
        registry.set_preload_group(
            session,
            "b".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.7),
        );
        let state = registry.get(session).unwrap();
        assert_eq!(state.preload_group_pending.len(), 2);
        assert_eq!(
            state.preload_group_pending["a"][&AttributeKey::intensity()]
                .value
                .normalized(),
            Some(0.4)
        );
    }

    #[test]
    fn preload_go_restamps_every_programmer_value_and_release_is_idempotent() {
        let entered_at = chrono::DateTime::parse_from_rfc3339("2026-07-16T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let clock = Arc::new(ManualClock::new(entered_at));
        let shared: SharedClock = clock.clone();
        let registry = ProgrammerRegistry::with_clock(shared);
        let session = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(session, UserId::new());
        assert!(registry.arm_preload(session, true));
        registry.set_faded_with_timing(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
            None,
            None,
        );
        assert!(registry.set_group_faded_with_timing(
            session,
            "back".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.7),
            Some(1_000),
            None,
        ));

        let committed_at = clock.advance_millis(2_500);
        assert!(registry.activate_preload_at(session, committed_at));
        let active = registry.get(session).unwrap();
        assert_eq!(active.preload_active[0].changed_at, committed_at);
        assert_eq!(active.preload_active[0].fade_millis, None);
        assert_eq!(
            active.preload_group_active["back"][&AttributeKey::intensity()].changed_at,
            committed_at
        );
        assert_eq!(
            active.preload_group_active["back"][&AttributeKey::intensity()].fade_millis,
            Some(1_000)
        );

        assert!(registry.release_preload(session));
        assert!(!registry.release_preload(session));
    }

    #[test]
    fn disabled_programmer_domain_stays_live_and_playback_verbs_retain_order() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(session, UserId::new());
        assert!(registry.arm_preload(session, false));
        registry.set(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.4),
        );
        assert!(registry.set_group(
            session,
            "front".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.6),
        ));
        for action in [
            "toggle", "go", "go-minus", "off", "on", "temp-on", "temp-off",
        ] {
            assert!(registry.queue_preload_playback_action(
                session,
                1,
                action.into(),
                "physical".into(),
            ));
        }
        let state = registry.get(session).unwrap();
        assert!(state.preload_pending.is_empty());
        assert!(state.preload_group_pending.is_empty());
        assert_eq!(state.values.len(), 1);
        assert!(state.group_values.contains_key("front"));
        assert_eq!(
            state
                .preload_playback_pending
                .iter()
                .map(|pending| pending.action.as_str())
                .collect::<Vec<_>>(),
            [
                "toggle", "go", "go-minus", "off", "on", "temp-on", "temp-off"
            ]
        );
    }

    #[test]
    fn disconnect_keeps_programmer_until_explicit_clear() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        registry.start(session, UserId::new());
        registry.disconnect(session);
        assert!(!registry.get(session).unwrap().connected);
        assert!(registry.clear(session));
        assert!(registry.get(session).is_none());
    }

    #[test]
    fn history_and_console_state_are_session_local() {
        let registry = ProgrammerRegistry::default();
        let first = SessionId::new();
        let second = SessionId::new();
        registry.start(first, UserId::new());
        registry.start(second, UserId::new());
        assert!(registry.set_command_line(first, "Fixture 1 At Full".into()));
        assert!(registry.set_modes(
            first,
            Some(true),
            None,
            Some(true),
            Some(Some("live".into()))
        ));
        assert!(registry.undo(first));
        assert!(!registry.get(first).unwrap().highlight);
        assert!(registry.redo(first));
        assert!(!registry.get(first).unwrap().highlight);
        assert!(registry.get(first).unwrap().blind);
        assert!(registry.get(second).unwrap().command_line.is_empty());
    }

    #[test]
    fn multi_channel_action_is_one_atomic_undo_step() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(session, UserId::new());
        registry.set_many(
            session,
            [
                (
                    fixture,
                    AttributeKey("__fixture_control_channel.one".into()),
                    AttributeValue::RawDmxExact(255),
                ),
                (
                    fixture,
                    AttributeKey("__fixture_control_channel.two".into()),
                    AttributeValue::RawDmxExact(128),
                ),
            ],
        );
        assert_eq!(registry.get(session).unwrap().values.len(), 2);
        registry.set_many_transient(
            session,
            [
                (
                    fixture,
                    AttributeKey("__fixture_control_channel.one".into()),
                    AttributeValue::RawDmxExact(0),
                ),
                (
                    fixture,
                    AttributeKey("__fixture_control_channel.two".into()),
                    AttributeValue::RawDmxExact(0),
                ),
            ],
        );

        assert!(registry.undo(session));
        assert!(registry.get(session).unwrap().values.is_empty());
    }

    #[test]
    fn preset_store_modes_are_explicit() {
        let fixture = FixtureId::new();
        let other = FixtureId::new();
        let mut preset = Preset {
            name: "A".into(),
            values: HashMap::from([(
                fixture,
                HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(0.5))]),
            )]),
            group_values: HashMap::new(),
        };
        preset.store(
            Preset {
                name: String::new(),
                values: HashMap::from([
                    (
                        fixture,
                        HashMap::from([(
                            AttributeKey("pan".into()),
                            AttributeValue::Normalized(0.2),
                        )]),
                    ),
                    (other, HashMap::new()),
                ]),
                group_values: HashMap::new(),
            },
            PresetStoreMode::AddMissingFixtures,
        );
        assert_eq!(preset.values[&fixture].len(), 1);
        assert!(preset.values.contains_key(&other));
        preset.store(
            Preset {
                name: "B".into(),
                values: HashMap::from([(
                    fixture,
                    HashMap::from([(AttributeKey("pan".into()), AttributeValue::Normalized(0.2))]),
                )]),
                group_values: HashMap::new(),
            },
            PresetStoreMode::Merge,
        );
        assert_eq!(preset.name, "B");
        assert_eq!(preset.values[&fixture].len(), 2);
    }

    #[test]
    fn update_content_captures_only_normal_programmer_edits_without_consuming_them() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        let preload_fixture = FixtureId::new();
        registry.start(session, UserId::new());
        registry.select(session, [fixture]);
        registry.set_faded_with_timing(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.75),
            Some(1_000),
            Some(250),
        );
        assert!(registry.set_group(
            session,
            "front".into(),
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.25),
        ));
        assert!(registry.arm_preload(session, true));
        registry.set(
            session,
            preload_fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );

        let before = registry.get(session).unwrap();
        let update = before.update_content();
        let after = registry.get(session).unwrap();

        assert_eq!(update.selected_fixtures, vec![fixture]);
        assert_eq!(update.fixture_values.len(), 1);
        assert_eq!(update.fixture_values[0].fixture_id, fixture);
        assert_eq!(update.fixture_values[0].fade_millis, Some(1_000));
        assert_eq!(update.fixture_values[0].delay_millis, Some(250));
        assert_eq!(update.group_values.len(), 1);
        assert_eq!(update.group_values[0].group_id, "front");
        assert_eq!(after.values.len(), before.values.len());
        assert_eq!(after.group_values.len(), before.group_values.len());
        assert_eq!(after.preload_pending.len(), 1);
        assert_eq!(after.preload_pending[0].fixture_id, preload_fixture);
    }

    #[test]
    fn ordered_group_merge_never_reorders_or_duplicates_existing_members() {
        let first = FixtureId::new();
        let second = FixtureId::new();
        let third = FixtureId::new();
        let fourth = FixtureId::new();

        assert_eq!(
            merge_ordered_group_membership(
                &[first, second],
                &[second, third, first, fourth, third]
            ),
            vec![first, second, third, fourth]
        );
    }
}
