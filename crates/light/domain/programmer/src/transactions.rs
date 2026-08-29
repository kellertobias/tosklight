use crate::command_state::CommandLineState;
use crate::history::HISTORY_LIMIT;
use crate::selection::SelectionContext;
use crate::{ProgrammerAlignmentState, ProgrammerRegistry, ProgrammerState};
use light_core::SessionId;
use parking_lot::{ReentrantMutex, RwLock};
use std::collections::HashSet;
use std::sync::Arc;

/// Opaque in-memory checkpoint used to roll back one application command that failed validation.
/// Persistence and transports never serialize this value.
#[derive(Clone)]
pub struct ProgrammerTransactionSnapshot {
    state: ProgrammerState,
    normal_values_generation: u64,
    preload_values_generation: u64,
    preload_playback_queue_generation: u64,
    priority_changed_at: chrono::DateTime<chrono::Utc>,
    selection: SelectionContext,
    command_line: CommandLineState,
    alignment: Option<ProgrammerAlignmentState>,
}

impl ProgrammerRegistry {
    /// Runs an operation against an isolated copy of one operator's Programmer without committing
    /// any of its mutations. This is the authoritative dry-run seam for callers that need the
    /// normal command grammar and current Programmer context during preflight validation.
    pub fn with_detached_transaction<T, E, F>(
        &self,
        session: SessionId,
        operation: F,
    ) -> Result<T, E>
    where
        E: From<String>,
        F: FnOnce(&ProgrammerRegistry) -> Result<T, E>,
    {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let detached = self
            .detached_session(session)
            .ok_or_else(|| E::from("programmer does not exist".to_owned()))?;
        operation(&detached)
    }

    /// Execute a fallible compound Programmer mutation atomically.
    ///
    /// Every public mutator uses this same reentrant gate, so a transaction may freely compose
    /// existing registry operations. On rejection, only the Programmer state and the initiating
    /// desk's selection/command interaction are restored; a mutation waiting on the gate then
    /// runs against that restored state instead of being overwritten by rollback.
    pub fn with_transaction<T, E, F>(&self, session: SessionId, transaction: F) -> Result<T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let mutation_gate = self.mutation_gate();
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
    /// mutation gate prevents another writer from racing the final commit, while global
    /// order/revision counters remain shared so staged work cannot duplicate identities used by
    /// a concurrent command from another surface.
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
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let staged = self
            .detached_session(session)
            .ok_or_else(|| E::from("programmer does not exist".to_owned()))?;
        let command_history = squash_command_history.then(|| {
            let state = staged.state.read();
            let state = state
                .as_ref()
                .expect("a detached session retains its staged Programmer state");
            (state.undo.clone(), Arc::new(state.snapshot()))
        });
        let result = transaction(&staged)?;
        if let Some((undo_before, command_checkpoint)) = command_history {
            let mut staged_state = staged.state.write();
            let state = staged_state
                .as_mut()
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
        let state = self.state.read().as_ref()?.clone();
        let normal_values_generation = self.normal_values_generations.get();
        let normal_values_revision = self.normal_values_revisions.get();
        let preload_values_generation = self.preload_values_generations.get();
        let preload_values_revision = self.preload_values_revisions.get();
        let preload_playback_queue_generation = self.preload_playback_queue_generations.get();
        let preload_playback_queue_revision = self.preload_playback_queue_revisions.get();
        let capture_mode_revision = self.capture_mode_revisions.get();
        let priority_revision = self.priority_revisions.get();
        let priority_changed_at = self
            .priority_changed_at
            .read()
            .unwrap_or(state.last_activity);
        let selection = self.selection_context.read().clone();
        let command = self.command_state.read().clone();
        let alignment = self.alignment_context.read().clone();
        Some(ProgrammerRegistry {
            state: Arc::new(RwLock::new(Some(state))),
            sessions: Arc::new(RwLock::new(HashSet::from([session]))),
            command_state: Arc::new(RwLock::new(command)),
            selection_context: Arc::new(RwLock::new(selection)),
            alignment_context: Arc::new(RwLock::new(alignment)),
            selection_revision: Arc::clone(&self.selection_revision),
            alignment_revision: Arc::clone(&self.alignment_revision),
            programmer_order: Arc::clone(&self.programmer_order),
            normal_values_generations: crate::desk_stamp::DeskStamp::seeded(
                normal_values_generation,
            ),
            normal_values_revisions: crate::desk_stamp::DeskStamp::seeded(normal_values_revision),
            preload_values_generations: crate::desk_stamp::DeskStamp::seeded(
                preload_values_generation,
            ),
            preload_values_revisions: crate::desk_stamp::DeskStamp::seeded(preload_values_revision),
            preload_playback_queue_generations: crate::desk_stamp::DeskStamp::seeded(
                preload_playback_queue_generation,
            ),
            preload_playback_queue_revisions: crate::desk_stamp::DeskStamp::seeded(
                preload_playback_queue_revision,
            ),
            capture_mode_revisions: crate::desk_stamp::DeskStamp::seeded(capture_mode_revision),
            priority_revisions: crate::desk_stamp::DeskStamp::seeded(priority_revision),
            priority_changed_at: Arc::new(RwLock::new(Some(priority_changed_at))),
            mutation_gate: Arc::new(ReentrantMutex::new(())),
            // A detached command suppresses command-line writes for the whole execution, staged
            // Programmer included: the staged command line is committed back over the live one.
            command_line_writes_suppressed: Arc::clone(&self.command_line_writes_suppressed),
            // The snapshot operates the same desk, so it inherits the same authority rather than
            // settling on one of its own.
            desk: self.desk.clone(),
            clock: Arc::clone(&self.clock),
        })
    }

