use crate::ProgrammerRegistry;
use crate::selection::{SelectionRule, apply_selection_rule};
use crate::state::ProgrammerValueTiming;
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Serialize)]
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
pub(crate) type GroupProgrammerValues =
    HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;

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

impl ProgrammerRegistry {
    pub fn set_group(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_group_with_fade(session, group_id, attribute, value, false)
    }
    pub fn set_group_faded(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
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
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
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
        let preload = state.blind && state.preload_capture_programmer;
        let target = if preload {
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
        let user_id = state.user_id;
        drop(states);
        if !preload {
            self.mark_normal_values_changed(user_id);
        }
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
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let preload = state.blind && state.preload_capture_programmer;
        let values = if preload {
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
        let values = if preload {
            &mut state.preload_pending
        } else {
            &mut state.values
        };
        values.retain(|value| value.fixture_id != fixture_id || value.attribute != *attribute);
        debug_assert!(values.len() < before);
        state.last_activity = self.clock.now();
        let user_id = state.user_id;
        drop(states);
        if !preload {
            self.mark_normal_values_changed(user_id);
        }
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
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let preload = state.blind && state.preload_capture_programmer;
        let target = if preload {
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
        let target = if preload {
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
        let user_id = state.user_id;
        drop(states);
        if !preload {
            self.mark_normal_values_changed(user_id);
        }
        true
    }
}
