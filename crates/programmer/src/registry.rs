use crate::command_state::CommandLineState;
use crate::selection::{ProgrammerSelection, SelectionContext};
use crate::state::ProgrammerState;
use light_core::{SessionId, SharedClock, SystemClock, UserId};
use parking_lot::{ReentrantMutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone)]
pub struct ProgrammerRegistry {
    pub(crate) states: Arc<RwLock<HashMap<SessionId, ProgrammerState>>>,
    pub(crate) sessions: Arc<RwLock<HashMap<SessionId, SessionId>>>,
    pub(crate) command_contexts: Arc<RwLock<HashMap<SessionId, SessionId>>>,
    pub(crate) command_states: Arc<RwLock<HashMap<SessionId, CommandLineState>>>,
    pub(crate) selection_contexts: Arc<RwLock<HashMap<SessionId, SelectionContext>>>,
    pub(crate) selection_revision: Arc<AtomicU64>,
    pub(crate) programmer_order: Arc<AtomicU64>,
    /// Cheap write stamp for normal recordable values. Low-level helpers may advance this more
    /// than once while composing one application action; the application boundary uses it only
    /// to detect whether a full value projection must be materialized.
    pub(crate) normal_values_generations: Arc<RwLock<HashMap<UserId, u64>>>,
    /// Monotonic public projection revision, advanced exactly once by the application service for
    /// each completed semantic normal-value transition.
    pub(crate) normal_values_revisions: Arc<RwLock<HashMap<UserId, u64>>>,
    /// Serializes compound mutations per user without preventing unrelated programmers from
    /// progressing concurrently. The mutex is reentrant because public mutation helpers compose
    /// other public helpers (for example, `activate_preload` calls `activate_preload_at`).
    pub(crate) mutation_gates: Arc<RwLock<HashMap<UserId, Arc<ReentrantMutex<()>>>>>,
    /// Failed mutations for unknown sessions share one gate instead of allocating a permanent
    /// real-user gate for every arbitrary UUID.
    pub(crate) unknown_mutation_gate: Arc<ReentrantMutex<()>>,
    pub(crate) clock: SharedClock,
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
            command_states: Arc::default(),
            selection_contexts: Arc::default(),
            selection_revision: Arc::default(),
            programmer_order: Arc::default(),
            normal_values_generations: Arc::default(),
            normal_values_revisions: Arc::default(),
            mutation_gates: Arc::default(),
            unknown_mutation_gate: Arc::new(ReentrantMutex::new(())),
            clock,
        }
    }

    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.clock)
    }

    pub(crate) fn mutation_gate_for_user(&self, user_id: UserId) -> Arc<ReentrantMutex<()>> {
        if let Some(gate) = self.mutation_gates.read().get(&user_id).cloned() {
            return gate;
        }
        Arc::clone(
            self.mutation_gates
                .write()
                .entry(user_id)
                .or_insert_with(|| Arc::new(ReentrantMutex::new(()))),
        )
    }

    /// Serialize a complete application-level transition for one user's shared Programmer.
    ///
    /// The gate is the same reentrant boundary used by every registry mutator, so callers may
    /// capture state, compose existing mutation helpers, and publish the final projection without
    /// another session for that user interleaving a write. Application services must acquire this
    /// user gate before any desk-interaction gate.
    pub fn with_user_serialized<R>(&self, user_id: UserId, operation: impl FnOnce() -> R) -> R {
        let gate = self.mutation_gate_for_user(user_id);
        let _guard = gate.lock();
        operation()
    }

    pub(crate) fn mutation_gate(&self, session: SessionId) -> Arc<ReentrantMutex<()>> {
        let state_key = self.key(session);
        let user_id = self
            .states
            .read()
            .get(&state_key)
            .map(|state| state.user_id);
        user_id.map_or_else(
            || Arc::clone(&self.unknown_mutation_gate),
            |user_id| self.mutation_gate_for_user(user_id),
        )
    }

    /// Run an operation while every currently addressable user gate is held. New user gates are
    /// prevented from appearing between the stable-set check and the operation, while ordinary
    /// per-user mutations remain independent at all other times.
    pub(crate) fn with_all_mutation_gates<R>(&self, operation: impl FnOnce() -> R) -> R {
        loop {
            let mut gates = self
                .mutation_gates
                .read()
                .iter()
                .map(|(user_id, gate)| (*user_id, Arc::clone(gate)))
                .collect::<Vec<_>>();
            gates.sort_unstable_by_key(|(user_id, _)| user_id.0);
            let guards = gates
                .iter()
                .map(|(_, gate)| gate.lock())
                .collect::<Vec<_>>();

            let registered = self.mutation_gates.read();
            let stable = registered.len() == gates.len()
                && gates.iter().all(|(user_id, gate)| {
                    registered
                        .get(user_id)
                        .is_some_and(|registered| Arc::ptr_eq(registered, gate))
                });
            if stable {
                let result = operation();
                drop(registered);
                drop(guards);
                return result;
            }
            drop(registered);
            drop(guards);
        }
    }

    pub fn set_priority(&self, session: SessionId, priority: i16) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
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
            .chain(
                state
                    .transient_values
                    .iter_mut()
                    .flat_map(|action| action.values.iter_mut()),
            )
        {
            value.priority = priority;
        }
        state.last_activity = self.clock.now();
        true
    }

    pub fn reset_all(&self) {
        self.with_all_mutation_gates(|| {
            self.states.write().clear();
            self.sessions.write().clear();
            self.command_contexts.write().clear();
            self.command_states.write().clear();
            self.selection_contexts.write().clear();
            self.selection_revision.store(0, Ordering::Relaxed);
            self.programmer_order.store(0, Ordering::Relaxed);
            self.normal_values_generations.write().clear();
            self.normal_values_revisions.write().clear();
        });
    }

    pub fn normal_values_generation(&self, session: SessionId) -> Option<u64> {
        let user_id = self.states.read().get(&self.key(session))?.user_id;
        Some(
            self.normal_values_generations
                .read()
                .get(&user_id)
                .copied()
                .unwrap_or(0),
        )
    }

    pub fn user_id(&self, session: SessionId) -> Option<UserId> {
        self.states
            .read()
            .get(&self.key(session))
            .map(|state| state.user_id)
    }

    pub fn normal_values_revision(&self, user_id: UserId) -> u64 {
        self.normal_values_revisions
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0)
    }

    pub fn advance_normal_values_revision(&self, user_id: UserId) -> u64 {
        let mut revisions = self.normal_values_revisions.write();
        let revision = revisions.entry(user_id).or_default();
        *revision = revision.saturating_add(1);
        *revision
    }

    pub(crate) fn mark_normal_values_changed(&self, user_id: UserId) {
        let mut generations = self.normal_values_generations.write();
        let generation = generations.entry(user_id).or_default();
        *generation = generation.saturating_add(1);
    }

    pub(crate) fn next_programmer_order(&self) -> u64 {
        self.programmer_order.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn next_selection_revision(&self) -> u64 {
        self.selection_revision.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn set_modes(
        &self,
        session: SessionId,
        blind: Option<bool>,
        preview: Option<bool>,
        highlight: Option<bool>,
        active_context: Option<Option<String>>,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
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

    pub fn clear_values(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let normal_values_changed = !state.values.is_empty() || !state.group_values.is_empty();
        state.checkpoint();
        state.values.clear();
        state.transient_values.clear();
        state.group_values.clear();
        state.last_activity = self.clock.now();
        let user_id = state.user_id;
        drop(states);
        if normal_values_changed {
            self.mark_normal_values_changed(user_id);
        }
        true
    }

    pub fn disconnect(&self, session: SessionId) {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let key = self.key(session);
        self.sessions.write().remove(&session);
        let still_connected = self.sessions.read().values().any(|bound| *bound == key);
        if let Some(state) = self.states.write().get_mut(&key) {
            state.connected = still_connected;
        }
    }
    pub fn connect(&self, session: SessionId) {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.connected = true;
            state.last_activity = self.clock.now();
        }
    }
    pub fn clear(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
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
        let command_states = self.command_states.read();
        let selection_contexts = self.selection_contexts.read();
        self.sessions
            .read()
            .iter()
            .filter_map(|(session, key)| {
                let mut state = states.get(key)?.clone();
                state.session_id = *session;
                let command_context = command_contexts.get(session).unwrap_or(session);
                state.command_line = command_states
                    .get(command_context)
                    .map(|command| command.legacy_text().to_owned())
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
        let state_key = self.key(session);
        let command_context = self.command_context(session);
        // Staged publication acquires these write locks in the same order. Holding all three read
        // guards while building a projection guarantees an old or new result, never a torn mix.
        let states = self.states.read();
        let command_states = self.command_states.read();
        let selection_contexts = self.selection_contexts.read();
        let mut state = states.get(&state_key).cloned()?;
        state.session_id = session;
        state.command_line = command_states
            .get(&command_context)
            .map(|command| command.legacy_text().to_owned())
            .unwrap_or_default();
        if let Some(selection) = selection_contexts.get(&command_context) {
            state.selected = selection.selected.clone();
            state.selection_expression = selection.expression.clone();
        } else {
            state.selected.clear();
            state.selection_expression = None;
        }
        Some(state)
    }

    pub fn selection(&self, session: SessionId) -> Option<ProgrammerSelection> {
        let context = self.command_context(session);
        self.selection_contexts
            .read()
            .get(&context)
            .map(|selection| ProgrammerSelection {
                selected: selection.selected.clone(),
                expression: selection.expression.clone(),
                revision: selection.revision,
                gesture_open: selection.gesture_open,
            })
    }
}
