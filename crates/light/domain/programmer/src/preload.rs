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
    pub fn has_active_preload(&self, _session: SessionId) -> Option<bool> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let states = self.state.read();
        let state = states.as_ref()?;
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

    /// Publishes pending assignments at one timestamp while preserving their stored timing.
    /// Release rows retain their original source cutoff, including their authored timestamp.
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
        _session: SessionId,
        committed_at: DateTime<Utc>,
        programmer_fade_millis: Option<u64>,
    ) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let pending_values_changed = {
            let mut states = self.state.write();
            let Some(state) = states.as_mut() else {
                return false;
            };
            let pending_values_changed = !state.preload_pending.is_empty()
                || !state.preload_dynamic_pending.is_empty()
                || !state.preload_group_pending.is_empty()
                || !state.preload_group_release_pending.is_empty();
            state.checkpoint();
            restamp_pending_orders(self, state);
            // A newly committed ordinary assignment supersedes the previous active Release in
            // that same scope. Keeping both would make Cue capture append the stale Release
            // after the visible Set. This does not clear a retained Release on a normal edit.
            if !state.preload_dynamic_active.is_empty() && !state.preload_pending.is_empty() {
                let replaced = state
                    .preload_pending
                    .iter()
                    .map(|value| (value.fixture_id, value.attribute.clone()))
                    .collect::<std::collections::HashSet<_>>();
                Arc::make_mut(&mut state.preload_dynamic_active).retain(|value| {
                    !matches!(value.value, light_dynamics::DynamicSemanticValue::Release)
                        || !replaced.contains(&(value.fixture_id, value.attribute.clone()))
                });
            }
            if !state.preload_group_release_active.is_empty()
                && !state.preload_group_pending.is_empty()
            {
                Arc::make_mut(&mut state.preload_group_release_active).retain(|value| {
                    !state
                        .preload_group_pending
                        .get(&value.group_id)
                        .is_some_and(|attributes| attributes.contains_key(&value.attribute))
                });
            }
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
            for incoming in std::mem::take(&mut state.preload_group_release_pending) {
                Arc::make_mut(&mut state.preload_group_release_active).retain(|stored| {
                    stored.group_id != incoming.group_id || stored.attribute != incoming.attribute
                });
                Arc::make_mut(&mut state.preload_group_release_active).push(incoming);
            }
            let committed_at_millis =
                u64::try_from(committed_at.timestamp_millis()).unwrap_or_default();
            for mut incoming in std::mem::take(Arc::make_mut(&mut state.preload_dynamic_pending)) {
                // Release is a cutoff over sources present when it was authored. Advancing
                // either part of its stamp would incorrectly suppress newer Live edits.
                if !incoming.value.is_programming_release() {
                    incoming.changed_at_millis = committed_at_millis;
                }
                Arc::make_mut(&mut state.preload_dynamic_active).retain(|stored| {
                    !incoming.value.replaces_address(
                        incoming.fixture_id,
                        &incoming.attribute,
                        stored.value.track_key(),
                        stored.fixture_id,
                        &stored.attribute,
                    )
                });
                Arc::make_mut(&mut state.preload_dynamic_active).push(incoming);
            }
            // Committed queued Playback activations keep the Preload scene releasable via
            // hold-to-release even when no attribute values were retained.
            state.preload_released_colors = Arc::default();
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
        _session: SessionId,
        playback_number: u16,
        page: Option<u8>,
        action: PreloadPlaybackQueueAction,
        surface: PreloadPlaybackQueueSurface,
        origin_desk_id: Option<Uuid>,
    ) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
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

    /// Remove exactly one displayed pending action. The application guards the queue revision
    /// under the shared mutation gate; duplicate actions and every remaining origin stay intact.
    pub fn remove_preload_dynamic_value(&self, _session: SessionId, index: usize) -> bool {
        let gate = self.mutation_gate();
        let _guard = gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
            return false;
        };
        if index >= state.preload_dynamic_pending.len() {
            return false;
        }
        state.checkpoint();
        Arc::make_mut(&mut state.preload_dynamic_pending).remove(index);
        state.prune_released_fixture_colors();
        state.last_activity = self.clock.now();
        drop(states);
        self.mark_preload_values_changed();
        true
    }
    pub fn remove_preload_group_release(&self, _session: SessionId, index: usize) -> bool {
        let gate = self.mutation_gate();
        let _guard = gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
            return false;
        };
        if index >= state.preload_group_release_pending.len() {
            return false;
        }
        state.checkpoint();
        let removed = state.preload_group_release_pending.remove(index);
        if !state
            .preload_group_release_pending
            .iter()
            .any(|entry| entry.group_id == removed.group_id && entry.attribute == removed.attribute)
        {
            state.clear_released_group_color(&removed.group_id, &removed.attribute);
        }
        state.last_activity = self.clock.now();
        drop(states);
        self.mark_preload_values_changed();
        true
    }
    pub fn remove_preload_playback_action(&self, _session: SessionId, index: usize) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
            return false;
        };
        if index >= state.preload_playback_pending.len() {
            return false;
        }
        state.checkpoint();
        state.preload_playback_pending.remove(index);
        state.last_activity = self.clock.now();
        drop(states);
        self.mark_preload_playback_queue_changed();
        true
    }

    /// Clone only the ordered queued playback actions, without materializing a Programmer state.
    pub fn preload_playback_actions(
        &self,
        _session: SessionId,
    ) -> Option<Vec<PreloadPlaybackAction>> {
        self.state
            .read()
            .as_ref()
            .map(|state| state.preload_playback_pending.clone())
    }

    pub fn take_preload_playback_actions(&self, _session: SessionId) -> Vec<PreloadPlaybackAction> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
            return Vec::new();
        };
        let drained = std::mem::take(&mut state.preload_playback_pending);
        drop(states);
        if !drained.is_empty() {
            self.mark_preload_playback_queue_changed();
        }
        drained
    }
    pub fn clear_preload_pending(&self, _session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let (pending_values_changed, queue_changed) = {
            let mut states = self.state.write();
            let Some(state) = states.as_mut() else {
                return false;
            };
            let pending_values_changed = !state.preload_pending.is_empty()
                || !state.preload_dynamic_pending.is_empty()
                || !state.preload_group_pending.is_empty()
                || !state.preload_group_release_pending.is_empty();
            let queue_changed = !state.preload_playback_pending.is_empty();
            state.checkpoint();
            state.preload_pending.clear();
            state.preload_released_colors = Arc::default();
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
    pub fn release_preload(&self, _session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
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
        state.preload_released_colors = Arc::default();
        Arc::make_mut(&mut state.preload_active).clear();
        Arc::make_mut(&mut state.preload_dynamic_pending).clear();
        Arc::make_mut(&mut state.preload_dynamic_active).clear();
        state.preload_group_pending.clear();
        Arc::make_mut(&mut state.preload_group_active).clear();
        state.preload_group_release_pending.clear();
        Arc::make_mut(&mut state.preload_group_release_active).clear();
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
        _session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
            return false;
        };
        state.checkpoint();
        state.clear_group_release(true, &group_id, &attribute);
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

    pub fn arm_preload(&self, _session: SessionId, capture_programmer: bool) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
            return false;
        };
        state.checkpoint();
        state.blind = true;
        state.preload_capture_programmer = capture_programmer;
        state.last_activity = self.clock.now();
        true
    }
}

