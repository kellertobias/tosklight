use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use super::super::*;
use super::support::{
    ExecutionGate, FakeBackend, QueuedRunner, playback_command, request, service,
};
use crate::ActionErrorKind;

struct WaitingRuntime;

impl MacroRuntime for WaitingRuntime {
    fn invoke(
        &self,
        _definition: &MacroDefinition,
        _invocation: &MacroInvocation,
        host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        host.wait_for(MacroWaitRequest::Timer {
            delay: Duration::from_secs(60),
        })?;
        Ok(MacroExecutionOutcome {
            completed: true,
            host_actions: 1,
        })
    }
}

struct CountingRuntime(Arc<AtomicUsize>);

impl MacroRuntime for CountingRuntime {
    fn invoke(
        &self,
        _definition: &MacroDefinition,
        _invocation: &MacroInvocation,
        _host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(MacroExecutionOutcome {
            completed: true,
            host_actions: 0,
        })
    }
}

struct PanickingRuntime;

impl MacroRuntime for PanickingRuntime {
    fn invoke(
        &self,
        _definition: &MacroDefinition,
        _invocation: &MacroInvocation,
        _host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        panic!("sandbox adapter panic must not escape supervision")
    }
}

struct IgnoringCancellationRuntime(Arc<ExecutionGate>);

impl MacroRuntime for IgnoringCancellationRuntime {
    fn invoke(
        &self,
        _definition: &MacroDefinition,
        _invocation: &MacroInvocation,
        host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        self.0.enter_and_wait();
        host.trigger_playback(playback_command())?;
        Ok(MacroExecutionOutcome {
            completed: true,
            host_actions: 1,
        })
    }
}

#[test]
fn service_exposes_waiting_cancellation_requested_and_cancelled_lifecycle() {
    let backend = Arc::new(FakeBackend::default());
    backend.block_waits();
    let runner = Arc::new(QueuedRunner::default());
    let service = service(Arc::new(WaitingRuntime), backend.clone(), runner.clone());
    let started = service.start(request()).unwrap();
    assert_eq!(started.phase, MacroExecutionPhase::Queued);

    let task = runner.take_next().unwrap();
    let worker = std::thread::spawn(task);
    backend.wait_until_waiting();
    assert_eq!(
        service.execution(started.execution_id).unwrap().phase,
        MacroExecutionPhase::Waiting(MacroWaitState::Timer)
    );

    assert_eq!(
        service.stop(started.execution_id).unwrap().phase,
        MacroExecutionPhase::CancellationRequested
    );
    worker.join().unwrap();
    assert_eq!(
        service.execution(started.execution_id).unwrap().phase,
        MacroExecutionPhase::Cancelled
    );
}

#[test]
fn queued_work_is_not_run_on_the_starting_request_path() {
    let calls = Arc::new(AtomicUsize::new(0));
    let backend = Arc::new(FakeBackend::default());
    let runner = Arc::new(QueuedRunner::default());
    let service = service(
        Arc::new(CountingRuntime(calls.clone())),
        backend,
        runner.clone(),
    );

    let started = service.start(request()).unwrap();
    assert_eq!(started.phase, MacroExecutionPhase::Queued);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    runner.run_next();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        service
            .execution(started.execution_id)
            .unwrap()
            .phase
            .is_terminal()
    );
}

#[test]
fn stopping_queued_work_prevents_the_runtime_from_starting() {
    let calls = Arc::new(AtomicUsize::new(0));
    let backend = Arc::new(FakeBackend::default());
    let runner = Arc::new(QueuedRunner::default());
    let service = service(
        Arc::new(CountingRuntime(calls.clone())),
        backend,
        runner.clone(),
    );
    let started = service.start(request()).unwrap();

    service.stop(started.execution_id).unwrap();
    runner.run_next();

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        service.execution(started.execution_id).unwrap().phase,
        MacroExecutionPhase::Cancelled
    );
    let missing = service
        .execution(MacroExecutionId(uuid::Uuid::nil()))
        .unwrap_err();
    assert_eq!(
        missing.kind,
        MacroErrorKind::Action(ActionErrorKind::NotFound)
    );
}

#[test]
fn runtime_panics_become_terminal_failures_instead_of_stuck_running_instances() {
    let backend = Arc::new(FakeBackend::default());
    let runner = Arc::new(QueuedRunner::default());
    let service = service(Arc::new(PanickingRuntime), backend, runner.clone());
    let started = service.start(request()).unwrap();

    runner.run_next();

    let MacroExecutionPhase::Failed(error) = service.execution(started.execution_id).unwrap().phase
    else {
        panic!("expected supervised runtime failure");
    };
    assert_eq!(error.kind, MacroErrorKind::Runtime);
    assert_eq!(error.message, "Macro runtime terminated unexpectedly");
}

#[test]
fn service_cancellation_prevents_host_actions_when_runtime_ignores_it() {
    let backend = Arc::new(FakeBackend::default());
    let runner = Arc::new(QueuedRunner::default());
    let gate = Arc::new(ExecutionGate::default());
    let service = service(
        Arc::new(IgnoringCancellationRuntime(gate.clone())),
        backend.clone(),
        runner.clone(),
    );
    let started = service.start(request()).unwrap();
    let worker = std::thread::spawn(runner.take_next().unwrap());
    gate.wait_until_entered();

    service.stop(started.execution_id).unwrap();
    gate.release();
    worker.join().unwrap();

    assert_eq!(
        service.execution(started.execution_id).unwrap().phase,
        MacroExecutionPhase::Cancelled
    );
    assert!(backend.playbacks().is_empty());
    assert!(
        !backend
            .contexts()
            .iter()
            .any(|(label, _)| label.starts_with("playback-"))
    );
}
