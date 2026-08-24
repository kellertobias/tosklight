use super::ProgrammingService;
use light_core::SessionId;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// The output of an adapter-owned Programming interaction and the authoritative event it emitted.
///
/// `event_sequence` is absent when the adapter work left both the command line and selection
/// unchanged.
#[derive(Debug)]
pub struct ProgrammingInteractionResult<T> {
    pub output: T,
    pub event_sequence: Option<u64>,
    pub capture_mode_event_sequence: Option<u64>,
    pub values_event_sequence: Option<u64>,
    pub preload_values_event_sequence: Option<u64>,
    pub preload_playback_queue_event_sequence: Option<u64>,
}

/// One desk-local selection projection that may change during a shared runtime installation.
///
/// A server adapter supplies the stable interaction-context identity for each desk. Programmer
/// values remain user-owned, while command lines and selections are desk-local projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingSelectionTarget {
    pub desk_id: Uuid,
    pub interaction_id: SessionId,
}

/// The authoritative Programming event published for one changed desk during a shared refresh.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingSelectionRefreshEvent {
    pub desk_id: Uuid,
    pub event_sequence: u64,
}

/// Output from a shared selection refresh and its deterministically ordered desk events.
#[derive(Debug)]
pub struct ProgrammingSelectionRefreshResult<T> {
    pub output: T,
    pub events: Vec<ProgrammingSelectionRefreshEvent>,
}

#[derive(Clone, Default)]
pub(super) struct DeskOperationGates {
    gates: Arc<Mutex<HashMap<Uuid, Arc<Mutex<()>>>>>,
}

impl DeskOperationGates {
    fn gate(&self, desk_id: Uuid) -> Arc<Mutex<()>> {
        let mut gates = self.gates.lock();
        gates.retain(|id, gate| *id == desk_id || Arc::strong_count(gate) > 1);
        Arc::clone(
            gates
                .entry(desk_id)
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    fn with_gates<T>(&self, desk_ids: &[Uuid], operation: impl FnOnce() -> T) -> T {
        let gates = desk_ids
            .iter()
            .map(|desk_id| self.gate(*desk_id))
            .collect::<Vec<_>>();
        let _ordered = gates.iter().map(|gate| gate.lock()).collect::<Vec<_>>();
        operation()
    }
}

impl ProgrammingService {
    pub(super) fn with_desk_gates<T>(&self, desk_ids: &[Uuid], operation: impl FnOnce() -> T) -> T {
        self.desk_gates.with_gates(desk_ids, operation)
    }

    /// Serializes a complete Programmer transition while preserving the global lock order: the
    /// Programmer first, then desk interaction. A second surface therefore cannot retain its desk
    /// gate while waiting for the Programmer boundary, which keeps nested selection reconciliation
    /// deadlock-free.
    pub(super) fn with_programmer_and_desk_gate<T>(
        &self,
        desk_id: Uuid,
        operation: impl FnOnce() -> T,
    ) -> T {
        self.programmers.serialized(|| {
            let desk_gate = self.desk_gates.gate(desk_id);
            let _desk = desk_gate.lock();
            operation()
        })
    }

    /// Runs one adapter-owned desk operation without exposing the synchronization primitive.
    pub fn run_desk_operation<T>(&self, desk_id: Uuid, operation: impl FnOnce() -> T) -> T {
        self.desk_gates.with_gates(&[desk_id], operation)
    }
}
