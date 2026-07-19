use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};
use uuid::Uuid;

use super::super::*;
use crate::playback::{
    PlaybackDeskProjection, PlaybackRuntimeIdentity, PlaybackRuntimeProjection, PlaybackShowScope,
    PlaybackTargetProjection,
};
use crate::{
    ActionContext, ActionError, ActionErrorKind, FixturePositionCommand, FixturePositionExecution,
    FixturePositionOutcome, FixturePositionPorts, FixtureProjection, PlaybackAction,
    PlaybackAddress, PlaybackCommand, PlaybackExecution, PlaybackPorts, PlaybackSurface,
    ResolvedPlaybackAddress, StagePosition,
};

mod host_backend;
mod setup;

pub(super) use host_backend::HttpSimulation;
pub(super) use setup::{QueuedRunner, http_request, playback_command, request, service};

#[derive(Default)]
pub(super) struct ExecutionGate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}

impl ExecutionGate {
    pub(super) fn enter_and_wait(&self) {
        let mut state = self.state.lock().unwrap();
        state.0 = true;
        self.changed.notify_all();
        while !state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }

    pub(super) fn wait_until_entered(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.0 {
            state = self.changed.wait(state).unwrap();
        }
    }

    pub(super) fn release(&self) {
        self.state.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
}

struct BackendState {
    fixture: FixtureProjection,
    contexts: Vec<(&'static str, ActionContext)>,
    playbacks: Vec<PlaybackCommand>,
    waits: Vec<MacroWaitRequest>,
    resumes: VecDeque<MacroResume>,
    block_wait: bool,
    wait_entries: usize,
    http_requests: Vec<MacroHttpRequest>,
    audits: Vec<MacroAuditEntry>,
    http_simulation: HttpSimulation,
}

impl Default for BackendState {
    fn default() -> Self {
        Self {
            fixture: FixtureProjection {
                id: "fixture-1".into(),
                name: "Key Light".into(),
                position: StagePosition {
                    x_mm: 0,
                    y_mm: 0,
                    z_mm: 3_000,
                },
                revision: 7,
            },
            contexts: Vec::new(),
            playbacks: Vec::new(),
            waits: Vec::new(),
            resumes: VecDeque::new(),
            block_wait: false,
            wait_entries: 0,
            http_requests: Vec::new(),
            audits: Vec::new(),
            http_simulation: HttpSimulation::default(),
        }
    }
}

pub(super) struct FakeBackend {
    state: Mutex<BackendState>,
    wait_changed: Condvar,
    http_policy: Mutex<MacroHttpPolicy>,
    fail_http_policy: AtomicBool,
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self {
            state: Mutex::new(BackendState::default()),
            wait_changed: Condvar::new(),
            http_policy: Mutex::new(MacroHttpPolicy {
                timeout: Duration::from_secs(2),
                max_request_bytes: 16,
                max_response_bytes: 16,
                max_redirects: 0,
            }),
            fail_http_policy: AtomicBool::new(false),
        }
    }
}

impl FakeBackend {
    pub(super) fn queue_resumes(&self, resumes: impl IntoIterator<Item = MacroResume>) {
        self.state.lock().unwrap().resumes.extend(resumes);
        self.wait_changed.notify_all();
    }

    pub(super) fn block_waits(&self) {
        self.state.lock().unwrap().block_wait = true;
    }

    pub(super) fn wait_until_waiting(&self) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut state = self.state.lock().unwrap();
        while state.wait_entries == 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (next, timeout) = self.wait_changed.wait_timeout(state, remaining).unwrap();
            state = next;
            assert!(!timeout.timed_out(), "Macro wait did not start");
        }
    }

    pub(super) fn contexts(&self) -> Vec<(&'static str, ActionContext)> {
        self.state.lock().unwrap().contexts.clone()
    }

    pub(super) fn fixture(&self) -> FixtureProjection {
        self.state.lock().unwrap().fixture.clone()
    }

    pub(super) fn playbacks(&self) -> Vec<PlaybackCommand> {
        self.state.lock().unwrap().playbacks.clone()
    }

    pub(super) fn waits(&self) -> Vec<MacroWaitRequest> {
        self.state.lock().unwrap().waits.clone()
    }

    fn record(&self, label: &'static str, context: &ActionContext) {
        self.state
            .lock()
            .unwrap()
            .contexts
            .push((label, context.clone()));
    }
}

impl FixturePositionPorts for FakeBackend {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.record("fixture-authorize", context);
        Ok(())
    }

    fn fixture(
        &self,
        context: &ActionContext,
        fixture_id: &str,
    ) -> Result<Option<FixtureProjection>, ActionError> {
        self.record("fixture-query", context);
        let fixture = self.state.lock().unwrap().fixture.clone();
        Ok((fixture.id == fixture_id).then_some(fixture))
    }

    fn set_position(
        &self,
        context: &ActionContext,
        command: &FixturePositionCommand,
        expected_revision: u64,
    ) -> Result<FixturePositionExecution, ActionError> {
        self.record("fixture-change", context);
        let mut state = self.state.lock().unwrap();
        if state.fixture.revision != expected_revision {
            return Err(ActionError::new(ActionErrorKind::Conflict, "stale fixture")
                .at_revision(state.fixture.revision));
        }
        state.fixture.position = command.position;
        state.fixture.revision += 1;
        Ok(FixturePositionExecution {
            outcome: FixturePositionOutcome {
                fixture_id: command.fixture_id.clone(),
                position: command.position,
            },
            revision: state.fixture.revision,
            event_sequence: Some(11),
        })
    }
}

impl PlaybackPorts for FakeBackend {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.record("playback-authorize", context);
        Ok(())
    }

    fn current_page(&self, _context: &ActionContext) -> Result<u8, ActionError> {
        Ok(1)
    }

    fn playback_at(&self, _page: u8, slot: u8) -> Result<Option<u16>, ActionError> {
        Ok(Some(u16::from(slot)))
    }

    fn execute(
        &self,
        context: &ActionContext,
        _address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        self.record("playback-execute", context);
        self.state.lock().unwrap().playbacks.push(PlaybackCommand {
            address: PlaybackAddress::Pool(1),
            action,
            surface,
        });
        Ok(PlaybackExecution::Pool {
            changed: true,
            pending: None,
        })
    }

    fn projection(
        &self,
        _context: &ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<PlaybackRuntimeProjection, ActionError> {
        Ok(PlaybackRuntimeProjection {
            scope: PlaybackShowScope {
                show_id: Uuid::nil(),
                show_revision: 0,
            },
            requested: identity,
            playback_number: match identity {
                PlaybackRuntimeIdentity::Playback(number) => Some(number),
                PlaybackRuntimeIdentity::CueList(_) => None,
            },
            target: PlaybackTargetProjection::Missing,
        })
    }

    fn desk_projection(
        &self,
        context: &ActionContext,
    ) -> Result<Option<PlaybackDeskProjection>, ActionError> {
        Ok(Some(PlaybackDeskProjection {
            scope: PlaybackShowScope {
                show_id: Uuid::nil(),
                show_revision: 0,
            },
            desk_id: context.desk_id,
            active_page: 1,
            selected_playback: None,
        }))
    }
}
