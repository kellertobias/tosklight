//! Thread-bound physical ownership proof. Only immutable captures, owned seeds/history batches
//! and the shared Engine cross into this worker; lanes/descriptors are constructed inside it.
//! This does not install a production executor, change lock ordering or publish Pending output.
use super::*;
use crate::runtime::dynamic_snapshot_publication::{ColdGenerationEvent, RetainedInputCapture};
use light_dynamics::DynamicControlBatch;

type Accepted = PendingHybridResult<Sidecar>;
struct SharedResult<E>(E);
impl<E: PendingPairEvaluator<Accepted>> PendingPairEvaluator<Arc<Accepted>> for SharedResult<E> {
    fn evaluate(
        &mut self,
        capture: &RetainedInputCapture,
        before: &mut DynamicRuntime,
        before_origins: &mut DynamicSourceOrigins,
        after: &mut DynamicRuntime,
        after_origins: &mut DynamicSourceOrigins,
    ) -> Result<Arc<Accepted>, String> {
        self.0
            .evaluate(capture, before, before_origins, after, after_origins)
            .map(Arc::new)
    }
}
fn assert_send<T: Send>() {}
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn dedicated_worker_keeps_physical_lanes_local_and_returns_owned_accepted_pairs() {
    assert_send_sync::<Engine>();
    assert_send_sync::<ProgrammerRegistry>();
    assert_send_sync::<RetainedInputCapture>();
    assert_send_sync::<Accepted>();
    assert_send_sync::<ColdGenerationEvent>();
    assert_send_sync::<DynamicControlBatch>();
    assert_send::<PendingHistorySeed>();

    let rig = Rig::new();
    let copy = install_mover(&rig, true).unwrap();
    fix_at(
        &rig,
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [2., 6., 1.],
        ))),
    );
    let programmer = rig.key.programmer;
    let target = rig.target;
    // The two seeds are detached from one authoritative recording boundary. No recording
    // runtime, physical adapter or lane is lent or transferred to the worker.
    let (before_seed, after_seed, controls) = {
        let mut live = rig.live.borrow_mut();
        let snapshot = rig.engine.snapshot();
        let (cold, controls) = rig
            .publication
            .begin_retained_history(&mut live, &snapshot, capacity(64))
            .unwrap();
        let inputs = rig.publication.input_capture_cursor().unwrap();
        let seed = |branch| PendingHistorySeed {
            key: PendingEpisodeKey { branch, ..rig.key },
            runtime: live.fork_for_pending_preview(),
            origins: DynamicSourceOrigins::default(),
            snapshot: Arc::clone(&snapshot),
            position: PendingHistoryPosition {
                inputs,
                cold,
                controls,
            },
            live_sample: live.committed_sample_boundary(),
        };
        (seed(BRANCHES[0]), seed(BRANCHES[1]), controls)
    };
    let first_capture = rig.capture();
    fix_at(
        &rig,
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [-2., 6., 2.],
        ))),
    );
    let second_capture = rig.capture();
    assert_ne!(first_capture.to, second_capture.to);
    let controls = rig.live.borrow().controls_since(controls).unwrap().unwrap();
    let engine = Arc::new(rig.engine);
    let engine_for_worker = Arc::clone(&engine);
    let caller_thread = std::thread::current().id();
    let (sender, receiver) = std::sync::mpsc::channel::<Arc<Accepted>>();
    let worker = std::thread::spawn(move || {
        assert_ne!(caller_thread, std::thread::current().id());
        // These !Send physical owners never leave this closure or become a shared resource.
        let lanes =
            PhysicalPreloadLanes::new(PositionAdapter::default(), PositionAdapter::default());
        let mut state = RetainedPreloadHybridState::default();
        let mut history =
            PairedPendingHistory::<Arc<Accepted>>::new(before_seed, after_seed).unwrap();
        let mut previous_continuity = None;
        let mut previous_identity = None;
        for capture in [first_capture, second_capture] {
            let previous_positions = history.positions();
            let window = history
                .prepare_window(
                    &[Arc::clone(&capture)],
                    &[],
                    &controls,
                    &[],
                    &controls,
                    limits(),
                )
                .unwrap();
            {
                let mut evaluator =
                    SharedResult(RetainedPreloadHybridEvaluator::with_state_and_observer(
                        &engine_for_worker,
                        programmer,
                        &mut state,
                        &lanes,
                        PositionPreloadObserver::new(&lanes),
                    ));
                let outcome = history.consume_window(window, &mut evaluator);
                assert_eq!(outcome.consumed_attempts, 1);
                assert_eq!(outcome.successful_attempts, 1);
                assert!(outcome.failed_attempts.is_empty() && outcome.stopped.is_none());
            }
            let accepted = history.last_success().unwrap();
            assert!(Arc::ptr_eq(&accepted.capture, &capture));
            assert_ne!(previous_positions.0.inputs, history.positions().0.inputs);
            assert_ne!(previous_positions.1.inputs, history.positions().1.inputs);
            let result = &accepted.value;
            verify_retained_native(result);
            let identity = state_identity(&result.after.frame_token);
            if let Some((previous, revision)) = &previous_identity {
                assert!(Arc::ptr_eq(previous, &identity.0));
                assert_eq!(identity.1, revision + 1);
            }
            previous_identity = Some(identity);
            let continuity = BRANCHES.map(|branch| {
                lanes
                    .lane(branch)
                    .continuity(target, ProgrammingOwner::Position)
                    .unwrap()
            });
            if let Some(previous) = &previous_continuity {
                assert_ne!(
                    &continuity, previous,
                    "both branches keep and advance their own accepted physical history"
                );
            }
            for (branch, sidecar) in [(BRANCHES[0], &result.before), (BRANCHES[1], &result.after)] {
                assert_eq!(
                    lanes.lane(branch).last_accepted(),
                    Some(sidecar.frame_token.clone())
                );
                assert_eq!(
                    lanes
                        .lane(branch)
                        .continuity(target, ProgrammingOwner::Position)
                        .unwrap()
                        .instances
                        .len(),
                    2
                );
                let physical = owned(sidecar);
                assert!(
                    physical
                        .achieved
                        .destinations
                        .iter()
                        .any(|d| d.destination == copy)
                );
                assert!(!physical.quality.held);
            }
            previous_continuity = Some(continuity);
            sender.send(Arc::clone(result)).unwrap();
        }
    });
    // Both immutable results can be retained and checked on the reader thread while the
    // worker has already advanced its own caches/history or dropped all physical descriptors.
    let first = receiver.recv().unwrap();
    let first_before = first.before.frame_token.clone();
    let first_after = first.after.frame_token.clone();
    let second = receiver.recv().unwrap();
    worker.join().unwrap();
    verify_retained_native(&first);
    verify_retained_native(&second);
    assert_eq!(first.before.frame_token, first_before);
    assert_eq!(first.after.frame_token, first_after);
    assert_ne!(first.after.frame_token, second.after.frame_token);
    assert!(
        !first
            .after
            .frame_token
            .same_capture(&second.after.frame_token)
    );
    assert!(receiver.try_recv().is_err());
}