/// GO assigns values after any Live edit made while this scene was pending. Preserve the
/// pending assignment order (and equality for one shared edit) across value lanes; assigning
/// in HashMap iteration order would change fixture/group arbitration. Release rows preserve
/// their authored cutoff instead, so newer Live sources survive the prepared Release.
fn restamp_pending_orders(registry: &ProgrammerRegistry, state: &mut crate::ProgrammerState) {
    use std::collections::BTreeMap;

    // Equal positive orders identify one edit. Keep its timestamp extent too: mixed legacy
    // rows compare against timestamps, even though modern rows compare by authored order.
    fn key(order: u64, millis: i128, submillis: u32) -> (u64, i128, u32) {
        if order == 0 {
            (0, millis, submillis)
        } else {
            (order, 0, 0)
        }
    }
    fn timed_key(order: u64, at: DateTime<Utc>) -> (u64, i128, u32) {
        key(
            order,
            i128::from(at.timestamp_millis()),
            at.timestamp_subsec_nanos() % 1_000_000,
        )
    }
    fn millis_key(order: u64, at: u64) -> (u64, i128, u32) {
        key(order, i128::from(at), 0)
    }

    let timed_stamp = |order, at: DateTime<Utc>| {
        (
            order,
            i128::from(at.timestamp_millis()),
            at.timestamp_subsec_nanos() % 1_000_000,
        )
    };
    let fixture = state
        .preload_pending
        .iter()
        .map(|value| timed_stamp(value.programmer_order, value.changed_at));
    let groups = state
        .preload_group_pending
        .values()
        .flat_map(|values| values.values())
        .map(|value| timed_stamp(value.programmer_order, value.changed_at));
    let dynamics = state
        .preload_dynamic_pending
        .iter()
        .filter(|value| !value.value.is_programming_release())
        .map(|value| {
            (
                value.programmer_order,
                i128::from(value.changed_at_millis),
                0,
            )
        });
    let mut edits = BTreeMap::<_, PreloadCommitEdit>::new();
    for (order, millis, nanos) in fixture.chain(groups).chain(dynamics) {
        let key = key(order, millis, nanos);
        let at = (millis, nanos);
        let edit = edits.entry(key).or_insert(PreloadCommitEdit {
            key,
            first_at: at,
            last_at: at,
        });
        edit.first_at = edit.first_at.min(at);
        edit.last_at = edit.last_at.max(at);
    }
    let edits = edits.into_values().collect::<Vec<_>>();
    let orders = preload_commit_edit_order(&edits)
        .into_iter()
        .map(|index| (edits[index].key, registry.next_programmer_order()))
        .collect::<BTreeMap<_, _>>();
    // The commit reorders edits, but their recalled source identity survives that boundary.
    for (key, new_order) in &orders {
        if let Some(origin) = state.preset_provenance.get(&key.0).cloned() {
            Arc::make_mut(&mut state.preset_provenance).insert(*new_order, origin);
        }
        if let Some(origin) = state.replacement_provenance.get(&key.0).cloned() {
            Arc::make_mut(&mut state.replacement_provenance).insert(*new_order, origin);
        }
    }
    for value in &mut state.preload_pending {
        value.programmer_order = orders[&timed_key(value.programmer_order, value.changed_at)];
    }
    for value in state
        .preload_group_pending
        .values_mut()
        .flat_map(|values| values.values_mut())
    {
        value.programmer_order = orders[&timed_key(value.programmer_order, value.changed_at)];
    }
    if !state.preload_dynamic_pending.is_empty() {
        for value in Arc::make_mut(&mut state.preload_dynamic_pending) {
            if !value.value.is_programming_release() {
                value.programmer_order =
                    orders[&millis_key(value.programmer_order, value.changed_at_millis)];
            }
        }
    }
}

