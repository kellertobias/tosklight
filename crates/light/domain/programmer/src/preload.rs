use crate::ProgrammerRegistry;
use crate::groups::GroupProgrammerValue;
use crate::{PreloadPlaybackQueueAction, PreloadPlaybackQueueSurface};
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, SessionId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreloadPlaybackAction {
    pub playback_number: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_desk_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u8>,
    pub action: PreloadPlaybackQueueAction,
    pub surface: PreloadPlaybackQueueSurface,
}

impl ProgrammerRegistry {
    /// Reads only whether retained active Preload values exist; no Programmer projection is
    /// cloned or serialized.
    pub fn has_active_preload(&self, session: SessionId) -> Option<bool> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let states = self.states.read();
        let state = states.get(&self.key(session))?;
        Some(
            !state.preload_active.is_empty()
                || !state.preload_dynamic_active.is_empty()
                || !state.preload_group_active.is_empty()
                || !state.preload_group_release_active.is_empty()
                || state.preload_playback_active,
        )
    }

    pub fn activate_preload(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        self.activate_preload_at(session, self.clock.now())
    }

    /// Publishes pending values at one timestamp while preserving their stored timing.
    /// Production Preload GO uses `activate_preload_at_with_fade` so trigger-time Programmer Fade
    /// replaces blind-edit fade metadata.
    pub fn activate_preload_at(&self, session: SessionId, committed_at: DateTime<Utc>) -> bool {
        self.activate_preload_at_with_timing(session, committed_at, None)
    }

    /// Publish pending values at one GO-owned timestamp and capture the supplied Programmer Fade
    /// for every static value. Blind-edit timing must not leak into the transition that starts at
    /// GO; changing the setting after this call likewise cannot alter the running fade.
    pub fn activate_preload_at_with_fade(
        &self,
        session: SessionId,
        committed_at: DateTime<Utc>,
        programmer_fade_millis: u64,
    ) -> bool {
        self.activate_preload_at_with_timing(session, committed_at, Some(programmer_fade_millis))
    }

    fn activate_preload_at_with_timing(
        &self,
        session: SessionId,
        committed_at: DateTime<Utc>,
        programmer_fade_millis: Option<u64>,
    ) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let pending_values_changed = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return false;
            };
            let pending_values_changed = !state.preload_pending.is_empty()
                || !state.preload_dynamic_pending.is_empty()
                || !state.preload_group_pending.is_empty()
                || !state.preload_group_release_pending.is_empty();
            state.checkpoint();
            for mut incoming in std::mem::take(&mut state.preload_pending) {
                incoming.changed_at = committed_at;
                if let Some(fade_millis) = programmer_fade_millis {
                    incoming.fade = true;
                    incoming.fade_millis = Some(fade_millis);
                }
                Arc::make_mut(&mut state.preload_active).retain(|value| {
                    !(value.fixture_id == incoming.fixture_id
                        && value.attribute == incoming.attribute)
                });
                Arc::make_mut(&mut state.preload_active).push(incoming);
            }
            for (group, mut attributes) in std::mem::take(&mut state.preload_group_pending) {
                for value in attributes.values_mut() {
                    value.changed_at = committed_at;
                    if let Some(fade_millis) = programmer_fade_millis {
                        value.fade = true;
                        value.fade_millis = Some(fade_millis);
                    }
                }
                Arc::make_mut(&mut state.preload_group_active)
                    .entry(group)
                    .or_default()
                    .extend(attributes);
            }
            for mut incoming in std::mem::take(&mut state.preload_group_release_pending) {
                incoming.changed_at_millis =
                    u64::try_from(committed_at.timestamp_millis()).unwrap_or_default();
                state.preload_group_release_active.retain(|stored| {
                    stored.group_id != incoming.group_id || stored.attribute != incoming.attribute
                });
                state.preload_group_release_active.push(incoming);
            }
            let committed_at_millis =
                u64::try_from(committed_at.timestamp_millis()).unwrap_or_default();
            for mut incoming in std::mem::take(Arc::make_mut(&mut state.preload_dynamic_pending)) {
                incoming.changed_at_millis = committed_at_millis;
                let instance_link = dynamic_instance_link(&incoming.value);
                Arc::make_mut(&mut state.preload_dynamic_active).retain(|stored| {
                    stored.fixture_id != incoming.fixture_id
                        || stored.attribute != incoming.attribute
                        || dynamic_instance_link(&stored.value) != instance_link
                });
                Arc::make_mut(&mut state.preload_dynamic_active).push(incoming);
            }
            // Committed queued Playback activations keep the Preload scene releasable via
            // hold-to-release even when no attribute values were retained.
            if !state.preload_playback_pending.is_empty() {
                state.preload_playback_active = true;
            }
            // GO publishes the prepared values, then returns input to the live
            // programmer. Entering preload again starts the next blind edit.
            state.blind = false;
            state.last_activity = committed_at;
            pending_values_changed
        };
        if pending_values_changed {
            self.mark_preload_values_changed();
        }
        true
    }

    pub fn queue_preload_playback_action(
        &self,
        session: SessionId,
        playback_number: u16,
        page: Option<u8>,
        action: PreloadPlaybackQueueAction,
        surface: PreloadPlaybackQueueSurface,
    ) -> bool {
        self.queue_preload_playback_action_with_origin(
            session,
            playback_number,
            page,
            action,
            surface,
            None,
        )
    }

    pub fn queue_preload_playback_action_with_origin(
        &self,
        session: SessionId,
        playback_number: u16,
        page: Option<u8>,
        action: PreloadPlaybackQueueAction,
        surface: PreloadPlaybackQueueSurface,
        origin_desk_id: Option<Uuid>,
    ) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        state.preload_playback_pending.push(PreloadPlaybackAction {
            playback_number,
            origin_desk_id,
            page,
            action,
            surface,
        });
        state.last_activity = self.clock.now();
        drop(states);
        self.mark_preload_playback_queue_changed();
        true
    }

    /// Clone only the ordered queued playback actions, without materializing a Programmer state.
    pub fn preload_playback_actions(
        &self,
        session: SessionId,
    ) -> Option<Vec<PreloadPlaybackAction>> {
        self.states
            .read()
            .get(&self.key(session))
            .map(|state| state.preload_playback_pending.clone())
    }

    pub fn take_preload_playback_actions(&self, session: SessionId) -> Vec<PreloadPlaybackAction> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return Vec::new();
        };
        let drained = std::mem::take(&mut state.preload_playback_pending);
        drop(states);
        if !drained.is_empty() {
            self.mark_preload_playback_queue_changed();
        }
        drained
    }
    pub fn clear_preload_pending(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let (pending_values_changed, queue_changed) = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return false;
            };
            let pending_values_changed = !state.preload_pending.is_empty()
                || !state.preload_dynamic_pending.is_empty()
                || !state.preload_group_pending.is_empty()
                || !state.preload_group_release_pending.is_empty();
            let queue_changed = !state.preload_playback_pending.is_empty();
            state.checkpoint();
            state.preload_pending.clear();
            Arc::make_mut(&mut state.preload_dynamic_pending).clear();
            state.preload_group_pending.clear();
            state.preload_group_release_pending.clear();
            state.preload_playback_pending.clear();
            state.last_activity = self.clock.now();
            (pending_values_changed, queue_changed)
        };
        if pending_values_changed {
            self.mark_preload_values_changed();
        }
        if queue_changed {
            self.mark_preload_playback_queue_changed();
        }
        true
    }
    pub fn release_preload(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let pending_values_changed = !state.preload_pending.is_empty()
            || !state.preload_dynamic_pending.is_empty()
            || !state.preload_group_pending.is_empty()
            || !state.preload_group_release_pending.is_empty();
        let queue_changed = !state.preload_playback_pending.is_empty();
        let changed = state.blind
            || !state.preload_pending.is_empty()
            || !state.preload_active.is_empty()
            || !state.preload_dynamic_pending.is_empty()
            || !state.preload_dynamic_active.is_empty()
            || !state.preload_group_pending.is_empty()
            || !state.preload_group_active.is_empty()
            || !state.preload_group_release_pending.is_empty()
            || !state.preload_group_release_active.is_empty()
            || !state.preload_playback_pending.is_empty()
            || state.preload_playback_active;
        if !changed {
            return false;
        }
        state.checkpoint();
        state.preload_pending.clear();
        Arc::make_mut(&mut state.preload_active).clear();
        Arc::make_mut(&mut state.preload_dynamic_pending).clear();
        Arc::make_mut(&mut state.preload_dynamic_active).clear();
        state.preload_group_pending.clear();
        Arc::make_mut(&mut state.preload_group_active).clear();
        state.preload_group_release_pending.clear();
        state.preload_group_release_active.clear();
        state.preload_playback_pending.clear();
        state.preload_playback_active = false;
        state.blind = false;
        state.last_activity = self.clock.now();
        drop(states);
        if pending_values_changed {
            self.mark_preload_values_changed();
        }
        if queue_changed {
            self.mark_preload_playback_queue_changed();
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
        let mutation_gate = self.mutation_gate();
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
        drop(states);
        self.mark_preload_values_changed();
        true
    }

    pub fn arm_preload(&self, session: SessionId, capture_programmer: bool) -> bool {
        let mutation_gate = self.mutation_gate();
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

fn dynamic_instance_link(value: &light_dynamics::DynamicSemanticValue) -> Option<Uuid> {
    match value {
        light_dynamics::DynamicSemanticValue::DynamicOn { instance_link, .. }
        | light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. } => {
            Some(*instance_link)
        }
        light_dynamics::DynamicSemanticValue::Static { .. }
        | light_dynamics::DynamicSemanticValue::FixAt { .. }
        | light_dynamics::DynamicSemanticValue::Release => None,
    }
}
