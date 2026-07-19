use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use uuid::Uuid;

use super::super::*;
use crate::{
    ActionSource, AssetId, AssetRevision, PlaybackAction, PlaybackAddress, PlaybackSurface,
};

pub(super) struct FakeTimelineRuntime;

impl TimelineRuntime for FakeTimelineRuntime {
    fn operate(
        &self,
        timeline: &TimelineExecution,
        operation: &dyn TimelineOperation,
        host: &dyn TimelineHost,
        cancellation: &dyn CancellationSignal,
    ) -> Result<(), TimelineError> {
        operation.execute(timeline, host, cancellation)
    }
}

pub(super) struct FakeOperation {
    asset: AssetReference,
    macro_invocation: MacroInvocation,
}

impl TimelineOperation for FakeOperation {
    fn execute(
        &self,
        _timeline: &TimelineExecution,
        host: &dyn TimelineHost,
        cancellation: &dyn CancellationSignal,
    ) -> Result<(), TimelineError> {
        if cancellation.is_cancelled() {
            return Err(TimelineError::new(
                TimelineErrorKind::Cancelled,
                "cancelled",
            ));
        }
        if matches!(
            host.asset_availability(self.asset)?,
            AssetAvailability::Missing(_)
        ) {
            return Err(TimelineError::new(
                TimelineErrorKind::MissingAsset,
                "timeline media asset is missing",
            ));
        }
        host.trigger_playback(playback_command())?;
        host.invoke_macro(self.macro_invocation.clone())?;
        Ok(())
    }
}

struct SharedCancellation(Arc<AtomicBool>);

impl CancellationSignal for SharedCancellation {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub(super) struct CancelBeforePlayback(Arc<AtomicBool>);

impl TimelineOperation for CancelBeforePlayback {
    fn execute(
        &self,
        _timeline: &TimelineExecution,
        host: &dyn TimelineHost,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<(), TimelineError> {
        self.0.store(true, Ordering::Release);
        host.trigger_playback(playback_command()).map(|_| ())
    }
}

pub(super) fn cancellation_during_operation() -> (impl CancellationSignal, impl TimelineOperation) {
    let shared = Arc::new(AtomicBool::new(false));
    (
        SharedCancellation(shared.clone()),
        CancelBeforePlayback(shared),
    )
}

pub(super) fn missing_asset() -> AssetReference {
    AssetReference {
        id: AssetId(Uuid::from_u128(90)),
        revision: AssetRevision(1),
    }
}

pub(super) fn operation(asset: AssetReference) -> FakeOperation {
    FakeOperation {
        asset,
        macro_invocation: MacroInvocation {
            id: MacroId("timeline-macro".into()),
            revision: 4,
            arguments: Default::default(),
        },
    }
}

pub(super) fn playback_command() -> PlaybackCommand {
    PlaybackCommand {
        address: PlaybackAddress::Pool(7),
        action: PlaybackAction::Go { pressed: true },
        surface: PlaybackSurface::Virtual,
    }
}

pub(super) fn trusted_context() -> ActionContext {
    let mut context = ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        ActionSource::Http,
    )
    .with_request_id("forged")
    .with_expected_revision(999);
    context.correlation_id = Uuid::from_u128(4);
    context
}
