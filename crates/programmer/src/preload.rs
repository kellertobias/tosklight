use crate::ProgrammerRegistry;
use crate::groups::GroupProgrammerValue;
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, SessionId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreloadPlaybackAction {
    pub playback_number: u16,
    pub action: String,
    pub surface: String,
}

impl ProgrammerRegistry {
    pub fn activate_preload(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.activate_preload_at(session, self.clock.now())
    }

    /// Publishes every pending programmer value at the one application timestamp owned by
    /// Preload GO. Values deliberately keep their explicit fade/delay metadata; only their
    /// transition origin moves from the blind-edit time to the commit time.
    pub fn activate_preload_at(&self, session: SessionId, committed_at: DateTime<Utc>) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let (user_id, pending_values_changed) = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return false;
            };
            let pending_values_changed =
                !state.preload_pending.is_empty() || !state.preload_group_pending.is_empty();
            state.checkpoint();
            for mut incoming in std::mem::take(&mut state.preload_pending) {
                incoming.changed_at = committed_at;
                state.preload_active.retain(|value| {
                    !(value.fixture_id == incoming.fixture_id
                        && value.attribute == incoming.attribute)
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
            (state.user_id, pending_values_changed)
        };
        if pending_values_changed {
            self.mark_preload_values_changed(user_id);
        }
        true
    }

    pub fn queue_preload_playback_action(
        &self,
        session: SessionId,
        playback_number: u16,
        action: String,
        surface: String,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
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
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return Vec::new();
        };
        std::mem::take(&mut state.preload_playback_pending)
    }
    pub fn clear_preload_pending(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let (user_id, pending_values_changed) = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return false;
            };
            let pending_values_changed =
                !state.preload_pending.is_empty() || !state.preload_group_pending.is_empty();
            state.checkpoint();
            state.preload_pending.clear();
            state.preload_group_pending.clear();
            state.preload_playback_pending.clear();
            state.last_activity = self.clock.now();
            (state.user_id, pending_values_changed)
        };
        if pending_values_changed {
            self.mark_preload_values_changed(user_id);
        }
        true
    }
    pub fn release_preload(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let pending_values_changed =
            !state.preload_pending.is_empty() || !state.preload_group_pending.is_empty();
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
        let user_id = state.user_id;
        drop(states);
        if pending_values_changed {
            self.mark_preload_values_changed(user_id);
        }
        true
    }
    pub fn set_preload_group(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
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
        let user_id = state.user_id;
        drop(states);
        self.mark_preload_values_changed(user_id);
        true
    }

    pub fn arm_preload(&self, session: SessionId, capture_programmer: bool) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
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
}
