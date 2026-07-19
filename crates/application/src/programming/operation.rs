use super::ProgrammingService;
use crate::EventDraft;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// A named adapter-owned Programming unit of work serialized with commands on the same desk.
///
/// The implementation receives no service internals or domain lock. It returns typed drafts for
/// publication before the private desk gate is released.
pub trait ProgrammingUnitOfWork {
    type Output;

    fn desk_id(&self) -> Uuid;
    fn execute(self) -> ProgrammingOperation<Self::Output>;
}

pub struct ProgrammingOperation<T> {
    pub output: T,
    pub events: Vec<EventDraft>,
}

impl<T> ProgrammingOperation<T> {
    pub fn new(output: T) -> Self {
        Self {
            output,
            events: Vec::new(),
        }
    }

    pub fn with_events(output: T, events: Vec<EventDraft>) -> Self {
        Self { output, events }
    }
}

pub struct ProgrammingOperationResult<T> {
    pub output: T,
    pub event_sequences: Vec<u64>,
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

    pub fn run_unit_of_work<O>(&self, operation: O) -> ProgrammingOperationResult<O::Output>
    where
        O: ProgrammingUnitOfWork,
    {
        self.with_desk_gate(operation.desk_id(), || {
            let completed = operation.execute();
            let event_sequences = completed
                .events
                .into_iter()
                .map(|draft| self.events.publish(draft).sequence)
                .collect();
            ProgrammingOperationResult {
                output: completed.output,
                event_sequences,
            }
        })
    }

    /// Compatibility-only raw gate access for server adapters not yet expressed as named units of
    /// work. New code must use `handle` or `run_unit_of_work` so publication remains ordered.
    pub fn desk_lock(&self, desk_id: Uuid) -> Arc<Mutex<()>> {
        self.desk_gates.gate(desk_id)
    }
}
