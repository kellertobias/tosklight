use crate::command_state::{CommandLineState, CommandTarget, canonical_command_text};
use crate::selection::SelectionContext;
use crate::{ProgrammerRegistry, ProgrammerState};
use light_core::{ProgrammerId, SessionId, UserId};
use std::collections::HashMap;
use std::sync::atomic::Ordering;

impl ProgrammerRegistry {
    pub fn start(&self, session_id: SessionId, user_id: UserId) -> ProgrammerState {
        let mutation_gate = self.mutation_gate_for_user(user_id);
        let _mutation_guard = mutation_gate.lock();
        self.priority_changed_at
            .write()
            .entry(user_id)
            .or_insert_with(|| self.clock.now());
        self.normal_values_generations
            .write()
            .entry(user_id)
            .or_default();
        self.normal_values_revisions
            .write()
            .entry(user_id)
            .or_default();
        self.preload_values_generations
            .write()
            .entry(user_id)
            .or_default();
        self.preload_values_revisions
            .write()
            .entry(user_id)
            .or_default();
        self.preload_playback_queue_generations
            .write()
            .entry(user_id)
            .or_default();
        self.preload_playback_queue_revisions
            .write()
            .entry(user_id)
            .or_default();
        self.capture_mode_revisions
            .write()
            .entry(user_id)
            .or_default();
        self.priority_revisions.write().entry(user_id).or_default();
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
            self.command_states
                .write()
                .entry(command_context)
                .or_default();
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
                .command_states
                .read()
                .get(&command_context)
                .map(|command| command.legacy_text().to_owned())
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
            transient_values: vec![],
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
        self.command_states
            .write()
            .entry(command_context)
            .or_default();
        self.selection_contexts
            .write()
            .entry(command_context)
            .or_default();
        state
    }
    /// Hydrate one persisted session while constructing a fresh runtime.
    ///
    /// Multiple persisted sessions for the same user intentionally collapse into one shared
    /// Programmer. Existing public authority revisions are retained so an incidental repeated
    /// restore cannot make a live client revision current again.
    pub fn restore(&self, state: ProgrammerState) {
        let mutation_gate = self.mutation_gate_for_user(state.user_id);
        let _mutation_guard = mutation_gate.lock();
        self.priority_changed_at
            .write()
            .entry(state.user_id)
            .or_insert_with(|| self.clock.now());
        self.normal_values_generations
            .write()
            .entry(state.user_id)
            .or_default();
        self.normal_values_revisions
            .write()
            .entry(state.user_id)
            .or_default();
        self.preload_values_generations
            .write()
            .entry(state.user_id)
            .or_default();
        self.preload_values_revisions
            .write()
            .entry(state.user_id)
            .or_default();
        self.preload_playback_queue_generations
            .write()
            .entry(state.user_id)
            .or_default();
        self.preload_playback_queue_revisions
            .write()
            .entry(state.user_id)
            .or_default();
        self.capture_mode_revisions
            .write()
            .entry(state.user_id)
            .or_default();
        self.priority_revisions
            .write()
            .entry(state.user_id)
            .or_default();
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
                revision: self.next_selection_revision(),
                gesture_open: false,
            },
        );
        self.command_contexts
            .write()
            .entry(session_id)
            .or_insert(session_id);
        let target = if state.command_line.trim().eq_ignore_ascii_case("GROUP") {
            CommandTarget::Group
        } else {
            CommandTarget::Fixture
        };
        let pristine = state.command_line.trim().is_empty()
            || state
                .command_line
                .trim()
                .eq_ignore_ascii_case(target.as_str());
        self.command_states.write().insert(
            session_id,
            CommandLineState {
                text: canonical_command_text(state.command_line.clone(), pristine),
                target,
                pristine,
                revision: 0,
                pending_choice: None,
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
    pub(crate) fn key(&self, session: SessionId) -> SessionId {
        self.sessions
            .read()
            .get(&session)
            .copied()
            .unwrap_or(session)
    }
    pub(crate) fn command_context(&self, session: SessionId) -> SessionId {
        self.command_contexts
            .read()
            .get(&session)
            .copied()
            .unwrap_or(session)
    }

    pub(crate) fn project_selection(&self, state: &mut ProgrammerState, context: SessionId) {
        let selections = self.selection_contexts.read();
        let selection = selections.get(&context);
        state.selected = selection
            .map(|selection| selection.selected.clone())
            .unwrap_or_default();
        state.selection_expression = selection.and_then(|selection| selection.expression.clone());
    }

    pub(crate) fn close_selection_gesture(&self, session: SessionId) -> bool {
        if let Some(selection) = self
            .selection_contexts
            .write()
            .get_mut(&self.command_context(session))
            && selection.gesture_open
        {
            selection.gesture_open = false;
            selection.revision = self.next_selection_revision();
            return true;
        }
        false
    }

    /// Finish the current desk-local sequence of ordinary selection presses without clearing its
    /// visible selection. Recording a target uses this boundary so the next fixture or Group press
    /// starts a fresh selection while the just-recorded source remains inspectable.
    pub fn finish_selection_gesture(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session)
    }

    /// Bind a controller session to the command interaction context for its desk.
    /// Programmer values remain shared by user identity, while button presses,
    /// partial command lines, selection gestures, and the active command target are shared only by
    /// sessions attached to this same context.
    pub fn attach_command_context(&self, session: SessionId, context: SessionId) -> bool {
        if self.sessions.read().contains_key(&session) && self.command_context(session) == context {
            return true;
        }
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains_key(&session) {
            return false;
        }
        let previous = self.command_context(session);
        if previous == context {
            return true;
        }

        let previous_command = self
            .command_states
            .read()
            .get(&previous)
            .cloned()
            .unwrap_or_default();
        let previous_selection = self
            .selection_contexts
            .read()
            .get(&previous)
            .cloned()
            .unwrap_or_default();
        let promote_previous = self
            .command_states
            .read()
            .get(&context)
            .is_none_or(|current| current.text.is_empty() && !previous_command.text.is_empty());

        self.command_contexts.write().insert(session, context);
        {
            let mut command_states = self.command_states.write();
            if promote_previous {
                command_states.insert(context, previous_command);
            } else {
                command_states.entry(context).or_default();
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
            self.command_states.write().remove(&previous);
            self.selection_contexts.write().remove(&previous);
        }
        true
    }
}
