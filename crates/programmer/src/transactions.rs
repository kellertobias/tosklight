use crate::command_state::CommandLineState;
use crate::history::HISTORY_LIMIT;
use crate::selection::SelectionContext;
use crate::{ProgrammerRegistry, ProgrammerState};
use light_core::SessionId;
use parking_lot::{ReentrantMutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;

/// Opaque in-memory checkpoint used to roll back one application command that failed validation.
/// Persistence and transports never serialize this value.
#[derive(Clone)]
pub struct ProgrammerTransactionSnapshot {
    state_key: SessionId,
    state: ProgrammerState,
    normal_values_generation: u64,
    preload_values_generation: u64,
    preload_playback_queue_generation: u64,
    priority_changed_at: chrono::DateTime<chrono::Utc>,
    interaction_context: SessionId,
    selection: Option<SelectionContext>,
    command_line: Option<CommandLineState>,
}

impl ProgrammerRegistry {
    /// Execute a fallible compound Programmer mutation atomically for one user.
    ///
    /// Every public mutator uses this same reentrant per-user gate, so a transaction may freely
    /// compose existing registry operations. On rejection, only the user's Programmer state and
    /// the initiating desk's selection/command interaction are restored; a mutation waiting on
    /// the gate then runs against that restored state instead of being overwritten by rollback.
    pub fn with_transaction<T, E, F>(&self, session: SessionId, transaction: F) -> Result<T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let snapshot = self.transaction_snapshot(session);
        match transaction() {
            Ok(value) => Ok(value),
            Err(error) => {
                if let Some(snapshot) = snapshot {
                    self.restore_transaction_snapshot(snapshot);
                }
                Err(error)
            }
        }
    }

    /// Execute a compound command against an isolated copy and publish its complete Programmer
    /// and desk-interaction result in one commit.
    ///
    /// Readers continue to observe the previous live state while `transaction` runs. The
    /// per-user mutation gate prevents another writer from racing the final commit, while global
    /// order/revision counters remain shared so staged work cannot duplicate identities used by
    /// another user's concurrent command.
    pub fn with_staged_transaction<T, E, F>(
        &self,
        session: SessionId,
        transaction: F,
    ) -> Result<T, E>
    where
        E: From<String>,
        F: FnOnce(&ProgrammerRegistry) -> Result<T, E>,
    {
        self.with_staged_transaction_internal(session, false, transaction)
    }

    /// Stage one entered Programmer command and collapse all of its internal helper checkpoints
    /// into one operator-visible Undo step.
    pub fn with_staged_command<T, E, F>(&self, session: SessionId, transaction: F) -> Result<T, E>
    where
        E: From<String>,
        F: FnOnce(&ProgrammerRegistry) -> Result<T, E>,
    {
        self.with_staged_transaction_internal(session, true, transaction)
    }

    fn with_staged_transaction_internal<T, E, F>(
        &self,
        session: SessionId,
        squash_command_history: bool,
        transaction: F,
    ) -> Result<T, E>
    where
        E: From<String>,
        F: FnOnce(&ProgrammerRegistry) -> Result<T, E>,
    {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let staged = self
            .detached_session(session)
            .ok_or_else(|| E::from("programmer does not exist".to_owned()))?;
        let staged_state_key = staged.key(session);
        let command_history = squash_command_history.then(|| {
            let states = staged.states.read();
            let state = states
                .get(&staged_state_key)
                .expect("a detached session retains its staged Programmer state");
            (state.undo.clone(), Arc::new(state.snapshot()))
        });
        let result = transaction(&staged)?;
        if let Some((undo_before, command_checkpoint)) = command_history {
            let mut states = staged.states.write();
            let state = states
                .get_mut(&staged_state_key)
                .ok_or_else(|| E::from("programmer does not exist".to_owned()))?;
            let history_changed = state.undo.len() != undo_before.len()
                || state
                    .undo
                    .iter()
                    .zip(&undo_before)
                    .any(|(after, before)| !Arc::ptr_eq(after, before));
            if history_changed {
                state.undo = undo_before;
                state.undo.push(command_checkpoint);
                if state.undo.len() > HISTORY_LIMIT {
                    state.undo.remove(0);
                }
            }
        }
        self.commit_detached_session(session, &staged)
            .then_some(result)
            .ok_or_else(|| E::from("programmer does not exist".to_owned()))
    }

    pub(crate) fn detached_session(&self, session: SessionId) -> Option<ProgrammerRegistry> {
        let state_key = self.key(session);
        let context = self.command_context(session);
        let state = self.states.read().get(&state_key)?.clone();
        let user_id = state.user_id;
        let normal_values_generation = self
            .normal_values_generations
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let normal_values_revision = self
            .normal_values_revisions
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let preload_values_generation = self
            .preload_values_generations
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let preload_values_revision = self
            .preload_values_revisions
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let preload_playback_queue_generation = self
            .preload_playback_queue_generations
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let preload_playback_queue_revision = self
            .preload_playback_queue_revisions
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let capture_mode_revision = self
            .capture_mode_revisions
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let priority_revision = self
            .priority_revisions
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let priority_changed_at = self
            .priority_changed_at
            .read()
            .get(&user_id)
            .cloned()
            .unwrap_or(state.last_activity);
        let selection = self.selection_contexts.read().get(&context).cloned();
        let command = self.command_states.read().get(&context).cloned();
        Some(ProgrammerRegistry {
            states: Arc::new(RwLock::new(HashMap::from([(state_key, state)]))),
            sessions: Arc::new(RwLock::new(HashMap::from([(session, state_key)]))),
            command_contexts: Arc::new(RwLock::new(HashMap::from([(session, context)]))),
            command_states: Arc::new(RwLock::new(
                command
                    .map(|command| HashMap::from([(context, command)]))
                    .unwrap_or_default(),
            )),
            selection_contexts: Arc::new(RwLock::new(
                selection
                    .map(|selection| HashMap::from([(context, selection)]))
                    .unwrap_or_default(),
            )),
            selection_revision: Arc::clone(&self.selection_revision),
            programmer_order: Arc::clone(&self.programmer_order),
            normal_values_generations: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                normal_values_generation,
            )]))),
            normal_values_revisions: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                normal_values_revision,
            )]))),
            preload_values_generations: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                preload_values_generation,
            )]))),
            preload_values_revisions: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                preload_values_revision,
            )]))),
            preload_playback_queue_generations: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                preload_playback_queue_generation,
            )]))),
            preload_playback_queue_revisions: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                preload_playback_queue_revision,
            )]))),
            capture_mode_revisions: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                capture_mode_revision,
            )]))),
            priority_revisions: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                priority_revision,
            )]))),
            priority_changed_at: Arc::new(RwLock::new(HashMap::from([(
                user_id,
                priority_changed_at,
            )]))),
            mutation_gates: Arc::default(),
            unknown_mutation_gate: Arc::new(ReentrantMutex::new(())),
            clock: Arc::clone(&self.clock),
        })
    }

    pub(crate) fn commit_detached_session(
        &self,
        session: SessionId,
        staged: &ProgrammerRegistry,
    ) -> bool {
        if !self.sessions.read().contains_key(&session) {
            return false;
        }
        let live_state_key = self.key(session);
        let staged_state_key = staged.key(session);
        let context = self.command_context(session);
        let Some(state) = staged.states.read().get(&staged_state_key).cloned() else {
            return false;
        };
        let user_id = state.user_id;
        let staged_values_generation = staged
            .normal_values_generations
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let staged_preload_values_generation = staged
            .preload_values_generations
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let staged_preload_playback_queue_generation = staged
            .preload_playback_queue_generations
            .read()
            .get(&user_id)
            .copied()
            .unwrap_or(0);
        let staged_priority_changed_at = staged
            .priority_changed_at
            .read()
            .get(&user_id)
            .cloned()
            .unwrap_or(state.last_activity);
        let selection = staged.selection_contexts.read().get(&context).cloned();
        let command = staged.command_states.read().get(&context).cloned();

        // Populate every replacement before releasing any write guard. A reader that needs more
        // than one projection either sees the complete previous set or waits and sees the complete
        // replacement set.
        let mut states = self.states.write();
        let mut commands = self.command_states.write();
        let mut selections = self.selection_contexts.write();
        states.insert(live_state_key, state);
        match command {
            Some(command) => {
                commands.insert(context, command);
            }
            None => {
                commands.remove(&context);
            }
        }
        match selection {
            Some(selection) => {
                selections.insert(context, selection);
            }
            None => {
                selections.remove(&context);
            }
        }
        drop(selections);
        drop(commands);
        drop(states);
        self.normal_values_generations
            .write()
            .insert(user_id, staged_values_generation);
        self.preload_values_generations
            .write()
            .insert(user_id, staged_preload_values_generation);
        self.preload_playback_queue_generations
            .write()
            .insert(user_id, staged_preload_playback_queue_generation);
        self.priority_changed_at
            .write()
            .insert(user_id, staged_priority_changed_at);
        true
    }

    /// Capture the complete user Programmer and desk interaction state before a fallible command.
    pub fn transaction_snapshot(
        &self,
        session: SessionId,
    ) -> Option<ProgrammerTransactionSnapshot> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains_key(&session) {
            return None;
        }
        let state_key = self.key(session);
        let interaction_context = self.command_context(session);
        let state = self.states.read().get(&state_key)?.clone();
        let normal_values_generation = self
            .normal_values_generations
            .read()
            .get(&state.user_id)
            .copied()
            .unwrap_or(0);
        let preload_values_generation = self
            .preload_values_generations
            .read()
            .get(&state.user_id)
            .copied()
            .unwrap_or(0);
        let preload_playback_queue_generation = self
            .preload_playback_queue_generations
            .read()
            .get(&state.user_id)
            .copied()
            .unwrap_or(0);
        let priority_changed_at = self
            .priority_changed_at
            .read()
            .get(&state.user_id)
            .cloned()
            .unwrap_or(state.last_activity);
        let selection = self
            .selection_contexts
            .read()
            .get(&interaction_context)
            .cloned();
        let command_line = self
            .command_states
            .read()
            .get(&interaction_context)
            .cloned();
        Some(ProgrammerTransactionSnapshot {
            state_key,
            state,
            normal_values_generation,
            preload_values_generation,
            preload_playback_queue_generation,
            priority_changed_at,
            interaction_context,
            selection,
            command_line,
        })
    }

    /// Restore an exact checkpoint after a command rejected without committing.
    pub fn restore_transaction_snapshot(&self, snapshot: ProgrammerTransactionSnapshot) {
        let mutation_gate = self.mutation_gate_for_user(snapshot.state.user_id);
        let _mutation_guard = mutation_gate.lock();
        let user_id = snapshot.state.user_id;
        self.states
            .write()
            .insert(snapshot.state_key, snapshot.state);
        self.normal_values_generations
            .write()
            .insert(user_id, snapshot.normal_values_generation);
        self.preload_values_generations
            .write()
            .insert(user_id, snapshot.preload_values_generation);
        self.preload_playback_queue_generations
            .write()
            .insert(user_id, snapshot.preload_playback_queue_generation);
        self.priority_changed_at
            .write()
            .insert(user_id, snapshot.priority_changed_at);
        let mut selections = self.selection_contexts.write();
        match snapshot.selection {
            Some(selection) => {
                selections.insert(snapshot.interaction_context, selection);
            }
            None => {
                selections.remove(&snapshot.interaction_context);
            }
        }
        drop(selections);
        let mut commands = self.command_states.write();
        match snapshot.command_line {
            Some(command_line) => {
                commands.insert(snapshot.interaction_context, command_line);
            }
            None => {
                commands.remove(&snapshot.interaction_context);
            }
        }
    }
}