    pub(crate) fn commit_detached_session(
        &self,
        session: SessionId,
        staged: &ProgrammerRegistry,
    ) -> bool {
        if !self.sessions.read().contains(&session) {
            return false;
        }
        let Some(state) = staged.state.read().as_ref().cloned() else {
            return false;
        };
        let staged_values_generation = staged.normal_values_generations.get();
        let staged_preload_values_generation = staged.preload_values_generations.get();
        let staged_preload_playback_queue_generation =
            staged.preload_playback_queue_generations.get();
        let staged_priority_changed_at = staged
            .priority_changed_at
            .read()
            .unwrap_or(state.last_activity);
        let selection = staged.selection_context.read().clone();
        let command = staged.command_state.read().clone();
        let alignment = staged.alignment_context.read().clone();

        // Populate every replacement before releasing any write guard. A reader that needs more
        // than one projection either sees the complete previous set or waits and sees the complete
        // replacement set.
        let mut live_state = self.state.write();
        let mut live_command = self.command_state.write();
        let mut live_selection = self.selection_context.write();
        let mut live_alignment = self.alignment_context.write();
        *live_state = Some(state);
        *live_command = command;
        *live_selection = selection;
        *live_alignment = alignment;
        drop(live_alignment);
        drop(live_selection);
        drop(live_command);
        drop(live_state);
        self.normal_values_generations.set(staged_values_generation);
        self.preload_values_generations
            .set(staged_preload_values_generation);
        self.preload_playback_queue_generations
            .set(staged_preload_playback_queue_generation);
        *self.priority_changed_at.write() = Some(staged_priority_changed_at);
        true
    }

    /// Capture the complete user Programmer and desk interaction state before a fallible command.
    pub fn transaction_snapshot(
        &self,
        session: SessionId,
    ) -> Option<ProgrammerTransactionSnapshot> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains(&session) {
            return None;
        }
        let state = self.state.read().as_ref()?.clone();
        let normal_values_generation = self.normal_values_generations.get();
        let preload_values_generation = self.preload_values_generations.get();
        let preload_playback_queue_generation = self.preload_playback_queue_generations.get();
        let priority_changed_at = self
            .priority_changed_at
            .read()
            .as_ref()
            .copied()
            .unwrap_or(state.last_activity);
        let selection = self.selection_context.read().clone();
        let command_line = self.command_state.read().clone();
        let alignment = self.alignment_context.read().clone();
        Some(ProgrammerTransactionSnapshot {
            state,
            normal_values_generation,
            preload_values_generation,
            preload_playback_queue_generation,
            priority_changed_at,
            selection,
            command_line,
            alignment,
        })
    }

    /// Restore an exact checkpoint after a command rejected without committing.
    pub fn restore_transaction_snapshot(&self, snapshot: ProgrammerTransactionSnapshot) {
        let mutation_gate = std::sync::Arc::clone(&self.mutation_gate);
        let _mutation_guard = mutation_gate.lock();
        *self.state.write() = Some(snapshot.state);
        self.normal_values_generations
            .set(snapshot.normal_values_generation);
        self.preload_values_generations
            .set(snapshot.preload_values_generation);
        self.preload_playback_queue_generations
            .set(snapshot.preload_playback_queue_generation);
        *self.priority_changed_at.write() = Some(snapshot.priority_changed_at);
        *self.selection_context.write() = snapshot.selection;
        *self.command_state.write() = snapshot.command_line;
        *self.alignment_context.write() = snapshot.alignment;
    }
}
