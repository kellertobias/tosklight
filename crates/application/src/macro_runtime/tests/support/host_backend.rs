use std::sync::atomic::Ordering;
use std::time::Duration;

use super::*;

#[derive(Debug)]
pub(crate) enum HttpSimulation {
    Response {
        status: u16,
        body: Vec<u8>,
        redirects: u8,
    },
    Timeout,
    TransportFailure,
}

impl Default for HttpSimulation {
    fn default() -> Self {
        Self::Response {
            status: 202,
            body: b"accepted".to_vec(),
            redirects: 0,
        }
    }
}

impl FakeBackend {
    pub(crate) fn audits(&self) -> Vec<MacroAuditEntry> {
        self.state.lock().unwrap().audits.clone()
    }

    pub(crate) fn http_request_count(&self) -> usize {
        self.state.lock().unwrap().http_requests.len()
    }

    pub(crate) fn set_http_simulation(&self, simulation: HttpSimulation) {
        self.state.lock().unwrap().http_simulation = simulation;
    }

    pub(crate) fn set_http_limits(&self, request: usize, response: usize, redirects: u8) {
        let mut policy = self.http_policy.lock().unwrap();
        policy.max_request_bytes = request;
        policy.max_response_bytes = response;
        policy.max_redirects = redirects;
    }

    pub(crate) fn fail_http_policy(&self) {
        self.fail_http_policy.store(true, Ordering::SeqCst);
    }
}

impl MacroHostBackend for FakeBackend {
    fn authorize_execution(
        &self,
        context: &ActionContext,
        _definition: &MacroDefinition,
    ) -> Result<(), MacroError> {
        self.record("macro-authorize", context);
        Ok(())
    }

    fn fixture_position_ports(&self) -> &dyn FixturePositionPorts {
        self
    }

    fn playback_ports(&self) -> &dyn PlaybackPorts {
        self
    }

    fn group(
        &self,
        context: &ActionContext,
        id: &str,
    ) -> Result<Option<GroupProjection>, MacroError> {
        self.record("group-query", context);
        Ok((id == "group-1").then(|| GroupProjection {
            id: id.into(),
            name: "Front".into(),
            fixture_ids: vec!["fixture-1".into()],
        }))
    }

    fn wait_for(
        &self,
        context: &ActionContext,
        request: MacroWaitRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<MacroResume, MacroError> {
        self.record("wait", context);
        let mut state = self.state.lock().unwrap();
        state.waits.push(request);
        state.wait_entries += 1;
        self.wait_changed.notify_all();
        while state.block_wait && state.resumes.is_empty() && !cancellation.is_cancelled() {
            state = self
                .wait_changed
                .wait_timeout(state, Duration::from_millis(5))
                .unwrap()
                .0;
        }
        if cancellation.is_cancelled() {
            return Err(MacroError::new(MacroErrorKind::Cancelled, "cancelled"));
        }
        state
            .resumes
            .pop_front()
            .ok_or_else(|| MacroError::new(MacroErrorKind::Host, "no resume queued"))
    }

    fn http_policy(
        &self,
        _context: &ActionContext,
        _execution_id: MacroExecutionId,
        _definition: &MacroDefinition,
    ) -> Result<MacroHttpPolicy, MacroError> {
        if self.fail_http_policy.swap(false, Ordering::SeqCst) {
            return Err(MacroError::new(
                MacroErrorKind::Host,
                "HTTP policy is unavailable",
            ));
        }
        Ok(self.http_policy.lock().unwrap().clone())
    }

    fn dispatch_http(
        &self,
        context: &ActionContext,
        request: &MacroHttpRequest,
        _policy: &MacroHttpPolicy,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroHttpTransportResponse, MacroHttpTransportError> {
        self.record("http", context);
        let simulation = {
            let mut state = self.state.lock().unwrap();
            state.http_requests.push(request.clone());
            std::mem::take(&mut state.http_simulation)
        };
        match simulation {
            HttpSimulation::Timeout => Err(MacroHttpTransportError::new(
                MacroHttpTransportErrorKind::Timeout,
                "timeout",
            )),
            HttpSimulation::TransportFailure => Err(MacroHttpTransportError::new(
                MacroHttpTransportErrorKind::Transport,
                "transport",
            )),
            HttpSimulation::Response {
                status,
                body,
                redirects,
            } => Ok(MacroHttpTransportResponse {
                status,
                headers: Default::default(),
                body,
                redirects,
            }),
        }
    }

    fn record_http_audit(&self, entry: MacroAuditEntry) -> Result<(), MacroError> {
        self.state.lock().unwrap().audits.push(entry);
        Ok(())
    }
}