struct PreloadCommitEdit {
    key: (u64, i128, u32),
    first_at: (i128, u32),
    last_at: (i128, u32),
}

/// Mixed legacy/current comparison is not necessarily transitive: order 1 at t=300,
/// order 2 at t=100 and legacy order 0 at t=200 form a cycle. Keep every acyclic precedence;
/// normalize only strongly connected cycles by timestamp, then old order. This is a cold GO
/// operation with O(n²) comparisons and O(n) workspace, without recursive traversal. Modern
/// batches and legacy-only batches retain the sorted-map fast path.
fn preload_commit_edit_order(edits: &[PreloadCommitEdit]) -> Vec<usize> {
    use std::collections::BTreeSet;
    let count = edits.len();
    if !edits.iter().any(|edit| edit.key.0 == 0) || !edits.iter().any(|edit| edit.key.0 > 0) {
        return (0..count).collect();
    }
    let precedes = |left: usize, right: usize| {
        if left == right {
            return false;
        }
        let (left, right) = (&edits[left], &edits[right]);
        if left.key.0 > 0 && right.key.0 > 0 {
            left.key.0 < right.key.0
        } else {
            (left.first_at, left.key.0) < (right.last_at, right.key.0)
        }
    };
    let canonical = |index: usize| (edits[index].first_at, edits[index].key.0);

    // Kosaraju with implicit edges avoids an n² allocation for old large pending scenes.
    let mut visited = vec![false; count];
    let mut finished = Vec::with_capacity(count);
    let mut traversal = Vec::<(usize, usize)>::new();
    for root in 0..count {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        traversal.push((root, 0));
        while let Some((node, next)) = traversal.last_mut() {
            let mut child = None;
            while *next < count {
                let candidate = *next;
                *next += 1;
                if !visited[candidate] && precedes(*node, candidate) {
                    child = Some(candidate);
                    break;
                }
            }
            if let Some(child) = child {
                visited[child] = true;
                traversal.push((child, 0));
            } else {
                finished.push(*node);
                traversal.pop();
            }
        }
    }
    let mut component = vec![usize::MAX; count];
    let mut components = Vec::<Vec<usize>>::new();
    let mut pending = Vec::new();
    for root in finished.into_iter().rev() {
        if component[root] != usize::MAX {
            continue;
        }
        let id = components.len();
        component[root] = id;
        pending.push(root);
        let mut members = Vec::new();
        while let Some(node) = pending.pop() {
            members.push(node);
            for parent in 0..count {
                if component[parent] == usize::MAX && precedes(parent, node) {
                    component[parent] = id;
                    pending.push(parent);
                }
            }
        }
        members.sort_by_key(|index| canonical(*index));
        components.push(members);
    }
    let mut incoming = vec![0_usize; components.len()];
    for left in 0..count {
        for right in 0..count {
            if component[left] != component[right] && precedes(left, right) {
                incoming[component[right]] += 1;
            }
        }
    }
    let mut ready = BTreeSet::new();
    for (id, members) in components.iter().enumerate() {
        if incoming[id] == 0 {
            ready.insert((canonical(members[0]), id));
        }
    }
    let mut result = Vec::with_capacity(count);
    while let Some((_, id)) = ready.pop_first() {
        result.extend_from_slice(&components[id]);
        for &left in &components[id] {
            for right in 0..count {
                let other = component[right];
                if other != id && precedes(left, right) {
                    incoming[other] -= 1;
                    if incoming[other] == 0 {
                        ready.insert((canonical(components[other][0]), other));
                    }
                }
            }
        }
    }
    debug_assert_eq!(result.len(), count);
    result
}
