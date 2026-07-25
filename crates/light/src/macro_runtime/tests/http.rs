use std::sync::Arc;

use super::super::*;
use super::support::{
    ExecutionGate, FakeBackend, HttpSimulation, QueuedRunner, http_request, request, service,
};

struct HttpRuntime(MacroHttpRequest);

impl MacroRuntime for HttpRuntime {
    fn invoke(
        &self,
        _definition: &MacroDefinition,
        _invocation: &MacroInvocation,
        host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        host.http(self.0.clone())?;
        Ok(MacroExecutionOutcome {
            completed: true,
            host_actions: 1,
        })
    }
}

struct GatedHttpRuntime(Arc<ExecutionGate>);

impl MacroRuntime for GatedHttpRuntime {
    fn invoke(
        &self,
        _definition: &MacroDefinition,
        _invocation: &MacroInvocation,
        host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        self.0.enter_and_wait();
        host.http(http_request())?;
        Ok(MacroExecutionOutcome {
            completed: true,
            host_actions: 1,
        })
    }
}

fn execute_http(
    backend: Arc<FakeBackend>,
    request_data: MacroHttpRequest,
) -> MacroExecutionSnapshot {
    let runner = Arc::new(QueuedRunner::default());
    let service = service(Arc::new(HttpRuntime(request_data)), backend, runner.clone());
    let started = service.start(request()).unwrap();
    runner.run_next();
    service.execution(started.execution_id).unwrap()
}

#[test]
fn application_http_port_audits_every_terminal_result() {
    let backend = Arc::new(FakeBackend::default());

    backend.fail_http_policy();
    assert_failed(
        execute_http(backend.clone(), http_request()),
        MacroErrorKind::Host,
    );

    backend.set_http_limits(2, 16, 0);
    let mut oversized = http_request();
    oversized.body = vec![0; 3];
    assert_failed(
        execute_http(backend.clone(), oversized),
        MacroErrorKind::LimitExceeded,
    );

    backend.set_http_limits(16, 1, 0);
    assert_failed(
        execute_http(backend.clone(), http_request()),
        MacroErrorKind::LimitExceeded,
    );

    backend.set_http_limits(16, 16, 0);
    backend.set_http_simulation(HttpSimulation::Response {
        status: 302,
        body: Vec::new(),
        redirects: 1,
    });
    assert_failed(
        execute_http(backend.clone(), http_request()),
        MacroErrorKind::LimitExceeded,
    );

    backend.set_http_simulation(HttpSimulation::Timeout);
    assert_failed(
        execute_http(backend.clone(), http_request()),
        MacroErrorKind::Host,
    );
    backend.set_http_simulation(HttpSimulation::TransportFailure);
    assert_failed(
        execute_http(backend.clone(), http_request()),
        MacroErrorKind::Host,
    );
    assert!(matches!(
        execute_http(backend.clone(), http_request()).phase,
        MacroExecutionPhase::Completed(_)
    ));

    let expected = [
        MacroHttpTerminal::Failed {
            kind: MacroHttpFailureKind::Policy,
        },
        MacroHttpTerminal::Failed {
            kind: MacroHttpFailureKind::RequestLimitExceeded,
        },
        MacroHttpTerminal::Failed {
            kind: MacroHttpFailureKind::ResponseLimitExceeded,
        },
        MacroHttpTerminal::Failed {
            kind: MacroHttpFailureKind::RedirectLimitExceeded,
        },
        MacroHttpTerminal::Failed {
            kind: MacroHttpFailureKind::Timeout,
        },
        MacroHttpTerminal::Failed {
            kind: MacroHttpFailureKind::Transport,
        },
        MacroHttpTerminal::Succeeded {
            status: 202,
            response_bytes: 8,
        },
    ];
    let audits = backend.audits();
    assert_eq!(audits.len(), expected.len() * 2);
    for (pair, terminal) in audits.chunks_exact(2).zip(expected) {
        assert_audit_pair(pair, terminal);
    }
    assert_eq!(backend.http_request_count(), 5);
}

#[test]
fn service_cancellation_is_used_for_http_even_when_runtime_does_not_check_it() {
    let backend = Arc::new(FakeBackend::default());
    let runner = Arc::new(QueuedRunner::default());
    let gate = Arc::new(ExecutionGate::default());
    let service = service(
        Arc::new(GatedHttpRuntime(gate.clone())),
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
    let audits = backend.audits();
    assert_eq!(audits.len(), 2);
    assert_audit_pair(
        &audits,
        MacroHttpTerminal::Failed {
            kind: MacroHttpFailureKind::Cancelled,
        },
    );
    assert_eq!(backend.http_request_count(), 0);
}

fn assert_failed(snapshot: MacroExecutionSnapshot, kind: MacroErrorKind) {
    let MacroExecutionPhase::Failed(error) = snapshot.phase else {
        panic!("expected failed Macro execution");
    };
    assert_eq!(error.kind, kind);
    assert!(error.audit_id.is_some());
}

fn assert_audit_pair(pair: &[MacroAuditEntry], terminal: MacroHttpTerminal) {
    assert_eq!(pair.len(), 2);
    assert_eq!(pair[0].execution_id, pair[1].execution_id);
    assert_eq!(pair[0].macro_id, pair[1].macro_id);
    assert_eq!(pair[0].correlation_id, pair[1].correlation_id);
    assert_eq!(pair[0].label, pair[1].label);
    let MacroAuditedAction::Http {
        request_id: first_request,
        audit_id: first_audit,
        event: MacroHttpAuditEvent::Attempted { .. },
    } = &pair[0].action
    else {
        panic!("expected attempted HTTP audit");
    };
    let MacroAuditedAction::Http {
        request_id: final_request,
        audit_id: final_audit,
        event: MacroHttpAuditEvent::Terminal(actual),
    } = &pair[1].action
    else {
        panic!("expected terminal HTTP audit");
    };
    assert_eq!(first_request, final_request);
    assert_eq!(first_audit, final_audit);
    assert_eq!(*actual, terminal);
}
