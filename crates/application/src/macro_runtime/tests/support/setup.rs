use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use uuid::Uuid;

use super::*;
use crate::{ActionSource, FixturePositionService, PlaybackService};

#[derive(Default)]
pub(crate) struct QueuedRunner {
    tasks: Mutex<VecDeque<MacroTask>>,
}

impl QueuedRunner {
    pub(crate) fn len(&self) -> usize {
        self.tasks.lock().unwrap().len()
    }

    pub(crate) fn run_next(&self) {
        self.take_next().expect("queued Macro task")();
    }

    pub(crate) fn take_next(&self) -> Option<MacroTask> {
        self.tasks.lock().unwrap().pop_front()
    }
}

impl MacroTaskRunner for QueuedRunner {
    fn spawn(&self, task: MacroTask) -> Result<(), MacroError> {
        self.tasks.lock().unwrap().push_back(task);
        Ok(())
    }
}

pub(crate) fn service(
    runtime: Arc<dyn MacroRuntime>,
    backend: Arc<FakeBackend>,
    runner: Arc<QueuedRunner>,
) -> MacroService {
    MacroService::new(
        runtime,
        backend,
        runner,
        FixturePositionService::default(),
        PlaybackService::default(),
    )
}

fn definition() -> MacroDefinition {
    MacroDefinition {
        id: MacroId("preset-macro".into()),
        revision: 3,
        language: MacroLanguageId("fake-language".into()),
        source: "fake program".into(),
        capabilities: [
            MacroCapability::QueryFixtures,
            MacroCapability::QueryGroups,
            MacroCapability::ChangeFixturePosition,
            MacroCapability::AwaitTimer,
            MacroCapability::AwaitEvent,
            MacroCapability::AwaitOperatorInput,
            MacroCapability::TriggerPlayback,
            MacroCapability::Http,
        ]
        .into_iter()
        .collect(),
        dependencies: vec![MacroDependency {
            kind: "group".into(),
            id: "group-1".into(),
            revision: Some(2),
        }],
    }
}

fn trusted_context() -> ActionContext {
    let mut context = ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        ActionSource::Http,
    )
    .with_request_id("incoming")
    .with_expected_revision(999);
    context.correlation_id = Uuid::from_u128(4);
    context
}

pub(crate) fn request() -> MacroExecutionRequest {
    MacroExecutionRequest {
        definition: definition(),
        context: trusted_context(),
        arguments: Default::default(),
    }
}

pub(crate) fn playback_command() -> PlaybackCommand {
    PlaybackCommand {
        address: PlaybackAddress::Pool(1),
        action: PlaybackAction::Go { pressed: true },
        surface: PlaybackSurface::Virtual,
    }
}

pub(crate) fn http_request() -> MacroHttpRequest {
    MacroHttpRequest {
        method: "POST".into(),
        url: "https://device.test/cue".into(),
        headers: Default::default(),
        body: b"go".to_vec(),
        audit_label: "trigger device cue".into(),
    }
}
