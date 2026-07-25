use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use parking_lot::Mutex;
use uuid::Uuid;

use super::{
    CancellationSignal, MacroError, MacroErrorKind, MacroExecutionId, MacroExecutionPhase,
    MacroExecutionRequest, MacroExecutionSnapshot, MacroHost, MacroHostBackend, MacroInvocation,
    MacroWaitRequest, MacroWaitState,
    host::{MacroLifecycleObserver, ScopedMacroHost, ScopedMacroHostDependencies},
};
use crate::{ActionErrorKind, FixturePositionService, PlaybackService};

pub type MacroTask = Box<dyn FnOnce() + Send + 'static>;

/// Application composition port whose implementation must enqueue or spawn rather than run inline.
pub trait MacroTaskRunner: Send + Sync {
    fn spawn(&self, task: MacroTask) -> Result<(), MacroError>;
}

pub trait MacroRuntime: Send + Sync {
    fn invoke(
        &self,
        definition: &super::MacroDefinition,
        invocation: &MacroInvocation,
        host: &dyn MacroHost,
        cancellation: &dyn CancellationSignal,
    ) -> Result<super::MacroExecutionOutcome, MacroError>;
}

#[derive(Clone)]
pub struct MacroService {
    runtime: Arc<dyn MacroRuntime>,
    backend: Arc<dyn MacroHostBackend>,
    runner: Arc<dyn MacroTaskRunner>,
    fixture_positions: FixturePositionService,
    playbacks: PlaybackService,
    executions: Arc<Mutex<BTreeMap<MacroExecutionId, Arc<ExecutionRecord>>>>,
}

impl MacroService {
    pub fn new(
        runtime: Arc<dyn MacroRuntime>,
        backend: Arc<dyn MacroHostBackend>,
        runner: Arc<dyn MacroTaskRunner>,
        fixture_positions: FixturePositionService,
        playbacks: PlaybackService,
    ) -> Self {
        Self {
            runtime,
            backend,
            runner,
            fixture_positions,
            playbacks,
            executions: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn start(
        &self,
        request: MacroExecutionRequest,
    ) -> Result<MacroExecutionSnapshot, MacroError> {
        let execution_id = MacroExecutionId(Uuid::new_v4());
        let mut context = request.context;
        context.source = crate::ActionSource::Macro;
        context.expected_revision = None;
        self.backend
            .authorize_execution(&context, &request.definition)?;
        let record = Arc::new(ExecutionRecord::new(
            execution_id,
            request.definition.id.clone(),
            request.definition.revision,
        ));
        self.executions.lock().insert(execution_id, record.clone());
        let task = self.task(
            request.definition,
            request.arguments,
            context,
            record.clone(),
        );
        if let Err(error) = self.runner.spawn(task) {
            self.executions.lock().remove(&execution_id);
            return Err(error);
        }
        Ok(record.snapshot())
    }

    pub fn execution(
        &self,
        execution_id: MacroExecutionId,
    ) -> Result<MacroExecutionSnapshot, MacroError> {
        self.record(execution_id).map(|record| record.snapshot())
    }

    pub fn stop(
        &self,
        execution_id: MacroExecutionId,
    ) -> Result<MacroExecutionSnapshot, MacroError> {
        let record = self.record(execution_id)?;
        record.request_cancellation();
        Ok(record.snapshot())
    }

    fn record(&self, execution_id: MacroExecutionId) -> Result<Arc<ExecutionRecord>, MacroError> {
        self.executions
            .lock()
            .get(&execution_id)
            .cloned()
            .ok_or_else(|| {
                MacroError::action(ActionErrorKind::NotFound, "Macro execution was not found")
            })
    }

    fn task(
        &self,
        definition: super::MacroDefinition,
        arguments: BTreeMap<String, super::MacroValue>,
        context: crate::ActionContext,
        record: Arc<ExecutionRecord>,
    ) -> MacroTask {
        let runtime = self.runtime.clone();
        let backend = self.backend.clone();
        let fixture_positions = self.fixture_positions.clone();
        let playbacks = self.playbacks.clone();
        Box::new(move || {
            if record.cancelled() {
                record.finish_cancelled();
                return;
            }
            record.running();
            let invocation = MacroInvocation {
                id: definition.id.clone(),
                revision: definition.revision,
                arguments,
            };
            let host = ScopedMacroHost::new(
                record.execution_id,
                definition.clone(),
                context,
                ScopedMacroHostDependencies {
                    backend,
                    fixture_positions,
                    playbacks,
                    lifecycle: record.clone(),
                    cancellation: record.clone(),
                },
            );
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                runtime.invoke(&definition, &invocation, &host, record.as_ref())
            }))
            .unwrap_or_else(|_| {
                Err(MacroError::new(
                    MacroErrorKind::Runtime,
                    "Macro runtime terminated unexpectedly",
                ))
            });
            record.finish(result);
        })
    }
}

struct ExecutionRecord {
    execution_id: MacroExecutionId,
    macro_id: super::MacroId,
    macro_revision: u64,
    cancellation: AtomicBool,
    phase: Mutex<MacroExecutionPhase>,
}

impl ExecutionRecord {
    fn new(execution_id: MacroExecutionId, macro_id: super::MacroId, macro_revision: u64) -> Self {
        Self {
            execution_id,
            macro_id,
            macro_revision,
            cancellation: AtomicBool::new(false),
            phase: Mutex::new(MacroExecutionPhase::Queued),
        }
    }

    fn snapshot(&self) -> MacroExecutionSnapshot {
        MacroExecutionSnapshot {
            execution_id: self.execution_id,
            macro_id: self.macro_id.clone(),
            macro_revision: self.macro_revision,
            phase: self.phase.lock().clone(),
        }
    }

    fn running(&self) {
        let mut phase = self.phase.lock();
        if !matches!(*phase, MacroExecutionPhase::CancellationRequested) && !phase.is_terminal() {
            *phase = MacroExecutionPhase::Running;
        }
    }

    fn request_cancellation(&self) {
        let mut phase = self.phase.lock();
        if phase.is_terminal() {
            return;
        }
        self.cancellation.store(true, Ordering::Release);
        *phase = MacroExecutionPhase::CancellationRequested;
    }

    fn finish(&self, result: Result<super::MacroExecutionOutcome, MacroError>) {
        if self.cancelled() {
            self.finish_cancelled();
            return;
        }
        *self.phase.lock() = match result {
            Ok(outcome) => MacroExecutionPhase::Completed(outcome),
            Err(error) if error.kind == MacroErrorKind::Cancelled => MacroExecutionPhase::Cancelled,
            Err(error) => MacroExecutionPhase::Failed(error),
        };
    }

    fn finish_cancelled(&self) {
        *self.phase.lock() = MacroExecutionPhase::Cancelled;
    }
}

impl CancellationSignal for ExecutionRecord {
    fn is_cancelled(&self) -> bool {
        self.cancelled()
    }
}

impl MacroLifecycleObserver for ExecutionRecord {
    fn waiting(&self, request: &MacroWaitRequest) {
        let mut phase = self.phase.lock();
        if !matches!(*phase, MacroExecutionPhase::CancellationRequested) && !phase.is_terminal() {
            *phase = MacroExecutionPhase::Waiting(MacroWaitState::from(request));
        }
    }

    fn running(&self) {
        ExecutionRecord::running(self);
    }
}

impl ExecutionRecord {
    fn cancelled(&self) -> bool {
        self.cancellation.load(Ordering::Acquire)
    }
}
