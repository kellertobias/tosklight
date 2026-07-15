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

const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug, Serialize)]
pub struct GroupProgrammerValue {
    pub value: AttributeValue,
    pub changed_at: DateTime<Utc>,
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
                fade,
                fade_millis,
                delay_millis,
            } => Self {
                value,
                changed_at,
                fade,
                fade_millis,
                delay_millis,
            },
            Repr::Legacy(value) => Self {
                value,
                changed_at: Utc::now(),
                fade: false,
                fade_millis: None,
                delay_millis: None,
            },
        })
    }
}
type GroupProgrammerValues = HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;

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
    pub command_line: String,
    pub blind: bool,
    pub preview: bool,
    pub highlight: bool,
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
    pub connected: bool,
    pub last_activity: DateTime<Utc>,
    #[serde(default)]
    pub command_line: String,
    #[serde(default)]
    pub blind: bool,
    #[serde(default)]
    pub preview: bool,
    #[serde(default)]
    pub highlight: bool,
    #[serde(default)]
    pub active_context: Option<String>,
    #[serde(default)]
    pub undo: Vec<ProgrammerSnapshot>,
    #[serde(default)]
    pub redo: Vec<ProgrammerSnapshot>,
}

impl ProgrammerState {
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
            command_line: self.command_line.clone(),
            blind: self.blind,
            preview: self.preview,
            highlight: self.highlight,
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
        self.command_line = snapshot.command_line;
        self.blind = snapshot.blind;
        self.preview = snapshot.preview;
        self.highlight = snapshot.highlight;
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct GroupDefinition {
    pub id: String,
    pub name: String,
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
#[derive(Clone)]
pub struct ProgrammerRegistry {
    states: Arc<RwLock<HashMap<SessionId, ProgrammerState>>>,
    sessions: Arc<RwLock<HashMap<SessionId, SessionId>>>,
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
            clock,
        }
    }

    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.clock)
    }

    pub fn reset_all(&self) {
        self.states.write().clear();
        self.sessions.write().clear();
    }

    pub fn start(&self, session_id: SessionId, user_id: UserId) -> ProgrammerState {
        let existing = self
            .states
            .read()
            .iter()
            .find_map(|(key, state)| (state.user_id == user_id).then_some(*key));
        if let Some(key) = existing {
            self.sessions.write().insert(session_id, key);
            let mut states = self.states.write();
            let state = states.get_mut(&key).expect("programmer disappeared");
            state.connected = true;
            state.last_activity = self.clock.now();
            return state.clone();
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
            connected: true,
            last_activity: self.clock.now(),
            command_line: String::new(),
            blind: false,
            preview: false,
            highlight: false,
            active_context: None,
            undo: vec![],
            redo: vec![],
        };
        self.states.write().insert(session_id, state.clone());
        state
    }
    pub fn restore(&self, state: ProgrammerState) {
        let existing = {
            self.states
                .read()
                .iter()
                .find_map(|(key, current)| (current.user_id == state.user_id).then_some(*key))
        };
        if let Some(existing) = existing {
            self.states.write().insert(existing, state);
        } else {
            self.states.write().insert(state.session_id, state);
        }
    }
    fn key(&self, session: SessionId) -> SessionId {
        self.sessions
            .read()
            .get(&session)
            .copied()
            .unwrap_or(session)
    }
    pub fn select(&self, session: SessionId, fixtures: impl IntoIterator<Item = FixtureId>) {
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            let mut seen = HashSet::new();
            state.selected = fixtures
                .into_iter()
                .filter(|fixture| seen.insert(*fixture))
                .collect();
            state.selection_expression = Some(SelectionExpression::Static);
            state.last_activity = self.clock.now();
        }
    }
    pub fn select_expression(
        &self,
        session: SessionId,
        fixtures: Vec<FixtureId>,
        expression: SelectionExpression,
    ) {
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            state.selected = fixtures;
            state.selection_expression = Some(expression);
            state.last_activity = self.clock.now();
        }
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
            true,
            fade_millis,
            delay_millis,
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
        self.set_with_timing(session, fixture_id, attribute, value, fade, None, None);
    }
    fn set_with_timing(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        fade: bool,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) {
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.checkpoint();
            let merge_mode = light_core::MergeMode::Ltp;
            let values = if state.blind {
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
                merge_mode,
                fade,
                fade_millis,
                delay_millis,
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
            true,
            fade_millis,
            delay_millis,
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
        self.set_group_with_timing(session, group_id, attribute, value, fade, None, None)
    }
    fn set_group_with_timing(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        fade: bool,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        let target = if state.blind {
            &mut state.preload_group_pending
        } else {
            &mut state.group_values
        };
        target.entry(group_id).or_default().insert(
            attribute,
            GroupProgrammerValue {
                value,
                changed_at: self.clock.now(),
                fade,
                fade_millis,
                delay_millis,
            },
        );
        state.last_activity = self.clock.now();
        true
    }
    pub fn activate_preload(&self, session: SessionId) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        for incoming in std::mem::take(&mut state.preload_pending) {
            state.preload_active.retain(|value| {
                !(value.fixture_id == incoming.fixture_id && value.attribute == incoming.attribute)
            });
            state.preload_active.push(incoming);
        }
        for (group, attributes) in std::mem::take(&mut state.preload_group_pending) {
            state
                .preload_group_active
                .entry(group)
                .or_default()
                .extend(attributes);
        }
        // GO publishes the prepared values, then returns input to the live
        // programmer. Entering preload again starts the next blind edit.
        state.blind = false;
        state.last_activity = self.clock.now();
        true
    }
    pub fn clear_preload_pending(&self, session: SessionId) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        state.preload_pending.clear();
        state.preload_group_pending.clear();
        state.last_activity = self.clock.now();
        true
    }
    pub fn release_preload(&self, session: SessionId) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        state.preload_pending.clear();
        state.preload_active.clear();
        state.preload_group_pending.clear();
        state.preload_group_active.clear();
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
        state
            .preload_group_pending
            .entry(group_id)
            .or_default()
            .insert(
                attribute,
                GroupProgrammerValue {
                    value,
                    changed_at: self.clock.now(),
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                },
            );
        state.last_activity = self.clock.now();
        true
    }
    pub fn set_command_line(&self, session: SessionId, command_line: String) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        state.command_line = command_line;
        state.last_activity = self.clock.now();
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
        if let Some(value) = highlight {
            state.highlight = value;
        }
        if let Some(value) = active_context {
            state.active_context = value;
        }
        state.last_activity = self.clock.now();
        true
    }
    pub fn clear_values(&self, session: SessionId) -> bool {
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
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let Some(previous) = state.undo.pop() else {
            return false;
        };
        state.redo.push(state.snapshot());
        state.restore_snapshot(previous, self.clock.now());
        true
    }
    pub fn redo(&self, session: SessionId) -> bool {
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let Some(next) = state.redo.pop() else {
            return false;
        };
        state.undo.push(state.snapshot());
        state.restore_snapshot(next, self.clock.now());
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
    pub fn get(&self, session: SessionId) -> Option<ProgrammerState> {
        self.states.read().get(&self.key(session)).cloned()
    }
    pub fn refresh_live_selections(&self, groups: &HashMap<String, GroupDefinition>) {
        for state in self.states.write().values_mut() {
            let Some(SelectionExpression::LiveGroup { group_id, rule }) =
                state.selection_expression.clone()
            else {
                continue;
            };
            if let Ok(fixtures) = resolve_group(&group_id, groups) {
                state.selected = apply_selection_rule(&fixtures, &rule);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn sessions_for_the_same_user_share_one_programmer() {
        let registry = ProgrammerRegistry::default();
        let user = UserId::new();
        let first = SessionId::new();
        let second = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(first, user);
        registry.select(first, [fixture]);
        registry.start(second, user);
        assert_eq!(registry.active().len(), 1);
        assert_eq!(registry.get(second).unwrap().selected, vec![fixture]);
        registry.disconnect(first);
        assert!(registry.get(second).unwrap().connected);
        registry.disconnect(second);
        assert!(!registry.active()[0].connected);
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
        assert!(registry.get(first).unwrap().highlight);
        assert!(registry.get(second).unwrap().command_line.is_empty());
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
}
