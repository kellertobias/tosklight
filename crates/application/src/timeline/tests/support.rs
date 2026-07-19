use std::{collections::VecDeque, sync::Mutex};

use uuid::Uuid;

use super::super::*;
use crate::playback::{
    PlaybackDeskProjection, PlaybackRuntimeIdentity, PlaybackRuntimeProjection, PlaybackShowScope,
    PlaybackTargetProjection,
};
use crate::{
    ActionError, AssetDescriptor, AssetId, AssetNamespace, FixturePositionCommand,
    FixturePositionExecution, FixturePositionPorts, FixtureProjection, GroupProjection,
    ImportAssetRequest, MacroCapability, MacroExecutionId, MacroExecutionOutcome, MacroHost,
    MacroHostBackend, MacroHttpRequest, MacroHttpTransportError, MacroHttpTransportErrorKind,
    MacroHttpTransportResponse, MacroLanguageId, MacroResume, MacroRuntime, MacroTask,
    MacroTaskRunner, MacroWaitRequest, ManagedAssetStore, PlaybackAction, PlaybackAddress,
    PlaybackExecution, PlaybackSurface, ResolvedPlaybackAddress,
    managed_assets::test_support::{FakeAssetStore, VecSource, fake_digest},
};

#[derive(Default)]
pub(super) struct TestCancellation(pub(super) bool);

impl CancellationSignal for TestCancellation {
    fn is_cancelled(&self) -> bool {
        self.0
    }
}

#[derive(Default)]
pub(super) struct FakeScheduler(Mutex<Vec<MonotonicMoment>>);

impl FakeScheduler {
    pub(super) fn wait_count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

impl MonotonicScheduler for FakeScheduler {
    fn wait_until(
        &self,
        deadline: MonotonicMoment,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<(), SchedulerError> {
        self.0.lock().unwrap().push(deadline);
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct QueuedRunner(Mutex<VecDeque<MacroTask>>);

impl QueuedRunner {
    pub(super) fn run_next(&self) {
        self.0.lock().unwrap().pop_front().unwrap()();
    }
}

impl MacroTaskRunner for QueuedRunner {
    fn spawn(&self, task: MacroTask) -> Result<(), MacroError> {
        self.0.lock().unwrap().push_back(task);
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct RecordingMacroRuntime(Mutex<Vec<MacroInvocation>>);

impl RecordingMacroRuntime {
    pub(super) fn invocation_count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

impl MacroRuntime for RecordingMacroRuntime {
    fn invoke(
        &self,
        _definition: &MacroDefinition,
        invocation: &MacroInvocation,
        _host: &dyn MacroHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroExecutionOutcome, MacroError> {
        self.0.lock().unwrap().push(invocation.clone());
        Ok(MacroExecutionOutcome {
            completed: true,
            host_actions: 0,
        })
    }
}

#[derive(Default)]
pub(super) struct FakeBackend {
    assets: FakeAssetStore,
    contexts: Mutex<Vec<(&'static str, ActionContext)>>,
    playbacks: Mutex<Vec<PlaybackCommand>>,
}

impl FakeBackend {
    pub(super) fn contexts(&self) -> Vec<(&'static str, ActionContext)> {
        self.contexts.lock().unwrap().clone()
    }

    pub(super) fn playbacks(&self) -> Vec<PlaybackCommand> {
        self.playbacks.lock().unwrap().clone()
    }

    pub(super) fn import_media(&self) -> AssetDescriptor {
        let bytes = b"timeline-audio";
        self.assets
            .import(
                ImportAssetRequest {
                    identity: Some(AssetId(Uuid::from_u128(70))),
                    namespace: AssetNamespace("show:main".into()),
                    name: "Timeline audio".into(),
                    media_type: "audio/wav".into(),
                    declared_length: bytes.len() as u64,
                    declared_digest: fake_digest(bytes),
                },
                &mut VecSource::new(bytes),
            )
            .unwrap()
    }

    fn record(&self, label: &'static str, context: &ActionContext) {
        self.contexts.lock().unwrap().push((label, context.clone()));
    }
}

impl FixturePositionPorts for FakeBackend {
    fn authorize(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn fixture(
        &self,
        _context: &ActionContext,
        _fixture_id: &str,
    ) -> Result<Option<FixtureProjection>, ActionError> {
        Ok(None)
    }

    fn set_position(
        &self,
        _context: &ActionContext,
        _command: &FixturePositionCommand,
        _expected_revision: u64,
    ) -> Result<FixturePositionExecution, ActionError> {
        unreachable!("timeline proof does not change fixture positions")
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
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        self.record("playback-execute", context);
        self.playbacks.lock().unwrap().push(PlaybackCommand {
            address: PlaybackAddress::Pool(address.playback_number().unwrap_or_default()),
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
        _context: &ActionContext,
        _id: &str,
    ) -> Result<Option<GroupProjection>, MacroError> {
        Ok(None)
    }

    fn wait_for(
        &self,
        _context: &ActionContext,
        _request: MacroWaitRequest,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroResume, MacroError> {
        Err(MacroError::new(MacroErrorKind::Host, "not used"))
    }

    fn http_policy(
        &self,
        _context: &ActionContext,
        _execution_id: MacroExecutionId,
        _definition: &MacroDefinition,
    ) -> Result<crate::MacroHttpPolicy, MacroError> {
        Ok(crate::MacroHttpPolicy {
            timeout: std::time::Duration::from_secs(1),
            max_request_bytes: 1,
            max_response_bytes: 1,
            max_redirects: 0,
        })
    }

    fn dispatch_http(
        &self,
        _context: &ActionContext,
        _request: &MacroHttpRequest,
        _policy: &crate::MacroHttpPolicy,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<MacroHttpTransportResponse, MacroHttpTransportError> {
        Err(MacroHttpTransportError::new(
            MacroHttpTransportErrorKind::Transport,
            "not used",
        ))
    }

    fn record_http_audit(&self, _entry: crate::MacroAuditEntry) -> Result<(), MacroError> {
        Ok(())
    }
}

impl TimelineHostBackend for FakeBackend {
    fn asset_availability(
        &self,
        context: &ActionContext,
        asset: AssetReference,
    ) -> Result<AssetAvailability, TimelineError> {
        self.record("asset", context);
        self.assets
            .availability(asset)
            .map_err(|error| TimelineError::new(TimelineErrorKind::Host, error.message))
    }

    fn playback_ports(&self) -> &dyn PlaybackPorts {
        self
    }

    fn macro_definition(
        &self,
        context: &ActionContext,
        id: &MacroId,
        revision: u64,
    ) -> Result<MacroDefinition, TimelineError> {
        self.record("macro-definition", context);
        Ok(MacroDefinition {
            id: id.clone(),
            revision,
            language: MacroLanguageId("fake".into()),
            source: "fake".into(),
            capabilities: [MacroCapability::TriggerPlayback].into_iter().collect(),
            dependencies: Vec::new(),
        })
    }
}
