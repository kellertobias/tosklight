use uuid::Uuid;

use super::{
    CancellationSignal, MacroAuditEntry, MacroAuditedAction, MacroDefinition, MacroError,
    MacroErrorKind, MacroExecutionId, MacroHostBackend, MacroHttpAuditEvent, MacroHttpFailureKind,
    MacroHttpRequest, MacroHttpResponse, MacroHttpTerminal, MacroHttpTransportErrorKind,
};
use crate::ActionContext;

pub(super) fn execute(
    backend: &dyn MacroHostBackend,
    context: &ActionContext,
    execution_id: MacroExecutionId,
    definition: &MacroDefinition,
    request: MacroHttpRequest,
    cancellation: &dyn CancellationSignal,
) -> Result<MacroHttpResponse, MacroError> {
    let scope = HttpScope {
        context,
        execution_id,
        definition,
        request_id: Uuid::new_v4(),
        audit_id: Uuid::new_v4(),
        request: &request,
    };
    record(
        backend,
        &scope,
        MacroHttpAuditEvent::Attempted {
            method: request.method.clone(),
            url: request.url.clone(),
            request_bytes: request.body.len(),
        },
    )?;
    let policy = match backend.http_policy(context, execution_id, definition) {
        Ok(policy) => policy,
        Err(error) => {
            record(
                backend,
                &scope,
                MacroHttpAuditEvent::Terminal(MacroHttpTerminal::Failed {
                    kind: MacroHttpFailureKind::Policy,
                }),
            )?;
            return Err(error.with_audit_id(scope.audit_id));
        }
    };
    if cancellation.is_cancelled() {
        return fail(
            backend,
            &scope,
            MacroHttpFailureKind::Cancelled,
            MacroErrorKind::Cancelled,
            "Macro HTTP request was cancelled",
        );
    }
    if request.body.len() > policy.max_request_bytes {
        return fail(
            backend,
            &scope,
            MacroHttpFailureKind::RequestLimitExceeded,
            MacroErrorKind::LimitExceeded,
            "Macro HTTP request body exceeds its limit",
        );
    }
    let response = match backend.dispatch_http(context, &request, &policy, cancellation) {
        Ok(response) => response,
        Err(error) => {
            let (failure, message) = match error.kind {
                MacroHttpTransportErrorKind::Timeout => (
                    MacroHttpFailureKind::Timeout,
                    "Macro HTTP request timed out",
                ),
                MacroHttpTransportErrorKind::Transport => (
                    MacroHttpFailureKind::Transport,
                    "Macro HTTP transport failed",
                ),
            };
            return fail(backend, &scope, failure, MacroErrorKind::Host, message);
        }
    };
    if cancellation.is_cancelled() {
        return fail(
            backend,
            &scope,
            MacroHttpFailureKind::Cancelled,
            MacroErrorKind::Cancelled,
            "Macro HTTP request was cancelled",
        );
    }
    if response.redirects > policy.max_redirects {
        return fail(
            backend,
            &scope,
            MacroHttpFailureKind::RedirectLimitExceeded,
            MacroErrorKind::LimitExceeded,
            "Macro HTTP redirect count exceeds its limit",
        );
    }
    if response.body.len() > policy.max_response_bytes {
        return fail(
            backend,
            &scope,
            MacroHttpFailureKind::ResponseLimitExceeded,
            MacroErrorKind::LimitExceeded,
            "Macro HTTP response body exceeds its limit",
        );
    }
    record(
        backend,
        &scope,
        MacroHttpAuditEvent::Terminal(MacroHttpTerminal::Succeeded {
            status: response.status,
            response_bytes: response.body.len(),
        }),
    )?;
    Ok(MacroHttpResponse {
        request_id: scope.request_id,
        status: response.status,
        headers: response.headers,
        body: response.body,
        audit_id: scope.audit_id,
    })
}

struct HttpScope<'a> {
    context: &'a ActionContext,
    execution_id: MacroExecutionId,
    definition: &'a MacroDefinition,
    request_id: Uuid,
    audit_id: Uuid,
    request: &'a MacroHttpRequest,
}

fn fail<T>(
    backend: &dyn MacroHostBackend,
    scope: &HttpScope<'_>,
    failure: MacroHttpFailureKind,
    kind: MacroErrorKind,
    message: &'static str,
) -> Result<T, MacroError> {
    record(
        backend,
        scope,
        MacroHttpAuditEvent::Terminal(MacroHttpTerminal::Failed { kind: failure }),
    )?;
    Err(MacroError::new(kind, message).with_audit_id(scope.audit_id))
}

fn record(
    backend: &dyn MacroHostBackend,
    scope: &HttpScope<'_>,
    event: MacroHttpAuditEvent,
) -> Result<(), MacroError> {
    backend
        .record_http_audit(MacroAuditEntry {
            execution_id: scope.execution_id,
            macro_id: scope.definition.id.clone(),
            correlation_id: scope.context.correlation_id,
            label: scope.request.audit_label.clone(),
            action: MacroAuditedAction::Http {
                request_id: scope.request_id,
                audit_id: scope.audit_id,
                event,
            },
        })
        .map_err(|error| error.with_audit_id(scope.audit_id))
}
