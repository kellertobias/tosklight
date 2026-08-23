use crate::alignment::ProgrammerAlignmentState;
use crate::command_state::CommandLineState;
use crate::selection::{ProgrammerSelection, SelectionContext};
use crate::state::{ProgrammerOutputState, ProgrammerState};
use light_core::{SessionId, SharedClock, SystemClock, UserId};
use parking_lot::{ReentrantMutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub type ActiveDynamicSessionSource = (
    uuid::Uuid,
    i16,
    Arc<Vec<light_dynamics::DynamicAddressValue>>,
    Arc<Vec<light_dynamics::DynamicAddressValue>>,
);

#[derive(Clone)]
pub struct ProgrammerRegistry {
    pub(crate) states: Arc<RwLock<HashMap<SessionId, ProgrammerState>>>,
    pub(crate) sessions: Arc<RwLock<HashMap<SessionId, SessionId>>>,
    pub(crate) command_contexts: Arc<RwLock<HashMap<SessionId, SessionId>>>,
    pub(crate) command_states: Arc<RwLock<HashMap<SessionId, CommandLineState>>>,
    pub(crate) selection_contexts: Arc<RwLock<HashMap<SessionId, SelectionContext>>>,
    pub(crate) alignment_contexts: Arc<RwLock<HashMap<SessionId, ProgrammerAlignmentState>>>,
    pub(crate) selection_revision: Arc<AtomicU64>,
    pub(crate) alignment_revision: Arc<AtomicU64>,
    pub(crate) programmer_order: Arc<AtomicU64>,
    /// Cheap write stamp for normal recordable values. Low-level helpers may advance this more
    /// than once while composing one application action; the application boundary uses it only
    /// to detect whether a full value projection must be materialized.
    pub(crate) normal_values_generations: crate::desk_stamp::DeskStamp,
    /// Monotonic public projection revision, advanced exactly once by the application service for
    /// each completed semantic normal-value transition.
    pub(crate) normal_values_revisions: crate::desk_stamp::DeskStamp,
    /// Cheap write stamp for the pending fixture and Group values prepared by Preload.
    pub(crate) preload_values_generations: crate::desk_stamp::DeskStamp,
    /// Monotonic public projection revision for pending Preload values.
    pub(crate) preload_values_revisions: crate::desk_stamp::DeskStamp,
    /// Cheap per-user stamp for the ordered pending Preload playback queue.
    pub(crate) preload_playback_queue_generations: crate::desk_stamp::DeskStamp,
    /// Monotonic public projection revision for the pending Preload playback queue.
    pub(crate) preload_playback_queue_revisions: crate::desk_stamp::DeskStamp,
    /// Runtime-only public revision for the exact capture-mode tuple. Domain helpers never
    /// advance it; the Programming application boundary advances it once per semantic tuple
    /// transition after all nested mutations and reconciliation have completed.
    pub(crate) capture_mode_revisions: crate::desk_stamp::DeskStamp,
    /// Monotonic public revision for the lightweight per-user Programmer priority authority.
    /// Priority changes intentionally do not advance the normal-values generation because that
    /// projection excludes interaction metadata.
    pub(crate) priority_revisions: crate::desk_stamp::DeskStamp,
    /// Timestamp paired with `priority_revisions`. General Programmer activity must never change
    /// this value because priority clients reconcile it under that independent revision.
    pub(crate) priority_changed_at: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
    /// Serializes compound mutations on the desk's one Programmer. The mutex is reentrant
    /// because public mutation helpers compose other public helpers (for example,
    /// `activate_preload` calls `activate_preload_at`).
    pub(crate) mutation_gate: Arc<ReentrantMutex<()>>,
    /// The one Programmer this desk has. Every session binds to it, whatever identity the session
    /// arrived holding, so the command line, selection and values converge across every screen,
    /// OSC client and attached hardware surface.
    pub(crate) desk: crate::DeskAuthority,
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
            alignment_contexts: Arc::default(),
            selection_revision: Arc::default(),
            alignment_revision: Arc::default(),
            programmer_order: Arc::default(),
            normal_values_generations: crate::desk_stamp::DeskStamp::default(),
            normal_values_revisions: crate::desk_stamp::DeskStamp::default(),
            preload_values_generations: crate::desk_stamp::DeskStamp::default(),
            preload_values_revisions: crate::desk_stamp::DeskStamp::default(),
            preload_playback_queue_generations: crate::desk_stamp::DeskStamp::default(),
            preload_playback_queue_revisions: crate::desk_stamp::DeskStamp::default(),
            capture_mode_revisions: crate::desk_stamp::DeskStamp::default(),
            priority_revisions: crate::desk_stamp::DeskStamp::default(),
            priority_changed_at: Arc::default(),
            mutation_gate: Arc::new(ReentrantMutex::new(())),
            desk: crate::DeskAuthority::default(),
            clock,
        }
    }

    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.clock)
    }

    /// The one Programmer this desk operates.
    pub fn desk(&self) -> &crate::DeskAuthority {
        &self.desk
    }

    /// The one interaction context every surface of this desk shares, once a surface exists.
    ///
    /// The desk's command line, ordered selection and Align state live here. `None` before the
    /// first connection, which is a real answer: there is nothing yet to publish a change to.
    pub fn desk_interaction_context(&self) -> Option<SessionId> {
        self.desk.settled_command_context()
    }

    /// Serialize a complete application-level transition on the desk's one Programmer.
    ///
    /// The gate is the same reentrant boundary used by every registry mutator, so callers may
    /// capture state, compose existing mutation helpers, and publish the final projection without
    /// another surface interleaving a write. Application services must acquire this gate before
    /// any desk-interaction gate.
    pub fn with_user_serialized<R>(&self, _user_id: UserId, operation: impl FnOnce() -> R) -> R {
        let _guard = self.mutation_gate.lock();
        operation()
    }

    /// Serialize one transition on the desk's Programmer.
    ///
    /// The desk has one, so the set of identities a caller names no longer selects which gates to
    /// take — but real concurrent writes from several connections are still serialized here.
    pub fn with_users_serialized<R>(
        &self,
        _users: impl IntoIterator<Item = UserId>,
        operation: impl FnOnce() -> R,
    ) -> R {
        let _guard = self.mutation_gate.lock();
        operation()
    }

    pub(crate) fn mutation_gate(&self, _session: SessionId) -> Arc<ReentrantMutex<()>> {
        Arc::clone(&self.mutation_gate)
    }

    /// Run an operation while the desk's mutation gate is held.
    pub(crate) fn with_all_mutation_gates<R>(&self, operation: impl FnOnce() -> R) -> R {
        let _guard = self.mutation_gate.lock();
        operation()
    }

    pub fn set_priority(&self, session: SessionId, priority: i16) -> bool {
        self.update_priority(session, priority).is_some()
    }

    /// Updates the desk's Programmer priority without materializing a normal-values projection.
    ///
    /// `None` means the session is absent, `Some(false)` is an exact semantic no-op, and
    /// `Some(true)` means the priority and the priority stamped onto retained values changed.
    pub fn update_priority(&self, session: SessionId, priority: i16) -> Option<bool> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.states.write();
        let state = states.get_mut(&self.key(session))?;
        if state.priority == priority {
            return Some(false);
        }
        state.priority = priority;
        // Each of these is its own shared buffer, so they are restamped one at a time rather
        // than through one chained borrow of the whole state.
        for value in Arc::make_mut(&mut state.values).iter_mut() {
            value.priority = priority;
        }
        for value in state.preload_pending.iter_mut() {
            value.priority = priority;
        }
        for value in Arc::make_mut(&mut state.preload_active).iter_mut() {
            value.priority = priority;
        }
        for action in Arc::make_mut(&mut state.transient_values).iter_mut() {
            for value in action.values.iter_mut() {
                value.priority = priority;
            }
        }
        let changed_at = self.clock.now();
        state.last_activity = changed_at;
        drop(states);
        *self.priority_changed_at.write() = Some(changed_at);
        Some(true)
    }

    /// Reset a fresh runtime during startup or a test-bench rebuild.
    ///
    /// Live Programmer deletion must use [`Self::clear`], which preserves public projection
    /// revisions so an old client cursor can never become current again.
    pub fn reset_all(&self) {
        self.with_all_mutation_gates(|| {
            self.states.write().clear();
            self.sessions.write().clear();
            self.desk.release();
            self.command_contexts.write().clear();
            self.command_states.write().clear();
            self.selection_contexts.write().clear();
            self.alignment_contexts.write().clear();
            self.selection_revision.store(0, Ordering::Relaxed);
            self.alignment_revision.store(0, Ordering::Relaxed);
            self.programmer_order.store(0, Ordering::Relaxed);
            self.normal_values_generations.clear();
            self.normal_values_revisions.clear();
            self.preload_values_generations.clear();
            self.preload_values_revisions.clear();
            self.preload_playback_queue_generations.clear();
            self.preload_playback_queue_revisions.clear();
            self.capture_mode_revisions.clear();
            self.priority_revisions.clear();
            *self.priority_changed_at.write() = None;
        });
    }

    pub fn normal_values_generation(&self, session: SessionId) -> Option<u64> {
        // Present only when the desk knows the session; the stamp itself is the desk's.
        self.states.read().get(&self.key(session))?;
        Some(self.normal_values_generation_for_user())
    }

    pub(crate) fn normal_values_generation_for_user(&self) -> u64 {
        self.normal_values_generations.get()
    }

    /// Whether this session operates the Programmer the presented identity names.
    ///
    /// A desk has one Programmer, so every session it knows operates it. An identity presented by
    /// an older client, saved hardware configuration, or a stored URL normalises to the desk's own
    /// rather than being rejected as foreign. `None` when the desk does not know the session at
    /// all, which remains a real answer: nothing is being operated.
    ///
    /// This is the one place legacy identities are accepted, and therefore the one place to change
    /// when they stop being accepted at all.
    pub fn session_operates_desk(&self, session: SessionId, presented: UserId) -> Option<bool> {
        let owner = self.user_id(session)?;
        Some(owner == self.desk.normalize(presented))
    }

    /// The desk's Programmer identity for a session, given whatever identity it presented.
    ///
    /// Unlike `operated_desk_user` this always answers, because callers use it to key state
    /// before they have established that the session exists at all; a session the desk does not
    /// know simply reads the desk's identity and then fails its own existence check.
    pub fn desk_user_for(&self, session: SessionId, presented: UserId) -> UserId {
        self.user_id(session)
            .unwrap_or(self.desk.normalize(presented))
    }

    /// The desk identity this session operates, given whatever identity it presented.
    ///
    /// `Some` whenever the desk knows the session and the presented identity resolves to the
    /// desk's own — which, with one Programmer, is every identity. Callers should read and report
    /// the returned identity rather than the presented one: a legacy identity names no state.
    pub fn operated_desk_user(&self, session: SessionId, presented: UserId) -> Option<UserId> {
        let owner = self.user_id(session)?;
        (owner == self.desk.normalize(presented)).then_some(owner)
    }

    pub fn user_id(&self, session: SessionId) -> Option<UserId> {
        self.states
            .read()
            .get(&self.key(session))
            .map(|state| state.user_id)
    }

    /// Reads only lightweight priority authority; retained Programmer values are never cloned.
    pub fn priority_state(
        &self,
        session: SessionId,
    ) -> Option<(UserId, i16, chrono::DateTime<chrono::Utc>)> {
        let states = self.states.read();
        let state = states.get(&self.key(session))?;
        let changed_at = self.priority_changed_at.read().as_ref().copied()?;
        Some((state.user_id, state.priority, changed_at))
    }

    pub fn normal_values_revision(&self) -> u64 {
        self.normal_values_revisions.get()
    }

    pub fn advance_normal_values_revision(&self) -> u64 {
        self.normal_values_revisions.advance()
    }

    pub fn priority_revision(&self) -> u64 {
        self.priority_revisions.get()
    }

    pub fn advance_priority_revision(&self) -> u64 {
        self.priority_revisions.advance()
    }

    pub fn preload_values_generation(&self, session: SessionId) -> Option<u64> {
        // Present only when the desk knows the session; the stamp itself is the desk's.
        self.states.read().get(&self.key(session))?;
        Some(self.preload_values_generation_for_user())
    }

    pub(crate) fn preload_values_generation_for_user(&self) -> u64 {
        self.preload_values_generations.get()
    }

    pub fn preload_values_revision(&self) -> u64 {
        self.preload_values_revisions.get()
    }

    pub fn advance_preload_values_revision(&self) -> u64 {
        self.preload_values_revisions.advance()
    }

    pub fn preload_playback_queue_generation(&self, session: SessionId) -> Option<u64> {
        // Present only when the desk knows the session; the stamp itself is the desk's.
        self.states.read().get(&self.key(session))?;
        Some(self.preload_playback_queue_generation_for_user())
    }

    pub(crate) fn preload_playback_queue_generation_for_user(&self) -> u64 {
        self.preload_playback_queue_generations.get()
    }

    pub fn preload_playback_queue_revision(&self) -> u64 {
        self.preload_playback_queue_revisions.get()
    }

    pub fn advance_preload_playback_queue_revision(&self) -> u64 {
        self.preload_playback_queue_revisions.advance()
    }

    pub fn capture_mode_revision(&self) -> u64 {
        self.capture_mode_revisions.get()
    }

    pub fn advance_capture_mode_revision(&self) -> u64 {
        self.capture_mode_revisions.advance()
    }

    pub(crate) fn mark_normal_values_changed(&self) {
        self.normal_values_generations.advance();
    }

    pub(crate) fn mark_preload_values_changed(&self) {
        self.preload_values_generations.advance();
    }

    pub(crate) fn mark_preload_playback_queue_changed(&self) {
        self.preload_playback_queue_generations.advance();
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
        let normal_values_changed = !state.values.is_empty()
            || !state.group_values.is_empty()
            || !state.dynamic_values.is_empty();
        state.checkpoint();
        Arc::make_mut(&mut state.values).clear();
        Arc::make_mut(&mut state.transient_values).clear();
        Arc::make_mut(&mut state.group_values).clear();
        Arc::make_mut(&mut state.dynamic_values).clear();
        state.last_activity = self.clock.now();
        drop(states);
        if normal_values_changed {
            self.mark_normal_values_changed();
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
        let Some(state) = self.states.write().remove(&key) else {
            return false;
        };
        if !state.values.is_empty()
            || !state.group_values.is_empty()
            || !state.dynamic_values.is_empty()
        {
            self.mark_normal_values_changed();
        }
        if !state.preload_pending.is_empty()
            || !state.preload_group_pending.is_empty()
            || !state.preload_dynamic_pending.is_empty()
        {
            self.mark_preload_values_changed();
        }
        if !state.preload_playback_pending.is_empty() {
            self.mark_preload_playback_queue_changed();
        }
        self.advance_priority_revision();
        *self.priority_changed_at.write() = None;
        true
    }
    pub fn active(&self) -> Vec<ProgrammerState> {
        self.states.read().values().cloned().collect()
    }
    pub fn active_output_states(&self) -> Vec<ProgrammerOutputState> {
        self.states
            .read()
            .values()
            // Reference counts, not copies. A render reads this every frame and the operator's
            // programming can be the whole show.
            .map(|state| ProgrammerOutputState {
                id: state.id,
                priority: state.priority,
                values: Arc::clone(&state.values),
                transient_values: Arc::clone(&state.transient_values),
                group_values: Arc::clone(&state.group_values),
                preload_active: Arc::clone(&state.preload_active),
                preload_group_active: Arc::clone(&state.preload_group_active),
            })
            .collect()
    }
    pub fn active_dynamic_sources_for_sessions(&self) -> Vec<ActiveDynamicSessionSource> {
        let states = self.states.read();
        let sessions = self.sessions.read();
        let mut active_programmers = HashSet::new();
        sessions
            .values()
            .filter(|key| active_programmers.insert(**key))
            .filter_map(|key| states.get(key))
            .map(|state| {
                (
                    state.id.0,
                    state.priority,
                    Arc::clone(&state.dynamic_values),
                    Arc::clone(&state.preload_dynamic_active),
                )
            })
            .collect()
    }
    pub fn active_for_sessions(&self) -> Vec<ProgrammerState> {
        self.active_sessions_for_user(None)
    }
    pub fn active_for_user_sessions(&self, user_id: UserId) -> Vec<ProgrammerState> {
        self.active_sessions_for_user(Some(user_id))
    }
    fn active_sessions_for_user(&self, user_id: Option<UserId>) -> Vec<ProgrammerState> {
        let states = self.states.read();
        let command_contexts = self.command_contexts.read();
        let command_states = self.command_states.read();
        let selection_contexts = self.selection_contexts.read();
        self.sessions
            .read()
            .iter()
            .filter_map(|(session, key)| {
                let source = states.get(key)?;
                if user_id.is_some_and(|user_id| source.user_id != user_id) {
                    return None;
                }
                let mut state = source.clone();
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
