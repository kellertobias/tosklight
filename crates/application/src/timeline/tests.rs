use std::sync::Arc;

use uuid::Uuid;

use super::*;
use crate::{ActionSource, FixturePositionService, PlaybackService};

mod scenario;
mod support;

use scenario::{
    FakeTimelineRuntime, cancellation_during_operation, missing_asset, operation, playback_command,
    trusted_context,
};
use support::{FakeBackend, FakeScheduler, QueuedRunner, RecordingMacroRuntime, TestCancellation};

#[test]
fn timeline_schedules_before_using_shared_services_and_binds_trusted_context() {
    let backend = Arc::new(FakeBackend::default());
    let asset = backend.import_media();
    let scheduler = Arc::new(FakeScheduler::default());
    let runner = Arc::new(QueuedRunner::default());
    let macro_runtime = Arc::new(RecordingMacroRuntime::default());
    let playbacks = PlaybackService::default();
    let macros = MacroService::new(
        macro_runtime.clone(),
        backend.clone(),
        runner.clone(),
        FixturePositionService::default(),
        playbacks.clone(),
    );
    let service = TimelineService::new(scheduler.clone(), playbacks, macros);
    let context = trusted_context();

    service
        .operate(
            TimelineExecutionRequest {
                timeline_id: TimelineId(Uuid::from_u128(2)),
                context: context.clone(),
                deadline: MonotonicMoment(std::time::Duration::from_secs(15)),
            },
            &FakeTimelineRuntime,
            &operation(asset.asset),
            backend.clone(),
            &TestCancellation::default(),
        )
        .unwrap();

    assert_eq!(scheduler.wait_count(), 1);
    assert_eq!(backend.playbacks(), [playback_command()]);
    assert_eq!(macro_runtime.invocation_count(), 0);
    runner.run_next();
    assert_eq!(macro_runtime.invocation_count(), 1);
    let contexts = backend.contexts();
    for (_, actual) in &contexts {
        assert_eq!(actual.desk_id, context.desk_id);
        assert_eq!(actual.user_id, context.user_id);
        assert_eq!(actual.session_id, context.session_id);
        assert_eq!(actual.correlation_id, context.correlation_id);
        assert_ne!(actual.request_id.as_deref(), Some("forged"));
    }
    assert!(contexts.iter().any(|(label, context)| {
        *label == "playback-execute" && context.source == ActionSource::Timecode
    }));
    assert!(contexts.iter().any(|(label, context)| {
        *label == "macro-authorize" && context.source == ActionSource::Macro
    }));
}

#[test]
fn cancellation_after_scheduling_prevents_every_timeline_action() {
    let backend = Arc::new(FakeBackend::default());
    let scheduler = Arc::new(FakeScheduler::default());
    let runner = Arc::new(QueuedRunner::default());
    let macro_runtime = Arc::new(RecordingMacroRuntime::default());
    let playbacks = PlaybackService::default();
    let macros = MacroService::new(
        macro_runtime,
        backend.clone(),
        runner,
        FixturePositionService::default(),
        playbacks.clone(),
    );
    let service = TimelineService::new(scheduler, playbacks, macros);

    let error = service
        .operate(
            TimelineExecutionRequest {
                timeline_id: TimelineId(Uuid::from_u128(2)),
                context: trusted_context(),
                deadline: MonotonicMoment(std::time::Duration::from_secs(15)),
            },
            &FakeTimelineRuntime,
            &operation(missing_asset()),
            backend.clone(),
            &TestCancellation(true),
        )
        .unwrap_err();

    assert_eq!(error.kind, TimelineErrorKind::Cancelled);
    assert!(backend.contexts().is_empty());
    assert!(backend.playbacks().is_empty());
}

#[test]
fn cancellation_during_runtime_prevents_shared_service_calls() {
    let backend = Arc::new(FakeBackend::default());
    let scheduler = Arc::new(FakeScheduler::default());
    let runner = Arc::new(QueuedRunner::default());
    let playbacks = PlaybackService::default();
    let macros = MacroService::new(
        Arc::new(RecordingMacroRuntime::default()),
        backend.clone(),
        runner,
        FixturePositionService::default(),
        playbacks.clone(),
    );
    let service = TimelineService::new(scheduler.clone(), playbacks, macros);
    let (cancellation, operation) = cancellation_during_operation();

    let error = service
        .operate(
            TimelineExecutionRequest {
                timeline_id: TimelineId(Uuid::from_u128(3)),
                context: trusted_context(),
                deadline: MonotonicMoment(std::time::Duration::from_secs(15)),
            },
            &FakeTimelineRuntime,
            &operation,
            backend.clone(),
            &cancellation,
        )
        .unwrap_err();

    assert_eq!(scheduler.wait_count(), 1);
    assert_eq!(error.kind, TimelineErrorKind::Cancelled);
    assert!(backend.contexts().is_empty());
    assert!(backend.playbacks().is_empty());
}
