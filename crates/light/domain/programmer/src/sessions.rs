use crate::command_state::{CommandLineState, CommandTarget, canonical_command_text};
use crate::selection::SelectionContext;
use crate::{ProgrammerRegistry, ProgrammerState};
use light_core::{ProgrammerId, SessionId};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

impl ProgrammerRegistry {
    pub fn start(&self, session_id: SessionId) -> ProgrammerState {
        let mutation_gate = std::sync::Arc::clone(&self.mutation_gate);
        let _mutation_guard = mutation_gate.lock();
        self.priority_changed_at
            .write()
            .get_or_insert_with(|| self.clock.now());
        // One desk, one Programmer. A session joins the Programmer the desk already has rather
        // than opening a second one beside it.
        self.sessions.write().insert(session_id);
        let _command_context = self.command_context(session_id);
        if let Some(state) = self.state.write().as_mut() {
            state.connected = true;
            state.last_activity = self.clock.now();
            let mut projected = state.clone();
            projected.session_id = session_id;
            projected.command_line = self.command_state.read().legacy_text().to_owned();
            self.project_selection(&mut projected);
            return projected;
        }
        let state = ProgrammerState {
            id: ProgrammerId::new(),
            session_id,
            priority: 100,
            selected: vec![],
            selection_expression: None,
            values: Arc::new(vec![]),
            dynamic_values: Arc::new(vec![]),
            transient_values: Arc::new(vec![]),
            group_values: Arc::new(HashMap::new()),
            group_release_values: vec![],
            preload_pending: vec![],
            preload_active: Arc::new(vec![]),
            preload_dynamic_pending: Arc::new(vec![]),
            preload_dynamic_active: Arc::new(vec![]),
            preload_group_pending: HashMap::new(),
            preload_group_active: Arc::new(HashMap::new()),
            preload_group_release_pending: vec![],
            preload_group_release_active: vec![],
            preload_playback_pending: vec![],
            preload_playback_active: false,
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
            active_value_undo_group: None,
        };
        *self.state.write() = Some(state.clone());
        state
    }
    /// Hydrate one persisted session while constructing a fresh runtime.
    ///
    /// Multiple persisted sessions for the same user intentionally collapse into one shared
    /// Programmer. Existing public authority revisions are retained so an incidental repeated
    /// restore cannot make a live client revision current again.
    pub fn restore(&self, state: ProgrammerState) {
        let mutation_gate = std::sync::Arc::clone(&self.mutation_gate);
        let _mutation_guard = mutation_gate.lock();
        self.priority_changed_at
            .write()
            .get_or_insert_with(|| self.clock.now());
        let restored_order = state
            .values
            .iter()
            .chain(&state.preload_pending)
            .chain(state.preload_active.iter())
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
        *self.selection_context.write() = SelectionContext {
            selected: state.selected.clone(),
            expression: state.selection_expression.clone(),
            revision: self.next_selection_revision(),
            gesture_open: false,
        };
        self.desk.command_context(session_id);
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
        *self.command_state.write() = CommandLineState {
            text: canonical_command_text(state.command_line.clone(), pristine),
            target,
            pristine,
            revision: 0,
            pending_choice: None,
        };
        // Persisted session ids retain their durable interaction snapshots, but they are not live
        // connections after a process restart. Only `start` may add a session to `self.sessions`;
        // otherwise every historical browser session is projected as connected and its desk-local
        // selection is added to lifecycle counts (and formerly duplicated output-side work).
        let existing_session = self.state.read().as_ref().map(|state| state.session_id);
        let mut shared = state;
        if let Some(existing) = existing_session {
            shared.session_id = existing;
        }
        shared.command_line.clear();
        *self.state.write() = Some(shared);
    }
    /// The interaction context this session operates.
    ///
    /// One desk has one of these. Every screen, OSC client, hardware surface and keyboard shares
    /// the command line, command target, selection gesture and Align state it holds, so the answer
    /// does not depend on which connection is asking.
    pub(crate) fn command_context(&self, session: SessionId) -> SessionId {
        self.desk.command_context(session)
    }

    pub(crate) fn project_selection(&self, state: &mut ProgrammerState) {
        let selection = self.selection_context.read();
        state.selected = selection.selected.clone();
        state.selection_expression = selection.expression.clone();
    }

    pub(crate) fn close_selection_gesture(&self, _session: SessionId) -> bool {
        let revision = self.next_selection_revision();
        let mut selection = self.selection_context.write();
        if selection.gesture_open {
            selection.gesture_open = false;
            selection.revision = revision;
            return true;
        }
        false
    }

    /// Finish the current desk-local sequence of ordinary selection presses without clearing its
    /// visible selection. Recording a target uses this boundary so the next fixture or Group press
    /// starts a fresh selection while the just-recorded source remains inspectable.
    pub fn finish_selection_gesture(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session)
    }

    /// Bind a controller session to the desk's one interaction context.
    ///
    /// The desk has a single command line, selection gesture and Align state, so a surface asking
    /// for a particular context is already where it belongs. The call remains because saved
    /// hardware configuration and existing clients still make it against a desk that no longer has
    /// contexts to choose between.
    pub fn attach_command_context(&self, session: SessionId, _context: SessionId) -> bool {
        self.sessions.read().contains(&session)
    }
}
