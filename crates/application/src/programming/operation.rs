use super::ProgrammingService;
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
}

impl ProgrammingService {
    pub(super) fn with_desk_gate<T>(&self, desk_id: Uuid, operation: impl FnOnce() -> T) -> T {
        let gate = self.desk_gates.gate(desk_id);
        let _ordered = gate.lock();
        operation()
    }

    /// Compatibility-only raw gate access for server adapters not yet expressed through the
    /// Programming service. New code must use `handle` or `run_external_interaction` so
    /// publication remains ordered.
    pub fn desk_lock(&self, desk_id: Uuid) -> Arc<Mutex<()>> {
        self.desk_gates.gate(desk_id)
    }
}
