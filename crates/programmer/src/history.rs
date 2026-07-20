use crate::selection::SelectionContext;
use crate::{ProgrammerRegistry, ProgrammerSnapshot, ProgrammerState};
use chrono::{DateTime, Utc};
use light_core::SessionId;
use std::sync::Arc;

pub(crate) const HISTORY_LIMIT: usize = 100;

impl ProgrammerState {
    pub(crate) fn snapshot(&self) -> ProgrammerSnapshot {
        ProgrammerSnapshot {
            selected: self.selected.clone(),
            selection_expression: self.selection_expression.clone(),
            values: self.values.clone(),
            group_values: self.group_values.clone(),
            preload_pending: self.preload_pending.clone(),
            preload_active: self.preload_active.clone(),
            preload_group_pending: self.preload_group_pending.clone(),
            preload_group_active: self.preload_group_active.clone(),
            preload_playback_pending: self.preload_playback_pending.clone(),
            command_line: self.command_line.clone(),
            blind: self.blind,
            preload_capture_programmer: self.preload_capture_programmer,
            preview: self.preview,
            active_context: self.active_context.clone(),
        }
    }

    pub(crate) fn restore_snapshot(&mut self, snapshot: ProgrammerSnapshot, now: DateTime<Utc>) {
        self.selected = snapshot.selected;
        self.selection_expression = snapshot.selection_expression;
        self.values = snapshot.values;
        self.group_values = snapshot.group_values;
        self.preload_pending = snapshot.preload_pending;
        self.preload_active = snapshot.preload_active;
        self.preload_group_pending = snapshot.preload_group_pending;
        self.preload_group_active = snapshot.preload_group_active;
        self.preload_playback_pending = snapshot.preload_playback_pending;
        self.command_line = snapshot.command_line;
        self.blind = snapshot.blind;
        self.preload_capture_programmer = snapshot.preload_capture_programmer;
        self.preview = snapshot.preview;
        self.highlight = false;
        self.active_context = snapshot.active_context;
        self.last_activity = now;
    }

    pub(crate) fn checkpoint(&mut self) {
        self.undo.push(Arc::new(self.snapshot()));
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
}

impl ProgrammerRegistry {
    pub fn undo(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let (selected, expression, user_id, values_changed, preload_values_changed, queue_changed) = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return false;
            };
            let Some(previous) = state.undo.pop() else {
                return false;
            };
            let values_changed =
                state.values != previous.values || state.group_values != previous.group_values;
            let preload_values_changed = state.preload_pending != previous.preload_pending
                || state.preload_group_pending != previous.preload_group_pending;
            let queue_changed = state.preload_playback_pending != previous.preload_playback_pending;
            state.redo.push(Arc::new(state.snapshot()));
            state.restore_snapshot(Arc::unwrap_or_clone(previous), self.clock.now());
            (
                state.selected.clone(),
                state.selection_expression.clone(),
                state.user_id,
                values_changed,
                preload_values_changed,
                queue_changed,
            )
        };
        self.selection_contexts.write().insert(
            self.command_context(session),
            SelectionContext {
                selected,
                expression,
                revision: self.next_selection_revision(),
                gesture_open: false,
            },
        );
        if values_changed {
            self.mark_normal_values_changed(user_id);
        }
        if preload_values_changed {
            self.mark_preload_values_changed(user_id);
        }
        if queue_changed {
            self.mark_preload_playback_queue_changed(user_id);
        }
        true
    }
    pub fn redo(&self, session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let (selected, expression, user_id, values_changed, preload_values_changed, queue_changed) = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return false;
            };
            let Some(next) = state.redo.pop() else {
                return false;
            };
            let values_changed =
                state.values != next.values || state.group_values != next.group_values;
            let preload_values_changed = state.preload_pending != next.preload_pending
                || state.preload_group_pending != next.preload_group_pending;
            let queue_changed = state.preload_playback_pending != next.preload_playback_pending;
            state.undo.push(Arc::new(state.snapshot()));
            state.restore_snapshot(Arc::unwrap_or_clone(next), self.clock.now());
            (
                state.selected.clone(),
                state.selection_expression.clone(),
                state.user_id,
                values_changed,
                preload_values_changed,
                queue_changed,
            )
        };
        self.selection_contexts.write().insert(
            self.command_context(session),
            SelectionContext {
                selected,
                expression,
                revision: self.next_selection_revision(),
                gesture_open: false,
            },
        );
        if values_changed {
            self.mark_normal_values_changed(user_id);
        }
        if preload_values_changed {
            self.mark_preload_values_changed(user_id);
        }
        if queue_changed {
            self.mark_preload_playback_queue_changed(user_id);
        }
        true
    }
}
