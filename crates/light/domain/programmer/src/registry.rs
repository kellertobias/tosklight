use crate::alignment::ProgrammerAlignmentState;
use crate::command_state::CommandLineState;
use crate::selection::{ProgrammerSelection, SelectionContext};
use crate::state::{ProgrammerOutputState, ProgrammerState};
use light_core::{SessionId, SharedClock, SystemClock};
use parking_lot::{ReentrantMutex, RwLock};
use std::collections::HashSet;
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
    /// The desk's one Programmer. `None` until a surface has connected.
    ///
    /// These were maps keyed by the session that connected, from when a desk could hold a
    /// Programmer per operator. Every one of them held a single entry.
    pub(crate) state: Arc<RwLock<Option<ProgrammerState>>>,
    /// Every window currently connected to it. Legitimately plural: a desk drives its main
    /// window, its optional screens, its OSC clients and its attached hardware at once.
    pub(crate) sessions: Arc<RwLock<HashSet<SessionId>>>,
    pub(crate) command_state: Arc<RwLock<CommandLineState>>,
    pub(crate) selection_context: Arc<RwLock<SelectionContext>>,
    pub(crate) alignment_context: Arc<RwLock<Option<ProgrammerAlignmentState>>>,
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
    /// Cheap write stamp for the ordered pending Preload playback queue.
    pub(crate) preload_playback_queue_generations: crate::desk_stamp::DeskStamp,
    /// Monotonic public projection revision for the pending Preload playback queue.
    pub(crate) preload_playback_queue_revisions: crate::desk_stamp::DeskStamp,
    /// Runtime-only public revision for the exact capture-mode tuple. Domain helpers never
    /// advance it; the Programming application boundary advances it once per semantic tuple
    /// transition after all nested mutations and reconciliation have completed.
    pub(crate) capture_mode_revisions: crate::desk_stamp::DeskStamp,
    /// Monotonic public revision for the lightweight Programmer priority authority.
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
            state: Arc::default(),
            sessions: Arc::default(),
            command_state: Arc::default(),
            selection_context: Arc::default(),
            alignment_context: Arc::default(),
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
    ///
    /// This used to take the identity, or the set of identities, whose gates to hold. The desk has
    /// one Programmer and one gate; real concurrent writes from several connections are still
    /// serialized, there is simply nothing left to name.
    pub fn serialized<R>(&self, operation: impl FnOnce() -> R) -> R {
        let _guard = self.mutation_gate.lock();
        operation()
    }

    pub(crate) fn mutation_gate(&self) -> Arc<ReentrantMutex<()>> {
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
    pub fn update_priority(&self, _session: SessionId, priority: i16) -> Option<bool> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.state.write();
        let state = states.as_mut()?;
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
            *self.state.write() = None;
            self.sessions.write().clear();
            self.desk.release();
            *self.command_state.write() = CommandLineState::default();
            *self.selection_context.write() = SelectionContext::default();
            *self.alignment_context.write() = None;
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

    pub fn normal_values_generation(&self, _session: SessionId) -> Option<u64> {
        // Present only when the desk knows the session; the stamp itself is the desk's.
        self.state.read().as_ref()?;
        Some(self.normal_values_generation_for_user())
    }

    pub(crate) fn normal_values_generation_for_user(&self) -> u64 {
        self.normal_values_generations.get()
    }

    /// Whether the desk knows this session, and therefore has a Programmer for it to operate.
    ///
    /// This used to ask whether the session operated the Programmer a presented identity named.
    /// A desk has one Programmer, so every session it knows operates it and the identity part of
    /// the question could only ever answer yes — leaving an unreachable "belongs to another user"
    /// error behind every call. What is left is the half that can still be false: the desk may
    /// not know the session at all.
    pub fn knows_session(&self, _session: SessionId) -> bool {
        self.state.read().is_some()
    }

    /// Reads only lightweight priority authority; retained Programmer values are never cloned.
    pub fn priority_state(
        &self,
        _session: SessionId,
    ) -> Option<(i16, chrono::DateTime<chrono::Utc>)> {
        let states = self.state.read();
        let state = states.as_ref()?;
        let changed_at = self.priority_changed_at.read().as_ref().copied()?;
        Some((state.priority, changed_at))
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

    pub fn preload_values_generation(&self, _session: SessionId) -> Option<u64> {
        // Present only when the desk knows the session; the stamp itself is the desk's.
        self.state.read().as_ref()?;
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

    pub fn preload_playback_queue_generation(&self, _session: SessionId) -> Option<u64> {
        // Present only when the desk knows the session; the stamp itself is the desk's.
        self.state.read().as_ref()?;
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
        _session: SessionId,
        blind: Option<bool>,
        preview: Option<bool>,
        highlight: Option<bool>,
        active_context: Option<Option<String>>,
    ) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
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
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
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
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        self.sessions.write().remove(&session);
        let still_connected = !self.sessions.read().is_empty();
        if let Some(state) = self.state.write().as_mut() {
            state.connected = still_connected;
        }
    }
    pub fn connect(&self, _session: SessionId) {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        if let Some(state) = self.state.write().as_mut() {
            state.connected = true;
            state.last_activity = self.clock.now();
        }
    }
    pub fn clear(&self, _session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        self.sessions.write().clear();
        let Some(state) = self.state.write().take() else {
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
    /// The desk's Programmer, as a collection because output and projection callers iterate.
    pub fn active(&self) -> Vec<ProgrammerState> {
        self.state.read().iter().cloned().collect()
    }
    pub fn active_output_states(&self) -> Vec<ProgrammerOutputState> {
        self.state
            .read()
            .iter()
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
        if self.sessions.read().is_empty() {
            return Vec::new();
        }
        self.state
            .read()
            .iter()
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

    /// The desk's Programmer as each connected surface sees it.
    ///
    /// One Programmer, one command line and one selection; the rows differ only in the session
    /// each is reported under, which is what a compatibility caller keys on.
    pub fn active_for_sessions(&self) -> Vec<ProgrammerState> {
        let Some(source) = self.state.read().clone() else {
            return Vec::new();
        };
        let command_line = self.command_state.read().legacy_text().to_owned();
        let selection = self.selection_context.read().clone();
        self.sessions
            .read()
            .iter()
            .map(|session| {
                let mut state = source.clone();
                state.session_id = *session;
                state.command_line = command_line.clone();
                state.selected = selection.selected.clone();
                state.selection_expression = selection.expression.clone();
                state
            })
            .collect()
    }

    pub fn get(&self, session: SessionId) -> Option<ProgrammerState> {
        // Staged publication acquires these write locks in the same order. Holding all three read
        // guards while building a projection guarantees an old or new result, never a torn mix.
        let stored = self.state.read();
        let command = self.command_state.read();
        let selection = self.selection_context.read();
        let mut state = stored.clone()?;
        state.session_id = session;
        state.command_line = command.legacy_text().to_owned();
        state.selected = selection.selected.clone();
        state.selection_expression = selection.expression.clone();
        Some(state)
    }

    pub fn selection(&self, _session: SessionId) -> Option<ProgrammerSelection> {
        let selection = self.selection_context.read();
        Some(ProgrammerSelection {
            selected: selection.selected.clone(),
            expression: selection.expression.clone(),
            revision: selection.revision,
            gesture_open: selection.gesture_open,
        })
    }
}
